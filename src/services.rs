use std::process::{Command, Stdio};

pub fn set_service_startup(service: &str, startup: &str) {
    let _ = Command::new("sc")
        .args(&["config", service, "start=", startup])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn reset_group_policy() {
    let _ = Command::new("secedit")
        .args(&["/configure", "/cfg", r"C:\Windows\inf\defltbase.inf", "/db", "defltbase.sdb", "/verbose"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    
    let _ = Command::new("cmd")
        .args(&["/c", "RD", "/S", "/Q", r"C:\Windows\System32\GroupPolicyUsers"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    
    let _ = Command::new("cmd")
        .args(&["/c", "RD", "/S", "/Q", r"C:\Windows\System32\GroupPolicy"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    
    let _ = Command::new("gpupdate")
        .arg("/force")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn get_service_status(service: &str) -> String {
    let output = Command::new("sc")
        .args(&["qc", service])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

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