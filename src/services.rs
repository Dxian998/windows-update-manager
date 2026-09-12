use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, OpenSCManagerW,
    OpenServiceW, QueryServiceStatus, StartServiceW,
    SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SERVICE_CONTROL_STOP,
    SERVICE_NO_CHANGE, SERVICE_START_TYPE, SERVICE_STATUS,
    ENUM_SERVICE_TYPE, SERVICE_ERROR, SERVICE_RUNNING,
};
use windows::core::PCWSTR;
use winreg::{enums::*, RegKey};

unsafe fn open_scm() -> Option<windows::Win32::System::Services::SC_HANDLE> {
    unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS).ok() }
}

unsafe fn open_service(
    scm: windows::Win32::System::Services::SC_HANDLE,
    name: &str,
) -> Option<windows::Win32::System::Services::SC_HANDLE> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { OpenServiceW(scm, PCWSTR(wide.as_ptr()), SERVICE_ALL_ACCESS).ok() }
}

pub fn stop_service(name: &str) -> bool {
    unsafe {
        let Some(scm) = open_scm() else { return false };
        let Some(svc) = open_service(scm, name) else {
            let _ = CloseServiceHandle(scm);
            return false;
        };
        let mut status = SERVICE_STATUS::default();
        let _ = ControlService(svc, SERVICE_CONTROL_STOP, &mut status);
        let _ = CloseServiceHandle(svc);
        let _ = CloseServiceHandle(scm);
        true
    }
}

pub fn start_service(name: &str) {
    unsafe {
        let Some(scm) = open_scm() else { return };
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        if let Ok(svc) = OpenServiceW(scm, PCWSTR(wide.as_ptr()), SERVICE_ALL_ACCESS) {
            let _ = StartServiceW(svc, None);
            let _ = CloseServiceHandle(svc);
        }
        let _ = CloseServiceHandle(scm);
    }
}

pub fn set_service_start(name: &str, start_type: SERVICE_START_TYPE) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", name);
    if let Ok(key) = hklm.open_subkey_with_flags(&path, KEY_SET_VALUE) {
        let _ = key.set_value("Start", &(start_type.0 as u32));
    }

    unsafe {
        let Some(scm) = open_scm() else { return };
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        if let Ok(svc) = OpenServiceW(scm, PCWSTR(wide.as_ptr()), SERVICE_ALL_ACCESS) {
            let _ = ChangeServiceConfigW(
                svc,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                start_type,
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            );
            let _ = CloseServiceHandle(svc);
        }
        let _ = CloseServiceHandle(scm);
    }
}

pub fn fix_service_img_path(name: &str, new_image_path: &str) {
    let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_path: Vec<u16> = new_image_path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let Some(scm) = open_scm() else { return };
        if let Ok(svc) = OpenServiceW(scm, PCWSTR(wide_name.as_ptr()), SERVICE_ALL_ACCESS) {
            let _ = ChangeServiceConfigW(
                svc,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                SERVICE_START_TYPE(SERVICE_NO_CHANGE),
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR(wide_path.as_ptr()),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            );
            let _ = CloseServiceHandle(svc);
        }
        let _ = CloseServiceHandle(scm);
    }
}

pub fn get_service_start_value(name: &str) -> u32 {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", name);
    let key = match hklm.open_subkey(&path) {
        Ok(k) => k,
        Err(_) => return 0,
    };
    key.get_value::<u32, _>("Start").unwrap_or(0)
}

#[allow(dead_code)]
pub fn service_start_label(name: &str) -> &'static str {
    match get_service_start_value(name) {
        2 => "Auto",
        3 => "Manual",
        4 => "Disabled",
        _ => "Unknown",
    }
}

pub fn is_service_running(name: &str) -> bool {
    unsafe {
        let Some(scm) = open_scm() else { return false };
        let Some(svc) = open_service(scm, name) else {
            let _ = CloseServiceHandle(scm);
            return false;
        };
        let mut status = SERVICE_STATUS::default();
        let running = QueryServiceStatus(svc, &mut status).is_ok()
            && status.dwCurrentState == SERVICE_RUNNING;
        let _ = CloseServiceHandle(svc);
        let _ = CloseServiceHandle(scm);
        running
    }
}
