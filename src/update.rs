// NOTE: Force updating still causes updates that way MS Store and xbox app work.

use winreg::{RegKey, enums::*};

// use crate::services::{get_service_status, reset_group_policy, set_service_startup};

pub fn block_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Configure registry
    let (au_key, _) = hklm
        .create_subkey(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU")
        .expect("Failed to create AU key");
    au_key
        .set_value("NoAutoUpdate", &1u32)
        .expect("Write failed");
    // au_key.set_value("AUOptions", &1u32).expect("Write failed");

    /*
    // Configure Delivery Optimization
    let (do_key, _) = hklm
        .create_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config")
        .expect("Failed to create DO key");
    do_key
        .set_value("DODownloadMode", &0u32)
        .expect("Write failed");

    // Disable services
    for service in &["BITS", "wuauserv"] {
        set_service_startup(service, "disabled");
    }
    */
}

pub fn enable_updates() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Reset registry
    if let Ok(au_key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
        KEY_WRITE,
    ) {
        let _ = au_key.set_value("NoAutoUpdate", &0u32);
        let _ = au_key.set_value("AUOptions", &3u32);
    }

    /*
    // Reset Delivery Optimization
    if let Ok(do_key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config",
        KEY_WRITE,
    ) {
        let _ = do_key.set_value("DODownloadMode", &1u32);
    }

    // Enable services
    for service in &["BITS", "wuauserv"] {
        set_service_startup(service, "auto");
    }
    */

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
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
            "NoAutoRebootWithLoggedOnUsers",
        ),
        (
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
            "AUPowerManagement",
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
    /*
    // Delete registry trees
    let trees_to_delete = vec![
        r"Software\Microsoft\Windows\CurrentVersion\Policies",
        r"Software\Microsoft\WindowsSelfHost",
        r"Software\Policies",
    ];

    for tree in trees_to_delete {
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(tree);
        let _ = RegKey::predef(HKEY_LOCAL_MACHINE).delete_subkey_all(tree);
        let _ = RegKey::predef(HKEY_LOCAL_MACHINE)
            .delete_subkey_all(&format!(r"SOFTWARE\WOW6432Node\{}", tree));
    }

    // Reset group policies
    reset_group_policy();
    */
}

pub fn check_update_status() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut is_blocked = false;

    // Check registry settings
    let au_key_path = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";
    let do_key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config";

    if let Ok(au_key) = hklm.open_subkey(au_key_path) {
        let no_auto_update: u32 = au_key.get_value("NoAutoUpdate").unwrap_or(0);
        let au_options: u32 = au_key.get_value("AUOptions").unwrap_or(0);

        if let Ok(do_key) = hklm.open_subkey(do_key_path) {
            let do_download_mode: u32 = do_key.get_value("DODownloadMode").unwrap_or(1);
            if no_auto_update == 1 && au_options == 1 && do_download_mode == 0 {
                is_blocked = true;
            }
        }
    }

    /*
    // Check service statuses
    let services = vec!["BITS", "wuauserv", "UsoSvc", "WaaSMedicSvc"];

    if services
        .iter()
        .any(|&service| get_service_status(service) == "Disabled")
    {
        is_blocked = true;
    }
    */
    is_blocked
}

pub fn get_update_status() -> (bool, Vec<(String, String)>) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut is_blocked = false;
    
    // Check registry settings
    let au_key_path = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";
    let do_key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config";
    
    let registry_status = if let Ok(au_key) = hklm.open_subkey(au_key_path) {
        let no_auto_update: u32 = au_key.get_value("NoAutoUpdate").unwrap_or(0);
        let au_options: u32 = au_key.get_value("AUOptions").unwrap_or(0);
        if let Ok(do_key) = hklm.open_subkey(do_key_path) {
            let do_download_mode: u32 = do_key.get_value("DODownloadMode").unwrap_or(1);
            if no_auto_update == 1 && au_options == 1 && do_download_mode == 0 {
                is_blocked = true;
                "Blocked".to_string()
            } else {
                "Enabled".to_string()
            }
        } else {
            "Unknown".to_string()
        }
    } else {
        "Enabled".to_string()
    };
    
    (is_blocked, vec![("Registry Status".to_string(), registry_status)])
}

/* Legacy Function
pub fn get_update_status() -> (bool, Vec<(String, String)>) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut is_blocked = false;
    let mut status_details = Vec::new();

    // Check registry settings
    let au_key_path = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";
    let do_key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config";

    let registry_status = if let Ok(au_key) = hklm.open_subkey(au_key_path) {
        let no_auto_update: u32 = au_key.get_value("NoAutoUpdate").unwrap_or(0);
        let au_options: u32 = au_key.get_value("AUOptions").unwrap_or(0);

        if let Ok(do_key) = hklm.open_subkey(do_key_path) {
            let do_download_mode: u32 = do_key.get_value("DODownloadMode").unwrap_or(1);
            if no_auto_update == 1 && au_options == 1 && do_download_mode == 0 {
                is_blocked = true;
                "Blocked".to_string()
            } else {
                "Enabled".to_string()
            }
        } else {
            "Unknown".to_string()
        }
    } else {
        "Enabled".to_string()
    };
    
    // Check service statuses
    let services = vec![
        ("BITS", get_service_status("BITS")),
        ("wuauserv", get_service_status("wuauserv")),
        ("UsoSvc", get_service_status("UsoSvc")),
        ("WaaSMedicSvc", get_service_status("WaaSMedicSvc")),
    ];

    if services.iter().any(|(_, status)| status == "Disabled") {
        is_blocked = true;
    }

    status_details.push(("Registry Status".to_string(), registry_status));
    for (service, status) in services {
        status_details.push((service.to_string(), status));
    }

    (is_blocked, status_details)
}
*/