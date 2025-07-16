use std::process::{Command, Stdio};

pub fn disable_service(service: &str) {
    // Stop service
    let _ = Command::new("net")
        .args(&["stop", service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Disable startup
    let _ = Command::new("sc")
        .args(&["config", service, "start=", "disabled"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn enable_service(service: &str) {
    // Enable startup
    let _ = Command::new("sc")
        .args(&["config", service, "start=", "auto"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Start service
    let _ = Command::new("net")
        .args(&["start", service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn enable_waasmedic(service: &str) {
    // Enable startup as demand (manual)
    let _ = Command::new("sc")
        .args(&["config", service, "start=", "demand"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Start service
    let _ = Command::new("net")
        .args(&["start", service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn get_service_status(service: &str) -> String {
    let output = Command::new("sc").args(&["qc", service]).output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.trim_start().starts_with("START_TYPE") {
                    return match line {
                        l if l.contains('4') => "Disabled".into(),
                        l if l.contains('3') => "Manual".into(),
                        l if l.contains('2') => "Automatic".into(),
                        _ => "Unknown".into(),
                    };
                }
            }
            "Unknown".into()
        }
        Err(_) => "Error".into(),
    }
}
