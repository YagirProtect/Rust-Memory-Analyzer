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
}