//! Windows Ctrl+V via SendInput, and console detach.

use super::{uipi_blocks, Elevation, PasteError};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

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

fn denied_or_unknown() -> Elevation {
    if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
        Elevation::Denied
    } else {
        Elevation::Unknown
    }
}

fn token_elevation(process: HANDLE) -> Elevation {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: process is a live handle or pseudo-handle; token is a valid out pointer.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return denied_or_unknown();
    }
    let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0u32;
    // SAFETY: token is open, and the buffer is a live TOKEN_ELEVATION of the size passed.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut info as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    // SAFETY: token came from OpenProcessToken above and is closed once.
    unsafe { CloseHandle(token) };
    match (ok != 0, info.TokenIsElevated != 0) {
        (false, _) => Elevation::Unknown,
        (true, true) => Elevation::Elevated,
        (true, false) => Elevation::Normal,
    }
}

fn foreground_elevation() -> Elevation {
    let mut pid = 0u32;
    // SAFETY: both calls only read window state; pid is a valid out pointer.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() || GetWindowThreadProcessId(hwnd, &mut pid) == 0 || pid == 0 {
            return Elevation::Unknown;
        }
    }
    // SAFETY: plain value arguments; a null return means the open failed.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return denied_or_unknown();
    }
    let elevation = token_elevation(process);
    // SAFETY: process came from OpenProcess above and is closed once.
    unsafe { CloseHandle(process) };
    elevation
}

pub fn send_paste() -> Result<(), PasteError> {
    // SAFETY: returns a pseudo-handle for this process that needs no closing.
    let ours = token_elevation(unsafe { GetCurrentProcess() });
    if uipi_blocks(ours, foreground_elevation()) {
        return Err(PasteError::ElevatedTarget);
    }
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
    // UIPI blocking reports success here (no count, no error), so a short count is some other failure.
    Err(PasteError::Failed(format!(
        "SendInput accepted {sent} of {} events: {}",
        inputs.len(),
        std::io::Error::last_os_error()
    )))
}

/// Stops children from inheriting this process's stdin, stdout and stderr, which may be a caller's pipe.
pub fn keep_std_handles_private() {
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: GetStdHandle only reads this process's table; a null or invalid handle makes SetHandleInformation fail harmlessly.
        unsafe {
            let handle = GetStdHandle(which);
            if !handle.is_null() {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
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
