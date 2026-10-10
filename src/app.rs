//! Tauri wiring: tray, windows, hotkey controller, transcription worker.

use crate::config::{self, Config, OverlayPos, Theme};
use crate::engine::{Engine, Opts};
use crate::hotkey::{self, HotkeyEvent};
use crate::hwprobe::{self, Hardware};
use crate::models::{self, ModelId};
use crate::overlay::{self, Screen};
use crate::recorder::{Recorder, Recording};
use crate::state::{Action, Input, Machine, MAX_RECORDING_MS};
use crate::{audio, clipboard, diag, download, history, output, paths, sound};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

pub const OVERLAY: &str = "overlay";
pub const MAIN: &str = "main";
pub const LIVE: &str = "live";
const LIVE_SIZE: (f64, f64) = (380.0, 96.0);
// Re-transcribing only the recent tail keeps live text cheap even in a five-minute dictation.
const LIVE_TAIL_SECS: u32 = 18;
const LIVE_EVERY_MS: u64 = 800;

pub enum Job {
    /// Frees the engine before the process exits; ggml's Metal backend asserts if it is still alive.
    Shutdown(Sender<()>),
    Partial {
        pcm: Vec<f32>,
        opts: Opts,
        session: u64,
    },
    Load(PathBuf, ModelId),
    Transcribe {
        pcm: Vec<f32>,
        opts: Opts,
        duration_ms: u64,
    },
    /// One pause-to-pause piece of a note's channel; runs only when no dictation job waits.
    NoteSegment {
        id: String,
        speaker: &'static str,
        start_ms: u64,
        pcm: Vec<f32>,
        opts: Opts,
    },
    /// Queued after a note's last segment, so it runs once they are all in.
    NoteFinish {
        id: String,
        ended_ms: u64,
    },
}

pub enum Msg {
    Hotkey(HotkeyEvent),
    Partial {
        session: u64,
        text: Option<String>,
    },
    Done {
        result: Result<String, String>,
        duration_ms: u64,
    },
}

/// Which shortcut a Settings capture is filling in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Hold,
    Toggle,
}

pub struct Shared {
    pub config: Mutex<Config>,
    pub hardware: Hardware,
    pub hook: Mutex<Option<hotkey::Hook>>,
    pub worker: Mutex<Sender<Job>>,
    pub ctrl: Mutex<Sender<Msg>>,
    pub downloading: Mutex<Option<ModelId>>,
    pub active_model: Mutex<Option<ModelId>>,
    pub capture_slot: Mutex<Option<Slot>>,
    moving_by_code: AtomicBool,
    last_user_move_ms: AtomicU64,
    snap_pending: AtomicBool,
    overlay_gen: AtomicU64,
    // Set while the final transcription waits; with abort_previews a running preview stops at its next step.
    final_pending: AtomicBool,
    pub update: Mutex<Option<crate::update::UpdateInfo>>,
    pub note: Mutex<Option<crate::notes::session::Running>>,
    /// True while the pill shows Notes, so the note's microphone level drives it.
    pub notes_pill: AtomicBool,
}

impl Shared {
    pub fn config(&self) -> Config {
        lock(&self.config).clone()
    }

    pub fn update_config(&self, f: impl FnOnce(&mut Config)) -> Config {
        let mut guard = lock(&self.config);
        f(&mut guard);
        let _ = config::save(&paths::config_path(), &guard);
        guard.clone()
    }
}

pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A panic elsewhere must not take the whole app down with a poisoned lock.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn run() -> Result<(), String> {
    paths::ensure_dirs().map_err(|e| format!("cannot create the app data folder: {e}"))?;
    // A second launch (Spotlight, login item, terminal) asks the running copy to show itself and quits.
    let (lock, unclean) = match crate::instance::claim(&paths::data_dir()) {
        crate::instance::Claim::Other => {
            println!("local-stt is already running; showing its window.");
            return Ok(());
        }
        crate::instance::Claim::Owner { lock, unclean } => (lock, unclean),
    };
    let cfg = config::load(&paths::config_path());
    output::detach_own_console();
    diag::set_enabled(cfg.diagnostic_log);
    diag::install_panic_hook();
    diag::capture_stderr();
    diag::log(&format!(
        "start: local-stt {} on {} {}{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if crate::demo::active() { " (demo)" } else { "" }
    ));
    if unclean {
        diag::log("the previous run did not quit normally (crash or forced quit)");
    }
    // A note cut short by a crash or quit is saved with what reached disk.
    let (smart, english) = (cfg.smart_format, english_output(&cfg));
    std::thread::spawn(move || {
        let recovered = crate::notes::recover(smart, english);
        if recovered > 0 {
            diag::log(&format!(
                "notes recovered after an unfinished run: {recovered}"
            ));
        }
    });
    let hardware = if crate::demo::active() {
        crate::demo::sample_hardware()
    } else {
        hwprobe::probe(&paths::data_dir())
    };
    let (worker_tx, worker_rx) = mpsc::channel::<Job>();
    let (ctrl_tx, ctrl_rx) = mpsc::channel::<Msg>();
    let shared = Arc::new(Shared {
        config: Mutex::new(cfg),
        hardware,
        hook: Mutex::new(None),
        worker: Mutex::new(worker_tx),
        ctrl: Mutex::new(ctrl_tx),
        downloading: Mutex::new(None),
        active_model: Mutex::new(None),
        capture_slot: Mutex::new(None),
        moving_by_code: AtomicBool::new(false),
        last_user_move_ms: AtomicU64::new(0),
        snap_pending: AtomicBool::new(false),
        overlay_gen: AtomicU64::new(0),
        final_pending: AtomicBool::new(false),
        update: Mutex::new(None),
        note: Mutex::new(None),
        notes_pill: AtomicBool::new(false),
    });

    let app = tauri::Builder::default()
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_state,
            crate::commands::set_config,
            crate::commands::start_hotkey_capture,
            crate::commands::cancel_hotkey_capture,
            crate::commands::choose_model,
            crate::commands::get_history,
            crate::commands::clear_history,
            crate::commands::delete_history,
            crate::commands::export_diagnostics,
            crate::commands::list_notes,
            crate::commands::get_note,
            crate::commands::start_note,
            crate::commands::stop_note,
            crate::commands::rename_note,
            crate::commands::delete_notes,
            crate::commands::reset_overlay_position,
            crate::commands::move_overlay,
            crate::commands::copy_text,
            crate::commands::install_app,
            crate::commands::check_for_updates,
            crate::commands::install_update,
            crate::commands::skip_update,
            crate::commands::open_release_notes,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            build_tray(&handle)?;
            create_overlay(&handle)?;
            create_live(&handle)?;
            spawn_worker(handle.clone(), worker_rx);
            spawn_controller(handle.clone(), ctrl_rx);
            if crate::demo::active() {
                // The tour starts no keyboard hook, download, update check, recording or paste on its own.
                crate::demo::spawn(handle.clone());
            } else {
                start_hotkey(&handle);
                first_run_or_load(&handle);
                spawn_update_checker(handle.clone());
            }
            // LOCAL_STT_EXIT_AFTER_MS lets CI and smoke tests exercise a full start and a clean quit.
            if let Some(ms) = std::env::var("LOCAL_STT_EXIT_AFTER_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
            {
                let quitter = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(ms));
                    quitter.exit(0);
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .map_err(|e| format!("cannot start the app: {e}"))?;

    app.run(move |app, event| match event {
        // Closing the main window must not quit a menu-bar app.
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        RunEvent::Exit => {
            if crate::notes::session::active(app).is_some() {
                diag::log("quit during a note; it is saved at the next start");
            }
            diag::log("quit");
            shutdown_engine(app);
            crate::instance::release(&lock);
            crate::demo::cleanup();
        }
        _ => {}
    });
    Ok(())
}

fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

// An available update is a quiet menu item, never a pop-up over the user's work.
fn tray_menu(
    app: &AppHandle,
    update: Option<&str>,
    note_running: bool,
) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open local-stt", true, None::<&str>)?;
    let note = if note_running {
        MenuItem::with_id(app, "note-stop", "Stop Notes", true, None::<&str>)?
    } else {
        MenuItem::with_id(app, "note-start", "New Note", true, None::<&str>)?
    };
    let check = MenuItem::with_id(app, "check", "Check for Updates...", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit local-stt", true, None::<&str>)?;
    match update {
        Some(version) => {
            let label = format!("Update to {version}...");
            let install = MenuItem::with_id(app, "update", label, true, None::<&str>)?;
            let sep_top = PredefinedMenuItem::separator(app)?;
            Menu::with_items(
                app,
                &[&install, &sep_top, &open, &note, &check, &sep, &quit],
            )
        }
        None => Menu::with_items(app, &[&open, &note, &check, &sep, &quit]),
    }
}

pub fn refresh_update_menu(app: &AppHandle) {
    let s = shared(app);
    let cfg = s.config();
    let offer = lock(&s.update)
        .as_ref()
        .filter(|u| u.available && u.latest != cfg.skip_update)
        .and_then(|u| u.latest.clone());
    let running = lock(&s.note).is_some();
    if let (Some(tray), Ok(menu)) = (
        app.tray_by_id("tray"),
        tray_menu(app, offer.as_deref(), running),
    ) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray_menu(app, None, false)?;
    TrayIconBuilder::with_id("tray")
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true)
        .tooltip(tray_tooltip(&shared(app)))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            "update" => {
                show_main(app);
                let _ = app.emit("show-update", ());
            }
            "check" => check_updates_now(app.clone(), true),
            "note-start" => {
                show_main(app);
                let _ = app.emit("show-tab", json!({ "tab": "notes", "newNote": true }));
            }
            "note-stop" => {
                let _ = crate::notes::session::stop(app);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn tray_tooltip(shared: &Shared) -> String {
    let cfg = shared.config();
    match cfg.toggle_hotkey.as_ref() {
        Some(t) => format!(
            "local-stt: hold {} to dictate, {} for hands-free",
            hotkey::display(&cfg.hotkey),
            hotkey::display(t)
        ),
        None => format!(
            "local-stt: hold {} to dictate",
            hotkey::display(&cfg.hotkey)
        ),
    }
}

pub fn refresh_tray(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("tray") {
        let _ = tray.set_tooltip(Some(tray_tooltip(&shared(app))));
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("main.html".into()))
        .title("local-stt")
        .inner_size(760.0, 580.0)
        .min_inner_size(640.0, 480.0)
        .center()
        .build();
    if let Ok(w) = built {
        let _ = w.set_focus();
    }
}

// ---------- overlay window ----------

fn pill_size(theme: Theme) -> (f64, f64) {
    match theme {
        Theme::Pill => (220.0, 44.0),
        Theme::Waveform => (260.0, 44.0),
        Theme::Minimal => (44.0, 44.0),
    }
}

fn create_overlay(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let theme = shared(app).config().theme;
    let (w, h) = pill_size(theme);
    let builder = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("overlay.html".into()))
        .title("local-stt recording")
        .inner_size(w, h)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        // Windows draws a square frame for a shadowed borderless window; the page draws its own shadow there.
        .shadow(cfg!(target_os = "macos"))
        .visible_on_all_workspaces(true)
        // Created hidden and never focusable, so showing it cannot steal focus from the user's app.
        .visible(false)
        .focused(false)
        .focusable(false);
    // The demo draws the pill solid, so a screenshot never shows what sits behind the glass.
    #[cfg(target_os = "macos")]
    let builder = if crate::demo::active() {
        builder
    } else {
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        builder.effects(
            EffectsBuilder::new()
                .effects([Effect::LiquidGlassRegular, Effect::HudWindow])
                .state(EffectState::Active)
                .radius(22.0)
                .build(),
        )
    };
    let window = builder.build()?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Moved(_) = event {
            on_overlay_moved(&handle);
        }
    });
    Ok(window)
}

