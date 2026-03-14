use std::io;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

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