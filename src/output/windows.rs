//! Windows Ctrl+V via SendInput, and console detach.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_V,
};

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_KEYUP } else { 0 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> u32 {
    // SAFETY: the pointer and count come from one live slice; cbSize is the real INPUT size.
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    }
}

pub fn send_paste() -> Result<(), String> {
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];
    let sent = send(&inputs);
    if sent as usize == inputs.len() {
        return Ok(());
    }
    // Blocked input (for example an elevated target window) reports fewer events than sent.
    Err(format!(
        "SendInput accepted {sent} of {} events: {}",
        inputs.len(),
        std::io::Error::last_os_error()
    ))
}

pub fn console_process_count() -> u32 {
    use windows_sys::Win32::System::Console::GetConsoleProcessList;
    let mut ids = [0u32; 2];
    // SAFETY: the buffer is a live array of the length passed.
    unsafe { GetConsoleProcessList(ids.as_mut_ptr(), ids.len() as u32) }
}

pub fn detach_own_console() {
    use windows_sys::Win32::System::Console::FreeConsole;
    // Only this process on the console means Windows made it for us, not a terminal the user opened.
    if console_process_count() == 1 {
        // SAFETY: takes no arguments; afterwards this process simply has no console.
        unsafe { FreeConsole() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_sequence_is_ctrl_v_down_then_up() {
        let seq = [
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ];
        let keys: Vec<(u16, bool)> = seq
            .iter()
            .map(|i| {
                // SAFETY: every INPUT here was built as a keyboard event, so the ki arm is the live one.
                let ki = unsafe { i.Anonymous.ki };
                (ki.wVk, ki.dwFlags & KEYEVENTF_KEYUP != 0)
            })
            .collect();
        assert_eq!(
            keys,
            vec![
                (VK_CONTROL, false),
                (VK_V, false),
                (VK_V, true),
                (VK_CONTROL, true)
            ]
        );
    }
}
