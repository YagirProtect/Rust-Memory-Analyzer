use std::io;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Memory::{MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS};
use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use crate::classes::c_memory_region::MemoryRegion;

pub fn open_in_explorer(path: &str) -> std::io::Result<()> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path))
        .spawn()?;
    Ok(())
}

pub fn terminate_process_by_pid(pid: u32) -> io::Result<()> {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };

    if handle == 0 {
        return Err(io::Error::last_os_error());
    }

    let ok = unsafe { TerminateProcess(handle, 1) };

    unsafe {
        CloseHandle(handle);
    }

    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn is_region_readable(region: &MemoryRegion) -> bool {
    if region.state != MEM_COMMIT {
        return false;
    }

    if region.protect & PAGE_GUARD != 0 {
        return false;
    }

    if region.protect & PAGE_NOACCESS != 0 {
        return false;
    }

    true
}
