use crate::classes::c_console::Console;
use crate::classes::c_console_row::ConsoleRow;
use crate::classes::c_memory_region::MemoryRegion;
use crate::classes::c_scan_result_row::{AsyncProcessScan, ResultRow, ScanMessage, ScanState};
use crate::classes::e_message_type::EMessageType;
use crate::classes::e_value_type::EValueType;
use eframe::egui;
use std::sync::mpsc;
use std::{
    ffi::c_void,
    io,
    mem::{size_of, zeroed},
    thread,
    time::{Duration, Instant},
};
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

        let value_type = self
            .scan
            .results
            .first()
            .map(|row| row.value_type)
            .unwrap_or(self.scan.selected_value_type);
        let wanted_bytes = match Self::typed_value_to_bytes(value_type, self.scan.input_value.trim()) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let value_size = wanted_bytes.len();

        let handle = self.handle;
        self.scan.results.retain_mut(|row| {
            match Self::read_bytes_from_handle(handle, row.address, value_size) {
                Ok(bytes) if bytes.len() == value_size => {
                    if let Ok(current_value) = Self::typed_bytes_to_string(value_type, bytes.as_slice()) {
                        row.cached_value = current_value;
                    }
                    bytes == wanted_bytes
                }
                _ => false,
            }
        });
    }

    pub fn refresh_watched(&mut self){
        let handle = self.handle;
        for x in self.watched_rows.iter_mut() {
            if x.is_frozen {
                if let Ok(bytes_to_write) =
                    Self::typed_value_to_bytes(x.value_type, x.cached_value.as_str())
                {
                    let _ = Self::write_bytes_to_handle(handle, x.address, bytes_to_write.as_slice());
                }
                continue;
            }

            match Self::read_typed_value_as_string(handle, x.address, x.value_type) {
                Ok(val) => x.cached_value = val,
                _ => continue
            }
        }
    }

    pub fn poll_write_verifications(&mut self) {
        let now = Instant::now();
        let handle = self.handle;

        for row in self.watched_rows.iter_mut() {
            let Some(deadline) = row.verify_after_at else {
                continue;
            };

            if now < deadline {
                continue;
            }

            row.value_after_100ms = match Self::read_typed_value_as_string(handle, row.address, row.value_type) {
                Ok(v) => Some(v),
                Err(e) => Some(format!("ERR: {e}")),
            };
            row.verify_after_at = None;
        }
    }

    pub fn update_watched_value(&mut self, index: usize) -> io::Result<()> {
        let Some(row) = self.watched_rows.get(index) else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid watched row index"));
        };

        let address = row.address;
        let value_type = row.value_type;
        let input_value = row.cached_value.clone();

        let bytes_to_write = Self::typed_value_to_bytes(value_type, input_value.as_str())?;
        Self::write_bytes_to_handle(self.handle, address, bytes_to_write.as_slice())?;

        let read_back_raw = Self::read_bytes_from_handle(self.handle, address, bytes_to_write.len())?;
        let read_back = Self::read_typed_value_as_string(self.handle, address, value_type)?;
        let write_ok = read_back_raw == bytes_to_write;

        if let Some(row_mut) = self.watched_rows.get_mut(index) {
            row_mut.cached_value = read_back;
            row_mut.write_ok = Some(write_ok);
            row_mut.value_after_write = Some(row_mut.cached_value.clone());
            row_mut.value_after_100ms = None;
            row_mut.verify_after_at = Some(Instant::now() + Duration::from_millis(100));
        }

        if !write_ok {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Write verification failed: value changed immediately",
            ));
        }

        Ok(())
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

    fn read_typed_value_as_string(
        handle: HANDLE,
        address: usize,
        value_type: EValueType,
    ) -> io::Result<String> {
        match value_type {
            EValueType::I32 => {
                let bytes = Self::read_bytes_from_handle(handle, address, 4)?;
                if bytes.len() != 4 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 4 bytes"));
                }
                Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string())
            }
            EValueType::I64 => {
                let bytes = Self::read_bytes_from_handle(handle, address, 8)?;
                if bytes.len() != 8 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 8 bytes"));
                }
                Ok(i64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]).to_string())
            }
            EValueType::F32 => {
                let bytes = Self::read_bytes_from_handle(handle, address, 4)?;
                if bytes.len() != 4 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 4 bytes"));
                }
                Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string())
            }
            EValueType::F64 => {
                let bytes = Self::read_bytes_from_handle(handle, address, 8)?;
                if bytes.len() != 8 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 8 bytes"));
                }
                Ok(f64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]).to_string())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported watched value type",
            )),
        }
    }

    fn typed_value_to_bytes(value_type: EValueType, value: &str) -> io::Result<Vec<u8>> {
        match value_type {
            EValueType::I32 => {
                let parsed = value
                    .trim()
                    .parse::<i32>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse i32"))?;
                Ok(parsed.to_le_bytes().to_vec())
            }
            EValueType::I64 => {
                let parsed = value
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse i64"))?;
                Ok(parsed.to_le_bytes().to_vec())
            }
            EValueType::F32 => {
                let parsed = value
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse f32"))?;
                Ok(parsed.to_le_bytes().to_vec())
            }
            EValueType::F64 => {
                let parsed = value
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse f64"))?;
                Ok(parsed.to_le_bytes().to_vec())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported watched value type",
            )),
        }
    }

    fn typed_bytes_to_string(value_type: EValueType, bytes: &[u8]) -> io::Result<String> {
        match value_type {
            EValueType::I32 => {
                if bytes.len() != 4 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 4 bytes"));
                }
                Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string())
            }
            EValueType::I64 => {
                if bytes.len() != 8 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 8 bytes"));
                }
                Ok(i64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]).to_string())
            }
            EValueType::F32 => {
                if bytes.len() != 4 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 4 bytes"));
                }
                Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string())
            }
            EValueType::F64 => {
                if bytes.len() != 8 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Expected 8 bytes"));
                }
                Ok(f64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]).to_string())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported scan value type",
            )),
        }
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
        let wanted_bytes = match Self::typed_value_to_bytes(value_type, input.as_str()) {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = tx.send(ScanMessage::Error(format!("Failed to parse input value: {e}")));
                ctx.request_repaint();
                return;
            }
        };
        let value_size = wanted_bytes.len();
        let wanted_display_normalized = match Self::typed_bytes_to_string(value_type, wanted_bytes.as_slice()) {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.send(ScanMessage::Error(format!("Failed to prepare scan value: {e}")));
                ctx.request_repaint();
                return;
            }
        };

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
                    if bytes.len() >= value_size {
                        for i in (0..=bytes.len() - value_size).step_by(value_size) {

                            if bytes[i..i + value_size] == wanted_bytes {
                                found += 1;

                                let _ = tx.send(ScanMessage::Found(ResultRow {
                                    description: None,
                                    address: address + i,
                                    value_type,
                                    cached_value: wanted_display_normalized.clone(),
                                    is_frozen: false,
                                    write_ok: None,
                                    value_after_write: None,
                                    value_after_100ms: None,
                                    verify_after_at: None,
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
