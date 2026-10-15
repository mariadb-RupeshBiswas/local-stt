//! Zoom's meeting through macOS Accessibility, which local-stt already holds for its shortcut.
//! Video tiles (AXTabGroup, labelled "Name, audio state, video state") give names and the active speaker; the panel adds rows.

use std::ffi::{c_char, c_void, CStr};

type CFTypeRef = *const c_void;

const UTF8: u32 = 0x0800_0100;
const AX_OK: i32 = 0;
// The walk stops here so a large meeting window never stalls the note.
const MAX_NODES: usize = 4_000;
const MAX_DEPTH: usize = 30;
const MAX_ROWS: usize = 300;
// Zoom's meeting window; any other Zoom window holding one tile is its mini window.
const MEETING_WINDOW: &str = "zm.meeting.window.main";
const CG_SIZE: u32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, seconds: f32) -> i32;
    fn AXValueGetValue(value: CFTypeRef, kind: u32, out: *mut c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        len: isize,
        encoding: u32,
        external: bool,
    ) -> CFTypeRef;
    fn CFStringGetCString(s: CFTypeRef, buf: *mut c_char, size: isize, encoding: u32) -> bool;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
}

/// A retained Core Foundation value, released once.
struct Owned(CFTypeRef);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer was returned retained (Create/Copy, or CFRetain) and is released exactly once.
            unsafe { CFRelease(self.0) }
        }
    }
}

fn cfstr(s: &str) -> Owned {
    // SAFETY: the bytes are valid UTF-8 for the given length; the result is owned and released by Owned.
    Owned(unsafe {
        CFStringCreateWithBytes(std::ptr::null(), s.as_ptr(), s.len() as isize, UTF8, false)
    })
}

