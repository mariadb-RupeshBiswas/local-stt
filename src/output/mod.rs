//! Synthetic paste keystroke. Owned by Task 5.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub fn send_paste() -> Result<(), String> {
    todo!("Task 5")
}
