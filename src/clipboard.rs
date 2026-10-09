//! Clipboard writes marked private, so clipboard managers, Windows history and cloud sync skip them.

#[cfg(target_os = "macos")]
use arboard::SetExtApple;
#[cfg(windows)]
use arboard::SetExtWindows;

pub fn write_private(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let set = clipboard.set();
    #[cfg(target_os = "macos")]
    let set = set.exclude_from_history();
    #[cfg(windows)]
    let set = set
        .exclude_from_history()
        .exclude_from_cloud()
        .exclude_from_monitoring();
    set.text(text.to_string()).map_err(|e| e.to_string())
}

pub fn read_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}
