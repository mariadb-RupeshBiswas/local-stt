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
const CG_POINT: u32 = 1;
// dlsym's "search every loaded image" handle on macOS.
const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
// The band around a tile, in points, where Zoom draws its speaking border.
const BORDER_BAND: f64 = 6.0;
// A call page is far bigger than Zoom's window; this bounds one read of it.
const MAX_WEB_NODES: usize = 6_000;

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
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: CFTypeRef,
    ) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGImageGetWidth(image: CFTypeRef) -> usize;
    fn CGImageGetHeight(image: CFTypeRef) -> usize;
    fn CGImageGetBytesPerRow(image: CFTypeRef) -> usize;
    fn CGImageGetBitsPerPixel(image: CFTypeRef) -> usize;
    fn CGImageGetDataProvider(image: CFTypeRef) -> CFTypeRef;
    fn CGDataProviderCopyData(provider: CFTypeRef) -> CFTypeRef;
}

#[link(name = "System")]
extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn proc_listallpids(buffer: *mut c_void, size: i32) -> i32;
    fn proc_name(pid: i32, buffer: *mut c_void, size: u32) -> i32;
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
    fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
    fn CFDataGetLength(data: CFTypeRef) -> isize;
    static kCFBooleanTrue: CFTypeRef;
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
    children_of(element, "AXChildren")
}

