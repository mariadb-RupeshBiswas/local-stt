//! Commands the UI may call. Every input is validated here; the UI is not trusted.

use crate::app::{self, lock, Shared, Slot};
use crate::config::Config;
use crate::models::{self, ModelId};
use crate::{autostart, history, hotkey, paths};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, State};

const HISTORY_LIMIT: usize = 500;
const MAX_COPY_CHARS: usize = 100_000;

#[tauri::command]
pub fn get_state(state: State<'_, Arc<Shared>>) -> Value {
    let cfg = state.config();
    let fits = models::evaluate(&state.hardware, &paths::models_dir());
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else {
        "windows"
    };
    json!({
        "config": cfg,
        "models": fits,
        "hardware": state.hardware,
        "inputs": crate::recorder::Recorder::list_inputs(),
        "version": env!("CARGO_PKG_VERSION"),
        "platform": platform,
        "hotkeyDisplay": hotkey::display(&cfg.hotkey),
        "toggleDisplay": cfg.toggle_hotkey.as_ref().map(hotkey::display),
        "installed": crate::install::is_installed(),
        "activeModel": *lock(&state.active_model),
        "downloading": *lock(&state.downloading),
        "autostartEnabled": autostart::is_enabled(),
    })
}

pub fn valid_language(lang: &str) -> bool {
    lang == "auto"
        || ((2..=3).contains(&lang.len()) && lang.chars().all(|c| c.is_ascii_lowercase()))
}

/// Merges a partial JSON object into the current config, validates it, saves and applies it.
pub fn merge_patch(current: &Config, patch: &Value) -> Result<Config, String> {
    let Value::Object(changes) = patch else {
        return Err("settings patch must be a JSON object".into());
    };
    let mut merged = serde_json::to_value(current).map_err(|e| e.to_string())?;
    let Value::Object(target) = &mut merged else {
        return Err("internal: config is not an object".into());
    };
    for (key, value) in changes {
        if !target.contains_key(key) {
            return Err(format!("unknown setting: {key}"));
        }
        target.insert(key.clone(), value.clone());
    }
    let next: Config =
        serde_json::from_value(merged).map_err(|e| format!("invalid setting value: {e}"))?;
    hotkey::validate(&next.hotkey)?;
    if let Some(t) = next.toggle_hotkey.as_ref() {
        hotkey::validate(t)?;
        if hotkey::same(t, &next.hotkey) {
            return Err("Push to talk and hands-free need different shortcuts.".into());
        }
    }
    if !valid_language(&next.language) {
        return Err(format!("unknown language code: {}", next.language));
    }
    Ok(next)
}

#[tauri::command]
pub fn set_config(
    app: AppHandle,
    state: State<'_, Arc<Shared>>,
    patch: Value,
) -> Result<Config, String> {
    let before = state.config();
    let next = merge_patch(&before, &patch)?;
    if next.autostart != before.autostart {
        let note = autostart::set(next.autostart)?;
        let _ = tauri::Emitter::emit(&app, "notice", json!({ "message": note }));
    }
    if before.save_history && !next.save_history {
        // "Off" means nothing kept on disk, not just nothing new.
        history::clear(&paths::history_path())
            .map_err(|e| format!("cannot delete history: {e}"))?;
        let _ = tauri::Emitter::emit(&app, "history-changed", ());
    }
    let saved = state.update_config(|c| *c = next.clone());
    if saved.hotkey != before.hotkey {
        if let Some(hook) = lock(&state.hook).as_ref() {
            hook.set_combo(saved.hotkey.clone());
        }
        app::refresh_tray(&app);
    }
    if saved.toggle_hotkey != before.toggle_hotkey {
        if let Some(hook) = lock(&state.hook).as_ref() {
            hook.set_toggle(saved.toggle_hotkey.clone());
        }
        app::refresh_tray(&app);
    }
    if saved.theme != before.theme || saved.show_overlay != before.show_overlay {
        app::overlay_theme_changed(&app);
    }
    Ok(saved)
}

