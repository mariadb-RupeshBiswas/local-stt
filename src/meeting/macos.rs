//! Who is using the microphone, from Core Audio's process objects (macOS 14 and later).

use std::ffi::{c_void, CStr};

type CFTypeRef = *const c_void;

#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

const SYSTEM_OBJECT: u32 = 1;
const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const PROCESS_LIST: u32 = u32::from_be_bytes(*b"prs#");
const PROCESS_BUNDLE_ID: u32 = u32::from_be_bytes(*b"pbid");
const PROCESS_RUNNING_INPUT: u32 = u32::from_be_bytes(*b"piri");
const UTF8: u32 = 0x0800_0100;
// Browsers whose window titles name a call; only read when a browser holds the microphone.
const BROWSER_PROCESSES: [&str; 6] = [
    "Google Chrome",
    "Microsoft Edge",
    "Brave Browser",
    "Arc",
    "Safari",
    "firefox",
];

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn AudioObjectGetPropertyData(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(s: CFTypeRef, buf: *mut i8, size: isize, encoding: u32) -> bool;
    fn CFRelease(cf: CFTypeRef);
}

fn address(selector: u32) -> Address {
    Address {
        selector,
        scope: SCOPE_GLOBAL,
        element: 0,
    }
}

fn process_objects() -> Vec<u32> {
    let addr = address(PROCESS_LIST);
    let mut size = 0u32;
    // SAFETY: a size query on the system object with a valid address and no qualifier.
    let rc = unsafe {
        AudioObjectGetPropertyDataSize(SYSTEM_OBJECT, &addr, 0, std::ptr::null(), &mut size)
    };
    if rc != 0 || size == 0 {
        return Vec::new();
    }
    let mut ids = vec![0u32; size as usize / std::mem::size_of::<u32>()];
    // SAFETY: the buffer holds `size` bytes; Core Audio writes at most that and updates `size`.
    let rc = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            ids.as_mut_ptr().cast(),
        )
    };
    if rc != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / std::mem::size_of::<u32>());
    ids
}

fn running_input(process: u32) -> bool {
    let addr = address(PROCESS_RUNNING_INPUT);
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: a u32 property read into a u32 with its size.
    let rc = unsafe {
        AudioObjectGetPropertyData(
            process,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            (&mut value as *mut u32).cast(),
        )
    };
    rc == 0 && value != 0
}

fn bundle_id(process: u32) -> Option<String> {
    let addr = address(PROCESS_BUNDLE_ID);
    let mut value: CFTypeRef = std::ptr::null();
    let mut size = std::mem::size_of::<CFTypeRef>() as u32;
    // SAFETY: the property is a retained CFString written into a pointer-sized slot; released below.
    let rc = unsafe {
        AudioObjectGetPropertyData(
            process,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            (&mut value as *mut CFTypeRef).cast(),
        )
    };
    if rc != 0 || value.is_null() {
        return None;
    }
    let mut buf = [0i8; 256];
    // SAFETY: value is a live CFString; the buffer is NUL-terminated by the call on success; released once.
    let text = unsafe {
        let ok = CFStringGetCString(value, buf.as_mut_ptr(), buf.len() as isize, UTF8);
        CFRelease(value);
        ok.then(|| CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    };
    text.filter(|t| !t.is_empty())
}

/// Bundle ids of the processes recording from any microphone now.
pub fn mic_users() -> Vec<String> {
    let mut out = Vec::new();
    for process in process_objects() {
        if !running_input(process) {
            continue;
        }
        if let Some(id) = bundle_id(process) {
            out.push(id);
        }
    }
    out
}

/// Window titles of the open browsers (the active tab's title), read through Accessibility.
pub fn browser_titles() -> Vec<String> {
    crate::notes::callnames::window_titles(&BROWSER_PROCESSES)
}
