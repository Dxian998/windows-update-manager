use std::ptr;
use winreg::enums::*;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, CreateWellKnownSid,
    WinBuiltinAdministratorsSid, WinLocalServiceSid, WinLocalSystemSid,
    WinNetworkServiceSid, WinWorldSid,
    LUID_AND_ATTRIBUTES, PSID, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    DACL_SECURITY_INFORMATION, OBJECT_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, UNPROTECTED_DACL_SECURITY_INFORMATION,
    ACL, ACE_FLAGS, GetLengthSid,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSidToSidW, SetEntriesInAclW, SetSecurityInfo, ACCESS_MODE,
    EXPLICIT_ACCESS_W, MULTIPLE_TRUSTEE_OPERATION, SE_REGISTRY_KEY, TRUSTEE_FORM,
    TRUSTEE_TYPE, TRUSTEE_W, SET_ACCESS,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, HKEY, REG_OPTION_BACKUP_RESTORE, REG_SAM_FLAGS,
};
use windows::core::PCWSTR;

const TRUSTED_INSTALLER_SID: &str = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";

const KEY_READ_RIGHTS: u32 = 0x20019;
const KEY_ALL_RIGHTS: u32 = 0xF003F;

const REQUIRED_PRIVILEGES: &[&str] = &[
    "SeTakeOwnershipPrivilege",
    "SeRestorePrivilege",
    "SeBackupPrivilege",
    "SeSecurityPrivilege",
];

pub fn enable_privileges() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return false;
        }

        let mut all_ok = true;
        for &name in REQUIRED_PRIVILEGES {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let mut luid = LUID::default();
            if windows::Win32::Security::LookupPrivilegeValueW(
                PCWSTR::null(),
                PCWSTR(wide.as_ptr()),
                &mut luid,
            )
            .is_ok()
            {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                if AdjustTokenPrivileges(
                    token,
                    false,
                    Some(&tp),
                    0,
                    None,
                    None,
                )
                .is_err()
                {
                    all_ok = false;
                }
            } else {
                all_ok = false;
            }
        }

        let _ = CloseHandle(token);
        all_ok
    }
}

pub unsafe fn build_well_known_sid(
    kind: windows::Win32::Security::WELL_KNOWN_SID_TYPE,
) -> Option<Vec<u8>> {
    let mut size: u32 = 0;
    let _ = unsafe { CreateWellKnownSid(kind, None, None, &mut size) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    let sid = PSID(buf.as_mut_ptr() as _);
    if unsafe { CreateWellKnownSid(kind, None, Some(sid), &mut size) }.is_ok() {
        Some(buf)
    } else {
        None
    }
}

pub unsafe fn sid_from_string(s: &str) -> Option<Vec<u8>> {
    let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let mut psid = PSID(ptr::null_mut());
    unsafe {
        if ConvertStringSidToSidW(PCWSTR(wide.as_ptr()), &mut psid).is_ok() && !psid.0.is_null() {
            let len = GetLengthSid(psid);
            let mut buf = vec![0u8; len as usize];
            ptr::copy_nonoverlapping(psid.0 as *const u8, buf.as_mut_ptr(), len as usize);
            let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(psid.0)));
            Some(buf)
        } else {
            None
        }
    }
}

pub unsafe fn explicit_access(sid_buf: &[u8], access: u32, mode: ACCESS_MODE) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: access,
        grfAccessMode: mode,
        grfInheritance: ACE_FLAGS(3),
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: MULTIPLE_TRUSTEE_OPERATION(0),
            TrusteeForm: TRUSTEE_FORM(0),
            TrusteeType: TRUSTEE_TYPE(2),
            ptstrName: windows::core::PWSTR(sid_buf.as_ptr() as *mut u16),
        },
    }
}

unsafe fn open_path_for_security(path: &str, sam: u32) -> Option<HKEY> {
    let subkey: Vec<u16> = path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut hkey = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            Some(REG_OPTION_BACKUP_RESTORE.0),
            REG_SAM_FLAGS(sam),
            &mut hkey,
        )
    };
    if status.is_ok() && !hkey.0.is_null() {
        Some(hkey)
    } else {
        None
    }
}

pub fn lock_service_registry_key(service: &str) {
    let path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", service);
    unsafe {
        let Some(sys_sid) = build_well_known_sid(WinLocalSystemSid) else { return };
        let Some(adm_sid) = build_well_known_sid(WinBuiltinAdministratorsSid) else { return };
        let Some(world_sid) = build_well_known_sid(WinWorldSid) else { return };

        let entries = [
            explicit_access(&sys_sid, KEY_READ_RIGHTS, SET_ACCESS),
            explicit_access(&adm_sid, KEY_READ_RIGHTS, SET_ACCESS),
            explicit_access(&world_sid, KEY_READ_RIGHTS, SET_ACCESS),
        ];

        let mut new_acl: *mut ACL = ptr::null_mut();
        let rc = SetEntriesInAclW(Some(&entries), None, &mut new_acl);
        if rc.0 != 0 { return; }

        let adm_psid = PSID(adm_sid.as_ptr() as _);

        if let Some(hkey) = open_path_for_security(&path, 0x00080000) {
            let _ = SetSecurityInfo(
                HANDLE(hkey.0 as _),
                SE_REGISTRY_KEY,
                OWNER_SECURITY_INFORMATION,
                Some(adm_psid),
                None,
                None,
                None,
            );
            let _ = RegCloseKey(hkey);
        }

        let dacl_flags = OBJECT_SECURITY_INFORMATION(
            DACL_SECURITY_INFORMATION.0 | PROTECTED_DACL_SECURITY_INFORMATION.0,
        );
        if let Some(hkey) = open_path_for_security(&path, 0x00040000) {
            let _ = SetSecurityInfo(
                HANDLE(hkey.0 as _),
                SE_REGISTRY_KEY,
                dacl_flags,
                None,
                None,
                Some(new_acl as *const _),
                None,
            );
            let _ = RegCloseKey(hkey);
        }

        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(new_acl as *mut _)));
    }
}

