use crate::classes::c_console::Console;
use crate::classes::c_console_row::ConsoleRow;
use crate::classes::c_memory_region::MemoryRegion;
use crate::classes::c_scan_result_row::{AsyncProcessScan, ResultRow, ScanMessage, ScanState};
use crate::classes::e_message_type::EMessageType;
use crate::classes::e_value_type::EValueType;
use eframe::egui;
use std::sync::mpsc;
use std::{ffi::c_void, io, mem::{size_of, zeroed}, thread};
use windows_sys::Win32::{
    Foundation::HANDLE,
    System::{
        Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory},
        Memory::{
            VirtualProtectEx, VirtualQueryEx, MEMORY_BASIC_INFORMATION,
            MEM_COMMIT, MEM_PRIVATE,
            PAGE_EXECUTE_READWRITE, PAGE_GUARD, PAGE_NOACCESS,
        },
        Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
            PROCESS_VM_WRITE,
        },
    },
};

const CHUNK_SIZE: usize = 64 * 1024;

pub struct OpenedProcess {
    handle: HANDLE,
    pub scan: ScanState,

    pub pid: u32,
    pub watched_rows: Vec<ResultRow>,

    async_scan_state: AsyncProcessScan

}


impl OpenedProcess {
    pub fn new(pid: u32) -> io::Result<Self> {
        let access =
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION;
        let handle = unsafe { OpenProcess(access, 0, pid) };

        if handle == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Self { handle, pid, watched_rows: Vec::new(), scan: ScanState::default(), async_scan_state: AsyncProcessScan::default() })
    }


    pub fn enumerate_regions(&self) -> io::Result<Vec<MemoryRegion>> {
        let mut regions = Vec::new();
        let mut addr = 0usize;

        loop {
            let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { zeroed() };

            let result = unsafe {
                VirtualQueryEx(
                    self.handle,
                    addr as *const c_void,
                    &mut mbi,
                    size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            };

            if result == 0 {
                break;
            }

            let base = mbi.BaseAddress as usize;
            let size = mbi.RegionSize;

            regions.push(MemoryRegion {
                base_address: base,
                allocation_base: mbi.AllocationBase as usize,
                region_size: size,
                state: mbi.State,
                protect: mbi.Protect,
                region_type: mbi.Type,
            });

            let next = base.saturating_add(size);
            if next <= addr {
                break;
            }

            addr = next;
        }

        Ok(regions)
    }
    const CHUNK_SIZE: usize = 64 * 1024;

    pub fn full_scan(&mut self, console: &mut Console, ctx: &egui::Context) {
        if self.async_scan_state.is_running {
            return;
        }

        let pid = self.pid;
        let input = self.scan.input_value.trim().to_string();
        let value_type = self.scan.selected_value_type;

        let (tx, rx) = mpsc::channel::<ScanMessage>();
        self.scan.results.clear();

        self.async_scan_state.is_running = true;
        self.async_scan_state.total_regions = 0;
        self.async_scan_state.scanned_regions = 0;
        self.async_scan_state.total_found = 0;
        self.async_scan_state.receiver = Some(rx);

        let ctx = ctx.clone();

        thread::spawn(move || {
            Self::run_full_scan_worker(pid, input, value_type, tx, ctx);
        });

        console.add_message(ConsoleRow::new(
            "Full scan started".to_string(),
            EMessageType::Log,
        ));
    }

    pub fn next_scan(&mut self) {
        if !self.scan.has_scan_session {
            return;
        }

        let wanted = match self.scan.input_value.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => return,
        };

        let handle = self.handle;
        self.scan.results.retain_mut(|row| {
            match Self::read_bytes_from_handle(handle, row.address, 4) {
                Ok(bytes) if bytes.len() == 4 => {
                    let val = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    row.cached_value = val.to_string();
                    val == wanted
                }
                _ => false,
            }
        });
    }

    pub fn refresh_watched(&mut self){
        let handle = self.handle;
        for x in self.watched_rows.iter_mut() {
            if x.is_frozen {
                if x.value_type == EValueType::I32 {
                    if let Ok(val) = x.cached_value.trim().parse::<i32>() {
                        let _ = Self::write_bytes_to_handle(handle, x.address, &val.to_le_bytes());
                    }
                }
                continue;
            }

            match Self::read_bytes_from_handle(handle, x.address, 4) {
                Ok(bytes) if bytes.len() == 4 => {
                    let val = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    x.cached_value = val.to_string();
                }
                _ => continue
            }
        }
    }

    pub fn update_watched_value(&mut self, index: usize) -> io::Result<()> {
        let Some(row) = self.watched_rows.get(index) else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid watched row index"));
        };

        if row.value_type != EValueType::I32 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Only I32 update is implemented",
            ));
        }

        let parsed = row
            .cached_value
            .trim()
            .parse::<i32>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse i32"))?;

        Self::write_bytes_to_handle(self.handle, row.address, &parsed.to_le_bytes())
    }

    pub fn reset_scan(&mut self) {
        self.scan.input_value.clear();
        self.scan.results.clear();
        self.scan.has_scan_session = false;
    }


    pub fn read_bytes(&self, address: usize, size: usize) -> io::Result<Vec<u8>> {
        Self::read_bytes_from_handle(self.handle, address, size)
    }

    fn read_bytes_from_handle(handle: HANDLE, address: usize, size: usize) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        let mut bytes_read = 0usize;

        let ok = unsafe {
            ReadProcessMemory(
                handle,
                address as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                size,
                &mut bytes_read,
            )
        };

        if ok == 0 {
            return Err(io::Error::last_os_error());
        }

        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    fn write_bytes_to_handle(handle: HANDLE, address: usize, bytes: &[u8]) -> io::Result<()> {
        let mut bytes_written = 0usize;
        let direct_ok = unsafe {
            WriteProcessMemory(
                handle,
                address as *const c_void,
                bytes.as_ptr() as *const c_void,
                bytes.len(),
                &mut bytes_written,
            )
        };

        if direct_ok != 0 && bytes_written == bytes.len() {
            return Ok(());
        }

        let direct_error = io::Error::last_os_error();

        let mut old_protect = 0u32;
        let protect_changed = unsafe {
            VirtualProtectEx(
                handle,
                address as *const c_void,
                bytes.len(),
                PAGE_EXECUTE_READWRITE,
                &mut old_protect,
            )
        };

        if protect_changed == 0 {
            return Err(direct_error);
        }

        bytes_written = 0;
        let write_after_protect = unsafe {
            WriteProcessMemory(
                handle,
                address as *const c_void,
                bytes.as_ptr() as *const c_void,
                bytes.len(),
                &mut bytes_written,
            )
        };

        let mut restored_protect_out = 0u32;
        let _ = unsafe {
            VirtualProtectEx(
                handle,
                address as *const c_void,
                bytes.len(),
                old_protect,
                &mut restored_protect_out,
            )
        };

        if write_after_protect == 0 || bytes_written != bytes.len() {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    fn run_full_scan_worker(
        pid: u32,
        input: String,
        value_type: EValueType,
        tx: std::sync::mpsc::Sender<ScanMessage>,
        ctx: egui::Context,
    ) {
        let wanted = match value_type {
            EValueType::I32 => match input.parse::<i32>() {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(ScanMessage::Error(format!("Failed to parse i32: {e}")));
                    ctx.request_repaint();
                    return;
                }
            },
            _ => {
                let _ = tx.send(ScanMessage::Error("Only I32 scan is implemented yet".to_string()));
                ctx.request_repaint();
                return;
            }
        };

        let wanted_bytes = wanted.to_le_bytes();

        let process = match OpenedProcess::new(pid) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(ScanMessage::Error(format!("OpenProcess failed: {e}")));
                ctx.request_repaint();
                return;
            }
        };

        let regions = match process.enumerate_regions() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(ScanMessage::Error(format!("enumerate_regions failed: {e}")));
                ctx.request_repaint();
                return;
            }
        };

        let total_regions = regions.len();
        let _ = tx.send(ScanMessage::Started { total_regions });
        ctx.request_repaint();

        let mut scanned_regions = 0usize;
        let mut found = 0usize;

        for region in regions {
            if region.state != MEM_COMMIT {
                continue;
            }
            if region.region_type != MEM_PRIVATE {
                continue;
            }
            if region.protect & PAGE_GUARD != 0 {
                continue;
            }
            if region.protect & PAGE_NOACCESS != 0 {
                continue;
            }

            scanned_regions += 1;

            let mut offset = 0usize;
            while offset < region.region_size {
                let to_read = CHUNK_SIZE.min(region.region_size - offset);
                let address = region.base_address + offset;

                if let Ok(bytes) = process.read_bytes(address, to_read) {
                    if bytes.len() >= 4 {
                        for i in (0..=bytes.len() - 4).step_by(4) {

                            if bytes[i..i + 4] == wanted_bytes {
                                found += 1;

                                let _ = tx.send(ScanMessage::Found(ResultRow {
                                    description: None,
                                    address: address + i,
                                    value_type: EValueType::I32,
                                    cached_value: wanted.to_string(),
                                    is_frozen: false,
                                }));
                            }
                        }
                    }
                }

                offset += to_read;
            }

            let _ = tx.send(ScanMessage::Progress {
                scanned_regions,
                found,
            });
            ctx.request_repaint();
        }

        let _ = tx.send(ScanMessage::Done { found });
        ctx.request_repaint();
    }

    pub fn pump_scan_messages(&mut self, console: &mut Console) {
        let Some(rx) = &self.async_scan_state.receiver else {
            return;
        };

        while let Ok(msg) = rx.try_recv() {
            match msg {
                ScanMessage::Started { total_regions } => {
                    self.async_scan_state.total_regions = total_regions;
                }
                ScanMessage::Progress { scanned_regions, found } => {
                    self.async_scan_state.scanned_regions = scanned_regions;
                    self.async_scan_state.total_found = found;
                }
                ScanMessage::Found(row) => {
                    self.scan.results.push(row);
                }
                ScanMessage::Done { found } => {
                    self.async_scan_state.is_running = false;
                    self.async_scan_state.total_found = found;
                    self.scan.has_scan_session = true;
                    console.add_message(ConsoleRow::new(
                        format!("Scan finished. Found {found} matches"),
                        EMessageType::Log,
                    ));
                }
                ScanMessage::Error(err) => {
                    self.async_scan_state.is_running = false;
                    console.add_message(ConsoleRow::new(err, EMessageType::Error));
                }
            }
        }
    }
}
