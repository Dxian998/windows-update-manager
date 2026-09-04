use std::io;
use std::process::Command;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub fn is_elevated() -> io::Result<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;

        let mut elevation = TOKEN_ELEVATION::default();
        let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let mut ret_size: u32 = 0;

        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size,
            &mut ret_size,
        );

        let _ = CloseHandle(token);

        result.map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
        Ok(elevation.TokenIsElevated != 0)
    }
}

pub fn elevate() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let exe_str = exe.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::Other, "Non-UTF-8 executable path")
    })?;
    Command::new("powershell")
        .args(["-Command", &format!("Start-Process -FilePath '{}' -Verb RunAs", exe_str)])
        .spawn()?;
    Ok(())
}