fn create_live(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, LIVE, WebviewUrl::App("live.html".into()))
        .title("local-stt live text")
        .inner_size(LIVE_SIZE.0, LIVE_SIZE.1)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(cfg!(target_os = "macos"))
        .visible_on_all_workspaces(true)
        .visible(false)
        .focused(false)
        .focusable(false);
    // The demo draws the pill solid, so a screenshot never shows what sits behind the glass.
    #[cfg(target_os = "macos")]
    let builder = if crate::demo::active() {
        builder
    } else {
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        builder.effects(
            EffectsBuilder::new()
                .effects([Effect::LiquidGlassRegular, Effect::HudWindow])
                .state(EffectState::Active)
                .radius(22.0)
                .build(),
        )
    };
    builder.build()
}

// Puts the live popover next to the pill in the same desktop units the pill uses.
fn place_live(
    app: &AppHandle,
    screen: &Screen,
    pill: (f64, f64),
    pill_size: (f64, f64),
    scale: f64,
) {
    let Some(win) = app.get_webview_window(LIVE) else {
        return;
    };
    let live_size = if cfg!(target_os = "macos") {
        LIVE_SIZE
    } else {
        (LIVE_SIZE.0 * scale, LIVE_SIZE.1 * scale)
    };
    let (pos, below) = overlay::live_pos(screen, pill, pill_size, live_size);
    let _ = win.set_size(tauri::LogicalSize::new(LIVE_SIZE.0, LIVE_SIZE.1));
    if cfg!(target_os = "macos") {
        let _ = win.set_position(tauri::LogicalPosition::new(pos.0, pos.1));
    } else {
        let _ = win.set_position(tauri::PhysicalPosition::new(
            pos.0.round() as i32,
            pos.1.round() as i32,
        ));
    }
    let _ = app.emit("live-place", json!({ "below": below }));
}

fn hide_live(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LIVE) {
        let _ = win.hide();
    }
}

fn units_scale(scale: f64) -> f64 {
    // macOS desktop coordinates are points; Windows uses physical pixels.
    if cfg!(target_os = "macos") {
        scale
    } else {
        1.0
    }
}

fn screens(app: &AppHandle) -> Vec<(Screen, f64)> {
    let monitors = app.available_monitors().unwrap_or_default();
    let mut out = Vec::new();
    for (i, m) in monitors.iter().enumerate() {
        let wa = m.work_area();
        let s = units_scale(m.scale_factor());
        let name = m
            .name()
            .cloned()
            .unwrap_or_else(|| format!("display-{}", i + 1));
        let screen = Screen {
            name,
            x: wa.position.x as f64 / s,
            y: wa.position.y as f64 / s,
            w: wa.size.width as f64 / s,
            h: wa.size.height as f64 / s,
        };
        out.push((screen, m.scale_factor()));
    }
    out
}

fn pointer(app: &AppHandle) -> (f64, f64) {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| units_scale(m.scale_factor()))
        .unwrap_or(1.0);
    app.cursor_position()
        .map(|p| (p.x / scale, p.y / scale))
        .unwrap_or((0.0, 0.0))
}

fn size_in_units(theme: Theme, monitor_scale: f64) -> (f64, f64) {
    let (w, h) = pill_size(theme);
    if cfg!(target_os = "macos") {
        (w, h)
    } else {
        (w * monitor_scale, h * monitor_scale)
    }
}

fn set_overlay_pos(app: &AppHandle, win: &WebviewWindow, pos: (f64, f64)) {
    let s = shared(app);
    s.moving_by_code.store(true, Ordering::SeqCst);
    if cfg!(target_os = "macos") {
        let _ = win.set_position(tauri::LogicalPosition::new(pos.0, pos.1));
    } else {
        let _ = win.set_position(tauri::PhysicalPosition::new(
            pos.0.round() as i32,
            pos.1.round() as i32,
        ));
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        shared(&app2).moving_by_code.store(false, Ordering::SeqCst);
    });
}

fn place_overlay(app: &AppHandle, win: &WebviewWindow) {
    let cfg = shared(app).config();
    let list = screens(app);
    let plain: Vec<Screen> = list.iter().map(|(s, _)| s.clone()).collect();
    let ptr = pointer(app);
    let target_scale = match cfg
        .overlay_pos
        .as_ref()
        .and_then(|p| list.iter().find(|(s, _)| s.name == p.monitor))
    {
        Some((_, scale)) => *scale,
        None => overlay::screen_for_point(&plain, ptr)
            .and_then(|s| list.iter().find(|(x, _)| x.name == s.name))
            .map(|(_, scale)| *scale)
            .unwrap_or(1.0),
    };
    let size = size_in_units(cfg.theme, target_scale);
    let (lw, lh) = pill_size(cfg.theme);
    let _ = win.set_size(tauri::LogicalSize::new(lw, lh));
    let pos = overlay::resolve(&plain, cfg.overlay_pos.as_ref(), ptr, size);
    set_overlay_pos(app, win, pos);
    let centre = (pos.0 + size.0 / 2.0, pos.1 + size.1 / 2.0);
    if let Some(screen) = overlay::screen_for_point(&plain, centre).cloned() {
        place_live(app, &screen, pos, size, target_scale);
    }
}

