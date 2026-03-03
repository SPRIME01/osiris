pub mod linux;
pub mod macos;
pub mod windows;

pub fn active_app_name() -> String {
    #[cfg(target_os = "linux")]
    { linux::get_active_app_name() }

    #[cfg(target_os = "macos")]
    { macos::get_active_app_name() }

    #[cfg(target_os = "windows")]
    { windows::get_active_app_name() }
}

pub fn active_window_title() -> String {
    #[cfg(target_os = "linux")]
    { linux::get_active_window_title() }

    #[cfg(target_os = "macos")]
    { macos::get_active_window_title() }

    #[cfg(target_os = "windows")]
    { windows::get_active_window_title() }
}

pub fn is_user_active() -> bool {
    #[cfg(target_os = "linux")]
    { linux::is_user_active() }

    #[cfg(target_os = "macos")]
    { macos::is_user_active() }

    #[cfg(target_os = "windows")]
    { windows::is_user_active() }
}
