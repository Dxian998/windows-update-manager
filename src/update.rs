// NOTE: Force updating still causes updates that way MS Store and xbox app work.

use winreg::{RegKey, enums::*};

pub fn block_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Configure registry
    let (au_key, _) = hklm
        .create_subkey(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU")
        .expect("Failed to create AU key");
    au_key
        .set_value("NoAutoUpdate", &1u32)
        .expect("Write failed");
}

pub fn enable_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Delete registry
    let _ = hklm.delete_subkey_all(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU");

    // Remove registry values
    let keys_to_clean = vec![
        (
            r"SOFTWARE\Policies\Microsoft\Windows\Device Metadata",
            "PreventDeviceMetadataFromNetwork",
        ),
        (
            r"SOFTWARE\Policies\Microsoft\Windows\DriverSearching",
            "DontPromptForWindowsUpdate",
        ),
        (
            r"SOFTWARE\Policies\Microsoft\Windows\DriverSearching",
            "DontSearchWindowsUpdate",
        ),
        (
            r"SOFTWARE\Policies\Microsoft\Windows\DriverSearching",
            "DriverUpdateWizardWuSearchEnabled",
        ),
        (
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate",
            "ExcludeWUDriversInQualityUpdate",
        ),
        (
            r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings",
            "BranchReadinessLevel",
        ),
        (
            r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings",
            "DeferFeatureUpdatesPeriodInDays",
        ),
        (
            r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings",
            "DeferQualityUpdatesPeriodInDays",
        ),
    ];

    for (key_path, value) in keys_to_clean {
        if let Ok(key) = hklm.open_subkey_with_flags(key_path, KEY_WRITE) {
            let _ = key.delete_value(value);
        }
    }
}

pub fn check_update_status() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    
    let au_key_path = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";
    
    if let Ok(au_key) = hklm.open_subkey(au_key_path) {
        let no_auto_update: u32 = au_key.get_value("NoAutoUpdate").unwrap_or(0);
        no_auto_update == 1
    } else {
        false
    }
}

pub fn get_update_status() -> (bool, Vec<(String, String)>) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    
    let au_key_path = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";
    
    let (is_blocked, registry_status) = if let Ok(au_key) = hklm.open_subkey(au_key_path) {
        let no_auto_update: u32 = au_key.get_value("NoAutoUpdate").unwrap_or(0);

        if no_auto_update == 1 {
            (true, "Blocked".to_string())
        } else {
            (false, "Enabled".to_string())
        }
    } else {
        (false, "Enabled".to_string())
    };
    
    (is_blocked, vec![("Registry Status".to_string(), registry_status)])
}