pub fn show_overlay(app: &AppHandle, state: &str, label: &str, started_at_ms: Option<u64>) {
    let s = shared(app);
    let cfg = s.config();
    if !cfg.show_overlay && state != "positioning" {
        return;
    }
    let Some(win) = app.get_webview_window(OVERLAY) else {
        return;
    };
    // Any pending hide from an earlier state is now stale.
    s.overlay_gen.fetch_add(1, Ordering::SeqCst);
    s.notes_pill.store(state == "notes", Ordering::SeqCst);
    if !win.is_visible().unwrap_or(false) {
        place_overlay(app, &win);
    }
    let _ = app.emit("overlay-theme", theme_payload(&cfg));
    let _ = app.emit(
        "overlay-state",
        json!({ "state": state, "label": label, "startedAtMs": started_at_ms }),
    );
    let _ = win.show();
    let live_wanted = cfg.live_transcription && matches!(state, "recording" | "transcribing");
    if live_wanted {
        if let Some(live) = app.get_webview_window(LIVE) {
            let _ = live.show();
        }
    } else {
        hide_live(app);
    }
}

/// The pill's Notes state: red dot and the note's timer, shown whenever no dictation needs the pill.
pub fn show_notes_pill(app: &AppHandle) {
    if let Some((_, started)) = crate::notes::session::active(app) {
        show_overlay(app, "notes", "Notes", Some(started));
    }
}

pub fn hide_overlay_after(app: &AppHandle, delay_ms: u64) {
    let generation = shared(app).overlay_gen.load(Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(delay_ms));
        if shared(&app).overlay_gen.load(Ordering::SeqCst) != generation {
            return;
        }
        // During a note the pill goes back to Notes instead of away.
        if crate::notes::session::active(&app).is_some() {
            show_notes_pill(&app);
            return;
        }
        if let Some(win) = app.get_webview_window(OVERLAY) {
            let _ = win.hide();
        }
        hide_live(&app);
    });
}

fn theme_payload(cfg: &Config) -> serde_json::Value {
    // Windows gets no native glass on a borderless capsule, so the page draws an opaque one.
    json!({ "theme": cfg.theme, "reducedMotion": false, "solid": cfg!(windows) || crate::demo::active() })
}

fn on_overlay_moved(app: &AppHandle) {
    let s = shared(app);
    if s.moving_by_code.load(Ordering::SeqCst) {
        return;
    }
    s.last_user_move_ms.store(now_ms(), Ordering::SeqCst);
    if s.snap_pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let s = shared(&app);
        // Wait for 250 ms without movement, so a drag in progress is never fought.
        loop {
            std::thread::sleep(Duration::from_millis(60));
            if now_ms().saturating_sub(s.last_user_move_ms.load(Ordering::SeqCst)) >= 250 {
                break;
            }
        }
        s.snap_pending.store(false, Ordering::SeqCst);
        snap_and_save(&app);
    });
}

fn snap_and_save(app: &AppHandle) {
    let Some(win) = app.get_webview_window(OVERLAY) else {
        return;
    };
    let Ok(phys) = win.outer_position() else {
        return;
    };
    let win_scale = win.scale_factor().unwrap_or(1.0);
    let u = units_scale(win_scale);
    let from = (phys.x as f64 / u, phys.y as f64 / u);
    let list = screens(app);
    let plain: Vec<Screen> = list.iter().map(|(s, _)| s.clone()).collect();
    let theme = shared(app).config().theme;
    let size = size_in_units(theme, win_scale);
    let center = (from.0 + size.0 / 2.0, from.1 + size.1 / 2.0);
    let Some(screen) = overlay::screen_for_point(&plain, center).cloned() else {
        return;
    };
    let to = overlay::snap(&screen, from, size);
    animate_to(app, &win, from, to);
    place_live(app, &screen, to, size, win_scale);
    shared(app).update_config(|c| {
        c.overlay_pos = Some(OverlayPos {
            monitor: screen.name.clone(),
            x: to.0,
            y: to.1,
        })
    });
}

fn animate_to(app: &AppHandle, win: &WebviewWindow, from: (f64, f64), to: (f64, f64)) {
    if (from.0 - to.0).abs() < 0.5 && (from.1 - to.1).abs() < 0.5 {
        return;
    }
    let frames = (overlay::SPRING_RESPONSE_S * 60.0 * 1.25) as u32;
    for i in 1..=frames {
        let t = i as f64 / 60.0;
        let p = (
            overlay::spring(from.0, to.0, t),
            overlay::spring(from.1, to.1, t),
        );
        set_overlay_pos(app, win, p);
        std::thread::sleep(Duration::from_millis(16));
    }
    set_overlay_pos(app, win, to);
}

// ---------- hotkey ----------

fn start_hotkey(app: &AppHandle) {
    let s = shared(app);
    // LOCAL_STT_NO_PERMISSION_PROMPTS lets developers and CI start the app without a system dialog.
    let prompt = std::env::var_os("LOCAL_STT_NO_PERMISSION_PROMPTS").is_none();
    let allowed = hotkey::accessibility_ok(prompt);
    diag::log(&format!(
        "accessibility: {}",
        if allowed { "allowed" } else { "not allowed" }
    ));
    if !allowed {
        set_tray_problem(
            app,
            "allow Accessibility in System Settings, then restart local-stt",
        );
    }
    let (tx, rx) = mpsc::channel::<HotkeyEvent>();
    let cfg = s.config();
    match hotkey::start(cfg.hotkey, tx) {
        Ok(hook) => {
            diag::log("keyboard hook started");
            hook.set_toggle(cfg.toggle_hotkey);
            *lock(&s.hook) = Some(hook);
        }
        Err(e) => {
            diag::log(&format!("keyboard hook failed: {e}"));
            set_tray_problem(app, &format!("shortcut unavailable: {e}"));
        }
    }
    let ctrl = lock(&s.ctrl).clone();
    std::thread::spawn(move || {
        for ev in rx {
            if ctrl.send(Msg::Hotkey(ev)).is_err() {
                break;
            }
        }
    });
}

