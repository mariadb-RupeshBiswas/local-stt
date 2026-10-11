//! macOS CGEventTap (listen-only) over hand-written FFI. Owned by Task 4.
//! Constants checked against the macOS SDK headers (CGEventTypes.h, IOLLEvent.h, Events.h).

use super::{dispatch, Core, HotkeyEvent, KeyState, Modifier};
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::ptr;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CGEventRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type TapCallback = extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

// kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, kCGEventTapOptionListenOnly.
const TAP_LOCATION_SESSION: u32 = 1;
const TAP_PLACEMENT_HEAD: u32 = 0;
const TAP_OPTION_DEFAULT: u32 = 0;
const TAP_OPTION_LISTEN_ONLY: u32 = 1;
// kCGEventKeyDown, kCGEventKeyUp, kCGEventFlagsChanged.
const EVENT_KEY_DOWN: u32 = 10;
const EVENT_KEY_UP: u32 = 11;
const EVENT_FLAGS_CHANGED: u32 = 12;
// kCGEventTapDisabledByTimeout, kCGEventTapDisabledByUserInput.
const EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
// kCGKeyboardEventKeycode.
const FIELD_KEYCODE: u32 = 9;
// kCGEventFlagMask{Shift,Control,Alternate,Command,SecondaryFn}.
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CTRL: u64 = 0x0004_0000;
const FLAG_ALT: u64 = 0x0008_0000;
const FLAG_CMD: u64 = 0x0010_0000;
const FLAG_FN: u64 = 0x0080_0000;
// kVK_Escape, kVK_Function.
const KEY_ESCAPE: i64 = 0x35;
const KEY_FUNCTION: i64 = 0x3F;

// Virtual keycodes (kVK_*) for the combo alphabet, plus Tab and forward Delete for clear errors.
const KEY_NAMES: &[(i64, &str)] = &[
    (0x00, "A"),
    (0x0B, "B"),
    (0x08, "C"),
    (0x02, "D"),
    (0x0E, "E"),
    (0x03, "F"),
    (0x05, "G"),
    (0x04, "H"),
    (0x22, "I"),
    (0x26, "J"),
    (0x28, "K"),
    (0x25, "L"),
    (0x2E, "M"),
    (0x2D, "N"),
    (0x1F, "O"),
    (0x23, "P"),
    (0x0C, "Q"),
    (0x0F, "R"),
    (0x01, "S"),
    (0x11, "T"),
    (0x20, "U"),
    (0x09, "V"),
    (0x0D, "W"),
    (0x07, "X"),
    (0x10, "Y"),
    (0x06, "Z"),
    (0x1D, "0"),
    (0x12, "1"),
    (0x13, "2"),
    (0x14, "3"),
    (0x15, "4"),
    (0x17, "5"),
    (0x16, "6"),
    (0x1A, "7"),
    (0x1C, "8"),
    (0x19, "9"),
    (0x7A, "F1"),
    (0x78, "F2"),
    (0x63, "F3"),
    (0x76, "F4"),
    (0x60, "F5"),
    (0x61, "F6"),
    (0x62, "F7"),
    (0x64, "F8"),
    (0x65, "F9"),
    (0x6D, "F10"),
    (0x67, "F11"),
    (0x6F, "F12"),
    (0x69, "F13"),
    (0x6B, "F14"),
    (0x71, "F15"),
    (0x6A, "F16"),
    (0x40, "F17"),
    (0x4F, "F18"),
    (0x50, "F19"),
    (0x5A, "F20"),
    (0x31, "Space"),
    (0x30, "Tab"),
    (0x75, "Delete"),
];

const NO_TAP_MESSAGE: &str = "Could not start the keyboard hook. Allow local-stt under System Settings > Privacy & Security > Accessibility (and Input Monitoring), then start it again.";

#[allow(non_upper_case_globals)]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
}

#[allow(non_upper_case_globals)]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopCommonModes: CFStringRef;
    static kCFBooleanTrue: CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFTypeRef,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFMachPortInvalidate(port: CFMachPortRef);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFDictionaryCreate(
        allocator: CFTypeRef,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
}

#[allow(non_upper_case_globals)]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
}

fn tap_create(mask: u64, user: *mut c_void, option: u32) -> CFMachPortRef {
    // SAFETY: plain call; the callback is a valid extern "C" fn and `user` outlives the tap.
    unsafe {
        CGEventTapCreate(
            TAP_LOCATION_SESSION,
            TAP_PLACEMENT_HEAD,
            option,
            mask,
            on_event,
            user,
        )
    }
}

