use std::ptr;
use winreg::{enums::*, RegKey};

use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, CreateWellKnownSid,
    WinBuiltinAdministratorsSid, WinLocalSystemSid, WinWorldSid,
    LUID_AND_ATTRIBUTES, PSID, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    DACL_SECURITY_INFORMATION, OBJECT_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, UNPROTECTED_DACL_SECURITY_INFORMATION,
    ACL, ACE_FLAGS,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSidToSidW, SetEntriesInAclW, SetNamedSecurityInfoW, ACCESS_MODE,
    EXPLICIT_ACCESS_W, MULTIPLE_TRUSTEE_OPERATION, SE_REGISTRY_KEY, TRUSTEE_FORM,
    TRUSTEE_TYPE, TRUSTEE_W, SET_ACCESS,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PCWSTR;

const TRUSTED_INSTALLER_SID: &str =
    "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";

const KEY_READ_RIGHTS: u32 = 0x2001F;
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
        for priv_name in REQUIRED_PRIVILEGES {
            let wide: Vec<u16> = priv_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut luid = LUID::default();
            if windows::Win32::Security::LookupPrivilegeValueW(
                PCWSTR::null(),
                PCWSTR(wide.as_ptr()),
                &mut luid,
            )
            .is_err()
            {
                all_ok = false;
                continue;
            }
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            let _ = AdjustTokenPrivileges(
                token,
                false,
                Some(&tp),
                std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                None,
                None,
            );
        }

        let _ = CloseHandle(token);
        all_ok
    }
}

unsafe fn build_well_known_sid(
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

unsafe fn explicit_access(sid_buf: &[u8], access: u32, mode: ACCESS_MODE) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: access,
        grfAccessMode: mode,
        grfInheritance: ACE_FLAGS(0),
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: MULTIPLE_TRUSTEE_OPERATION(0),
            TrusteeForm: TRUSTEE_FORM(0),
            TrusteeType: TRUSTEE_TYPE(2),
            ptstrName: windows::core::PWSTR(sid_buf.as_ptr() as *mut u16),
        },
    }
}

fn registry_named_path(service: &str) -> Vec<u16> {
    format!(
        "MACHINE\\SYSTEM\\CurrentControlSet\\Services\\{}",
        service
    )
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect()
}

pub fn lock_service_registry_key(service: &str) {
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

        let path = registry_named_path(service);
        let pcwstr = PCWSTR(path.as_ptr());

        let adm_psid = PSID(adm_sid.as_ptr() as _);
        let _ = SetNamedSecurityInfoW(
            pcwstr,
            SE_REGISTRY_KEY,
            OWNER_SECURITY_INFORMATION,
            Some(adm_psid),
            None,
            None,
            None,
        );

        let dacl_flags = OBJECT_SECURITY_INFORMATION(
            DACL_SECURITY_INFORMATION.0 | PROTECTED_DACL_SECURITY_INFORMATION.0,
        );
        let _ = SetNamedSecurityInfoW(
            pcwstr,
            SE_REGISTRY_KEY,
            dacl_flags,
            None,
            None,
            Some(new_acl as *const _),
            None,
        );

        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(new_acl as *mut _)));
    }
}

pub fn unlock_service_registry_key(service: &str) {
    unsafe {
        let Some(sys_sid) = build_well_known_sid(WinLocalSystemSid) else { return };
        let Some(adm_sid) = build_well_known_sid(WinBuiltinAdministratorsSid) else { return };
        let Some(world_sid) = build_well_known_sid(WinWorldSid) else { return };

        let entries = [
            explicit_access(&sys_sid, KEY_ALL_RIGHTS, SET_ACCESS),
            explicit_access(&adm_sid, KEY_ALL_RIGHTS, SET_ACCESS),
            explicit_access(&world_sid, KEY_READ_RIGHTS, SET_ACCESS),
        ];

        let mut new_acl: *mut ACL = ptr::null_mut();
        let rc = SetEntriesInAclW(Some(&entries), None, &mut new_acl);
        if rc.0 != 0 { return; }

        let path = registry_named_path(service);
        let pcwstr = PCWSTR(path.as_ptr());

        let adm_psid = PSID(adm_sid.as_ptr() as _);
        let _ = SetNamedSecurityInfoW(
            pcwstr,
            SE_REGISTRY_KEY,
            OWNER_SECURITY_INFORMATION,
            Some(adm_psid),
            None,
            None,
            None,
        );

        let dacl_flags = OBJECT_SECURITY_INFORMATION(
            DACL_SECURITY_INFORMATION.0 | UNPROTECTED_DACL_SECURITY_INFORMATION.0,
        );
        let _ = SetNamedSecurityInfoW(
            pcwstr,
            SE_REGISTRY_KEY,
            dacl_flags,
            None,
            None,
            Some(new_acl as *const _),
            None,
        );

        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(new_acl as *mut _)));

        let ti_wide: Vec<u16> = TRUSTED_INSTALLER_SID
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut ti_psid = PSID(ptr::null_mut());
        if ConvertStringSidToSidW(PCWSTR(ti_wide.as_ptr()), &mut ti_psid).is_ok() {
            let _ = SetNamedSecurityInfoW(
                pcwstr,
                SE_REGISTRY_KEY,
                OWNER_SECURITY_INFORMATION,
                Some(ti_psid),
                None,
                None,
                None,
            );
            let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(ti_psid.0)));
        }
    }
}

pub fn is_registry_key_locked(service: &str) -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", service);
    match hklm.open_subkey_with_flags(&path, KEY_WRITE) {
        Ok(_key) => false,
        Err(e) => e.raw_os_error() == Some(5),
    }
}