fn set_tray_problem(app: &AppHandle, problem: &str) {
    if let Some(tray) = app.tray_by_id("tray") {
        let _ = tray.set_tooltip(Some(format!("local-stt: {problem}")));
    }
}

fn on_captured(app: &AppHandle, combo: hotkey::Combo) {
    let s = shared(app);
    if let Some(hook) = lock(&s.hook).as_ref() {
        hook.set_capture(false);
    }
    let slot = lock(&s.capture_slot).take().unwrap_or(Slot::Hold);
    let cfg = s.config();
    let other = match slot {
        Slot::Hold => cfg.toggle_hotkey.clone(),
        Slot::Toggle => Some(cfg.hotkey.clone()),
    };
    let checked = hotkey::validate(&combo).and_then(|()| match other {
        Some(o) if hotkey::same(&o, &combo) => {
            Err("That shortcut is already used for the other mode.".to_string())
        }
        _ => Ok(()),
    });
    match checked {
        Ok(()) => {
            let cfg = s.update_config(|c| match slot {
                Slot::Hold => c.hotkey = combo.clone(),
                Slot::Toggle => c.toggle_hotkey = Some(combo.clone()),
            });
            if let Some(hook) = lock(&s.hook).as_ref() {
                hook.set_combo(cfg.hotkey.clone());
                hook.set_toggle(cfg.toggle_hotkey.clone());
            }
            refresh_tray(app);
            emit_captured(app, slot, &cfg, None);
        }
        Err(e) => emit_captured(app, slot, &cfg, Some(e)),
    }
}

fn emit_captured(app: &AppHandle, slot: Slot, cfg: &Config, error: Option<String>) {
    let combo = match slot {
        Slot::Hold => Some(cfg.hotkey.clone()),
        Slot::Toggle => cfg.toggle_hotkey.clone(),
    };
    let display = combo.as_ref().map(hotkey::display);
    let slot_name = match slot {
        Slot::Hold => "hold",
        Slot::Toggle => "toggle",
    };
    let _ = app.emit(
        "hotkey-captured",
        json!({ "slot": slot_name, "combo": combo, "display": display, "error": error }),
    );
}

// ---------- controller ----------

