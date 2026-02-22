//! Windows console helpers.
//!
//! Classic Windows console hosts (conhost/cmd/powershell) may not process ANSI escape
//! sequences unless virtual terminal processing is enabled.

#[cfg(target_os = "windows")]
pub fn enable_virtual_terminal_processing() -> std::io::Result<()> {
    use std::io;

    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };

    unsafe {
        let handle: HANDLE = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode as *mut u32) == 0 {
            // Not a console (e.g. redirected output) or failed.
            let err = io::Error::last_os_error();
            // Some hosts return ERROR_INVALID_HANDLE for redirected output.
            // We still return the error to the caller to decide.
            return Err(err);
        }

        let new_mode = mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        if new_mode != mode {
            if SetConsoleMode(handle, new_mode) == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn enable_virtual_terminal_processing() -> std::io::Result<()> {
    Ok(())
}
