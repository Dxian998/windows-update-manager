use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::TaskScheduler::{ITaskService, TaskScheduler};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::core::BSTR;

const UPDATE_TASKS: &[(&str, &str)] = &[
    (r"\Microsoft\Windows\WindowsUpdate", "Scheduled Start"),
    (r"\Microsoft\Windows\WaaSMedic", "MaintenanceWork"),
];

const OPTIONAL_TASKS: &[(&str, &str)] = &[
    (r"\Microsoft\Windows\WindowsUpdate", "sih"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Schedule Scan"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Scheduled Start"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Schedule Maintenance Work"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Schedule Wake To Work"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "USO_UxBroker"),
];

pub fn disable_update_tasks() {
    set_tasks_enabled(false);
}

pub fn enable_update_tasks() {
    set_tasks_enabled(true);
}

pub fn are_tasks_blocked() -> bool {
    with_task_service(|svc| {
        let mut found_any = false;
        for (folder_path, task_name) in UPDATE_TASKS {
            let folder = match unsafe { svc.GetFolder(&BSTR::from(*folder_path)) } {
                Ok(f) => f,
                Err(_) => continue,
            };
            let task = match unsafe { folder.GetTask(&BSTR::from(*task_name)) } {
                Ok(t) => t,
                Err(_) => continue,
            };
            found_any = true;
            let enabled = match unsafe { task.Enabled() } {
                Ok(v) => v,
                Err(_) => continue,
            };
            if enabled.0 != 0 {
                return false;
            }
        }
        found_any
    })
    .unwrap_or(false)
}

fn set_tasks_enabled(enable: bool) {
    let flag = if enable {
        VARIANT_BOOL(-1)
    } else {
        VARIANT_BOOL(0)
    };

    let _ = with_task_service(|svc| {
        for (folder_path, task_name) in UPDATE_TASKS.iter().chain(OPTIONAL_TASKS.iter()) {
            let folder = match unsafe { svc.GetFolder(&BSTR::from(*folder_path)) } {
                Ok(f) => f,
                Err(_) => continue,
            };
            let task = match unsafe { folder.GetTask(&BSTR::from(*task_name)) } {
                Ok(t) => t,
                Err(_) => continue,
            };
            let _ = unsafe { task.SetEnabled(flag) };
        }
        true
    });
}

fn with_task_service<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&ITaskService) -> R,
{
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.0 < 0 && hr.0 != -2147417850 {
            return None;
        }

        let svc: windows::core::Result<ITaskService> =
            CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER);
        let svc = match svc {
            Ok(s) => s,
            Err(_) => {
                if hr.0 >= 0 {
                    CoUninitialize();
                }
                return None;
            }
        };

        let empty = VARIANT::default();
        if svc.Connect(&empty, &empty, &empty, &empty).is_err() {
            if hr.0 >= 0 {
                CoUninitialize();
            }
            return None;
        }

        let ret = f(&svc);
        drop(svc);
        if hr.0 >= 0 {
            CoUninitialize();
        }
        Some(ret)
    }
}