fn to_string(value: CFTypeRef) -> Option<String> {
    // SAFETY: the type check runs before the value is read as a string; the buffer is large enough and NUL-terminated.
    unsafe {
        if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
            return None;
        }
        let mut buf = [0 as c_char; 512];
        if !CFStringGetCString(value, buf.as_mut_ptr(), buf.len() as isize, UTF8) {
            return None;
        }
        Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

fn attribute(element: CFTypeRef, name: &str) -> Option<Owned> {
    let key = cfstr(name);
    let mut out: CFTypeRef = std::ptr::null();
    // SAFETY: element is a live AXUIElement; on success `out` is a retained value we own.
    let rc = unsafe { AXUIElementCopyAttributeValue(element, key.0, &mut out) };
    (rc == AX_OK && !out.is_null()).then_some(Owned(out))
}

fn text(element: CFTypeRef, name: &str) -> Option<String> {
    attribute(element, name)
        .and_then(|v| to_string(v.0))
        .filter(|s| !s.trim().is_empty())
}

fn children(element: CFTypeRef) -> Vec<Owned> {
    let Some(array) = attribute(element, "AXChildren") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // SAFETY: the type is checked before use as an array; each item is retained before the array goes.
    unsafe {
        if CFGetTypeID(array.0) != CFArrayGetTypeID() {
            return out;
        }
        for i in 0..CFArrayGetCount(array.0) {
            out.push(Owned(CFRetain(CFArrayGetValueAtIndex(array.0, i))));
        }
    }
    out
}

// Every label a node carries, for matching "Participants" in any of them.
fn labels(element: CFTypeRef) -> String {
    ["AXTitle", "AXDescription", "AXIdentifier", "AXValue"]
        .iter()
        .filter_map(|a| text(element, a))
        .collect::<Vec<_>>()
        .join(" ")
}

// A row's text: its own label, else its visible text pieces joined like a Windows row name.
fn row_text(row: CFTypeRef, budget: &mut usize) -> Option<String> {
    if let Some(own) = text(row, "AXDescription").or_else(|| text(row, "AXTitle")) {
        return Some(own);
    }
    let mut parts = Vec::new();
    let mut stack = children(row);
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        if text(node.0, "AXRole").as_deref() == Some("AXStaticText") {
            if let Some(v) = text(node.0, "AXValue").or_else(|| text(node.0, "AXTitle")) {
                parts.push(v);
            }
        }
        stack.extend(children(node.0));
    }
    parts.reverse();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn walk(
    element: CFTypeRef,
    depth: usize,
    inside: bool,
    budget: &mut usize,
    rows: &mut Vec<String>,
) {
    if *budget == 0 || depth > MAX_DEPTH || rows.len() >= MAX_ROWS {
        return;
    }
    *budget -= 1;
    let role = text(element, "AXRole").unwrap_or_default();
    let inside = inside || labels(element).to_lowercase().contains("participant");
    if inside && matches!(role.as_str(), "AXRow" | "AXCell") {
        if let Some(t) = row_text(element, budget) {
            rows.push(t);
        }
        return;
    }
    for child in children(element) {
        walk(child.0, depth + 1, inside, budget, rows);
    }
}

fn zoom_pid() -> Option<i32> {
    let out = crate::proc::command("/usr/bin/pgrep")
        .args(["-x", "zoom.us"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

// Width times height of an element on screen, 0 when it has none.
fn area(element: CFTypeRef) -> f64 {
    let Some(size) = attribute(element, "AXSize") else {
        return 0.0;
    };
    let mut wh = [0f64; 2];
    // SAFETY: AXSize holds a CGSize (two f64); the type tag makes the call refuse anything else.
    let ok = unsafe { AXValueGetValue(size.0, CG_SIZE, wh.as_mut_ptr().cast()) };
    if ok {
        wh[0] * wh[1]
    } else {
        0.0
    }
}

// A window's video tiles as (label, area); tiles sit right under the window, so this reads one level only.
fn tiles(window: CFTypeRef) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for child in children(window) {
        if text(child.0, "AXRole").as_deref() != Some("AXTabGroup") {
            continue;
        }
        if let Some(label) = text(child.0, "AXDescription") {
            out.push((label, area(child.0)));
        }
    }
    out
}

// Zoom lists "Zoom Meeting" in its Window menu while a meeting is open, even full screen in another Space.
fn meeting_in_menu(app: CFTypeRef) -> bool {
    let Some(bar) = attribute(app, "AXMenuBar") else {
        return false;
    };
    for item in children(bar.0) {
        for menu in children(item.0) {
            for entry in children(menu.0) {
                if text(entry.0, "AXTitle").as_deref() == Some("Zoom Meeting") {
                    return true;
                }
            }
        }
    }
    false
}

pub fn look(full: bool) -> Option<super::Look> {
    // SAFETY: a read-only permission check with no arguments.
    if !unsafe { AXIsProcessTrusted() } {
        return None;
    }
    let pid = zoom_pid()?;
    // SAFETY: creates an owned AX element for a running process; released by Owned.
    let app = Owned(unsafe { AXUIElementCreateApplication(pid) });
    if app.0.is_null() {
        return None;
    }
    // SAFETY: app is live; a short timeout keeps a busy Zoom from blocking the sampler.
    unsafe { AXUIElementSetMessagingTimeout(app.0, 0.5) };
    let windows = attribute(app.0, "AXWindows")?;
    let mut budget = MAX_NODES;
    let mut look = super::Look::default();
    // SAFETY: the type is checked before use as an array; items stay alive while `windows` is held.
    unsafe {
        if CFGetTypeID(windows.0) != CFArrayGetTypeID() {
            return None;
        }
        for i in 0..CFArrayGetCount(windows.0) {
            let window = CFArrayGetValueAtIndex(windows.0, i);
            let shown = tiles(window);
            let main = text(window, "AXIdentifier").as_deref() == Some(MEETING_WINDOW);
            if look.speaking.is_none() {
                look.speaking = super::active_tile(&shown, main);
            }
            look.in_meeting |= main || !shown.is_empty();
            look.rows.extend(shown.into_iter().map(|(label, _)| label));
            if full {
                walk(window, 0, false, &mut budget, &mut look.rows);
            }
        }
    }
    if full && !look.in_meeting {
        look.in_meeting = meeting_in_menu(app.0);
    }
    Some(look)
}