fn spawn_controller(app: AppHandle, rx: Receiver<Msg>) {
    std::thread::spawn(move || {
        let mut machine = Machine::new();
        let mut recorder: Option<Recorder> = None;
        let mut partial_in_flight = false;
        let mut last_partial_ms = 0u64;
        loop {
            let msg = rx.recv_timeout(Duration::from_millis(200));
            let now = now_ms();
            if crate::instance::take_show_request(&paths::data_dir()) {
                show_main(&app);
            }
            if let (Some(r), Some(since)) = (recorder.as_ref(), machine.recording_since()) {
                let cfg = shared(&app).config();
                let due = now.saturating_sub(last_partial_ms) >= LIVE_EVERY_MS
                    && now.saturating_sub(since) >= 600;
                if cfg.live_transcription && cfg.show_overlay && !partial_in_flight && due {
                    last_partial_ms = now;
                    let pcm = r.snapshot_tail(LIVE_TAIL_SECS);
                    // Whisper invents words on silence, so quiet stretches never reach the popover.
                    if !audio::is_silent(&pcm, audio::SILENCE_RMS) {
                        let job = Job::Partial {
                            pcm,
                            opts: transcribe_opts(&cfg),
                            session: since,
                        };
                        partial_in_flight = lock(&shared(&app).worker).send(job).is_ok();
                    }
                }
            }
            if let Some(since) = machine.recording_since() {
                if now.saturating_sub(since) >= MAX_RECORDING_MS {
                    let action = machine.on(Input::Timeout, now);
                    apply(&app, action, &mut recorder, &mut machine, now);
                }
            }
            match msg {
                Ok(Msg::Hotkey(HotkeyEvent::Captured(combo))) => on_captured(&app, combo),
                Ok(Msg::Hotkey(HotkeyEvent::Cancel))
                    if lock(&shared(&app).capture_slot).is_some() =>
                {
                    // Esc during shortcut capture ends the capture, not a recording.
                    let slot = lock(&shared(&app).capture_slot)
                        .take()
                        .unwrap_or(Slot::Hold);
                    let cfg = shared(&app).config();
                    emit_captured(&app, slot, &cfg, Some("Cancelled".to_string()));
                }
                Ok(Msg::Hotkey(ev)) => {
                    let input = match ev {
                        HotkeyEvent::Pressed => Input::Pressed,
                        HotkeyEvent::Released => Input::Released,
                        HotkeyEvent::Toggle => Input::Toggle,
                        HotkeyEvent::Cancel => Input::Cancel,
                        HotkeyEvent::Captured(_) => continue,
                    };
                    let action = machine.on(input, now);
                    apply(&app, action, &mut recorder, &mut machine, now);
                }
                Ok(Msg::Partial { session, text }) => {
                    partial_in_flight = false;
                    let current = machine.recording_since() == Some(session);
                    if let Some(text) = text.filter(|t| current && !t.trim().is_empty()) {
                        let cfg = shared(&app).config();
                        let formatted = cfg.smart_format && english_output(&cfg);
                        let _ =
                            app.emit("live-text", json!({ "text": text, "formatted": formatted }));
                    }
                }
                Ok(Msg::Done {
                    result,
                    duration_ms,
                }) => {
                    machine.on(Input::TranscribeDone, now);
                    deliver(&app, result, duration_ms);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn apply(
    app: &AppHandle,
    action: Action,
    recorder: &mut Option<Recorder>,
    machine: &mut Machine,
    now: u64,
) {
    let cfg = shared(app).config();
    match action {
        Action::StartRecording => {
            if cfg.sounds {
                sound::play(sound::Cue::Start);
            }
            let level_app = app.clone();
            let last = Mutex::new(Instant::now() - Duration::from_secs(1));
            let on_level = Box::new(move |level: f32| {
                let mut t = lock(&last);
                if t.elapsed() >= Duration::from_millis(33) {
                    *t = Instant::now();
                    let _ = level_app.emit("overlay-level", json!({ "level": level }));
                }
            });
            crate::engine::abort_previews(false);
            let _ = app.emit("live-reset", json!({}));
            match Recorder::start(cfg.microphone.as_deref(), on_level) {
                Ok(r) => {
                    *recorder = Some(r);
                    let label = if machine.is_hands_free() {
                        "Hands-free"
                    } else {
                        "Recording"
                    };
                    diag::log(&format!("recording started ({label})"));
                    show_overlay(app, "recording", label, Some(now));
                }
                Err(e) => {
                    // The message can name the chosen microphone, which stays out of the log.
                    let shown = match cfg.microphone.as_deref() {
                        Some(name) if !name.is_empty() => e.replace(name, "<microphone>"),
                        _ => e.clone(),
                    };
                    diag::log(&format!("recording could not start: {shown}"));
                    machine.on(Input::Cancel, now);
                    // The pill fits about two short lines; long device names fall back to a generic label.
                    let label = if e.chars().count() <= 60 {
                        e
                    } else {
                        "Microphone unavailable".to_string()
                    };
                    fail(app, &label, cfg.sounds);
                }
            }
        }
        Action::StopAndTranscribe => {
            let Some(r) = recorder.take() else {
                machine.on(Input::TranscribeDone, now);
                return;
            };
            if cfg.sounds {
                sound::play(sound::Cue::Stop);
            }
            let rec: Recording = r.stop();
            let rms = audio::rms(&rec.samples_16k);
            diag::log(&format!(
                "recording stopped: {:.1} s, level {}",
                rec.duration_ms as f64 / 1000.0,
                if rms > 0.0 {
                    format!("{:.0} dBFS", 20.0 * rms.log10())
                } else {
                    "silent".into()
                }
            ));
            if rec.samples_16k.is_empty() || audio::is_silent(&rec.samples_16k, audio::SILENCE_RMS)
            {
                machine.on(Input::TranscribeDone, now);
                fail(app, "No speech heard", cfg.sounds);
                return;
            }
            show_overlay(app, "transcribing", "Transcribing", None);
            let job = Job::Transcribe {
                pcm: rec.samples_16k,
                opts: transcribe_opts(&cfg),
                duration_ms: rec.duration_ms,
            };
            shared(app).final_pending.store(true, Ordering::SeqCst);
            // Stop a preview already running, so the final pass starts at once.
            crate::engine::abort_previews(true);
            if lock(&shared(app).worker).send(job).is_err() {
                shared(app).final_pending.store(false, Ordering::SeqCst);
                machine.on(Input::TranscribeDone, now);
                fail(app, "Speech engine stopped", cfg.sounds);
            }
        }
        Action::CancelRecording => {
            if let Some(r) = recorder.take() {
                r.cancel();
                diag::log("recording cancelled");
            }
            hide_overlay_after(app, 0);
        }
        Action::HandsFree => {
            show_overlay(app, "recording", "Hands-free", machine.recording_since());
        }
        Action::Idle | Action::None => {}
    }
}

/// Translation always yields English; otherwise only an explicit English setting guarantees it.
pub(crate) fn english_output(cfg: &Config) -> bool {
    cfg.translate || cfg.language == "en"
}

pub(crate) fn transcribe_opts(cfg: &Config) -> Opts {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
        .min(8);
    Opts {
        translate: cfg.translate,
        language: cfg.language.clone(),
        threads,
    }
}

fn fail(app: &AppHandle, message: &str, sounds: bool) {
    if sounds {
        sound::play(sound::Cue::Error);
    }
    show_overlay(app, "error", message, None);
    hide_overlay_after(app, 3_000);
}

fn deliver(app: &AppHandle, result: Result<String, String>, duration_ms: u64) {
    let s = shared(app);
    let cfg = s.config();
    let model = lock(&s.active_model)
        .map(model_name)
        .unwrap_or("-")
        .to_string();
    let result = result.map(|t| {
        if cfg.smart_format {
            crate::format::tidy_for(&t, english_output(&cfg))
        } else {
            t
        }
    });
    let text = match result {
        // A transcript of only fillers or punctuation is nothing to paste.
        Ok(t) if t.chars().any(char::is_alphanumeric) => t,
        Ok(_) => {
            diag::log("transcript held only fillers or punctuation");
            return fail(app, "No speech heard", cfg.sounds);
        }
        Err(e) => {
            diag::log(&format!("transcription failed: {e}"));
            record(&cfg, &model, "", duration_ms, false);
            return fail(app, &e, cfg.sounds);
        }
    };
    let previous = if cfg.restore_clipboard {
        clipboard::read_text()
    } else {
        None
    };
    if let Err(e) = clipboard::write_private(&text) {
        diag::log(&format!("clipboard write failed: {e}"));
        return fail(app, &format!("Clipboard unavailable: {e}"), cfg.sounds);
    }
    let mut pasted = !cfg.paste;
    if cfg.paste {
        // Give the clipboard a beat to settle before the target app reads it.
        std::thread::sleep(Duration::from_millis(40));
        let sent = output::send_paste();
        let allowed = hotkey::accessibility_ok(false);
        pasted = sent.is_ok() && allowed;
        diag::log(&match (&sent, allowed) {
            (Ok(()), true) => "pasted".to_string(),
            (Ok(()), false) => "copied; paste blocked (Accessibility not allowed)".to_string(),
            (Err(e), _) => format!("copied; paste failed: {e}"),
        });
    } else {
        diag::log("copied (paste is off)");
    }
    if let Some(prev) = previous {
        let ours = text.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            // Never overwrite something the user copied in the meantime.
            if clipboard::read_text().as_deref() == Some(ours.as_str()) {
                let _ = clipboard::write_private(&prev);
            }
        });
    }
    record(&cfg, &model, &text, duration_ms, true);
    let _ = app.emit("history-changed", ());
    if pasted {
        let label = if cfg.paste { "Pasted" } else { "Copied" };
        show_overlay(app, "done", label, None);
        hide_overlay_after(app, 700);
    } else {
        show_overlay(app, "warning", "Copied. Allow Accessibility to paste", None);
        hide_overlay_after(app, 3_000);
    }
}

fn record(cfg: &Config, model: &str, text: &str, duration_ms: u64, ok: bool) {
    if !cfg.save_history {
        return;
    }
    let entry = history::Entry {
        id: 0,
        ts_ms: now_ms(),
        text: text.to_string(),
        duration_ms,
        model: model.to_string(),
        ok,
    };
    let _ = history::append(&paths::history_path(), &entry);
}

pub fn model_name(id: ModelId) -> &'static str {
    models::info(id).label
}

// ---------- worker and models ----------

// Checks quietly a little after launch, then daily; the user is told by a menu item and a banner, not a dialog.
fn spawn_update_checker(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            if shared(&app).config().check_updates {
                check_updates_now(app.clone(), false);
            }
            std::thread::sleep(Duration::from_secs(24 * 60 * 60));
        }
    });
}

