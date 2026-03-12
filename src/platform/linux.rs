use std::process::Command;
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref RE_WINDOW_ID: Regex = Regex::new(r"window id # (0x[0-9a-fA-F]+)").unwrap();
    static ref RE_WM_CLASS: Regex = Regex::new(r#"WM_CLASS\(STRING\) = "([^"]+)""#).unwrap();
    static ref RE_NET_WM_NAME: Regex = Regex::new(r#"_NET_WM_NAME\(UTF8_STRING\) = "([^"]*)""#).unwrap();
    static ref RE_WM_NAME: Regex = Regex::new(r#"WM_NAME\([^)]*\) = "([^"]*)""#).unwrap();
}

fn get_active_window_id() -> Option<String> {
    let output = Command::new("xprop")
        .args(&["-root", "_NET_ACTIVE_WINDOW"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    RE_WINDOW_ID.captures(&stdout).map(|cap| cap[1].to_string())
}

pub fn get_active_app_name() -> String {
    let window_id = match get_active_window_id() {
        Some(id) => id,
        None => return String::new(),
    };

    let output = Command::new("xprop")
        .args(&["-id", &window_id, "WM_CLASS"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(cap) = RE_WM_CLASS.captures(&stdout) {
                return cap[1].to_string();
            }
        }
    }

    String::new()
}

pub fn get_active_window_title() -> String {
    let window_id = match get_active_window_id() {
        Some(id) => id,
        None => return String::new(),
    };

    for prop in &["_NET_WM_NAME", "WM_NAME"] {
        let output = Command::new("xprop")
            .args(&["-id", &window_id, prop])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);

                let re = if *prop == "_NET_WM_NAME" { &*RE_NET_WM_NAME } else { &*RE_WM_NAME };

                if let Some(cap) = re.captures(&stdout) {
                    return cap[1].to_string();
                }
            }
        }
    }

    String::new()
}

pub fn is_user_active() -> bool {
    let output = Command::new("xprintidle").output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(idle_ms) = stdout.trim().parse::<u64>() {
                return (idle_ms as f64 / 1000.0) < 5.0;
            }
        }
    }

    true // Fallback
}
