use std::fs;
use std::path::PathBuf;
use winreg::{enums::*, RegKey};

use crate::{scheduler, security, services};
use windows::Win32::System::Services::{
    SERVICE_AUTO_START, SERVICE_DEMAND_START, SERVICE_DISABLED,
};

const BLOCK_SERVICES: &[(&str, windows::Win32::System::Services::SERVICE_START_TYPE)] = &[
    ("wuauserv", SERVICE_DISABLED),
    ("WaaSMedicSvc", SERVICE_DISABLED),
    ("UsoSvc", SERVICE_DISABLED),
    ("dosvc", SERVICE_AUTO_START),
    ("BITS", SERVICE_DEMAND_START),
];

const LOCK_SERVICES: &[&str] = &["wuauserv", "WaaSMedicSvc"];

const RESTORE_START: &[(&str, windows::Win32::System::Services::SERVICE_START_TYPE)] = &[
    ("wuauserv", SERVICE_AUTO_START),
    ("WaaSMedicSvc", SERVICE_DEMAND_START),
    ("UsoSvc", SERVICE_AUTO_START),
    ("dosvc", SERVICE_AUTO_START),
    ("BITS", SERVICE_AUTO_START),
];

const IFEO_TARGETS: &[&str] = &[
    "WaaSMedic.exe",
    "WaasMedicAgent.exe",
    "UsoClient.exe",
    "SihClient.exe",
    "remsh.exe",
    "UpdateAssistant.exe",
    "Windows10Upgrade.exe",
    "MusNotification.exe",
    "MusNotificationUx.exe",
    "MoNotificationUx.exe",
];

const IFEO_BASE: &str =
    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options";

pub fn block_updates(protect_service_settings: bool) {
    security::enable_privileges();

    for (name, _) in BLOCK_SERVICES {
        if *name != "dosvc" {
            services::stop_service(name);
        }
    }

    for (name, start_type) in BLOCK_SERVICES {
        services::set_service_start(name, *start_type);
    }

    if protect_service_settings {
        for name in LOCK_SERVICES {
            security::lock_service_registry_key(name);
        }
    }

    apply_au_policy(true);
    apply_ifeo_blocks(true);
    scheduler::disable_update_tasks();
    clean_software_distribution();
}

pub fn set_protect_service_settings(protect: bool) {
    security::enable_privileges();
    for name in LOCK_SERVICES {
        if protect {
            security::lock_service_registry_key(name);
        } else {
            security::unlock_service_registry_key(name);
        }
    }
}

