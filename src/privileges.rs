use std::io;
use std::process::Command;

pub fn is_elevated() -> io::Result<bool> {
    use winapi::um::{
        handleapi::CloseHandle,
        processthreadsapi::{GetCurrentProcess, OpenProcessToken},
        securitybaseapi::GetTokenInformation,
        winnt::{HANDLE, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    };

    unsafe {
        let mut token_handle: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let mut ret_size = 0;

        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size,
            &mut ret_size,
        );

        CloseHandle(token_handle);

        if result == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(elevation.TokenIsElevated != 0)
    }
}

pub fn elevate() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let status = Command::new("powershell")
        .args(&[
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -Verb RunAs",
                exe.to_str().ok_or_else(|| io::Error::new(
                    io::ErrorKind::Other,
                    "Invalid executable path"
                ))?
            ),
        ])
        .spawn();

    match status {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
