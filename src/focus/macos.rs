//! The frontmost window from the window list: bounds and layer need no permission.

use std::ffi::c_void;

type CFTypeRef = *const c_void;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CGRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

const ON_SCREEN_ONLY: u32 = 1;
const EXCLUDE_DESKTOP: u32 = 1 << 4;
const SINT32: isize = 3;
// Smaller windows are tooltips, badges and invisible helpers, not something being typed into.
const MIN_SIDE: f64 = 100.0;

#[allow(non_upper_case_globals)]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    static kCGWindowLayer: CFTypeRef;
    static kCGWindowBounds: CFTypeRef;
    fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFTypeRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFTypeRef, rect: *mut CGRect) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, kind: isize, out: *mut c_void) -> bool;
    fn CFRelease(cf: CFTypeRef);
}

fn int_value(dict: CFTypeRef, key: CFTypeRef) -> Option<i32> {
    let mut out = 0i32;
    // SAFETY: `dict` is a live window dictionary; the value, if present, is a CFNumber.
    let ok = unsafe {
        let number = CFDictionaryGetValue(dict, key);
        !number.is_null() && CFNumberGetValue(number, SINT32, (&mut out as *mut i32).cast())
    };
    ok.then_some(out)
}

/// The list runs front to back, so the first normal-layer window is the focused one.
pub fn window_centre() -> Option<(f64, f64)> {
    // SAFETY: the copied array is released below and only read while alive.
    let list = unsafe { CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0) };
    if list.is_null() {
        return None;
    }
    let mut windows = Vec::new();
    // SAFETY: indexes stay below the array's count; entries are window dictionaries.
    unsafe {
        for i in 0..CFArrayGetCount(list) {
            let window = CFArrayGetValueAtIndex(list, i);
            let bounds = CFDictionaryGetValue(window, kCGWindowBounds);
            let mut r = CGRect::default();
            if !bounds.is_null() && CGRectMakeWithDictionaryRepresentation(bounds, &mut r) {
                windows.push((int_value(window, kCGWindowLayer), r));
            }
        }
        CFRelease(list);
    }
    first_focusable(&windows)
}

// Our own windows count (setup's Try it box); the pill and live text float above layer 0.
fn first_focusable(windows: &[(Option<i32>, CGRect)]) -> Option<(f64, f64)> {
    windows
        .iter()
        .find(|(layer, r)| *layer == Some(0) && r.w >= MIN_SIDE && r.h >= MIN_SIDE)
        .map(|(_, r)| (r.x + r.w / 2.0, r.y + r.h / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> CGRect {
        CGRect { x, y, w, h }
    }

    #[test]
    fn own_setup_window_in_front_is_the_focus() {
        // Pill floating, setup on the built-in screen, Terminal on the screen to its left.
        let windows = [
            (Some(3), rect(646.0, 900.0, 220.0, 44.0)),
            (Some(0), rect(376.0, 218.0, 760.0, 581.0)),
            (Some(0), rect(-1360.0, -68.0, 1357.0, 729.0)),
        ];
        assert_eq!(first_focusable(&windows), Some((756.0, 508.5)));
    }

    #[test]
    fn small_and_floating_windows_never_count() {
        let windows = [
            (Some(24), rect(0.0, 0.0, 1512.0, 33.0)),
            (Some(0), rect(10.0, 10.0, 80.0, 400.0)),
            (Some(0), rect(-1360.0, -68.0, 1360.0, 738.0)),
        ];
        assert_eq!(first_focusable(&windows), Some((-680.0, 301.0)));
        assert_eq!(first_focusable(&[]), None);
    }
}