fn children_of(element: CFTypeRef, name: &str) -> Vec<Owned> {
    let Some(array) = attribute(element, name) else {
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

// Process ids by exact name, from the kernel's process list (no child process every second).
fn pids_named(names: &[&str]) -> Vec<i32> {
    let mut pids = vec![0i32; 4096];
    let bytes = (pids.len() * std::mem::size_of::<i32>()) as i32;
    // SAFETY: the buffer holds `bytes` bytes; the call returns how many pids it wrote.
    let n = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    pids.truncate(n.max(0) as usize);
    let mut found = Vec::new();
    for pid in pids {
        let mut buf = [0u8; 64];
        // SAFETY: the buffer is 64 bytes and the call writes at most `size` bytes, returning the length.
        let len = unsafe { proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
        let name = std::str::from_utf8(&buf[..len.max(0) as usize]).unwrap_or_default();
        if names.contains(&name) {
            found.push(pid);
        }
    }
    found
}

fn zoom_pid() -> Option<i32> {
    pids_named(&["zoom.us"]).first().copied()
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

// The mini window's "Talking: Name" line, among its few direct children.
fn talking(window: CFTypeRef) -> Option<String> {
    for child in children(window).iter().take(32) {
        if text(child.0, "AXRole").as_deref() != Some("AXStaticText") {
            continue;
        }
        if let Some(name) = text(child.0, "AXValue")
            .as_deref()
            .and_then(super::talking_label)
        {
            return Some(name);
        }
    }
    None
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

// Apps whose web accessibility tree was switched on; Chromium and Electron build it only when asked.
static WOKEN: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

fn wake(app: CFTypeRef, pid: i32) {
    let mut woken = WOKEN.lock().unwrap_or_else(|e| e.into_inner());
    if woken.contains(&pid) {
        return;
    }
    woken.push(pid);
    let manual = cfstr("AXManualAccessibility");
    // SAFETY: app is a live AX element; kCFBooleanTrue is a static CFBoolean.
    let rc = unsafe { AXUIElementSetAttributeValue(app, manual.0, kCFBooleanTrue) };
    if rc != AX_OK {
        let enhanced = cfstr("AXEnhancedUserInterface");
        // SAFETY: as above; browsers that ignore the Electron switch take this one.
        unsafe { AXUIElementSetAttributeValue(app, enhanced.0, kCFBooleanTrue) };
    }
}

fn classes(element: CFTypeRef) -> Vec<String> {
    let Some(list) = attribute(element, "AXDOMClassList") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // SAFETY: the type is checked before use as an array; items are only read while `list` is held.
    unsafe {
        if CFGetTypeID(list.0) != CFArrayGetTypeID() {
            return out;
        }
        for i in 0..CFArrayGetCount(list.0) {
            if let Some(c) = to_string(CFArrayGetValueAtIndex(list.0, i)) {
                out.push(c);
            }
        }
    }
    out
}

fn has_class(element: CFTypeRef, class: &str) -> bool {
    classes(element).iter().any(|c| c == class)
}

// A tile's person: its own label, else the first text inside it.
fn tile_name(tile: CFTypeRef, budget: &mut usize) -> Option<String> {
    let own = text(tile, "AXDescription").or_else(|| text(tile, "AXTitle"));
    if let Some(name) = own.as_deref().and_then(super::web_name) {
        return Some(name);
    }
    let mut stack = children(tile);
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if text(node.0, "AXRole").as_deref() == Some("AXStaticText") {
            let value = text(node.0, "AXValue").or_else(|| text(node.0, "AXTitle"));
            if let Some(name) = value.as_deref().and_then(super::web_name) {
                return Some(name);
            }
        }
        stack.extend(children(node.0));
    }
    None
}

fn subtree_has_class(root: CFTypeRef, class: &str, budget: &mut usize) -> bool {
    let mut stack = vec![Owned(unsafe_retain(root))];
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        if has_class(node.0, class) {
            return true;
        }
        stack.extend(children(node.0));
    }
    false
}

fn unsafe_retain(element: CFTypeRef) -> CFTypeRef {
    // SAFETY: element is live; the extra retain is released by the Owned that takes it.
    unsafe { CFRetain(element) }
}

// The nearest ancestor label that reads as a name, for apps whose tiles carry no class of their own.
fn ancestor_name(element: CFTypeRef) -> Option<String> {
    let mut node = attribute(element, "AXParent")?;
    for _ in 0..8 {
        let own = text(node.0, "AXDescription").or_else(|| text(node.0, "AXTitle"));
        if let Some(name) = own.as_deref().and_then(super::web_name) {
            return Some(name);
        }
        node = attribute(node.0, "AXParent")?;
    }
    None
}

// One web call's page: tile names, and the tile holding the speaking marker.
fn read_web(call: &super::WebCall, window: CFTypeRef, look: &mut super::Look, budget: &mut usize) {
    let mut stack = vec![Owned(unsafe_retain(window))];
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let cls = classes(node.0);
        if !call.tile.is_empty() && cls.iter().any(|c| c == call.tile) {
            look.web_live = true;
            look.source = look.source.or(Some(call.source));
            let name = tile_name(node.0, budget);
            let talking = subtree_has_class(node.0, call.marker, budget);
            if let Some(name) = name {
                if talking && look.speaking.is_none() {
                    look.speaking = Some(name.clone());
                }
                look.rows.push(name);
            }
            continue;
        }
        if call.tile.is_empty() && cls.iter().any(|c| c == call.marker) {
            look.web_live = true;
            look.source = look.source.or(Some(call.source));
            if look.speaking.is_none() {
                look.speaking = ancestor_name(node.0);
            }
        }
        stack.extend(children(node.0));
    }
}

fn read_web_calls(look: &mut super::Look) {
    for call in &super::WEB_CALLS {
        for pid in pids_named(call.processes) {
            // SAFETY: creates an owned AX element for a running process; released by Owned.
            let app = Owned(unsafe { AXUIElementCreateApplication(pid) });
            if app.0.is_null() {
                continue;
            }
            // SAFETY: app is live; a short timeout keeps a busy app from blocking the sampler.
            unsafe { AXUIElementSetMessagingTimeout(app.0, 0.5) };
            let Some(windows) = attribute(app.0, "AXWindows") else {
                continue;
            };
            let mut budget = MAX_WEB_NODES;
            // SAFETY: the type is checked before use as an array; items stay alive while `windows` is held.
            unsafe {
                if CFGetTypeID(windows.0) != CFArrayGetTypeID() {
                    continue;
                }
                for i in 0..CFArrayGetCount(windows.0) {
                    let window = CFArrayGetValueAtIndex(windows.0, i);
                    let title = text(window, "AXTitle").unwrap_or_default();
                    if !title.starts_with(call.title_prefix) {
                        continue;
                    }
                    wake(app.0, pid);
                    look.in_meeting |= !call.title_prefix.is_empty();
                    read_web(call, window, look, &mut budget);
                }
            }
        }
    }
}

/// Asks macOS once for Screen Recording, which the speaking border needs; true when already allowed.
pub fn ask_screen_access() -> bool {
    // SAFETY: plain permission calls with no arguments; the request shows the system prompt at most once.
    unsafe { CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() }
}

// An element's frame on screen, in points.
fn frame(element: CFTypeRef) -> Option<Rect> {
    let pos = attribute(element, "AXPosition")?;
    let size = attribute(element, "AXSize")?;
    let mut xy = [0f64; 2];
    let mut wh = [0f64; 2];
    // SAFETY: AXPosition holds a CGPoint and AXSize a CGSize (two f64 each); the type tags make the calls refuse anything else.
    let ok = unsafe {
        AXValueGetValue(pos.0, CG_POINT, xy.as_mut_ptr().cast())
            && AXValueGetValue(size.0, CG_SIZE, wh.as_mut_ptr().cast())
    };
    ok.then_some(Rect {
        x: xy[0],
        y: xy[1],
        w: wh[0],
        h: wh[1],
    })
}

// A function looked up by name at run time; None when this macOS does not have it.
fn lookup(name: &str) -> Option<*mut c_void> {
    let name = std::ffi::CString::new(name).ok()?;
    // SAFETY: the name is NUL-terminated and RTLD_DEFAULT searches the images already loaded.
    let f = unsafe { dlsym(RTLD_DEFAULT, name.as_ptr()) };
    (!f.is_null()).then_some(f)
}

// The window server's id for an AX window, through the same private call screen readers' helpers use.
fn window_id(window: CFTypeRef) -> Option<u32> {
    type GetWindow = unsafe extern "C" fn(CFTypeRef, *mut u32) -> i32;
    let f = lookup("_AXUIElementGetWindow")?;
    let mut id = 0u32;
    // SAFETY: the symbol has this signature; window is a live AX element and id receives the number.
    let rc = unsafe { std::mem::transmute::<*mut c_void, GetWindow>(f)(window, &mut id) };
    (rc == AX_OK && id != 0).then_some(id)
}

/// One window's pixels in memory (BGRA, one point per pixel), dropped right after use.
struct Pixels {
    data: Owned,
    width: usize,
    height: usize,
    row: usize,
}

impl Pixels {
    fn bgr(&self, x: usize, y: usize) -> Option<(u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        // SAFETY: data is a live CFData of at least row * height bytes (checked when captured).
        let bytes = unsafe {
            std::slice::from_raw_parts(CFDataGetBytePtr(self.data.0), self.row * self.height)
        };
        let at = y * self.row + x * 4;
        Some((bytes[at], bytes[at + 1], bytes[at + 2]))
    }
}

// Captures one window even while other windows cover it; needs Screen Recording.
fn capture(id: u32) -> Option<Pixels> {
    type CreateImage = unsafe extern "C" fn(Rect, u32, u32, u32) -> CFTypeRef;
    // SAFETY: a read-only permission check; never prompts.
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return None;
    }
    let f = lookup("CGWindowListCreateImage")?;
    let null = Rect {
        x: f64::INFINITY,
        y: f64::INFINITY,
        w: 0.0,
        h: 0.0,
    };
    // Including just this window; bounds without the shadow, at one pixel per point.
    let (only_window, plain_nominal) = (8u32, 1u32 | 16u32);
    // SAFETY: the symbol has this signature; the result is a retained CGImage released by Owned.
    let image = Owned(unsafe {
        std::mem::transmute::<*mut c_void, CreateImage>(f)(null, only_window, id, plain_nominal)
    });
    if image.0.is_null() {
        return None;
    }
    // SAFETY: image is a live CGImage; its data is copied into a retained CFData released by Owned.
    unsafe {
        if CGImageGetBitsPerPixel(image.0) != 32 {
            return None;
        }
        let pixels = Pixels {
            data: Owned(CGDataProviderCopyData(CGImageGetDataProvider(image.0))),
            width: CGImageGetWidth(image.0),
            height: CGImageGetHeight(image.0),
            row: CGImageGetBytesPerRow(image.0),
        };
        let enough = !pixels.data.0.is_null()
            && CFDataGetLength(pixels.data.0) as usize >= pixels.row * pixels.height;
        enough.then_some(pixels)
    }
}

// Each tile's share of border-green pixels in a thin band just around it, from one capture of the window.
fn border_shares(window: CFTypeRef) -> Vec<(String, f64)> {
    let tiles: Vec<Owned> = children(window)
        .into_iter()
        .filter(|c| text(c.0, "AXRole").as_deref() == Some("AXTabGroup"))
        .collect();
    let (Some(win), Some(id)) = (frame(window), window_id(window)) else {
        return Vec::new();
    };
    let Some(pixels) = capture(id) else {
        return Vec::new();
    };
    let scale = pixels.width as f64 / win.w.max(1.0);
    let mut shares = Vec::new();
    for tile in &tiles {
        let (Some(label), Some(r)) = (text(tile.0, "AXDescription"), frame(tile.0)) else {
            continue;
        };
        let left = ((r.x - win.x - 3.0) * scale).max(0.0) as usize;
        let top = ((r.y - win.y - 3.0) * scale).max(0.0) as usize;
        let right = ((r.x - win.x + r.w + 3.0) * scale) as usize;
        let bottom = ((r.y - win.y + r.h + 3.0) * scale) as usize;
        let band = (BORDER_BAND * scale) as usize;
        let mut green = 0usize;
        let mut total = 0usize;
        for y in (top..bottom).step_by(2) {
            for x in (left..right).step_by(2) {
                let edge =
                    x < left + band || x + band >= right || y < top + band || y + band >= bottom;
                if !edge {
                    continue;
                }
                if let Some((b, g, r)) = pixels.bgr(x, y) {
                    total += 1;
                    if super::border_green(r, g, b) {
                        green += 1;
                    }
                }
            }
        }
        if total > 0 {
            shares.push((label, green as f64 / total as f64));
        }
    }
    shares
}

pub fn look(full: bool, web: bool, border: bool) -> Option<super::Look> {
    // SAFETY: a read-only permission check with no arguments.
    if !unsafe { AXIsProcessTrusted() } {
        return None;
    }
    let mut look = super::Look::default();
    let zoom = zoom_pid();
    if let Some(pid) = zoom {
        read_zoom(pid, full, border, &mut look);
    }
    if full || web {
        read_web_calls(&mut look);
    }
    let found = zoom.is_some() || look.web_live || look.in_meeting;
    found.then_some(look)
}

fn read_zoom(pid: i32, full: bool, border: bool, look: &mut super::Look) {
    // SAFETY: creates an owned AX element for a running process; released by Owned.
    let app = Owned(unsafe { AXUIElementCreateApplication(pid) });
    if app.0.is_null() {
        return;
    }
    // SAFETY: app is live; a short timeout keeps a busy Zoom from blocking the sampler.
    unsafe { AXUIElementSetMessagingTimeout(app.0, 0.5) };
    let Some(windows) = attribute(app.0, "AXWindows") else {
        return;
    };
    let mut budget = MAX_NODES;
    // SAFETY: the type is checked before use as an array; items stay alive while `windows` is held.
    unsafe {
        if CFGetTypeID(windows.0) != CFArrayGetTypeID() {
            return;
        }
        let count = CFArrayGetCount(windows.0);
        let mut main_tiles = Vec::new();
        let mut strip_tiles = Vec::new();
        for i in 0..count {
            let window = CFArrayGetValueAtIndex(windows.0, i);
            let shown = tiles(window);
            let main = text(window, "AXIdentifier").as_deref() == Some(MEETING_WINDOW);
            if look.speaking.is_none() && !main {
                look.speaking = talking(window);
            }
            if look.speaking.is_none() {
                look.speaking = super::active_tile(&shown, main);
            }
            if main {
                main_tiles.extend(shown.iter().cloned());
            } else {
                strip_tiles.extend(shown.iter().cloned());
            }
            look.in_meeting |= main || !shown.is_empty();
            if !shown.is_empty() {
                look.source = look.source.or(Some("Zoom"));
            }
            look.rows.extend(shown.into_iter().map(|(label, _)| label));
            if full {
                walk(window, 0, false, &mut budget, &mut look.rows);
            }
        }
        if look.speaking.is_none() {
            look.speaking = super::strip_talker(&main_tiles, &strip_tiles);
        }
        // Gallery and the share panel show the talker only as a green border, which only pixels reveal.
        for i in 0..count {
            if look.speaking.is_some() || !border {
                break;
            }
            let window = CFArrayGetValueAtIndex(windows.0, i);
            if tiles(window).len() >= 2 {
                look.speaking = super::border_talker(&border_shares(window));
            }
        }
    }
    if full && !look.in_meeting {
        look.in_meeting = meeting_in_menu(app.0);
    }
}

/// Window titles of running apps with these process names; empty without Accessibility.
pub fn window_titles(processes: &[&str]) -> Vec<String> {
    // SAFETY: a read-only permission check with no arguments.
    if !unsafe { AXIsProcessTrusted() } {
        return Vec::new();
    }
    let mut titles = Vec::new();
    for pid in pids_named(processes) {
        // SAFETY: creates an owned AX element for a running process; released by Owned.
        let app = Owned(unsafe { AXUIElementCreateApplication(pid) });
        if app.0.is_null() {
            continue;
        }
        // SAFETY: app is live; a short timeout keeps a busy browser from blocking the caller.
        unsafe { AXUIElementSetMessagingTimeout(app.0, 0.5) };
        for window in children_of(app.0, "AXWindows") {
            if let Some(title) = text(window.0, "AXTitle") {
                titles.push(title);
            }
        }
    }
    titles
}
