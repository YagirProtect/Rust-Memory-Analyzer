use std::{ffi::c_void, io, mem::{size_of, zeroed}};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Diagnostics::Debug::ReadProcessMemory,
        Memory::{
            VirtualQueryEx, MEMORY_BASIC_INFORMATION,
            MEM_COMMIT, MEM_FREE, MEM_IMAGE, MEM_MAPPED, MEM_PRIVATE, MEM_RESERVE,
            PAGE_GUARD, PAGE_NOACCESS,
        },
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
};
use crate::classes::c_memory_region::MemoryRegion;

pub struct OpenedProcess {
    handle: HANDLE,
    pub pid: u32,
}


impl OpenedProcess {
    pub fn new(pid: u32) -> io::Result<Self> {
        let access = PROCESS_QUERY_INFORMATION | PROCESS_VM_READ;
        let handle = unsafe { OpenProcess(access, 0, pid) };

        if handle == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Self { handle, pid })
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
}