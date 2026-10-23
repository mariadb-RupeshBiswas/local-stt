//! Synthetic paste keystroke, and on Windows letting go of a console window the app does not need.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Why a paste keystroke was not sent.
#[derive(Debug, PartialEq, Eq)]
pub enum PasteError {
    /// The focused window runs elevated and ours does not, so Windows would drop the keys silently.
    ElevatedTarget,
    Failed(String),
}

impl std::fmt::Display for PasteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasteError::ElevatedTarget => f.write_str("the focused window runs as administrator"),
            PasteError::Failed(why) => f.write_str(why),
        }
    }
}

/// How a process token reads for the UIPI check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Elevation {
    Elevated,
    Normal,
    /// Access denied while opening the process or its token.
    Denied,
    Unknown,
}

/// UIPI drops input to a higher-integrity window with no error, so the only signal is this check.
/// Denied counts as blocked: a normal process cannot open an elevated one's token (access denied).
/// Unknown sends anyway; a wrong skip costs the paste, a wrong send only repeats the silent drop.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn uipi_blocks(ours: Elevation, target: Elevation) -> bool {
    ours == Elevation::Normal && matches!(target, Elevation::Elevated | Elevation::Denied)
}

pub fn send_paste() -> Result<(), PasteError> {
    #[cfg(target_os = "macos")]
    return macos::send_paste().map_err(PasteError::Failed);
    #[cfg(windows)]
    return windows::send_paste();
    #[cfg(not(any(target_os = "macos", windows)))]
    Err(PasteError::Failed(
        "paste is only supported on macOS and Windows".to_string(),
    ))
}

/// A long-lived child must not hold the caller's pipe open, or a script reading our output waits forever.
#[cfg(windows)]
pub fn keep_std_handles_private() {
    windows::keep_std_handles_private();
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

#[cfg(test)]
mod tests {
    use super::*;
    use Elevation::*;

    #[test]
    fn normal_us_elevated_or_denied_target_is_blocked() {
        assert!(uipi_blocks(Normal, Elevated));
        assert!(uipi_blocks(Normal, Denied));
    }

    #[test]
    fn same_or_lower_target_is_not_blocked() {
        assert!(!uipi_blocks(Normal, Normal));
        assert!(!uipi_blocks(Elevated, Elevated));
        assert!(!uipi_blocks(Elevated, Normal));
        assert!(!uipi_blocks(Elevated, Denied));
    }

    #[test]
    fn unreadable_state_sends_anyway() {
        assert!(!uipi_blocks(Normal, Unknown));
        assert!(!uipi_blocks(Unknown, Elevated));
    }
}
