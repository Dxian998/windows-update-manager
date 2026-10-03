use crate::{scheduler, security, services, update};
use winreg::enums::*;
use winreg::RegKey;
use windows::Win32::System::Services::{
    SERVICE_AUTO_START, SERVICE_DEMAND_START, SERVICE_DISABLED,
};

pub fn toggle_store_access(enable: bool, protect: bool) {
    security::enable_privileges();

    let was_locked = protect || security::is_registry_key_locked("wuauserv") || security::is_registry_key_locked("WaaSMedicSvc");

    if enable {
        if was_locked {
            security::unlock_service_registry_key("wuauserv");
        }
        services::set_service_start("wuauserv", SERVICE_DEMAND_START);
        services::start_service("wuauserv");

        services::set_service_start("BITS", SERVICE_DEMAND_START);
        services::start_service("BITS");

        services::set_service_start("dosvc", SERVICE_AUTO_START);
        services::start_service("dosvc");

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(au_key) = hklm.open_subkey_with_flags(
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
            KEY_SET_VALUE,
        ) {
            let _ = au_key.delete_value("UseWUServer");
            let _ = au_key.set_value("NoAutoUpdate", &1u32);
            let _ = au_key.set_value("AUOptions", &2u32);
        }
        if let Ok(wu_key) = hklm.open_subkey_with_flags(
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate",
            KEY_SET_VALUE,
        ) {
            let _ = wu_key.delete_value("WUServer");
            let _ = wu_key.delete_value("WUStatusServer");
        }

        if was_locked {
            security::lock_service_registry_key("wuauserv");
        }
    } else {
        if was_locked {
            security::unlock_service_registry_key("wuauserv");
        }
        services::stop_service("wuauserv");
        services::set_service_start("wuauserv", SERVICE_DISABLED);

        if was_locked {
            security::lock_service_registry_key("wuauserv");
        }

        for _ in 0..3 {
            scheduler::disable_update_tasks();
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        update::apply_au_policy(true);
        update::clean_software_distribution();
    }
}

pub fn is_store_access_allowed() -> bool {
    let wua_start = services::get_service_start_value("wuauserv");
    wua_start != 4
}
