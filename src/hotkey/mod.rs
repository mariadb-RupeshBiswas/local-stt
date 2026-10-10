//! Configurable push-to-talk combo: pure matcher plus OS hooks. Owned by Task 4.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(windows, test))]
mod windows;

#[cfg(target_os = "macos")]
use macos as os;
#[cfg(windows)]
use windows as os;

#[cfg(not(any(target_os = "macos", windows)))]
mod os {
    use super::{Core, HotkeyEvent};
    use std::sync::mpsc::Sender;
    use std::sync::{Arc, Mutex};

    pub struct Guard;

    pub fn spawn(_core: Arc<Mutex<Core>>, _tx: Sender<HotkeyEvent>) -> Result<Guard, String> {
        Err("global hotkeys are only supported on macOS and Windows".into())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
    Fn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub modifiers: Vec<Modifier>,
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
    /// The hands-free combo went down: start listening, or stop if already listening.
    Toggle,
    Cancel,
    Captured(Combo),
}

pub struct KeyState {
    pub modifiers: BTreeSet<Modifier>,
    pub key: Option<String>,
    pub escape: bool,
}

impl KeyState {
    #[cfg(target_os = "macos")]
    fn nothing_held() -> KeyState {
        KeyState {
            modifiers: BTreeSet::new(),
            key: None,
            escape: false,
        }
    }
}

// Pairs of (modifiers that must all be present, key) that the OS owns.
const REFUSED: &[(&[Modifier], &str)] = &[
    (&[Modifier::Meta], "Q"),
    (&[Modifier::Meta], "W"),
    (&[Modifier::Meta], "Tab"),
    (&[Modifier::Meta], "Space"),
    (&[Modifier::Meta], "H"),
    (&[Modifier::Meta], "M"),
    (&[Modifier::Alt], "Tab"),
    (&[Modifier::Alt], "F4"),
    (&[Modifier::Ctrl, Modifier::Alt], "Delete"),
    (&[Modifier::Ctrl, Modifier::Shift], "Escape"),
    (&[Modifier::Ctrl], "Escape"),
];

// Display order: Apple style, Fn first.
const DISPLAY_ORDER: [Modifier; 5] = [
    Modifier::Fn,
    Modifier::Ctrl,
    Modifier::Alt,
    Modifier::Shift,
    Modifier::Meta,
];

pub struct Matcher {
    modifiers: BTreeSet<Modifier>,
    key: Option<String>,
    held: bool,
    // After an interrupted press, ignore the combo until the modifiers are let go.
    latched: bool,
}

impl Matcher {
    pub fn new(combo: Combo) -> Matcher {
        Matcher {
            modifiers: combo.modifiers.into_iter().collect(),
            key: combo.key,
            held: false,
            latched: false,
        }
    }

    pub fn set_combo(&mut self, combo: Combo) {
        self.modifiers = combo.modifiers.into_iter().collect();
        self.key = combo.key;
        self.reset();
    }

    fn reset(&mut self) {
        self.held = false;
        self.latched = false;
    }

    pub fn update(&mut self, state: &KeyState) -> Option<HotkeyEvent> {
        let modifiers_ok = state.modifiers == self.modifiers;
        let hit = modifiers_ok && !state.escape && state.key == self.key;
        if !self.held {
            if !modifiers_ok {
                self.latched = false;
            }
            if hit && !self.latched {
                self.held = true;
                return Some(HotkeyEvent::Pressed);
            }
            return None;
        }
        if state.escape {
            self.held = false;
            self.latched = modifiers_ok;
            return Some(HotkeyEvent::Cancel);
        }
        if hit {
            return None;
        }
        self.held = false;
        // Letting go of the combo key alone is a normal release, anything else is interference.
        let key_let_go = self.key.is_some() && state.key.is_none();
        self.latched = modifiers_ok && !key_let_go;
        Some(HotkeyEvent::Released)
    }
}

// Modifiers and key seen since the first key went down, sent once everything is up.
#[derive(Default)]
struct Peak {
    modifiers: BTreeSet<Modifier>,
    key: Option<String>,
}

// Push-to-talk and hands-free matchers plus one-shot capture mode, shared between the hook thread and `Hook`.
pub(crate) struct Core {
    matcher: Matcher,
    toggle: Option<Matcher>,
    capture: Option<Peak>,
    escape_down: bool,
}

impl Core {
    fn new(combo: Combo) -> Core {
        Core {
            matcher: Matcher::new(combo),
            toggle: None,
            capture: None,
            escape_down: false,
        }
    }

