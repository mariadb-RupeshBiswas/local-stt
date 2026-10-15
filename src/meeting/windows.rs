//! Who is using the microphone on Windows, from the privacy settings' record of each app's microphone use.
//! An app whose last use has a start time and no stop time is recording now.

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible};

const STORE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// An open registry key, closed once.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key was opened by RegOpenKeyExW and is closed exactly once.
        unsafe { RegCloseKey(self.0) };
    }
}

fn open(parent: HKEY, path: &str) -> Option<Key> {
    let path = wide(path);
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: path is NUL-terminated UTF-16 and `key` receives the opened handle.
    let rc = unsafe { RegOpenKeyExW(parent, path.as_ptr(), 0, KEY_READ, &mut key) };
    (rc == ERROR_SUCCESS).then_some(Key(key))
}

fn subkeys(key: &Key) -> Vec<String> {
    let mut out = Vec::new();
    for index in 0u32.. {
        let mut name = [0u16; 512];
        let mut len = name.len() as u32;
        // SAFETY: the buffer holds `len` UTF-16 units; the call writes the name and its length.
        let rc = unsafe {
            RegEnumKeyExW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut len,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            break;
        }
        out.push(String::from_utf16_lossy(&name[..len as usize]));
    }
    out
}

fn qword(key: &Key, value: &str) -> Option<u64> {
    let value = wide(value);
    let mut data = 0u64;
    let mut size = std::mem::size_of::<u64>() as u32;
    // SAFETY: an 8-byte buffer with its size; the value name is NUL-terminated UTF-16.
    let rc = unsafe {
        RegQueryValueExW(
            key.0,
            value.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            (&mut data as *mut u64).cast(),
            &mut size,
        )
    };
    (rc == ERROR_SUCCESS).then_some(data)
}

fn recording(key: &Key) -> bool {
    qword(key, "LastUsedTimeStop") == Some(0) && qword(key, "LastUsedTimeStart").unwrap_or(0) != 0
}

/// Executable names ("zoom.exe") and package names ("MSTeams_8wekyb3d8bbwe") recording from a microphone now.
pub fn mic_users() -> Vec<String> {
    let Some(store) = open(HKEY_CURRENT_USER, STORE) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in subkeys(&store) {
        if name == "NonPackaged" {
            continue;
        }
        if open(store.0, &name).is_some_and(|k| recording(&k)) {
            out.push(name);
        }
    }
    let Some(plain) = open(store.0, "NonPackaged") else {
        return out;
    };
    // Desktop apps are keyed by their path with '#' for '\': "C:#Program Files#Zoom#bin#Zoom.exe".
    for name in subkeys(&plain) {
        if open(plain.0, &name).is_some_and(|k| recording(&k)) {
            out.push(name.rsplit('#').next().unwrap_or(&name).to_string());
        }
    }
    out
}

unsafe extern "system" fn collect_title(window: HWND, titles: LPARAM) -> i32 {
    // SAFETY: `titles` is the Vec passed by browser_titles, alive for the whole EnumWindows call.
    let titles = unsafe { &mut *(titles as *mut Vec<String>) };
    // SAFETY: window is a live top-level window handle given by EnumWindows.
    if unsafe { IsWindowVisible(window) } == 0 {
        return 1;
    }
    let mut buf = [0u16; 512];
    // SAFETY: the buffer holds 512 UTF-16 units; the call returns how many it wrote.
    let len = unsafe { GetWindowTextW(window, buf.as_mut_ptr(), buf.len() as i32) };
    if len > 0 {
        titles.push(String::from_utf16_lossy(&buf[..len as usize]));
    }
    1
}

/// Titles of visible top-level windows; a browser's names its active tab.
pub fn browser_titles() -> Vec<String> {
    let mut titles: Vec<String> = Vec::new();
    // SAFETY: the callback only touches `titles`, which outlives this synchronous call.
    unsafe {
        EnumWindows(
            Some(collect_title),
            &mut titles as *mut Vec<String> as LPARAM,
        )
    };
    titles
}
