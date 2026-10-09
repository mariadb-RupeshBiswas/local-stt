//! Synthetic paste keystroke.

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