    fn set_capture(&mut self, on: bool) {
        self.capture = on.then(Peak::default);
        self.matcher.reset();
        if let Some(t) = self.toggle.as_mut() {
            t.reset();
        }
    }

    fn set_toggle(&mut self, combo: Option<Combo>) {
        self.toggle = combo.map(Matcher::new);
    }

    #[cfg(test)]
    fn feed(&mut self, state: &KeyState) -> Option<HotkeyEvent> {
        self.feed_all(state).into_iter().next()
    }

    // The hands-free event goes first, so a push-to-talk hold that grows into it is converted, not ended.
    fn feed_all(&mut self, state: &KeyState) -> Vec<HotkeyEvent> {
        if self.capture.is_some() {
            return self.capture(state).into_iter().collect();
        }
        let mut events = Vec::new();
        if let Some(t) = self.toggle.as_mut() {
            if t.update(state) == Some(HotkeyEvent::Pressed) {
                events.push(HotkeyEvent::Toggle);
            }
        }
        match self.matcher.update(state) {
            Some(HotkeyEvent::Cancel) | None => {}
            Some(event) => events.push(event),
        }
        // Esc cancels any recording, including hands-free when no combo is held.
        let escape_pressed = state.escape && !self.escape_down;
        self.escape_down = state.escape;
        if escape_pressed {
            events.push(HotkeyEvent::Cancel);
        }
        events
    }

    fn capture(&mut self, state: &KeyState) -> Option<HotkeyEvent> {
        let peak = self.capture.as_mut()?;
        if state.escape {
            self.capture = None;
            return Some(HotkeyEvent::Cancel);
        }
        if state.modifiers.is_empty() && state.key.is_none() {
            if peak.modifiers.is_empty() && peak.key.is_none() {
                return None;
            }
            let combo = Combo {
                modifiers: peak.modifiers.iter().copied().collect(),
                key: peak.key.take(),
            };
            self.capture = None;
            return Some(HotkeyEvent::Captured(combo));
        }
        peak.modifiers.extend(state.modifiers.iter().copied());
        if state.key.is_some() {
            peak.key = state.key.clone();
        }
        None
    }
}

fn lock(core: &Mutex<Core>) -> MutexGuard<'_, Core> {
    core.lock().unwrap_or_else(PoisonError::into_inner)
}

// Called from the OS hook threads: match under the lock, send after releasing it.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
pub(crate) fn dispatch(core: &Mutex<Core>, tx: &Sender<HotkeyEvent>, state: &KeyState) {
    let events = lock(core).feed_all(state);
    for event in events {
        let _ = tx.send(event);
    }
}

pub struct Hook {
    core: Arc<Mutex<Core>>,
    _guard: os::Guard,
}

impl Hook {
    pub fn set_combo(&self, combo: Combo) {
        lock(&self.core).matcher.set_combo(combo);
    }

    pub fn set_capture(&self, on: bool) {
        lock(&self.core).set_capture(on);
    }

    pub fn set_toggle(&self, combo: Option<Combo>) {
        lock(&self.core).set_toggle(combo);
    }
}

pub fn default_combo() -> Combo {
    if cfg!(target_os = "macos") {
        Combo {
            modifiers: vec![Modifier::Fn, Modifier::Shift],
            key: None,
        }
    } else {
        Combo {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: None,
        }
    }
}

/// True when both combos use the same keys, whatever order the modifiers were listed in.
pub fn same(a: &Combo, b: &Combo) -> bool {
    let left: BTreeSet<Modifier> = a.modifiers.iter().copied().collect();
    let right: BTreeSet<Modifier> = b.modifiers.iter().copied().collect();
    left == right && a.key == b.key
}

