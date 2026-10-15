//! Zoom's meeting and web calls through Windows UI Automation; Windows needs no permission for it.
//! Zoom video tiles are named "Name, computer audio ..."; web apps put their HTML class in the ClassName property.

use std::collections::HashMap;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
    TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_ListControlTypeId,
    UIA_ListItemControlTypeId, UIA_TextControlTypeId,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};

// Each walk stops here so a large window never stalls the note.
const MAX_NODES: usize = 4_000;
const MAX_WEB_NODES: usize = 6_000;
const MAX_ROWS: i32 = 300;
const MEETING_CLASS: &str = "ConfMultiTabContentWndClass";
const ZOOM_EXES: [&str; 2] = ["zoom.exe", "cpthost.exe"];
// The label Zoom puts on a video tile, after the name.
const TILE_MARK: &str = ", computer audio";
// The band around a tile, in pixels at 96 dpi, where Zoom draws its speaking border.
const BORDER_BAND: i32 = 6;
// PrintWindow flag that also captures DirectX-rendered content, which Zoom uses.
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);
// A window larger than this is not a call window; bounds the capture's memory.
const MAX_CAPTURE_SIDE: i32 = 8_192;

struct Top {
    hwnd: HWND,
    exe: String,
    class: String,
    title: String,
}

struct Uia {
    auto: IUIAutomation,
    walker: IUIAutomationTreeWalker,
}

impl Uia {
    fn children(&self, element: &IUIAutomationElement) -> Vec<IUIAutomationElement> {
        let mut out = Vec::new();
        // SAFETY: the element is a live UIA interface; a missing child or sibling comes back as an error.
        unsafe {
            let mut next = self.walker.GetFirstChildElement(element).ok();
            while let Some(child) = next {
                next = self.walker.GetNextSiblingElement(&child).ok();
                out.push(child);
            }
        }
        out
    }

    fn parent(&self, element: &IUIAutomationElement) -> Option<IUIAutomationElement> {
        // SAFETY: the element is a live UIA interface; the root has no parent and gives an error.
        unsafe { self.walker.GetParentElement(element).ok() }
    }
}

fn name(element: &IUIAutomationElement) -> Option<String> {
    // SAFETY: a read-only property call on a live UIA element.
    let name = unsafe { element.CurrentName() }.ok()?.to_string();
    (!name.trim().is_empty()).then_some(name)
}

// The HTML class attribute of a web element, split into its classes.
fn classes(element: &IUIAutomationElement) -> Vec<String> {
    // SAFETY: a read-only property call on a live UIA element.
    let raw = unsafe { element.CurrentClassName() }.map(|c| c.to_string());
    raw.unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn area(element: &IUIAutomationElement) -> f64 {
    // SAFETY: a read-only property call on a live UIA element.
    let Ok(r) = (unsafe { element.CurrentBoundingRectangle() }) else {
        return 0.0;
    };
    f64::from((r.right - r.left).max(0)) * f64::from((r.bottom - r.top).max(0))
}

fn is_text(element: &IUIAutomationElement) -> bool {
    // SAFETY: a read-only property call on a live UIA element.
    unsafe { element.CurrentControlType() }.is_ok_and(|t| t == UIA_TextControlTypeId)
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the address of the Vec that top_windows keeps alive for the whole EnumWindows call.
    let list = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    // SAFETY: a read-only visibility check on a handle Windows just handed over.
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        list.push(hwnd);
    }
    BOOL(1)
}

// The executable file name, lowercase, of the process owning a window; empty when it cannot be read.
fn exe_of(hwnd: HWND, cache: &mut HashMap<u32, String>) -> String {
    let mut pid = 0u32;
    // SAFETY: pid is a valid out pointer for the call.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    cache
        .entry(pid)
        .or_insert_with(|| {
            // SAFETY: the handle is closed below; the buffer length is passed in and updated by the call.
            unsafe {
                let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                    return String::new();
                };
                let mut buf = [0u16; 520];
                let mut len = buf.len() as u32;
                let read = QueryFullProcessImageNameW(
                    process,
                    PROCESS_NAME_WIN32,
                    PWSTR(buf.as_mut_ptr()),
                    &mut len,
                );
                let _ = CloseHandle(process);
                if read.is_err() {
                    return String::new();
                }
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                path.rsplit('\\').next().unwrap_or_default().to_lowercase()
            }
        })
        .clone()
}

