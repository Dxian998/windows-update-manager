use crate::services::{
    disable_service, enable_service, enable_waasmedic, get_service_status,
};
use winreg::{RegKey, enums::*};

pub fn block_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Create registry keys
    let _ = hklm.create_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate");
    let (au_key, _) = hklm
        .create_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate\\AU")
        .expect("Failed to create AU registry key");

    // Set registry values
    au_key
        .set_value("NoAutoUpdate", &1u32)
        .expect("Failed to set NoAutoUpdate");
    au_key
        .set_value("AUOptions", &1u32)
        .expect("Failed to set AUOptions");

    // Block update access
    if let Ok((update_key, _)) =
        hklm.create_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate")
    {
        update_key
            .set_value("DisableWindowsUpdateAccess", &1u32)
            .expect("Failed to set DisableWindowsUpdateAccess");
    }

    // Disable services
    disable_service("wuauserv");
    disable_service("UsoSvc");
    disable_service("WaaSMedicSvc");
}

pub fn enable_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Remove registry keys
    let _ = hklm.delete_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate\\AU");
    let _ = hklm.delete_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate");

    // Enable services
    enable_service("wuauserv");
    enable_service("UsoSvc");
    enable_waasmedic("WaaSMedicSvc");
}

pub fn check_update_status() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut reg_blocked = false;

    // Check registry settings
    if let Ok(au_key) = hklm.open_subkey_with_flags(
        "SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate\\AU",
        KEY_READ,
    ) {
        if let Ok(no_auto_update) = au_key.get_value::<u32, _>("NoAutoUpdate") {
            if no_auto_update == 1 {
                reg_blocked = true;
            }
        }
    }

    // Check service status
    let wuauserv_disabled = get_service_status("wuauserv") == "Disabled";
    let usosvc_disabled = get_service_status("UsoSvc") == "Disabled";

    // if (registry blocked AND wuauserv disabled) OR (wuauserv AND UsoSvc disabled)
    (reg_blocked && wuauserv_disabled) || (wuauserv_disabled && usosvc_disabled)
}

pub fn get_registry_status() -> String {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    if let Ok(au) = hklm.open_subkey_with_flags(
        "SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate\\AU",
        KEY_READ,
    ) {
        if let Ok(no_auto_update) = au.get_value::<u32, _>("NoAutoUpdate") {
            if no_auto_update == 1 {
                return "Blocked".into();
            }
        }
    }
    "Enabled".into()
}