/// Hands-free default: the push-to-talk combo plus Space.
pub fn default_toggle_combo() -> Combo {
    let mut combo = default_combo();
    combo.key = Some("Space".into());
    combo
}

fn is_known_key(key: &str) -> bool {
    match key.as_bytes() {
        b"Space" | [b'A'..=b'Z'] | [b'0'..=b'9'] => true,
        [b'F', digits @ ..] if digits.first().is_some_and(|d| (b'1'..=b'9').contains(d)) => {
            digits.iter().all(u8::is_ascii_digit) && matches!(key[1..].parse::<u8>(), Ok(1..=24))
        }
        _ => false,
    }
}

pub fn validate(c: &Combo) -> Result<(), String> {
    let modifiers: BTreeSet<Modifier> = c.modifiers.iter().copied().collect();
    if modifiers.is_empty() {
        return Err("Add at least one modifier (Ctrl, Alt, Shift, Cmd/Win or Fn).".into());
    }
    if cfg!(windows) && modifiers.contains(&Modifier::Fn) {
        return Err("Windows cannot see the Fn key, pick another modifier.".into());
    }
    let Some(key) = c.key.as_deref() else {
        if modifiers.len() < 2 {
            return Err(
                "A single modifier fires while typing, add a second modifier or a key.".into(),
            );
        }
        return Ok(());
    };
    for (needed, refused_key) in REFUSED {
        if key == *refused_key && needed.iter().all(|m| modifiers.contains(m)) {
            return Err(format!(
                "{} is a system shortcut, pick another combo.",
                display(c)
            ));
        }
    }
    if key == "Escape" {
        return Err("Esc cancels a recording, so it cannot be part of the hotkey.".into());
    }
    if !is_known_key(key) {
        return Err(format!(
            "Key \"{key}\" is not supported, use Space, A-Z, 0-9 or F1-F24."
        ));
    }
    Ok(())
}

pub fn display(c: &Combo) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for m in DISPLAY_ORDER {
        if !c.modifiers.contains(&m) {
            continue;
        }
        parts.push(match m {
            Modifier::Fn => "Fn",
            Modifier::Ctrl => "Ctrl",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
            Modifier::Meta if cfg!(target_os = "macos") => "Cmd",
            Modifier::Meta => "Win",
        });
    }
    if let Some(key) = c.key.as_deref() {
        parts.push(key);
    }
    parts.join(" + ")
}

pub fn start(combo: Combo, tx: Sender<HotkeyEvent>) -> Result<Hook, String> {
    let core = Arc::new(Mutex::new(Core::new(combo)));
    let guard = os::spawn(Arc::clone(&core), tx)?;
    Ok(Hook {
        core,
        _guard: guard,
    })
}

#[cfg(target_os = "macos")]
pub fn accessibility_ok(prompt: bool) -> bool {
    macos::accessibility_ok(prompt)
}

#[cfg(not(target_os = "macos"))]
pub fn accessibility_ok(_prompt: bool) -> bool {
    true
}

#[cfg(test)]
mod dual_tests {
    use super::*;
    fn st(m: &[Modifier], key: Option<&str>, esc: bool) -> KeyState {
        KeyState {
            modifiers: m.iter().copied().collect(),
            key: key.map(String::from),
            escape: esc,
        }
    }
    fn core() -> Core {
        let mut c = Core::new(Combo {
            modifiers: vec![Modifier::Fn, Modifier::Shift],
            key: None,
        });
        c.set_toggle(Some(Combo {
            modifiers: vec![Modifier::Fn, Modifier::Shift],
            key: Some("Space".into()),
        }));
        c
    }
    const FS: &[Modifier] = &[Modifier::Fn, Modifier::Shift];

    #[test]
    fn hold_then_space_emits_toggle_before_release() {
        let mut c = core();
        assert_eq!(c.feed_all(&st(FS, None, false)), vec![HotkeyEvent::Pressed]);
        assert_eq!(
            c.feed_all(&st(FS, Some("Space"), false)),
            vec![HotkeyEvent::Toggle, HotkeyEvent::Released]
        );
        // Letting go of Space while still holding Fn+Shift must not start a new hold.
        assert!(c.feed_all(&st(FS, None, false)).is_empty());
        assert!(c.feed_all(&st(&[], None, false)).is_empty());
    }

