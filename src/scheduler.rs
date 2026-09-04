use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::TaskScheduler::{ITaskService, CLSID_CTaskScheduler};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::core::BSTR;

const UPDATE_TASKS: &[(&str, &str)] = &[
    (r"\Microsoft\Windows\WaaSMedic", "PerformRemediation"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Schedule Scan"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Scheduled Start"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Schedule Maintenance Work"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "Schedule Wake To Work"),
    (r"\Microsoft\Windows\UpdateOrchestrator", "USO_UxBroker"),
    (r"\Microsoft\Windows\WindowsUpdate", "Scheduled Start"),
    (r"\Microsoft\Windows\WindowsUpdate", "sih"),
];

pub fn disable_update_tasks() {
    set_tasks_enabled(false);
}

pub fn enable_update_tasks() {
    set_tasks_enabled(true);
}

pub fn are_tasks_blocked() -> bool {
    with_task_service(|svc| {
        let folder = match unsafe {
            svc.GetFolder(&BSTR::from(r"\Microsoft\Windows\WaaSMedic"))
        } {
            Ok(f) => f,
            Err(_) => return false,
        };
        let task = match unsafe { folder.GetTask(&BSTR::from("PerformRemediation")) } {
            Ok(t) => t,
            Err(_) => return false,
        };
        let enabled = match unsafe { task.Enabled() } {
            Ok(v) => v,
            Err(_) => return false,
        };
        enabled == VARIANT_BOOL(0)
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
        for (folder_path, task_name) in UPDATE_TASKS {
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
        if hr.0 < 0 {
            return None;
        }

        let result = (|| -> windows::core::Result<R> {
            let svc: ITaskService =
                CoCreateInstance(&CLSID_CTaskScheduler, None, CLSCTX_INPROC_SERVER)?;

            let empty = VARIANT::default();
            svc.Connect(&empty, &empty, &empty, &empty)?;

            Ok(f(&svc))
        })();

        CoUninitialize();
        result.ok()
    }
}