#[tauri::command]
pub fn start_hotkey_capture(
    state: State<'_, Arc<Shared>>,
    slot: Option<String>,
) -> Result<(), String> {
    let slot = match slot.as_deref() {
        None | Some("hold") => Slot::Hold,
        Some("toggle") => Slot::Toggle,
        Some(other) => return Err(format!("unknown shortcut slot: {other}")),
    };
    match lock(&state.hook).as_ref() {
        Some(hook) => {
            *lock(&state.capture_slot) = Some(slot);
            hook.set_capture(true);
            Ok(())
        }
        None => Err("The shortcut listener is not running. Check Accessibility permission.".into()),
    }
}

#[tauri::command]
pub fn cancel_hotkey_capture(state: State<'_, Arc<Shared>>) {
    *lock(&state.capture_slot) = None;
    if let Some(hook) = lock(&state.hook).as_ref() {
        hook.set_capture(false);
    }
}

#[tauri::command]
pub fn choose_model(
    app: AppHandle,
    state: State<'_, Arc<Shared>>,
    id: ModelId,
) -> Result<(), String> {
    let fits = models::evaluate(&state.hardware, &paths::models_dir());
    let Some(fit) = fits.iter().find(|f| f.id == id) else {
        return Err("unknown model".into());
    };
    if fit.downloaded {
        app::activate_model(&app, id);
        return Ok(());
    }
    if !fit.supported {
        return Err(fit.reasons.join(" "));
    }
    app::start_download(app.clone(), id)
}

#[tauri::command]
pub fn get_history() -> Result<Vec<history::Entry>, String> {
    history::read_newest_first(&paths::history_path(), HISTORY_LIMIT).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), String> {
    history::clear(&paths::history_path()).map_err(|e| e.to_string())?;
    let _ = tauri::Emitter::emit(&app, "history-changed", ());
    Ok(())
}

#[tauri::command]
pub fn reset_overlay_position(state: State<'_, Arc<Shared>>) {
    state.update_config(|c| c.overlay_pos = None);
}

#[tauri::command]
pub fn move_overlay(app: AppHandle, on: bool) {
    app::overlay_positioning(&app, on);
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    if text.chars().count() > MAX_COPY_CHARS {
        return Err("text too long".into());
    }
    crate::clipboard::write_private(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_changes_only_named_fields() {
        let base = Config::default();
        let next = merge_patch(&base, &json!({ "paste": false, "theme": "minimal" })).unwrap();
        assert!(!next.paste);
        assert_eq!(next.theme, crate::config::Theme::Minimal);
        assert_eq!(next.toggle_hotkey, base.toggle_hotkey);
    }

    #[test]
    fn unknown_setting_rejected() {
        assert!(merge_patch(&Config::default(), &json!({ "evil": 1 })).is_err());
    }

    #[test]
    fn bad_value_rejected() {
        assert!(merge_patch(&Config::default(), &json!({ "theme": "neon" })).is_err());
        assert!(merge_patch(&Config::default(), &json!({ "language": "../../etc" })).is_err());
    }

    #[test]
    fn invalid_hotkey_rejected() {
        let patch = json!({ "hotkey": { "modifiers": [], "key": "A" } });
        assert!(merge_patch(&Config::default(), &patch).is_err());
    }

    #[test]
    fn hands_free_equal_to_push_to_talk_rejected() {
        let base = Config::default();
        let patch = json!({ "toggle_hotkey": base.hotkey });
        assert!(merge_patch(&base, &patch).is_err());
    }

    #[test]
    fn hands_free_can_be_turned_off() {
        let next = merge_patch(&Config::default(), &json!({ "toggle_hotkey": null })).unwrap();
        assert_eq!(next.toggle_hotkey, None);
    }

    #[test]
    fn non_object_patch_rejected() {
        assert!(merge_patch(&Config::default(), &json!([1, 2])).is_err());
    }

    #[test]
    fn language_codes() {
        assert!(
            valid_language("auto")
                && valid_language("en")
                && valid_language("hi")
                && valid_language("haw")
        );
        assert!(!valid_language("EN") && !valid_language("") && !valid_language("english"));
    }
}