    #[test]
    fn second_press_of_hands_free_combo_toggles_again() {
        let mut c = core();
        c.feed_all(&st(FS, None, false));
        c.feed_all(&st(FS, Some("Space"), false));
        c.feed_all(&st(&[], None, false));
        assert_eq!(c.feed_all(&st(FS, None, false)), vec![HotkeyEvent::Pressed]);
        assert_eq!(
            c.feed_all(&st(FS, Some("Space"), false)),
            vec![HotkeyEvent::Toggle, HotkeyEvent::Released]
        );
    }

    #[test]
    fn escape_alone_cancels_hands_free() {
        let mut c = core();
        assert_eq!(c.feed_all(&st(&[], None, true)), vec![HotkeyEvent::Cancel]);
        assert!(c.feed_all(&st(&[], None, true)).is_empty());
        assert!(c.feed_all(&st(&[], None, false)).is_empty());
    }

    #[test]
    fn no_toggle_configured_means_no_toggle_events() {
        let mut c = core();
        c.set_toggle(None);
        c.feed_all(&st(FS, None, false));
        assert_eq!(
            c.feed_all(&st(FS, Some("Space"), false)),
            vec![HotkeyEvent::Released]
        );
    }

    #[test]
    fn same_ignores_modifier_order() {
        let a = Combo {
            modifiers: vec![Modifier::Fn, Modifier::Shift],
            key: None,
        };
        let b = Combo {
            modifiers: vec![Modifier::Shift, Modifier::Fn],
            key: None,
        };
        assert!(same(&a, &b));
        assert!(!same(&a, &default_toggle_combo()) || cfg!(windows));
    }

    #[test]
    fn default_toggle_is_valid_and_differs_from_hold() {
        assert!(validate(&default_toggle_combo()).is_ok());
        assert_ne!(default_toggle_combo(), default_combo());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    fn st(m: &[Modifier], key: Option<&str>, esc: bool) -> KeyState {
        KeyState {
            modifiers: m.iter().copied().collect::<BTreeSet<_>>(),
            key: key.map(String::from),
            escape: esc,
        }
    }
    fn fn_shift() -> Combo {
        Combo {
            modifiers: vec![Modifier::Fn, Modifier::Shift],
            key: None,
        }
    }

    #[test]
    fn press_and_release_modifier_only() {
        let mut m = Matcher::new(fn_shift());
        assert_eq!(m.update(&st(&[Modifier::Fn], None, false)), None);
        assert_eq!(
            m.update(&st(&[Modifier::Fn, Modifier::Shift], None, false)),
            Some(HotkeyEvent::Pressed)
        );
        assert_eq!(
            m.update(&st(&[Modifier::Fn, Modifier::Shift], None, false)),
            None
        );
        assert_eq!(
            m.update(&st(&[Modifier::Fn], None, false)),
            Some(HotkeyEvent::Released)
        );
    }
    #[test]
    fn extra_modifier_does_not_match() {
        let mut m = Matcher::new(fn_shift());
        assert_eq!(
            m.update(&st(
                &[Modifier::Fn, Modifier::Shift, Modifier::Meta],
                None,
                false
            )),
            None
        );
    }
    #[test]
    fn typing_a_letter_while_held_releases() {
        let mut m = Matcher::new(fn_shift());
        m.update(&st(&[Modifier::Fn, Modifier::Shift], None, false));
        assert_eq!(
            m.update(&st(&[Modifier::Fn, Modifier::Shift], Some("A"), false)),
            Some(HotkeyEvent::Released)
        );
    }
    #[test]
    fn combo_with_key() {
        let c = Combo {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: Some("Space".into()),
        };
        let mut m = Matcher::new(c);
        assert_eq!(
            m.update(&st(&[Modifier::Ctrl, Modifier::Alt], None, false)),
            None
        );
        assert_eq!(
            m.update(&st(&[Modifier::Ctrl, Modifier::Alt], Some("Space"), false)),
            Some(HotkeyEvent::Pressed)
        );
        assert_eq!(
            m.update(&st(&[Modifier::Ctrl, Modifier::Alt], None, false)),
            Some(HotkeyEvent::Released)
        );
    }
    #[test]
    fn escape_while_held_cancels() {
        let mut m = Matcher::new(fn_shift());
        m.update(&st(&[Modifier::Fn, Modifier::Shift], None, false));
        assert_eq!(
            m.update(&st(&[Modifier::Fn, Modifier::Shift], None, true)),
            Some(HotkeyEvent::Cancel)
        );
        assert_eq!(m.update(&st(&[], None, false)), None);
    }
    #[test]
    fn validate_rules() {
        // Windows hooks cannot see Fn, so Fn combos are refused there by design.
        assert_eq!(validate(&fn_shift()).is_ok(), !cfg!(windows));
        assert!(validate(&default_combo()).is_ok());
        assert!(validate(&Combo {
            modifiers: vec![],
            key: Some("A".into())
        })
        .is_err());
        assert!(validate(&Combo {
            modifiers: vec![Modifier::Meta],
            key: Some("Q".into())
        })
        .is_err());
        assert!(validate(&Combo {
            modifiers: vec![Modifier::Meta],
            key: Some("Tab".into())
        })
        .is_err());
        assert!(validate(&Combo {
            modifiers: vec![Modifier::Alt],
            key: Some("F4".into())
        })
        .is_err());
        assert!(validate(&Combo {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: Some("Delete".into())
        })
        .is_err());
        assert!(validate(&Combo {
            modifiers: vec![Modifier::Shift],
            key: None
        })
        .is_err());
    }
    #[test]
    fn display_is_readable() {
        assert_eq!(display(&fn_shift()), "Fn + Shift");
        assert_eq!(
            display(&Combo {
                modifiers: vec![Modifier::Alt, Modifier::Ctrl],
                key: Some("Space".into())
            }),
            "Ctrl + Alt + Space"
        );
    }
    #[test]
    fn combo_json_roundtrip() {
        let c = fn_shift();
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Combo>(&s).unwrap(), c);
    }