/// Runs a check off the UI thread; `asked` means the user clicked, so "up to date" is worth saying.
pub fn check_updates_now(app: AppHandle, asked: bool) {
    std::thread::spawn(move || {
        let result = crate::update::check();
        diag::log(&match &result {
            Ok(info) => format!(
                "update check: latest {}, running {}",
                info.latest.as_deref().unwrap_or("-"),
                info.current
            ),
            Err(e) => format!("update check failed: {e}"),
        });
        let s = shared(&app);
        match result {
            Ok(info) => {
                let skipped = !asked && info.latest == s.config().skip_update;
                *lock(&s.update) = Some(info.clone());
                refresh_update_menu(&app);
                if info.available && !skipped {
                    let _ = app.emit("update-available", &info);
                } else if asked {
                    show_main(&app);
                    let message = format!("local-stt {} is up to date.", info.current);
                    let _ = app.emit("notice", json!({ "message": message, "kind": "update" }));
                }
            }
            Err(e) if asked => {
                show_main(&app);
                let _ = app.emit("notice", json!({ "message": e, "kind": "update" }));
            }
            Err(_) => {}
        }
    });
}

// Waits briefly for the worker to drop the whisper context, so exit-time destructors find nothing to free.
fn shutdown_engine(app: &AppHandle) {
    crate::engine::abort_previews(true);
    let (tx, rx) = mpsc::channel();
    if lock(&shared(app).worker).send(Job::Shutdown(tx)).is_ok() {
        // Long enough for a large model's checksum pass to finish before the engine can be freed.
        if rx.recv_timeout(Duration::from_secs(10)).is_err() {
            diag::log("speech engine did not stop within 10 s");
        }
    }
}

fn spawn_worker(app: AppHandle, rx: Receiver<Job>) {
    std::thread::spawn(move || {
        let mut engine: Option<Engine> = None;
        let mut verified: Vec<ModelId> = Vec::new();
        // Note jobs wait here and run only when nothing else is queued, so a paste never waits behind a meeting.
        // ponytail: unbounded; a CPU far slower than real time grows it about 2 MB per queued segment, cap it if that shows up in reports.
        let mut note_jobs: std::collections::VecDeque<Job> = std::collections::VecDeque::new();
        loop {
            let job = match rx.try_recv() {
                Ok(job @ (Job::NoteSegment { .. } | Job::NoteFinish { .. })) => {
                    note_jobs.push_back(job);
                    continue;
                }
                Ok(job) => job,
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => match note_jobs.pop_front() {
                    Some(job) => job,
                    None => match rx.recv() {
                        Ok(job @ (Job::NoteSegment { .. } | Job::NoteFinish { .. })) => {
                            note_jobs.push_back(job);
                            continue;
                        }
                        Ok(job) => job,
                        Err(_) => break,
                    },
                },
            };
            match job {
                Job::Shutdown(done) => {
                    drop(engine.take());
                    let _ = done.send(());
                    break;
                }
                Job::Load(path, id) => {
                    let started = Instant::now();
                    let loaded =
                        verify_model(&path, id, &mut verified).and_then(|()| Engine::load(&path));
                    match loaded {
                        Ok(e) => {
                            diag::log(&format!(
                                "model {} loaded in {} ms",
                                model_name(id),
                                started.elapsed().as_millis()
                            ));
                            engine = Some(e);
                            *lock(&shared(&app).active_model) = Some(id);
                            let _ = app.emit("model-done", json!({ "id": id, "ok": true }));
                        }
                        Err(e) => {
                            diag::log(&format!("model {} failed to load: {e}", model_name(id)));
                            let _ = app
                                .emit("model-done", json!({ "id": id, "ok": false, "error": e }));
                        }
                    }
                }
                Job::Partial { pcm, opts, session } => {
                    let skip = shared(&app).final_pending.load(Ordering::SeqCst);
                    let text = match (skip, engine.as_ref()) {
                        (false, Some(e)) => e.transcribe_preview(&pcm, &opts).ok(),
                        _ => None,
                    };
                    let ctrl = lock(&shared(&app).ctrl).clone();
                    let _ = ctrl.send(Msg::Partial { session, text });
                }
                Job::Transcribe {
                    pcm,
                    opts,
                    duration_ms,
                } => {
                    let started = Instant::now();
                    let result = match engine.as_ref() {
                        Some(e) => e.transcribe(&pcm, &opts),
                        None => {
                            Err("No model loaded yet. Open local-stt to download one.".to_string())
                        }
                    };
                    // Only sizes and timings; the words themselves never reach the log.
                    if let Ok(text) = &result {
                        diag::log(&format!(
                            "transcribed {:.1} s of audio in {} ms ({} characters)",
                            duration_ms as f64 / 1000.0,
                            started.elapsed().as_millis(),
                            text.chars().count()
                        ));
                    }
                    shared(&app).final_pending.store(false, Ordering::SeqCst);
                    let ctrl = lock(&shared(&app).ctrl).clone();
                    let _ = ctrl.send(Msg::Done {
                        result,
                        duration_ms,
                    });
                }
                Job::NoteSegment {
                    id,
                    speaker,
                    start_ms,
                    pcm,
                    opts,
                } => {
                    if let Some(e) = engine.as_ref() {
                        let samples = pcm.len();
                        // whisper.cpp returns nothing for under a second of audio; quiet padding fixes that.
                        let mut padded = pcm;
                        if padded.len() < 17_600 {
                            padded.resize(17_600, 0.0);
                        }
                        if let Ok(text) = e.transcribe(&padded, &opts) {
                            crate::notes::session::add_segment(
                                &app, &id, speaker, start_ms, samples, &text,
                            );
                        }
                    }
                }
                Job::NoteFinish { id, ended_ms } => {
                    crate::notes::session::finish(&app, &id, ended_ms);
                }
            }
        }
    });
}

