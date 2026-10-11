//! The frontmost window from the window list: bounds and layer need no permission.

use std::ffi::c_void;

type CFTypeRef = *const c_void;

#[repr(C)]
#[derive(Default)]
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
    static kCGWindowOwnerPID: CFTypeRef;
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

/// The list runs front to back, so the first normal-layer window of another app is the focused one.
pub fn window_centre() -> Option<(f64, f64)> {
    let own = std::process::id() as i32;
    // SAFETY: the copied array is released below and only read while alive.
    let list = unsafe { CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0) };
    if list.is_null() {
        return None;
    }
    let mut found = None;
    // SAFETY: indexes stay below the array's count; entries are window dictionaries.
    unsafe {
        for i in 0..CFArrayGetCount(list) {
            let window = CFArrayGetValueAtIndex(list, i);
            if int_value(window, kCGWindowLayer) != Some(0)
                || int_value(window, kCGWindowOwnerPID) == Some(own)
            {
                continue;
            }
            let bounds = CFDictionaryGetValue(window, kCGWindowBounds);
            let mut r = CGRect::default();
            if bounds.is_null() || !CGRectMakeWithDictionaryRepresentation(bounds, &mut r) {
                continue;
            }
            if r.w >= MIN_SIDE && r.h >= MIN_SIDE {
                found = Some((r.x + r.w / 2.0, r.y + r.h / 2.0));
                break;
            }
        }
        CFRelease(list);
    }
    found
}