    #[test]
    fn letter_released_while_modifiers_held_does_not_repress() {
        let mut m = Matcher::new(fn_shift());
        let both = [Modifier::Fn, Modifier::Shift];
        m.update(&st(&both, None, false));
        assert_eq!(
            m.update(&st(&both, Some("A"), false)),
            Some(HotkeyEvent::Released)
        );
        assert_eq!(m.update(&st(&both, None, false)), None);
        assert_eq!(m.update(&st(&[Modifier::Fn], None, false)), None);
        assert_eq!(
            m.update(&st(&both, None, false)),
            Some(HotkeyEvent::Pressed)
        );
    }

    #[test]
    fn escape_released_while_modifiers_held_does_not_repress() {
        let mut m = Matcher::new(fn_shift());
        let both = [Modifier::Fn, Modifier::Shift];
        m.update(&st(&both, None, false));
        assert_eq!(m.update(&st(&both, None, true)), Some(HotkeyEvent::Cancel));
        assert_eq!(m.update(&st(&both, None, false)), None);
    }

    #[test]
    fn key_combo_can_be_tapped_repeatedly_while_modifiers_stay_down() {
        let mods = [Modifier::Ctrl, Modifier::Alt];
        let mut m = Matcher::new(Combo {
            modifiers: mods.to_vec(),
            key: Some("Space".into()),
        });
        assert_eq!(
            m.update(&st(&mods, Some("Space"), false)),
            Some(HotkeyEvent::Pressed)
        );
        assert_eq!(
            m.update(&st(&mods, None, false)),
            Some(HotkeyEvent::Released)
        );
        assert_eq!(
            m.update(&st(&mods, Some("Space"), false)),
            Some(HotkeyEvent::Pressed)
        );
    }

