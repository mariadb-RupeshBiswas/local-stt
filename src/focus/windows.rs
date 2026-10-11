//! The foreground window's rectangle.

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

pub fn window_centre() -> Option<(f64, f64)> {
    // SAFETY: GetForegroundWindow takes nothing; GetWindowRect only fills `r` for that handle.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return None;
        }
        Some((
            (r.left + r.right) as f64 / 2.0,
            (r.top + r.bottom) as f64 / 2.0,
        ))
    }
}
