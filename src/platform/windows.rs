#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, GetWindowTextW};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::LASTINPUTINFO;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::GetLastInputInfo;
#[cfg(target_os = "windows")]
use windows::Win32::System::SystemInformation::GetTickCount;

#[cfg(target_os = "windows")]
pub fn get_active_app_name() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return String::new();
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return String::new();
        }

        if let Ok(proc) = psutil::process::Process::new(pid) {
            if let Ok(name) = proc.name() {
                return name.to_string();
            }
        }
    }
    String::new()
}

#[cfg(target_os = "windows")]
pub fn get_active_window_title() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return String::new();
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len > 0 {
            return String::from_utf16_lossy(&buf[..len as usize]);
        }
    }
    String::new()
}

#[cfg(target_os = "windows")]
pub fn is_user_active() -> bool {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).is_ok() {
            let current = GetTickCount();
            let idle_ms = current.saturating_sub(info.dwTime);
            return (idle_ms as f64 / 1000.0) < 5.0;
        }
    }
    true // Fallback
}

#[cfg(not(target_os = "windows"))]
pub fn get_active_app_name() -> String { String::new() }
#[cfg(not(target_os = "windows"))]
pub fn get_active_window_title() -> String { String::new() }
#[cfg(not(target_os = "windows"))]
pub fn is_user_active() -> bool { true }
