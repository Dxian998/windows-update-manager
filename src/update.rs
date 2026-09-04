use std::fs;
use std::path::PathBuf;
use winreg::{enums::*, RegKey};

use crate::{scheduler, security, services};
use windows::Win32::System::Services::{
    SERVICE_AUTO_START, SERVICE_DEMAND_START, SERVICE_DISABLED,
};

const BLOCK_SERVICES: &[(&str, bool)] = &[
    ("wuauserv", true),
    ("WaaSMedicSvc", true),
    ("UsoSvc", true),
    ("dosvc", true),
    ("BITS", false),
];

const LOCK_SERVICES: &[&str] = &["wuauserv", "WaaSMedicSvc", "UsoSvc", "dosvc"];

const RESTORE_START: &[(&str, windows::Win32::System::Services::SERVICE_START_TYPE)] = &[
    ("wuauserv", SERVICE_DEMAND_START),
    ("WaaSMedicSvc", SERVICE_DEMAND_START),
    ("UsoSvc", SERVICE_DEMAND_START),
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

    for (name, _should_disable) in BLOCK_SERVICES {
        services::stop_service(name);
    }

    services::set_service_start("BITS", SERVICE_DEMAND_START);

    for (name, should_disable) in BLOCK_SERVICES {
        if *should_disable {
            services::set_service_start(name, SERVICE_DISABLED);
        } else {
            services::set_service_start(name, SERVICE_DEMAND_START);
        }
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

    for name in LOCK_SERVICES {
        security::unlock_service_registry_key(name);
    }

    for (name, start_type) in RESTORE_START {
        services::set_service_start(name, *start_type);
    }

    apply_au_policy(false);
    apply_ifeo_blocks(false);
    scheduler::enable_update_tasks();
}

pub fn check_update_status() -> bool {
    let locked = security::is_registry_key_locked("wuauserv");
    let start = services::get_service_start_value("wuauserv");
    let uso_start = services::get_service_start_value("UsoSvc");
    start == 4 && (locked || uso_start == 4)
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
    let uso_start = services::get_service_start_value("UsoSvc");
    let medic_start = services::get_service_start_value("WaaSMedicSvc");
    let bits_start = services::get_service_start_value("BITS");
    let bits_running = services::is_service_running("BITS");
    let registry_locked = security::is_registry_key_locked("wuauserv");
    let ifeo_active = is_ifeo_active();
    let tasks_blocked = scheduler::are_tasks_blocked();

    let is_blocked = wua_start == 4 && (registry_locked || uso_start == 4);

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
            "wuauserv (Update)".to_string(),
            fmt_service(wua_start, wua_running),
        ),
        (
            "WaaSMedicSvc (Watchdog)".to_string(),
            fmt_service(medic_start, false),
        ),
        (
            "UsoSvc (Orchestrator)".to_string(),
            fmt_service(uso_start, services::is_service_running("UsoSvc")),
        ),
        (
            "BITS (Transfer)".to_string(),
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
        let _ = hklm.delete_subkey_all(r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU");

        if let Ok(wu_key) = hklm.open_subkey_with_flags(
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate",
            KEY_WRITE,
        ) {
            let _ = wu_key.delete_value("WUServer");
            let _ = wu_key.delete_value("WUStatusServer");
        }
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
