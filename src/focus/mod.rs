//! Where the user is typing, so the pill shows on that screen.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Centre of the focused window in desktop units (points on macOS, pixels on Windows), if known.
pub fn window_centre() -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    return macos::window_centre();
    #[cfg(windows)]
    return windows::window_centre();
    #[cfg(not(any(target_os = "macos", windows)))]
    return None;
}
