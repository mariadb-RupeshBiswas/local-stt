//! macOS Cmd+V via CGEventPost.

use std::ffi::c_void;

type CGEventSourceRef = *mut c_void;
type CGEventRef = *mut c_void;

const COMBINED_SESSION_STATE: i32 = 0; // kCGEventSourceStateCombinedSessionState
const HID_EVENT_TAP: u32 = 0; // kCGHIDEventTap
const COMMAND_MASK: u64 = 0x0010_0000; // kCGEventFlagMaskCommand (NX_COMMANDMASK)
const KEY_V: u16 = 9; // kVK_ANSI_V

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceCreate(state_id: i32) -> CGEventSourceRef;
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

// Owns one retained CF object and releases it on drop.
struct Cf(*mut c_void);

impl Cf {
    fn new(raw: *mut c_void) -> Option<Cf> {
        if raw.is_null() {
            None
        } else {
            Some(Cf(raw))
        }
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        // SAFETY: self.0 is a non-null object we own from a Create call, released once here.
        unsafe { CFRelease(self.0) }
    }
}

fn new_source() -> Option<Cf> {
    // SAFETY: plain value argument; returns an owned source or null.
    Cf::new(unsafe { CGEventSourceCreate(COMBINED_SESSION_STATE) })
}

fn key_event(source: &Cf, key_down: bool) -> Option<Cf> {
    // SAFETY: source is a live CGEventSource; returns an owned event or null.
    let event = Cf::new(unsafe { CGEventCreateKeyboardEvent(source.0, KEY_V, key_down) })?;
    // SAFETY: event is a live CGEvent.
    unsafe { CGEventSetFlags(event.0, COMMAND_MASK) };
    Some(event)
}

fn post(event: &Cf) {
    // SAFETY: event is a live CGEvent; posting does not consume it.
    unsafe { CGEventPost(HID_EVENT_TAP, event.0) }
}

pub fn send_paste() -> Result<(), String> {
    let source = new_source().ok_or("could not create a keyboard event source")?;
    let down = key_event(&source, true).ok_or("could not create the V key-down event")?;
    let up = key_event(&source, false).ok_or("could not create the V key-up event")?;
    post(&down);
    post(&up);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Builds the events without posting, so no keystroke ever reaches the focused app.
    #[test]
    fn paste_events_build_without_posting() {
        let source = new_source().expect("event source");
        assert!(key_event(&source, true).is_some());
        assert!(key_event(&source, false).is_some());
    }
}