fn top_windows() -> Vec<Top> {
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: the callback only runs during this call, and the Vec outlives it.
    let enumerated = unsafe {
        EnumWindows(
            Some(collect),
            LPARAM(&mut handles as *mut Vec<HWND> as isize),
        )
    };
    if enumerated.is_err() {
        return Vec::new();
    }
    let mut pids = HashMap::new();
    let mut tops = Vec::new();
    for hwnd in handles {
        let mut title = [0u16; 256];
        let mut class = [0u16; 128];
        // SAFETY: the buffers are valid for their lengths; the calls return the characters written.
        let (t, c) = unsafe {
            (
                GetWindowTextW(hwnd, &mut title),
                GetClassNameW(hwnd, &mut class),
            )
        };
        tops.push(Top {
            hwnd,
            exe: exe_of(hwnd, &mut pids),
            class: String::from_utf16_lossy(&class[..c.max(0) as usize]),
            title: String::from_utf16_lossy(&title[..t.max(0) as usize]),
        });
    }
    tops
}

// The rows of Zoom's participants panel while it is open; Zoom must show the panel, local-stt never opens it.
fn panel_rows(uia: &Uia, root: &IUIAutomationElement) -> Vec<String> {
    let mut rows = Vec::new();
    // SAFETY: every interface is reference counted by the windows crate; failures end the read with what was found.
    unsafe {
        let Ok(lists) = uia.auto.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_ListControlTypeId.0),
        ) else {
            return rows;
        };
        let Ok(list) = root.FindFirst(TreeScope_Descendants, &lists) else {
            return rows;
        };
        let Ok(items) = uia.auto.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_ListItemControlTypeId.0),
        ) else {
            return rows;
        };
        let Ok(found) = list.FindAll(TreeScope_Descendants, &items) else {
            return rows;
        };
        for i in 0..found.Length().unwrap_or(0).min(MAX_ROWS) {
            if let Some(label) = found.GetElement(i).ok().as_ref().and_then(name) {
                rows.push(label);
            }
        }
    }
    rows
}

type Rects = Vec<(String, RECT)>;

// A Zoom window's video tiles as (label, area) and (label, rectangle), and the mini window's "Talking: Name" line if any.
fn zoom_tiles(
    uia: &Uia,
    root: &IUIAutomationElement,
) -> (Vec<(String, f64)>, Rects, Option<String>) {
    let mut budget = MAX_NODES;
    let mut tiles = Vec::new();
    let mut rects = Vec::new();
    let mut talking = None;
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        let own = name(&node);
        if talking.is_none() {
            talking = own.as_deref().and_then(super::talking_label);
        }
        let label =
            own.filter(|l| l.to_lowercase().contains(TILE_MARK) && super::parse_row(l).is_some());
        if let Some(label) = label {
            // SAFETY: a read-only property call on a live UIA element.
            if let Ok(r) = unsafe { node.CurrentBoundingRectangle() } {
                rects.push((label.clone(), r));
            }
            tiles.push((label, area(&node)));
            continue;
        }
        stack.extend(uia.children(&node));
    }
    (tiles, rects, talking)
}

// A captured window: a top-down 32-bit DIB that frees its GDI objects when dropped.
struct Shot {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *const u8,
    width: usize,
    height: usize,
    left: i32,
    top: i32,
}

impl Shot {
    fn pixels(&self) -> &[u8] {
        // SAFETY: bits points at the DIB's width * height * 4 bytes, which live until the bitmap is deleted in drop.
        unsafe { std::slice::from_raw_parts(self.bits, self.width * self.height * 4) }
    }
}

