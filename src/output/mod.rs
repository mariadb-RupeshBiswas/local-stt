//! Synthetic paste keystroke, and on Windows letting go of a console window the app does not need.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub fn send_paste() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::send_paste();
    #[cfg(windows)]
    return windows::send_paste();
    #[cfg(not(any(target_os = "macos", windows)))]
    Err("paste is only supported on macOS and Windows".to_string())
}

/// True when this process shares its console with others, which means a terminal started it.
#[cfg(windows)]
pub fn console_is_shared() -> bool {
    windows::console_process_count() > 1
}

/// Closes the console Windows opened for a Start menu or login launch; a terminal keeps its own.
pub fn detach_own_console() {
    #[cfg(windows)]
    windows::detach_own_console();
}
