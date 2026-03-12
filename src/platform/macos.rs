use std::process::Command;

pub fn get_active_app_name() -> String {
    // using osascript to get frontmost app
    let script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }

    String::new()
}

pub fn get_active_window_title() -> String {
    let app_name = get_active_app_name();
    if app_name.is_empty() {
        return String::new();
    }

    // Sanitize app_name to prevent AppleScript injection.
    // Escape backslashes first, then double quotes, so that an attacker-controlled
    // process name cannot break out of the quoted string in the script.
    let safe_app_name = app_name.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"tell application "System Events" to tell process "{}" to get name of front window"#,
        safe_app_name
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }

    String::new()
}

pub fn is_user_active() -> bool {
    let output = Command::new("ioreg")
        .args(&["-c", "IOHIDSystem", "-r", "-k", "HIDIdleTime"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("HIDIdleTime") {
                    if let Some(val_str) = line.split('=').last() {
                        if let Ok(idle_ns) = val_str.trim().parse::<u64>() {
                            let idle_s = idle_ns as f64 / 1_000_000_000.0;
                            return idle_s < 5.0;
                        }
                    }
                }
            }
        }
    }

    true // Fallback
}
