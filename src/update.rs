use crate::services;
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
    services::disable_service("wuauserv");
    services::disable_service("UsoSvc");
    services::disable_service("WaaSMedicSvc");
}

pub fn enable_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Remove registry keys
    let _ = hklm.delete_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate\\AU");
    let _ = hklm.delete_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate");

    // Enable services
    services::enable_service("wuauserv");
    services::enable_service("UsoSvc");
    services::enable_service("WaaSMedicSvc");
}

pub fn check_update_status() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Check registry settings
    if let Ok(au_key) = hklm.open_subkey_with_flags(
        "SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsUpdate\\AU",
        KEY_READ,
    ) {
        if let Ok(no_auto_update) = au_key.get_value::<u32, _>("NoAutoUpdate") {
            if no_auto_update == 1 {
                return true;
            }
        }
    }

    // Check service status
    services::get_service_status("wuauserv") == "Disabled"
        || services::get_service_status("UsoSvc") == "Disabled"
        || services::get_service_status("WaaSMedicSvc") == "Disabled"
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
