//! Zoom's participant list through Windows UI Automation; Windows needs no permission for it.
//! Layout from public code that drives Zoom 7: the meeting window's first list holds one item per participant.

use windows::core::w;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_ControlTypePropertyId,
    UIA_ListControlTypeId, UIA_ListItemControlTypeId,
};
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

const MAX_ROWS: i32 = 300;

/// The rows of Zoom's participants panel while it is open; Zoom must show the panel, local-stt never opens it.
pub fn rows() -> Option<Vec<String>> {
    // SAFETY: COM is set up for this thread before any call; every interface is reference counted by the windows crate.
    unsafe {
        // S_FALSE (already set up on this thread) is fine; a different apartment model is not.
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return None;
        }
        let meeting = FindWindowW(w!("ConfMultiTabContentWndClass"), None).ok()?;
        let uia: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let root = uia.ElementFromHandle(meeting).ok()?;
        let lists = uia
            .CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_ListControlTypeId.0),
            )
            .ok()?;
        let list = root.FindFirst(TreeScope_Descendants, &lists).ok()?;
        let items = uia
            .CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_ListItemControlTypeId.0),
            )
            .ok()?;
        let found = list.FindAll(TreeScope_Descendants, &items).ok()?;
        let count = found.Length().ok()?.min(MAX_ROWS);
        let mut rows = Vec::new();
        for i in 0..count {
            let Ok(item) = found.GetElement(i) else {
                continue;
            };
            if let Ok(name) = item.CurrentName() {
                let name = name.to_string();
                if !name.trim().is_empty() {
                    rows.push(name);
                }
            }
        }
        Some(rows)
    }
}

pub fn look(full: bool, web: bool) -> Option<super::Look> {
    let _ = web;
    // The panel is the only Zoom source read on Windows; it changes slowly, so only full looks read it.
    if !full {
        return None;
    }
    rows().map(|rows| super::Look {
        rows,
        in_meeting: true,
        source: Some("Zoom"),
        ..Default::default()
    })
}