impl Drop for Shot {
    fn drop(&mut self) {
        // SAFETY: the DC and bitmap were created by capture and are released exactly once, the old object selected back first.
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

// One PrintWindow capture of a window's pixels; None when the window is gone, huge or refuses to paint.
fn capture(hwnd: HWND) -> Option<Shot> {
    let mut r = RECT::default();
    // SAFETY: r is a valid out pointer; a closed window gives an error.
    unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 || w > MAX_CAPTURE_SIDE || h > MAX_CAPTURE_SIDE {
        return None;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: every GDI object made here is released on each path: the screen DC right after the bitmap, the rest by Shot::drop.
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let dc = CreateCompatibleDC(Some(screen));
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, screen);
        let bitmap = match bitmap {
            Ok(b) if !dc.is_invalid() && !bits.is_null() => b,
            Ok(b) => {
                let _ = DeleteObject(b.into());
                let _ = DeleteDC(dc);
                return None;
            }
            Err(_) => {
                let _ = DeleteDC(dc);
                return None;
            }
        };
        let old = SelectObject(dc, bitmap.into());
        let shot = Shot {
            dc,
            bitmap,
            old,
            bits: bits as *const u8,
            width: w as usize,
            height: h as usize,
            left: r.left,
            top: r.top,
        };
        PrintWindow(hwnd, dc, PW_RENDERFULLCONTENT)
            .as_bool()
            .then_some(shot)
    }
}

// Share of border-green pixels in the outer `band` of a rectangle (window coordinates), sampling every 2nd pixel.
fn band_share(px: &[u8], width: usize, height: usize, r: RECT, band: i32) -> Option<f64> {
    let (mut green, mut total) = (0usize, 0usize);
    for y in (r.top.max(0)..r.bottom.min(height as i32)).step_by(2) {
        for x in (r.left.max(0)..r.right.min(width as i32)).step_by(2) {
            let edge = x < r.left + band
                || x + band >= r.right
                || y < r.top + band
                || y + band >= r.bottom;
            if !edge {
                continue;
            }
            let i = (y as usize * width + x as usize) * 4;
            total += 1;
            if super::border_green(px[i + 2], px[i + 1], px[i]) {
                green += 1;
            }
        }
    }
    (total > 0).then(|| green as f64 / total as f64)
}

// Each tile's share of border-green pixels in a thin band just around it, from one capture of the window.
fn border_shares(hwnd: HWND, tiles: &Rects) -> Vec<(String, f64)> {
    let Some(shot) = capture(hwnd) else {
        return Vec::new();
    };
    // SAFETY: a read-only call on the window handle; 0 means it is gone and falls back to 96 dpi.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
    let scale = |px: i32| px * dpi / 96;
    let (band, pad) = (scale(BORDER_BAND), scale(3));
    let px = shot.pixels();
    let mut shares = Vec::new();
    for (label, r) in tiles {
        let around = RECT {
            left: r.left - shot.left - pad,
            top: r.top - shot.top - pad,
            right: r.right - shot.left + pad,
            bottom: r.bottom - shot.top + pad,
        };
        if let Some(share) = band_share(px, shot.width, shot.height, around, band) {
            shares.push((label.clone(), share));
        }
    }
    shares
}

// Reads one Zoom window into `look`; returns its tiles, for telling a floated speaker-view strip apart.
fn read_zoom(
    uia: &Uia,
    top: &Top,
    main: bool,
    full: bool,
    border: bool,
    look: &mut super::Look,
) -> Vec<(String, f64)> {
    // SAFETY: the handle came from EnumWindows moments ago; a window that closed since gives an error.
    let Ok(root) = (unsafe { uia.auto.ElementFromHandle(top.hwnd) }) else {
        return Vec::new();
    };
    let (shown, rects, talking) = zoom_tiles(uia, &root);
    if look.speaking.is_none() && !main {
        look.speaking = talking;
    }
    if look.speaking.is_none() {
        look.speaking = super::active_tile(&shown, main);
    }
    // Gallery and the share panel show the talker only as a green border, which only pixels reveal.
    if look.speaking.is_none() && border && rects.len() >= 2 {
        look.speaking = super::border_talker(&border_shares(top.hwnd, &rects));
    }
    look.in_meeting |= main || !shown.is_empty();
    if main || !shown.is_empty() {
        look.source = look.source.or(Some("Zoom"));
    }
    look.rows
        .extend(shown.iter().map(|(label, _)| label.clone()));
    if full && main {
        look.rows.extend(panel_rows(uia, &root));
    }
    shown
}

// A tile's person: its own label, else the first text inside it.
fn tile_name(uia: &Uia, tile: &IUIAutomationElement, budget: &mut usize) -> Option<String> {
    if let Some(found) = name(tile).as_deref().and_then(super::web_name) {
        return Some(found);
    }
    let mut stack = uia.children(tile);
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if is_text(&node) {
            if let Some(found) = name(&node).as_deref().and_then(super::web_name) {
                return Some(found);
            }
        }
        stack.extend(uia.children(&node));
    }
    None
}

fn subtree_has_class(
    uia: &Uia,
    root: &IUIAutomationElement,
    class: &str,
    budget: &mut usize,
) -> bool {
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        if classes(&node).iter().any(|c| c == class) {
            return true;
        }
        stack.extend(uia.children(&node));
    }
    false
}

// The nearest ancestor label that reads as a name, for apps whose tiles carry no class of their own.
fn ancestor_name(uia: &Uia, element: &IUIAutomationElement) -> Option<String> {
    let mut node = uia.parent(element)?;
    for _ in 0..8 {
        if let Some(found) = name(&node).as_deref().and_then(super::web_name) {
            return Some(found);
        }
        node = uia.parent(&node)?;
    }
    None
}