fn tap_enable(tap: CFMachPortRef, enable: bool) {
    // SAFETY: `tap` is a live CFMachPort created by `tap_create`.
    unsafe { CGEventTapEnable(tap, enable) }
}

fn event_flags(event: CGEventRef) -> u64 {
    // SAFETY: `event` is the live event handed to the tap callback.
    unsafe { CGEventGetFlags(event) }
}

fn event_keycode(event: CGEventRef) -> i64 {
    // SAFETY: `event` is the live event handed to the tap callback.
    unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) }
}

fn common_modes() -> CFStringRef {
    // SAFETY: reads an immutable CoreFoundation constant.
    unsafe { kCFRunLoopCommonModes }
}

fn run_loop_source(tap: CFMachPortRef) -> CFRunLoopSourceRef {
    // SAFETY: `tap` is a live CFMachPort; a NULL allocator means the default one.
    unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) }
}

fn current_run_loop() -> CFRunLoopRef {
    // SAFETY: no preconditions, returns the calling thread's loop.
    unsafe { CFRunLoopGetCurrent() }
}

fn add_source(rl: CFRunLoopRef, source: CFRunLoopSourceRef) {
    // SAFETY: both refs are live and owned by the hook thread.
    unsafe { CFRunLoopAddSource(rl, source, common_modes()) }
}

fn remove_source(rl: CFRunLoopRef, source: CFRunLoopSourceRef) {
    // SAFETY: both refs are live and owned by the hook thread.
    unsafe { CFRunLoopRemoveSource(rl, source, common_modes()) }
}

fn run_loop_run() {
    // SAFETY: runs the calling thread's loop until `CFRunLoopStop`.
    unsafe { CFRunLoopRun() }
}

fn run_loop_stop(rl: CFRunLoopRef) {
    // SAFETY: documented thread-safe, and `Guard` keeps `rl` retained while it calls this.
    unsafe { CFRunLoopStop(rl) }
}

fn retain(cf: CFTypeRef) {
    // SAFETY: `cf` is a live CoreFoundation object.
    unsafe { CFRetain(cf) };
}

fn release(cf: CFTypeRef) {
    // SAFETY: `cf` is a live CoreFoundation object the caller owns one reference to.
    unsafe { CFRelease(cf) }
}

fn invalidate_port(port: CFMachPortRef) {
    // SAFETY: `port` is a live CFMachPort.
    unsafe { CFMachPortInvalidate(port) }
}

// Safe query of the Accessibility trust, optionally showing the system prompt.
pub(super) fn accessibility_ok(prompt: bool) -> bool {
    if !prompt {
        // SAFETY: NULL options is allowed and only reads the trust state.
        return unsafe { AXIsProcessTrustedWithOptions(ptr::null()) } != 0;
    }
    // SAFETY: keys and values are CF constants, the dictionary is released right after the call.
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            ptr::addr_of!(kCFTypeDictionaryKeyCallBacks).cast(),
            ptr::addr_of!(kCFTypeDictionaryValueCallBacks).cast(),
        );
        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        if !options.is_null() {
            CFRelease(options);
        }
        trusted
    }
}

fn key_name(code: i64) -> &'static str {
    KEY_NAMES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or("Other", |(_, name)| name)
}

// Turns raw tap events into a KeyState; holds only the keys currently down.
#[derive(Default)]
struct MacKeys {
    fn_down: bool,
    escape_down: bool,
    down: Vec<i64>,
}

impl MacKeys {
    fn update(&mut self, event_type: u32, flags: u64, keycode: i64) -> KeyState {
        match event_type {
            // Arrow and F keys set the Fn flag themselves, so only the Fn key's own event counts.
            EVENT_FLAGS_CHANGED if keycode == KEY_FUNCTION => self.fn_down = flags & FLAG_FN != 0,
            EVENT_FLAGS_CHANGED if flags & FLAG_FN == 0 => self.fn_down = false,
            EVENT_KEY_DOWN => self.key_down(keycode),
            EVENT_KEY_UP => self.key_up(keycode),
            _ => {}
        }
        let mut modifiers = BTreeSet::new();
        for (mask, modifier) in [
            (FLAG_SHIFT, Modifier::Shift),
            (FLAG_CTRL, Modifier::Ctrl),
            (FLAG_ALT, Modifier::Alt),
            (FLAG_CMD, Modifier::Meta),
        ] {
            if flags & mask != 0 {
                modifiers.insert(modifier);
            }
        }
        if self.fn_down {
            modifiers.insert(Modifier::Fn);
        }
        let key = self.down.last().map(|code| key_name(*code).to_string());
        KeyState {
            modifiers,
            key,
            escape: self.escape_down,
        }
    }

