//! Keeps Windows from throttling the app while it works in the background.

#[cfg(windows)]
mod windows;

/// Opts the process out of Windows power throttling (EcoQoS), which parks a background app on slow cores.
pub fn stay_fast() -> Result<(), String> {
    #[cfg(windows)]
    return windows::stay_fast();
    #[cfg(not(windows))]
    return Ok(());
}

#[cfg(test)]
mod tests {
    #[test]
    fn stay_fast_succeeds() {
        assert_eq!(super::stay_fast(), Ok(()));
    }
}