pub fn enable_updates() {
    security::enable_privileges();

    apply_ifeo_blocks(false);
    apply_au_policy(false);
    scheduler::enable_update_tasks();

    for name in LOCK_SERVICES {
        security::unlock_service_registry_key(name);
    }

    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    for (name, start_type) in RESTORE_START {
        let path = format!("SYSTEM\\CurrentControlSet\\Services\\{}", name);
        if let Ok(key) = hklm.open_subkey_with_flags(&path, winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_QUERY_VALUE) {
            let _ = key.delete_value("WubLock");

            if let Ok(image_path) = key.get_value::<String, _>("ImagePath") {
                if image_path.contains("wusvcs") {
                    let correct_group = if *name == "WaaSMedicSvc" { "WaaSMedicSvc" } else { "netsvcs" };
                    let fixed = image_path.replace("wusvcs", correct_group);
                    let utf16: Vec<u8> = fixed.encode_utf16()
                        .chain(std::iter::once(0))
                        .flat_map(|c| c.to_ne_bytes())
                        .collect();
                    let raw = winreg::RegValue {
                        vtype: winreg::enums::REG_EXPAND_SZ,
                        bytes: utf16,
                    };
                    let _ = key.set_raw_value("ImagePath", &raw);
                    services::fix_service_img_path(name, &fixed);
                }
            }

            let _ = key.set_value("Start", &(start_type.0 as u32));
        }
        services::set_service_start(name, *start_type);
    }

    services::start_service("UsoSvc");
    services::start_service("BITS");
    services::start_service("dosvc");

    for _ in 0..5 {
        if services::is_service_running("wuauserv") {
            break;
        }
        services::start_service("wuauserv");
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

pub fn check_update_status() -> bool {
    let locked = security::is_registry_key_locked("wuauserv");
    let start = services::get_service_start_value("wuauserv");
    let medic_start = services::get_service_start_value("WaaSMedicSvc");
    start == 4 && (locked || medic_start == 4)
}

pub fn toggle_bits(update_blocked: bool) {
    let current = services::get_service_start_value("BITS");
    if current == 4 {
        let target = if update_blocked {
            SERVICE_DEMAND_START
        } else {
            SERVICE_AUTO_START
        };
        services::set_service_start("BITS", target);
    } else {
        services::stop_service("BITS");
        services::set_service_start("BITS", SERVICE_DISABLED);
    }
}

pub fn get_update_status() -> (bool, Vec<(String, String)>) {
    let wua_start = services::get_service_start_value("wuauserv");
    let wua_running = services::is_service_running("wuauserv");
    let dosvc_start = services::get_service_start_value("dosvc");
    let dosvc_running = services::is_service_running("dosvc");
    let uso_start = services::get_service_start_value("UsoSvc");
    let medic_start = services::get_service_start_value("WaaSMedicSvc");
    let bits_start = services::get_service_start_value("BITS");
    let bits_running = services::is_service_running("BITS");
    let registry_locked = security::is_registry_key_locked("wuauserv");
    let ifeo_active = is_ifeo_active();
    let tasks_blocked = scheduler::are_tasks_blocked();
    let is_blocked = wua_start == 4 && (registry_locked || medic_start == 4);

    let fmt_service = |start: u32, running: bool| -> String {
        match start {
            4 => "Disabled".to_string(),
            3 => {
                if running {
                    "Manual (Running)".to_string()
                } else {
                    "Manual (Stopped)".to_string()
                }
            }
            2 => {
                if running {
                    "Auto (Running)".to_string()
                } else {
                    "Auto".to_string()
                }
            }
            _ => "Unknown".to_string(),
        }
    };

    let details = vec![
        (
            "wuauserv".to_string(),
            fmt_service(wua_start, wua_running),
        ),
        (
            "dosvc".to_string(),
            fmt_service(dosvc_start, dosvc_running),
        ),
        (
            "WaaSMedicSvc".to_string(),
            fmt_service(medic_start, false),
        ),
        (
            "UsoSvc".to_string(),
            fmt_service(uso_start, services::is_service_running("UsoSvc")),
        ),
        (
            "BITS".to_string(),
            fmt_service(bits_start, bits_running),
        ),
        (
            "Registry ACL Lock".to_string(),
            if registry_locked {
                "Locked".to_string()
            } else {
                "Unlocked".to_string()
            },
        ),
        (
            "IFEO Shield".to_string(),
            if ifeo_active {
                "Active".to_string()
            } else {
                "Inactive".to_string()
            },
        ),
        (
            "Scheduled Tasks".to_string(),
            if tasks_blocked {
                "Blocked".to_string()
            } else {
                "Enabled".to_string()
            },
        ),
    ];

    (is_blocked, details)
}

fn apply_au_policy(block: bool) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    if block {
        if let Ok((au_key, _)) =
            hklm.create_subkey(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU")
        {
            au_key.set_value("NoAutoUpdate", &1u32).ok();
            au_key.set_value("AUOptions", &1u32).ok();
            au_key.set_value("UseWUServer", &1u32).ok();
        }

        if let Ok((wu_key, _)) =
            hklm.create_subkey(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate")
        {
            wu_key.set_value("WUServer", &"").ok();
            wu_key.set_value("WUStatusServer", &"").ok();
        }
    } else {
        if let Ok(au_key) = hklm.open_subkey_with_flags(
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
            KEY_SET_VALUE,
        ) {
            let _ = au_key.delete_value("NoAutoUpdate");
            let _ = au_key.delete_value("AUOptions");
            let _ = au_key.delete_value("UseWUServer");
        }
        if let Ok(wu_key) = hklm.open_subkey_with_flags(
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate",
            KEY_SET_VALUE,
        ) {
            let _ = wu_key.delete_value("WUServer");
            let _ = wu_key.delete_value("WUStatusServer");
            let _ = wu_key.delete_value("DisableWindowsUpdateAccess");
        }
        let _ = hklm.delete_subkey_all(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate");
    }
}

fn apply_ifeo_blocks(block: bool) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    for exe in IFEO_TARGETS {
        let path = format!("{}\\{}", IFEO_BASE, exe);
        if block {
            if let Ok((key, _)) = hklm.create_subkey(&path) {
                let _ = key.set_value("Debugger", &"/");
            }
        } else {
            if let Ok(key) = hklm.open_subkey_with_flags(&path, KEY_WRITE) {
                let _ = key.delete_value("Debugger");
            }
            let _ = hklm.delete_subkey(&path);
        }
    }

    if !block {
        if let Ok(ifeo_root) = hklm.open_subkey(IFEO_BASE) {
            let subkeys: Vec<String> = ifeo_root.enum_keys().filter_map(|k| k.ok()).collect();
            for name in subkeys {
                let path = format!("{}\\{}", IFEO_BASE, name);
                if let Ok(key) = hklm.open_subkey_with_flags(&path, KEY_READ | KEY_SET_VALUE) {
                    let debugger: Result<String, _> = key.get_value("Debugger");
                    if let Ok(d) = debugger {
                        if d == "/" || d.is_empty() {
                            let _ = key.delete_value("Debugger");
                        }
                    }
                }
            }
        }
    }
}

fn is_ifeo_active() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!("{}\\{}", IFEO_BASE, "WaasMedicAgent.exe");
    if let Ok(key) = hklm.open_subkey(&path) {
        let val: Result<String, _> = key.get_value("Debugger");
        return val.is_ok();
    }
    false
}

fn clean_software_distribution() {
    let dl_path = PathBuf::from(r"C:\Windows\SoftwareDistribution\Download");
    if let Ok(entries) = fs::read_dir(&dl_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let _ = fs::remove_dir_all(&p);
            } else {
                let _ = fs::remove_file(&p);
            }
        }
    }
}
