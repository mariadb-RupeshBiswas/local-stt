//! Shortcuts the system already uses: a combo that matches one may never reach local-stt.
use super::{same, Combo, Modifier};
use serde::Serialize;

/// One of local-stt's shortcuts that the system also uses.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Clash {
    pub slot: &'static str,
    pub with: String,
    pub fix: String,
}

/// A combo the system uses, what for, and how to free it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taken {
    pub combo: Combo,
    pub with: String,
    pub fix: String,
}

pub fn clashes(hold: &Combo, toggle: Option<&Combo>, taken: &[Taken]) -> Vec<Clash> {
    let mut out = Vec::new();
    for (slot, combo) in [("hold", Some(hold)), ("toggle", toggle)] {
        let Some(combo) = combo else { continue };
        if let Some(t) = taken.iter().find(|t| same(&t.combo, combo)) {
            out.push(Clash {
                slot,
                with: t.with.clone(),
                fix: t.fix.clone(),
            });
        }
    }
    out
}

fn combo(modifiers: &[Modifier], key: Option<&str>) -> Combo {
    Combo {
        modifiers: modifiers.to_vec(),
        key: key.map(String::from),
    }
}

const PICK_ANOTHER: &str = "Pick another shortcut.";
#[cfg(any(target_os = "macos", test))]
const MAC_SETTINGS: &str =
    "Pick another shortcut, or turn this one off in System Settings > Keyboard > Keyboard Shortcuts.";

// Apple's list of system-wide shortcuts, limited to keys a local-stt shortcut can use.
#[cfg(any(target_os = "macos", test))]
pub fn mac_defaults() -> Vec<Taken> {
    use Modifier::{Alt, Ctrl, Fn, Meta, Shift};
    let rows: [(&[Modifier], &str, &str, &str); 9] = [
        (&[Meta], "Space", "Spotlight", MAC_SETTINGS),
        (
            &[Ctrl],
            "Space",
            "switching to the previous input source",
            MAC_SETTINGS,
        ),
        (
            &[Ctrl, Alt],
            "Space",
            "switching to the next input source",
            MAC_SETTINGS,
        ),
        (&[Shift, Meta], "3", "taking a screenshot", MAC_SETTINGS),
        (
            &[Shift, Meta],
            "4",
            "taking a screenshot of an area",
            MAC_SETTINGS,
        ),
        (
            &[Shift, Meta],
            "5",
            "screenshots and screen recording",
            MAC_SETTINGS,
        ),
        (
            &[Ctrl, Meta],
            "Space",
            "the emoji and symbols viewer",
            PICK_ANOTHER,
        ),
        (&[Fn], "E", "the emoji and symbols viewer", PICK_ANOTHER),
        (&[Ctrl, Meta], "Q", "locking the screen", PICK_ANOTHER),
    ];
    let mut out: Vec<Taken> = rows
        .iter()
        .map(|(mods, key, with, fix)| Taken {
            combo: combo(mods, Some(key)),
            with: (*with).into(),
            fix: (*fix).into(),
        })
        .collect();
    out.push(Taken {
        combo: combo(&[Meta], Some("Tab")),
        with: "switching apps".into(),
        fix: PICK_ANOTHER.into(),
    });
    out
}

// Microsoft's list of Windows shortcuts, limited to keys a local-stt shortcut can use.
#[cfg(any(windows, test))]
pub fn windows_defaults() -> Vec<Taken> {
    use Modifier::{Alt, Ctrl, Meta, Shift};
    let rows: [(&[Modifier], Option<&str>, &str); 7] = [
        (&[Meta], Some("Space"), "switching input language"),
        (&[Meta], Some("H"), "voice dictation"),
        (&[Meta], Some("L"), "locking the computer"),
        (&[Meta, Shift], Some("S"), "screenshots"),
        (&[Alt], Some("Tab"), "switching windows"),
        (
            &[Ctrl, Shift],
            None,
            "switching keyboard layout when you have more than one",
        ),
        (
            &[Ctrl],
            Some("Space"),
            "turning the Chinese input method on and off",
        ),
    ];
    rows.iter()
        .map(|(mods, key, with)| Taken {
            combo: combo(mods, *key),
            with: (*with).into(),
            fix: PICK_ANOTHER.into(),
        })
        .collect()
}

/// Folds in what the user changed in macOS Keyboard Shortcuts: off entries free their combo, on ones take theirs.
#[cfg(any(target_os = "macos", test))]
pub fn apply_mac_overrides(mut taken: Vec<Taken>, overrides: &[(bool, Combo)]) -> Vec<Taken> {
    for (on, c) in overrides {
        if !on {
            taken.retain(|t| !same(&t.combo, c));
        } else if !taken.iter().any(|t| same(&t.combo, c)) {
            taken.push(Taken {
                combo: c.clone(),
                with: "a macOS keyboard shortcut".into(),
                fix: MAC_SETTINGS.into(),
            });
        }
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    use Modifier::{Alt, Ctrl, Fn, Meta, Shift};

    #[test]
    fn a_shortcut_the_system_uses_is_reported_for_its_slot() {
        let taken = mac_defaults();
        let spotlight = combo(&[Meta], Some("Space"));
        let found = clashes(&spotlight, None, &taken);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slot, "hold");
        assert_eq!(found[0].with, "Spotlight");
        let toggle = combo(&[Alt, Ctrl], Some("Space"));
        let found = clashes(&combo(&[Fn, Shift], None), Some(&toggle), &taken);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slot, "toggle");
    }

    #[test]
    fn the_default_shortcuts_clash_with_nothing() {
        let mac = clashes(
            &combo(&[Fn, Shift], None),
            Some(&combo(&[Fn, Shift], Some("Space"))),
            &mac_defaults(),
        );
        let win = clashes(
            &combo(&[Ctrl, Alt], None),
            Some(&combo(&[Ctrl, Alt], Some("Space"))),
            &windows_defaults(),
        );
        assert!(mac.is_empty() && win.is_empty());
    }

    #[test]
    fn windows_modifier_only_ctrl_shift_switches_layout() {
        let found = clashes(&combo(&[Shift, Ctrl], None), None, &windows_defaults());
        assert!(found[0].with.starts_with("switching keyboard layout"));
    }

    #[test]
    fn keyboard_settings_turn_defaults_off_and_add_custom_ones() {
        let off_spotlight = (false, combo(&[Meta], Some("Space")));
        let custom = (true, combo(&[Ctrl, Shift], Some("K")));
        let taken = apply_mac_overrides(mac_defaults(), &[off_spotlight, custom.clone()]);
        assert!(clashes(&combo(&[Meta], Some("Space")), None, &taken).is_empty());
        let found = clashes(&custom.1, None, &taken);
        assert_eq!(found[0].with, "a macOS keyboard shortcut");
        let again = apply_mac_overrides(taken.clone(), &[(true, combo(&[Ctrl], Some("Space")))]);
        assert_eq!(again.len(), taken.len());
    }
}