pub fn unlock_registry_path(path: &str) -> bool {
    unsafe {
        let Some(sys_sid) = build_well_known_sid(WinLocalSystemSid) else { return false };
        let Some(adm_sid) = build_well_known_sid(WinBuiltinAdministratorsSid) else { return false };
        let Some(loc_svc_sid) = build_well_known_sid(WinLocalServiceSid) else { return false };
        let Some(net_svc_sid) = build_well_known_sid(WinNetworkServiceSid) else { return false };
        let Some(world_sid) = build_well_known_sid(WinWorldSid) else { return false };

        let adm_psid = PSID(adm_sid.as_ptr() as _);
        if let Some(hkey) = open_path_for_security(path, 0x00080000) {
            let _ = SetSecurityInfo(
                HANDLE(hkey.0 as _),
                SE_REGISTRY_KEY,
                OWNER_SECURITY_INFORMATION,
                Some(adm_psid),
                None,
                None,
                None,
            );
            let _ = RegCloseKey(hkey);
        }

        let ti_sid = sid_from_string(TRUSTED_INSTALLER_SID);

        let mut entries = vec![
            explicit_access(&sys_sid, KEY_ALL_RIGHTS, SET_ACCESS),
            explicit_access(&adm_sid, KEY_ALL_RIGHTS, SET_ACCESS),
            explicit_access(&loc_svc_sid, KEY_ALL_RIGHTS, SET_ACCESS),
            explicit_access(&net_svc_sid, KEY_ALL_RIGHTS, SET_ACCESS),
            explicit_access(&world_sid, KEY_READ_RIGHTS, SET_ACCESS),
        ];

        if let Some(ti) = &ti_sid {
            entries.push(explicit_access(ti, KEY_ALL_RIGHTS, SET_ACCESS));
        }

        let mut new_acl: *mut ACL = ptr::null_mut();
        if SetEntriesInAclW(Some(&entries), None, &mut new_acl).0 != 0 {
            return false;
        }

        let dacl_flags = OBJECT_SECURITY_INFORMATION(
            DACL_SECURITY_INFORMATION.0 | UNPROTECTED_DACL_SECURITY_INFORMATION.0,
        );
        let mut success = false;
        if let Some(hkey) = open_path_for_security(path, 0x00040000) {
            let res = SetSecurityInfo(
                HANDLE(hkey.0 as _),
                SE_REGISTRY_KEY,
                dacl_flags,
                None,
                None,
                Some(new_acl as *const _),
                None,
            );
            success = res.is_ok();
            let _ = RegCloseKey(hkey);
        }

        if let Some(ti) = &ti_sid {
            if let Some(hkey) = open_path_for_security(path, 0x00080000) {
                let _ = SetSecurityInfo(
                    HANDLE(hkey.0 as _),
                    SE_REGISTRY_KEY,
                    OWNER_SECURITY_INFORMATION,
                    Some(PSID(ti.as_ptr() as _)),
                    None,
                    None,
                    None,
                );
                let _ = RegCloseKey(hkey);
            }
        }

        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(new_acl as *mut _)));
        success
    }
}

pub fn is_registry_key_locked(service: &str) -> bool {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", service);
    match hklm.open_subkey_with_flags(&path, KEY_WRITE) {
        Ok(_key) => false,
        Err(e) => e.raw_os_error() == Some(5),
    }
}

pub fn unlock_service_registry_key(service: &str) {
    let root_path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", service);
    unlock_registry_path(&root_path);

    let mut subkeys = Vec::new();
    unsafe {
        if let Some(hkey) = open_path_for_security(&root_path, KEY_READ) {
            let mut index = 0;
            let mut name_buf = [0u16; 256];
            loop {
                let mut name_len = name_buf.len() as u32;
                let status = RegEnumKeyExW(
                    hkey,
                    index,
                    Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                );
                if status.0 != 0 {
                    break;
                }
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                subkeys.push(name);
                index += 1;
            }
            let _ = RegCloseKey(hkey);
        }
    }

    for known in &["Parameters", "Security", "TriggerInfo"] {
        if !subkeys.iter().any(|s| s.eq_ignore_ascii_case(known)) {
            subkeys.push(known.to_string());
        }
    }

    for child in &subkeys {
        let child_path = format!("{}\\{}", root_path, child);
        unlock_registry_path(&child_path);
    }
}