    fn key_down(&mut self, keycode: i64) {
        if keycode == KEY_ESCAPE {
            self.escape_down = true;
        } else if !self.down.contains(&keycode) {
            self.down.push(keycode);
        }
    }

    fn key_up(&mut self, keycode: i64) {
        if keycode == KEY_ESCAPE {
            self.escape_down = false;
        } else {
            self.down.retain(|code| *code != keycode);
        }
    }
}

// Lives on the heap for the whole hook thread, the tap callback gets a raw pointer to it.
struct Ctx {
    core: Arc<Mutex<Core>>,
    tx: Sender<HotkeyEvent>,
    keys: MacKeys,
    tap: CFMachPortRef,
    // A listen-only tap cannot drop events, so the callback must not try.
    active: bool,
}

impl Ctx {
    // Returns true when the event should be dropped (only the hands-free key).
    fn handle(&mut self, event_type: u32, event: CGEventRef) -> bool {
        if event_type == EVENT_TAP_DISABLED_BY_TIMEOUT
            || event_type == EVENT_TAP_DISABLED_BY_USER_INPUT
        {
            tap_enable(self.tap, true);
            // Key-ups may have been missed while disabled, so start from a clean slate.
            self.keys = MacKeys::default();
            dispatch(&self.core, &self.tx, &KeyState::nothing_held(), None);
            return false;
        }
        let keycode = event_keycode(event);
        let state = self.keys.update(event_type, event_flags(event), keycode);
        let key = matches!(event_type, EVENT_KEY_DOWN | EVENT_KEY_UP).then(|| key_name(keycode));
        dispatch(&self.core, &self.tx, &state, key)
    }
}

extern "C" fn on_event(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user: *mut c_void,
) -> CGEventRef {
    // SAFETY: `user` is the Ctx leaked in `run`, only this thread's run loop calls the tap.
    let ctx = unsafe { &mut *user.cast::<Ctx>() };
    if ctx.handle(event_type, event) && ctx.active {
        // Returning NULL from an active tap deletes the event, so the hands-free Space types nothing.
        return ptr::null_mut();
    }
    event
}

fn set_tap(ctx: *mut Ctx, tap: CFMachPortRef, active: bool) {
    // SAFETY: `ctx` is live and the tap cannot call back before the run loop starts.
    unsafe {
        (*ctx).tap = tap;
        (*ctx).active = active;
    }
}

fn free_ctx(ctx: *mut Ctx) {
    // SAFETY: `ctx` came from Box::into_raw in `run` and the tap that used it is gone.
    drop(unsafe { Box::from_raw(ctx) });
}

fn run(core: Arc<Mutex<Core>>, tx: Sender<HotkeyEvent>, ready: Sender<Result<usize, String>>) {
    let ctx = Box::into_raw(Box::new(Ctx {
        core,
        tx,
        keys: MacKeys::default(),
        tap: ptr::null_mut(),
        active: false,
    }));
    let mask = (1u64 << EVENT_KEY_DOWN) | (1u64 << EVENT_KEY_UP) | (1u64 << EVENT_FLAGS_CHANGED);
    // An active tap can drop the hands-free key; without that right, listening still works.
    let mut tap = tap_create(mask, ctx.cast(), TAP_OPTION_DEFAULT);
    let active = !tap.is_null();
    if tap.is_null() {
        tap = tap_create(mask, ctx.cast(), TAP_OPTION_LISTEN_ONLY);
    }
    if tap.is_null() {
        free_ctx(ctx);
        let _ = ready.send(Err(NO_TAP_MESSAGE.into()));
        return;
    }
    set_tap(ctx, tap, active);
    crate::diag::log(if active {
        "keyboard tap: active"
    } else {
        "keyboard tap: listen-only, so the hands-free key also reaches the focused app"
    });
    let source = run_loop_source(tap);
    if source.is_null() {
        invalidate_port(tap);
        release(tap);
        free_ctx(ctx);
        let _ = ready.send(Err(
            "Could not attach the keyboard hook to a run loop.".into()
        ));
        return;
    }
    let run_loop = current_run_loop();
    add_source(run_loop, source);
    tap_enable(tap, true);
    let _ = ready.send(Ok(run_loop as usize));
    run_loop_run();
    tap_enable(tap, false);
    remove_source(run_loop, source);
    invalidate_port(tap);
    release(source);
    release(tap);
    free_ctx(ctx);
}

