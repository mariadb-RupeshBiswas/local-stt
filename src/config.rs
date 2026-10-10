//! User settings in config.json.

use crate::hotkey::Combo;
use crate::models::ModelId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Pill,
    Waveform,
    Minimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OverlayPos {
    pub monitor: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub hotkey: Combo,
    /// Hands-free shortcut: press to start listening, press again to stop. None turns it off.
    pub toggle_hotkey: Option<Combo>,
    pub translate: bool,
    pub language: String,
    pub paste: bool,
    /// Tidy the transcript before pasting: spoken lists, fillers, spacing.
    pub smart_format: bool,
    pub restore_clipboard: bool,
    pub microphone: Option<String>,
    pub theme: Theme,
    pub show_overlay: bool,
    /// Live text above the pill while speaking (a preview; the pasted text is the final pass).
    pub live_transcription: bool,
    pub overlay_pos: Option<OverlayPos>,
    pub sounds: bool,
    pub save_history: bool,
    pub autostart: bool,
    /// Daily version check against PyPI; nothing but the request is sent.
    pub check_updates: bool,
    /// A version the user chose to skip; it is not offered again.
    pub skip_update: Option<String>,
    /// A local troubleshooting log of app events, never content; see diag.rs.
    pub diagnostic_log: bool,
    pub model: ModelId,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: crate::hotkey::default_combo(),
            toggle_hotkey: Some(crate::hotkey::default_toggle_combo()),
            translate: true,
            language: "auto".into(),
            paste: true,
            smart_format: true,
            restore_clipboard: false,
            microphone: None,
            theme: Theme::Pill,
            show_overlay: true,
            live_transcription: true,
            overlay_pos: None,
            sounds: true,
            save_history: true,
            autostart: false,
            check_updates: true,
            skip_update: None,
            diagnostic_log: true,
            model: ModelId::Small,
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

// Owner-only on Unix; the mode applies only when the file is created.
#[cfg(unix)]
pub(crate) fn private_options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;
    let mut opts = OpenOptions::new();
    opts.mode(0o600);
    opts
}

// Windows profile folders are already per-user by ACL.
#[cfg(not(unix))]
pub(crate) fn private_options() -> OpenOptions {
    OpenOptions::new()
}

fn backup(path: &Path) {
    let _ = std::fs::copy(path, with_suffix(path, ".bak"));
}

// Keeps every field that parses on its own; a bad field falls back to its default.
fn parse_fields(parsed: Map<String, Value>) -> (Config, bool) {
    if let Ok(cfg) = serde_json::from_value::<Config>(Value::Object(parsed.clone())) {
        return (cfg, false);
    }
    let mut fields = match serde_json::to_value(Config::default()) {
        Ok(Value::Object(map)) => map,
        _ => return (Config::default(), true),
    };
    let mut cfg = Config::default();
    for (key, value) in parsed {
        if !fields.contains_key(&key) {
            continue;
        }
        let previous = fields.insert(key.clone(), value);
        match serde_json::from_value::<Config>(Value::Object(fields.clone())) {
            Ok(candidate) => cfg = candidate,
            Err(_) => {
                if let Some(previous) = previous {
                    fields.insert(key, previous);
                }
            }
        }
    }
    (cfg, true)
}

pub fn load(path: &Path) -> Config {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return Config::default(),
    };
    let parsed = match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(map)) => map,
        _ => {
            backup(path);
            return Config::default();
        }
    };
    let (mut cfg, mut repaired) = parse_fields(parsed);
    if crate::hotkey::validate(&cfg.hotkey).is_err() {
        cfg.hotkey = crate::hotkey::default_combo();
        repaired = true;
    }
    let toggle_bad = cfg.toggle_hotkey.as_ref().is_some_and(|t| {
        crate::hotkey::validate(t).is_err() || crate::hotkey::same(t, &cfg.hotkey)
    });
    if toggle_bad {
        let fallback = crate::hotkey::default_toggle_combo();
        cfg.toggle_hotkey = (!crate::hotkey::same(&fallback, &cfg.hotkey)).then_some(fallback);
        repaired = true;
    }
    if repaired {
        backup(path);
    }
    cfg
}

pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(cfg).map_err(std::io::Error::other)?;
    let tmp = with_suffix(path, ".tmp");
    // A leftover tmp could carry looser permissions.
    let _ = std::fs::remove_file(&tmp);
    let written = private_options()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(&json));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("lstt-cfg-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join("config.json")
    }
    #[test]
    fn missing_file_gives_defaults() {
        assert_eq!(load(&tmp("missing")), Config::default());
    }
    #[test]
    fn roundtrip() {
        let p = tmp("rt");
        let c = Config {
            theme: Theme::Waveform,
            paste: false,
            ..Config::default()
        };
        save(&p, &c).unwrap();
        assert_eq!(load(&p), c);
    }
    #[test]
    fn corrupt_file_falls_back_and_backs_up() {
        let p = tmp("corrupt");
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(load(&p), Config::default());
        assert!(p.with_extension("json.bak").exists());
    }
    #[test]
    fn unknown_fields_ignored_and_missing_fields_default() {
        let p = tmp("partial");
        std::fs::write(&p, r#"{"theme":"minimal","bogus":1}"#).unwrap();
        let c = load(&p);
        assert_eq!(c.theme, Theme::Minimal);
        assert_eq!(c.toggle_hotkey, Some(crate::hotkey::default_toggle_combo()));
    }
    #[test]
    fn invalid_hotkey_replaced_by_default() {
        let p = tmp("hk");
        std::fs::write(&p, r#"{"hotkey":{"modifiers":[],"key":"A"}}"#).unwrap();
        assert_eq!(load(&p).hotkey, crate::hotkey::default_combo());
    }
    #[test]
    fn unknown_theme_falls_back_but_other_fields_survive() {
        let p = tmp("badtheme");
        std::fs::write(&p, r#"{"theme":"neon","paste":false}"#).unwrap();
        let c = load(&p);
        assert_eq!(c.theme, Theme::Pill);
        assert!(!c.paste);
        assert!(p.with_extension("json.bak").exists());
    }
    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!((c.theme, c.model), (Theme::Pill, ModelId::Small));
        assert!(c.translate && c.paste && c.show_overlay && c.sounds && c.save_history);
        assert!(!c.restore_clipboard && !c.autostart);
        assert_eq!(c.language, "auto");
        assert!(c.microphone.is_none() && c.overlay_pos.is_none());
        assert!(c.hotkey.key.is_none() && crate::hotkey::validate(&c.hotkey).is_ok());
    }
    #[test]
    fn toggle_equal_to_hold_is_replaced() {
        let p = tmp("toggle-same");
        let hold = crate::hotkey::default_combo();
        let json = serde_json::json!({ "hotkey": hold, "toggle_hotkey": hold });
        std::fs::write(&p, json.to_string()).unwrap();
        let c = load(&p);
        assert_ne!(c.toggle_hotkey.as_ref(), Some(&c.hotkey));
    }
    #[test]
    fn toggle_can_be_turned_off() {
        let p = tmp("toggle-off");
        std::fs::write(&p, r#"{"toggle_hotkey":null}"#).unwrap();
        assert_eq!(load(&p).toggle_hotkey, None);
    }
    #[cfg(unix)]
    #[test]
    fn saved_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let p = tmp("perm");
        save(&p, &Config::default()).unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
