//! Zoom's meeting and web calls through Windows UI Automation; Windows needs no permission for it.
//! Zoom video tiles are named "Name, computer audio ..."; web apps put their HTML class in the ClassName property.

use std::collections::HashMap;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
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
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

// Each walk stops here so a large window never stalls the note.
const MAX_NODES: usize = 4_000;
const MAX_WEB_NODES: usize = 6_000;
const MAX_ROWS: i32 = 300;
const MEETING_CLASS: &str = "ConfMultiTabContentWndClass";
const ZOOM_EXES: [&str; 2] = ["zoom.exe", "cpthost.exe"];
// The label Zoom puts on a video tile, after the name.
const TILE_MARK: &str = ", computer audio";

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

// A Zoom window's video tiles as (label, area), and the mini window's "Talking: Name" line if any.
fn zoom_tiles(uia: &Uia, root: &IUIAutomationElement) -> (Vec<(String, f64)>, Option<String>) {
    let mut budget = MAX_NODES;
    let mut tiles = Vec::new();
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
            tiles.push((label, area(&node)));
            continue;
        }
        stack.extend(uia.children(&node));
    }
    (tiles, talking)
}

fn read_zoom(uia: &Uia, top: &Top, main: bool, full: bool, look: &mut super::Look) {
    // SAFETY: the handle came from EnumWindows moments ago; a window that closed since gives an error.
    let Ok(root) = (unsafe { uia.auto.ElementFromHandle(top.hwnd) }) else {
        return;
    };
    let (shown, talking) = zoom_tiles(uia, &root);
    if look.speaking.is_none() && !main {
        look.speaking = talking;
    }
    if look.speaking.is_none() {
        look.speaking = super::active_tile(&shown, main);
    }
    look.in_meeting |= main || !shown.is_empty();
    if main || !shown.is_empty() {
        look.source = look.source.or(Some("Zoom"));
    }
    look.rows.extend(shown.into_iter().map(|(label, _)| label));
    if full && main {
        look.rows.extend(panel_rows(uia, &root));
    }
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

pub fn look(full: bool, web: bool) -> Option<super::Look> {
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
    for top in &tops {
        let main = top.class == MEETING_CLASS;
        let mini = ZOOM_EXES.contains(&top.exe.as_str())
            && (top.class.starts_with("Zoom") || top.class.starts_with("ZP"));
        if main || mini {
            read_zoom(&uia, top, main, full, &mut look);
        }
    }
    if full || web {
        read_web_calls(&uia, &tops, &mut look);
    }
    (look.in_meeting || look.web_live).then_some(look)
}