// Stops the hook thread when dropped.
pub struct Guard {
    run_loop: usize,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let run_loop = self.run_loop as CFRunLoopRef;
        if let Some(thread) = self.thread.take() {
            // Stop may land before the loop is running, so repeat until the thread exits.
            while !thread.is_finished() {
                run_loop_stop(run_loop);
                thread::sleep(Duration::from_millis(5));
            }
            let _ = thread.join();
        }
        release(run_loop);
    }
}

pub fn spawn(core: Arc<Mutex<Core>>, tx: Sender<HotkeyEvent>) -> Result<Guard, String> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread = thread::Builder::new()
        .name("hotkey".into())
        .spawn(move || run(core, tx, ready_tx))
        .map_err(|e| format!("Could not start the hotkey thread: {e}"))?;
    match ready_rx.recv() {
        Ok(Ok(run_loop)) => {
            // Keeps the loop object valid for `Guard::drop` even after the thread exits.
            retain(run_loop as CFRunLoopRef);
            Ok(Guard {
                run_loop,
                thread: Some(thread),
            })
        }
        Ok(Err(message)) => {
            let _ = thread.join();
            Err(message)
        }
        Err(_) => Err("The hotkey thread stopped before it was ready.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_then_shift_builds_fn_shift() {
        let mut keys = MacKeys::default();
        let s = keys.update(EVENT_FLAGS_CHANGED, FLAG_FN, KEY_FUNCTION);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Fn]));
        let s = keys.update(EVENT_FLAGS_CHANGED, FLAG_FN | FLAG_SHIFT, 0x38);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Fn, Modifier::Shift]));
        assert_eq!(s.key, None);
        let s = keys.update(EVENT_FLAGS_CHANGED, FLAG_FN, 0x38);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Fn]));
        let s = keys.update(EVENT_FLAGS_CHANGED, 0, KEY_FUNCTION);
        assert!(s.modifiers.is_empty());
    }

    #[test]
    fn arrow_key_fn_flag_is_not_the_fn_key() {
        let mut keys = MacKeys::default();
        keys.update(EVENT_FLAGS_CHANGED, FLAG_SHIFT, 0x38);
        let down = keys.update(EVENT_KEY_DOWN, FLAG_SHIFT | FLAG_FN, 0x7B);
        assert_eq!(down.modifiers, BTreeSet::from([Modifier::Shift]));
        assert_eq!(down.key.as_deref(), Some("Other"));
        let up = keys.update(EVENT_KEY_UP, FLAG_SHIFT | FLAG_FN, 0x7B);
        assert_eq!(up.modifiers, BTreeSet::from([Modifier::Shift]));
        assert_eq!(up.key, None);
    }

    #[test]
    fn stale_fn_clears_when_flags_drop_it() {
        let mut keys = MacKeys::default();
        keys.update(EVENT_FLAGS_CHANGED, FLAG_FN, KEY_FUNCTION);
        let s = keys.update(EVENT_FLAGS_CHANGED, FLAG_SHIFT, 0x38);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Shift]));
    }

    #[test]
    fn keys_and_escape_are_tracked_separately() {
        let mut keys = MacKeys::default();
        let s = keys.update(EVENT_KEY_DOWN, FLAG_CTRL | FLAG_ALT, 0x31);
        assert_eq!(s.key.as_deref(), Some("Space"));
        assert!(!s.escape);
        let s = keys.update(EVENT_KEY_DOWN, FLAG_CTRL | FLAG_ALT, KEY_ESCAPE);
        assert!(s.escape);
        assert_eq!(s.key.as_deref(), Some("Space"));
        let s = keys.update(EVENT_KEY_UP, FLAG_CTRL | FLAG_ALT, KEY_ESCAPE);
        assert!(!s.escape);
        let s = keys.update(EVENT_KEY_UP, FLAG_CTRL | FLAG_ALT, 0x31);
        assert_eq!(s.key, None);
    }

    #[test]
    fn key_names_cover_the_alphabet_without_duplicates() {
        let mut codes = BTreeSet::new();
        let mut names = BTreeSet::new();
        for (code, name) in KEY_NAMES {
            assert!(codes.insert(*code), "duplicate keycode {code:#x}");
            assert!(names.insert(*name), "duplicate name {name}");
        }
        assert_eq!(key_name(0x00), "A");
        assert_eq!(key_name(0x1D), "0");
        assert_eq!(key_name(0x7A), "F1");
        assert_eq!(key_name(0x5A), "F20");
        assert_eq!(key_name(0x7B), "Other");
    }

    #[test]
    fn trust_query_links_and_returns() {
        // Only reads the trust state, never prompts and never creates a tap.
        let _ = accessibility_ok(false);
    }
}