// Re-checks the pinned checksum once per run, so a damaged or swapped file is never parsed.
fn verify_model(
    path: &std::path::Path,
    id: ModelId,
    verified: &mut Vec<ModelId>,
) -> Result<(), String> {
    if verified.contains(&id) {
        return Ok(());
    }
    let actual =
        download::sha256_file(path).map_err(|e| format!("cannot read the model file: {e}"))?;
    if !actual.eq_ignore_ascii_case(models::info(id).sha256) {
        return Err("Model file damaged. Re-download it in the Model tab.".into());
    }
    verified.push(id);
    Ok(())
}

fn first_run_or_load(app: &AppHandle) {
    let id = shared(app).config().model;
    let path = models::path(&paths::models_dir(), id);
    if path.exists() {
        let _ = lock(&shared(app).worker).send(Job::Load(path, id));
        return;
    }
    show_main(app);
    let _ = start_download(app.clone(), id);
}

pub fn activate_model(app: &AppHandle, id: ModelId) {
    let s = shared(app);
    s.update_config(|c| c.model = id);
    let path = models::path(&paths::models_dir(), id);
    let _ = lock(&s.worker).send(Job::Load(path, id));
}

pub fn start_download(app: AppHandle, id: ModelId) -> Result<(), String> {
    let s = shared(&app);
    {
        let mut busy = lock(&s.downloading);
        if busy.is_some() {
            return Err("Another model is downloading. Try again when it finishes.".into());
        }
        *busy = Some(id);
    }
    std::thread::spawn(move || {
        let info = models::info(id);
        let dest = models::path(&paths::models_dir(), id);
        let total = info.size_bytes;
        let last = std::cell::Cell::new(Instant::now() - Duration::from_secs(1));
        let emitter = app.clone();
        let progress = move |done: u64| {
            if last.get().elapsed() >= Duration::from_millis(200) {
                last.set(Instant::now());
                let _ = emitter.emit(
                    "model-progress",
                    json!({ "id": id, "downloaded": done, "total": total }),
                );
            }
        };
        diag::log(&format!("model {} download started", info.label));
        let result = download::download(&models::url(id), info.sha256, &dest, &progress);
        *lock(&shared(&app).downloading) = None;
        diag::log(&match &result {
            Ok(()) => format!("model {} downloaded and verified", info.label),
            Err(e) => format!("model {} download failed: {e}", info.label),
        });
        match result {
            Ok(()) => {
                let _ = app.emit(
                    "model-progress",
                    json!({ "id": id, "downloaded": total, "total": total }),
                );
                activate_model(&app, id);
            }
            Err(e) => {
                let _ = app.emit("model-done", json!({ "id": id, "ok": false, "error": e }));
            }
        }
    });
    Ok(())
}

/// Starts the freshly installed copy and quits this one (the exit handler frees the engine first).
pub fn restart_into_installed(app: &AppHandle) {
    if let Some(exe) = crate::install::installed_exe().filter(|p| p.is_file()) {
        let replaced = std::process::id().to_string();
        // The new copy waits for this one to quit instead of handing its window back to it.
        if crate::proc::command(exe)
            .env(crate::instance::REPLACES, replaced)
            .spawn()
            .is_ok()
        {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(400));
                app.exit(0);
            });
        }
    }
}

pub fn ctrl_send(app: &AppHandle, msg: Msg) {
    let _ = lock(&shared(app).ctrl).send(msg);
}

pub fn shared_of(app: &AppHandle) -> Arc<Shared> {
    shared(app)
}

pub fn overlay_theme_changed(app: &AppHandle) {
    let cfg = shared(app).config();
    let (w, h) = pill_size(cfg.theme);
    if let Some(win) = app.get_webview_window(OVERLAY) {
        let _ = win.set_size(tauri::LogicalSize::new(w, h));
    }
    let _ = app.emit("overlay-theme", theme_payload(&cfg));
}

pub fn overlay_positioning(app: &AppHandle, on: bool) {
    if on {
        show_overlay(app, "positioning", "Drag me anywhere", None);
    } else {
        hide_overlay_after(app, 0);
    }
}
