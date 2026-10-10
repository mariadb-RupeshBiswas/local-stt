//! Commands the UI may call. Every input is validated here; the UI is not trusted.

use crate::app::{self, lock, Shared, Slot};
use crate::config::Config;
use crate::models::{self, ModelId};
use crate::{autostart, diag, history, hotkey, paths};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, State};

const HISTORY_LIMIT: usize = 500;
const MAX_DELETE_IDS: usize = 10_000;
const MAX_COPY_CHARS: usize = 100_000;

#[tauri::command]
pub fn get_state(app: AppHandle, state: State<'_, Arc<Shared>>) -> Value {
    let cfg = state.config();
    let notes_ok = crate::notes::session::support();
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
        "smartFormatActive": cfg.smart_format && app::english_output(&cfg),
        "notes": {
            "supported": notes_ok.is_ok(),
            "reason": notes_ok.err(),
            "active": crate::notes::session::active(&app).map(|(id, started)| json!({ "id": id, "startedMs": started })),
        },
        "demo": crate::demo::active(),
        "toggleDisplay": cfg.toggle_hotkey.as_ref().map(hotkey::display),
        "installed": crate::install::is_installed(),
        "update": lock(&state.update).clone(),
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
pub async fn set_config(
    app: AppHandle,
    state: State<'_, Arc<Shared>>,
    patch: Value,
) -> Result<Config, String> {
    let before = state.config();
    let next = merge_patch(&before, &patch)?;
    if next.autostart != before.autostart {
        // Turning this on may install the app (copy, codesign), which must not freeze the window.
        let enable = next.autostart;
        let note = tauri::async_runtime::spawn_blocking(move || autostart::set(enable))
            .await
            .map_err(|e| format!("start at login failed: {e}"))?
            .inspect_err(|e| diag::log(&format!("start at login failed: {e}")))?;
        diag::log(&format!("start at login: {note}"));
        let _ = tauri::Emitter::emit(&app, "notice", json!({ "message": note }));
    }
    if before.save_history && !next.save_history {
        // "Off" means nothing kept on disk, not just nothing new.
        history::clear(&paths::history_path())
            .map_err(|e| format!("cannot delete history: {e}"))?;
        let _ = tauri::Emitter::emit(&app, "history-changed", ());
    }
    if before.diagnostic_log != next.diagnostic_log {
        if next.diagnostic_log {
            diag::set_enabled(true);
            diag::log("troubleshooting log turned on");
        } else {
            // "Off" removes what was kept, like history.
            diag::set_enabled(false);
            diag::clear();
        }
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

/// Deletes the chosen dictations, named by their timestamps, and returns how many went.
#[tauri::command]
pub fn delete_history(app: AppHandle, ids: Vec<u64>) -> Result<usize, String> {
    if ids.is_empty() || ids.len() > MAX_DELETE_IDS {
        return Err("Choose between 1 and 10,000 dictations to delete.".into());
    }
    let ids = ids.into_iter().collect();
    let removed = history::delete(&paths::history_path(), &ids)
        .map_err(|e| format!("cannot delete dictations: {e}"))?;
    let _ = tauri::Emitter::emit(&app, "history-changed", ());
    Ok(removed)
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
pub fn check_for_updates(app: AppHandle) {
    app::check_updates_now(app, true);
}

static UPDATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Clears the in-progress flag however the update ends.
struct UpdateGuard;
impl Drop for UpdateGuard {
    fn drop(&mut self) {
        UPDATING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Installs the version the last check found, then restarts into it.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<String, String> {
    if UPDATING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("An update is already being installed.".into());
    }
    let _guard = UpdateGuard;
    let shared = app::shared_of(&app);
    let version = lock(&shared.update)
        .as_ref()
        .filter(|u| u.available)
        .and_then(|u| u.latest.clone())
        .ok_or("No update is available.")?;
    // Re-checked right before installing, so a stale or hostile reply can never downgrade.
    if !crate::update::is_newer(&version, crate::update::current()) {
        return Err("That version is not newer than this one.".into());
    }
    let target = version.clone();
    diag::log(&format!("installing update {version}"));
    let notes = tauri::async_runtime::spawn_blocking(move || crate::update::install(&target))
        .await
        .map_err(|e| format!("update task failed: {e}"))?
        .inspect_err(|e| diag::log(&format!("update install failed: {e}")))?;
    app::restart_into_installed(&app);
    Ok(format!("Updated to {version}. Restarting. {notes}"))
}

#[tauri::command]
pub fn skip_update(app: AppHandle, state: State<'_, Arc<Shared>>) {
    let latest = lock(&state.update).as_ref().and_then(|u| u.latest.clone());
    state.update_config(|c| c.skip_update = latest);
    app::refresh_update_menu(&app);
}

#[tauri::command]
pub fn open_release_notes(state: State<'_, Arc<Shared>>) -> Result<(), String> {
    let url = lock(&state.update)
        .as_ref()
        .map(|u| u.notes_url.clone())
        .unwrap_or_else(|| crate::update::RELEASES_URL.to_string());
    crate::update::open_url(&url)
}

#[tauri::command]
pub async fn install_app() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let exe = crate::autostart::current_exe()?;
        let result = crate::install::install(&exe);
        diag::log(&match &result {
            Ok(_) => "install: done".to_string(),
            Err(e) => format!("install failed: {e}"),
        });
        let (_installed, notes) = result?;
        Ok(notes.join(" "))
    })
    .await
    .map_err(|e| format!("install failed: {e}"))?
}

/// Saves a diagnostic report in the data folder and shows it in Finder or Explorer.
#[tauri::command]
pub async fn export_diagnostics(state: State<'_, Arc<Shared>>) -> Result<String, String> {
    let cfg = state.config();
    let hardware = state.hardware.clone();
    let path = tauri::async_runtime::spawn_blocking(move || diag::export(&cfg, &hardware))
        .await
        .map_err(|e| format!("report failed: {e}"))??;
    diag::reveal(&path);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(format!("Saved {name}. Read it before you share it."))
}

#[tauri::command]
pub fn list_notes() -> Vec<crate::notes::Listed> {
    crate::notes::list()
}

#[tauri::command]
pub fn get_note(id: String) -> Result<crate::notes::Note, String> {
    if !crate::notes::valid_id(&id) {
        return Err("Unknown note.".into());
    }
    crate::notes::get(&id).map_err(|_| "This note could not be opened.".to_string())
}

#[tauri::command]
pub fn start_note(app: AppHandle) -> Result<String, String> {
    crate::notes::session::start(&app)
}

#[tauri::command]
pub fn stop_note(app: AppHandle) -> Result<(), String> {
    crate::notes::session::stop(&app)
}

#[tauri::command]
pub fn rename_note(
    app: AppHandle,
    id: String,
    title: String,
) -> Result<crate::notes::Note, String> {
    if !crate::notes::valid_id(&id) || title.chars().count() > 1_000 {
        return Err("Unknown note.".into());
    }
    let note = crate::notes::rename(&id, &title).map_err(|e| format!("Could not rename: {e}"))?;
    let _ = tauri::Emitter::emit(&app, "notes-changed", ());
    Ok(note)
}

/// Deletes the chosen notes, never the one recording, and returns how many went.
#[tauri::command]
pub fn delete_notes(app: AppHandle, ids: Vec<String>) -> Result<usize, String> {
    if ids.is_empty() || ids.len() > MAX_DELETE_IDS {
        return Err("Choose between 1 and 10,000 notes to delete.".into());
    }
    let recording = crate::notes::session::active(&app).map(|(id, _)| id);
    let ids = ids.into_iter().collect();
    let removed = crate::notes::delete(&ids, recording.as_deref())
        .map_err(|e| format!("cannot delete notes: {e}"))?;
    diag::log(&format!("notes deleted: {removed}"));
    let _ = tauri::Emitter::emit(&app, "notes-changed", ());
    Ok(removed)
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