// One web call's page: tile names, and the tile holding the speaking marker.
fn read_web(
    uia: &Uia,
    call: &super::WebCall,
    root: IUIAutomationElement,
    look: &mut super::Look,
    budget: &mut usize,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let cls = classes(&node);
        if !call.tile.is_empty() && cls.iter().any(|c| c == call.tile) {
            look.web_live = true;
            look.source = look.source.or(Some(call.source));
            let who = tile_name(uia, &node, budget);
            let talking = subtree_has_class(uia, &node, call.marker, budget);
            if let Some(who) = who {
                if talking && look.speaking.is_none() {
                    look.speaking = Some(who.clone());
                }
                look.rows.push(who);
            }
            continue;
        }
        if call.tile.is_empty() && cls.iter().any(|c| c == call.marker) {
            look.web_live = true;
            look.source = look.source.or(Some(call.source));
            if look.speaking.is_none() {
                look.speaking = ancestor_name(uia, &node);
            }
        }
        stack.extend(uia.children(&node));
    }
}

// Executables that host each web call on Windows; the shared list names Mac processes.
fn web_exes(source: &str) -> &'static [&'static str] {
    match source {
        "Google Meet" => &["chrome.exe", "msedge.exe", "brave.exe", "arc.exe"],
        "Microsoft Teams" => &["ms-teams.exe", "teams.exe"],
        "Slack" => &["slack.exe"],
        _ => &[],
    }
}

fn read_web_calls(uia: &Uia, tops: &[Top], look: &mut super::Look) {
    for call in &super::WEB_CALLS {
        let exes = web_exes(call.source);
        let mut budget = MAX_WEB_NODES;
        for top in tops {
            if !exes.contains(&top.exe.as_str()) || !top.title.starts_with(call.title_prefix) {
                continue;
            }
            // SAFETY: the handle came from EnumWindows moments ago; a window that closed since gives an error.
            let Ok(root) = (unsafe { uia.auto.ElementFromHandle(top.hwnd) }) else {
                continue;
            };
            look.in_meeting |= !call.title_prefix.is_empty();
            read_web(uia, call, root, look, &mut budget);
        }
    }
}

pub fn look(full: bool, web: bool, border: bool) -> Option<super::Look> {
    // SAFETY: COM is set up for this thread before any call; S_FALSE (already set up) is fine, a different apartment model is not.
    let uia = unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return None;
        }
        let auto: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let walker = auto.RawViewWalker().ok()?;
        Uia { auto, walker }
    };
    let tops = top_windows();
    let mut look = super::Look::default();
    let mut main_tiles = Vec::new();
    let mut strip_tiles = Vec::new();
    for top in &tops {
        let main = top.class == MEETING_CLASS;
        let mini = ZOOM_EXES.contains(&top.exe.as_str())
            && (top.class.starts_with("Zoom") || top.class.starts_with("ZP"));
        if main || mini {
            let shown = read_zoom(&uia, top, main, full, border, &mut look);
            if main {
                main_tiles.extend(shown);
            } else {
                strip_tiles.extend(shown);
            }
        }
    }
    if look.speaking.is_none() {
        look.speaking = super::strip_talker(&main_tiles, &strip_tiles);
    }
    if full || web {
        read_web_calls(&uia, &tops, &mut look);
    }
    (look.in_meeting || look.web_live).then_some(look)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: usize, h: usize, lit: bool) -> Vec<u8> {
        let mut px = vec![28u8; w * h * 4];
        if lit {
            for i in 0..w * h {
                let (x, y) = (i % w, i / w);
                if x < 6 || y < 6 || x >= w - 6 || y >= h - 6 {
                    px[i * 4..i * 4 + 3].copy_from_slice(&[70, 205, 72]);
                }
            }
        }
        px
    }

    #[test]
    fn a_green_frame_fills_the_band_and_a_dark_one_does_not() {
        let r = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 80,
        };
        assert!(band_share(&frame(100, 80, true), 100, 80, r, 6).unwrap() > 0.9);
        assert_eq!(band_share(&frame(100, 80, false), 100, 80, r, 6), Some(0.0));
        let outside = RECT {
            left: 200,
            top: 200,
            right: 300,
            bottom: 300,
        };
        assert_eq!(band_share(&frame(100, 80, true), 100, 80, outside, 6), None);
    }
}
