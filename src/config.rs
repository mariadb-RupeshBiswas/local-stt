//! User settings in config.json.

use crate::hotkey::{Combo, Modifier};
use crate::models::ModelId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Hold,
    Toggle,
}

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
    pub mode: Mode,
    pub translate: bool,
    pub language: String,
    pub paste: bool,
    pub restore_clipboard: bool,
    pub microphone: Option<String>,
    pub theme: Theme,
    pub show_overlay: bool,
    pub overlay_pos: Option<OverlayPos>,
    pub sounds: bool,
    pub save_history: bool,
    pub autostart: bool,
    pub model: ModelId,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: default_hotkey(),
            mode: Mode::Hold,
            translate: true,
            language: "auto".into(),
            paste: true,
            restore_clipboard: false,
            microphone: None,
            theme: Theme::Pill,
            show_overlay: true,
            overlay_pos: None,
            sounds: true,
            save_history: true,
            autostart: false,
            model: ModelId::Small,
        }
    }
}

// Private copy of hotkey::default_combo so this module builds before Task 4 lands.
fn default_hotkey() -> Combo {
    let modifiers = if cfg!(target_os = "macos") {
        vec![Modifier::Fn, Modifier::Shift]
    } else {
        vec![Modifier::Ctrl, Modifier::Alt]
    };
    Combo {
        modifiers,
        key: None,
    }
}

// Private copy of the minimal hotkey::validate rule.
fn hotkey_ok(c: &Combo) -> bool {
    match c.modifiers.len() {
        0 => false,
        1 => c.key.is_some(),
        _ => true,
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

// Owner-only on Unix; the mode applies only when the file is created.
pub(crate) fn private_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts
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
    if !hotkey_ok(&cfg.hotkey) {
        cfg.hotkey = default_hotkey();
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
        assert_eq!(c.mode, Mode::Hold);
    }
    #[test]
    fn invalid_hotkey_replaced_by_default() {
        let p = tmp("hk");
        std::fs::write(&p, r#"{"hotkey":{"modifiers":[],"key":"A"}}"#).unwrap();
        assert_eq!(load(&p).hotkey, default_hotkey());
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
        assert_eq!(
            (c.mode, c.theme, c.model),
            (Mode::Hold, Theme::Pill, ModelId::Small)
        );
        assert!(c.translate && c.paste && c.show_overlay && c.sounds && c.save_history);
        assert!(!c.restore_clipboard && !c.autostart);
        assert_eq!(c.language, "auto");
        assert!(c.microphone.is_none() && c.overlay_pos.is_none());
        assert!(c.hotkey.key.is_none() && hotkey_ok(&c.hotkey));
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