    #[test]
    fn escape_without_a_hold_is_ignored() {
        let mut m = Matcher::new(fn_shift());
        assert_eq!(m.update(&st(&[], None, true)), None);
        assert_eq!(
            m.update(&st(&[Modifier::Fn, Modifier::Shift], None, true)),
            None
        );
    }

    #[test]
    fn set_combo_switches_the_trigger() {
        let mut m = Matcher::new(fn_shift());
        m.set_combo(Combo {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: None,
        });
        assert_eq!(
            m.update(&st(&[Modifier::Fn, Modifier::Shift], None, false)),
            None
        );
        let s = st(&[Modifier::Ctrl, Modifier::Alt], None, false);
        assert_eq!(m.update(&s), Some(HotkeyEvent::Pressed));
    }

    #[test]
    fn capture_sends_the_peak_combo_once_everything_is_up() {
        let mut core = Core::new(fn_shift());
        core.set_capture(true);
        assert_eq!(core.feed(&st(&[Modifier::Ctrl], None, false)), None);
        assert_eq!(
            core.feed(&st(&[Modifier::Ctrl, Modifier::Alt], None, false)),
            None
        );
        assert_eq!(
            core.feed(&st(&[Modifier::Ctrl, Modifier::Alt], Some("Space"), false)),
            None
        );
        assert_eq!(core.feed(&st(&[Modifier::Ctrl], None, false)), None);
        let want = Combo {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: Some("Space".into()),
        };
        assert_eq!(
            core.feed(&st(&[], None, false)),
            Some(HotkeyEvent::Captured(want))
        );
        // One-shot: the next hold is a normal press again.
        let both = [Modifier::Fn, Modifier::Shift];
        assert_eq!(
            core.feed(&st(&both, None, false)),
            Some(HotkeyEvent::Pressed)
        );
    }

    #[test]
    fn capture_with_nothing_pressed_sends_nothing() {
        let mut core = Core::new(fn_shift());
        core.set_capture(true);
        assert_eq!(core.feed(&st(&[], None, false)), None);
    }

    #[test]
    fn escape_aborts_capture() {
        let mut core = Core::new(fn_shift());
        core.set_capture(true);
        core.feed(&st(&[Modifier::Ctrl], None, false));
        assert_eq!(
            core.feed(&st(&[Modifier::Ctrl], None, true)),
            Some(HotkeyEvent::Cancel)
        );
        assert_eq!(core.feed(&st(&[], None, false)), None);
    }

    #[test]
    fn validate_accepts_the_documented_alphabet() {
        let ok = |key: &str| {
            validate(&Combo {
                modifiers: vec![Modifier::Ctrl, Modifier::Alt],
                key: Some(key.into()),
            })
        };
        for key in ["Space", "A", "Z", "0", "9", "F1", "F9", "F10", "F24"] {
            assert!(ok(key).is_ok(), "{key} should be accepted");
        }
        for key in [
            "a", "F0", "F25", "F01", "F+1", "Tab", "Escape", "Enter", "", "AB",
        ] {
            assert!(ok(key).is_err(), "{key} should be refused");
        }
    }

    #[test]
    fn validate_refuses_system_shortcuts_with_extra_modifiers() {
        let shift_cmd_q = Combo {
            modifiers: vec![Modifier::Meta, Modifier::Shift],
            key: Some("Q".into()),
        };
        assert!(validate(&shift_cmd_q).is_err());
        let ctrl_esc = Combo {
            modifiers: vec![Modifier::Ctrl],
            key: Some("Escape".into()),
        };
        assert!(validate(&ctrl_esc).is_err());
        let ctrl_alt_a = Combo {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: Some("A".into()),
        };
        assert!(validate(&ctrl_alt_a).is_ok());
    }

    #[test]
    fn display_names_meta_per_platform() {
        let c = Combo {
            modifiers: vec![Modifier::Shift, Modifier::Meta],
            key: Some("K".into()),
        };
        let want = if cfg!(target_os = "macos") {
            "Shift + Cmd + K"
        } else {
            "Shift + Win + K"
        };
        assert_eq!(display(&c), want);
    }

    #[test]
    fn default_combo_is_valid() {
        assert_eq!(validate(&default_combo()), Ok(()));
    }
}
