//! The "call detected" toast: a small window at the bottom of the screen in use, offering to take a note.

use crate::app;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const TOAST: &str = "toast";
const SIZE: (f64, f64) = (360.0, 60.0);
const GAP: f64 = 28.0;
/// How long an ignored toast stays; the page pauses this while the pointer is on it.
const SHOW_MS: u64 = 20_000;

/// Made hidden at start, so its page is already listening when the first call is offered.
pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, TOAST, WebviewUrl::App("toast.html".into()))
        .title("local-stt call detected")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(cfg!(target_os = "macos"))
        .visible_on_all_workspaces(true)
        // Never focusable, so it cannot take typing away from the call or the user's app.
        .visible(false)
        .focused(false)
        .focusable(false);
    #[cfg(target_os = "macos")]
    let builder = {
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        builder.effects(
            EffectsBuilder::new()
                .effects([Effect::LiquidGlassRegular, Effect::HudWindow])
                .state(EffectState::Active)
                .radius(18.0)
                .build(),
        )
    };
    builder.build()
}

// Bottom centre of the screen the user is working on, in the units each OS positions windows in.
fn place(app: &AppHandle, win: &WebviewWindow) {
    let list = app::screens(app);
    let plain: Vec<crate::overlay::Screen> = list.iter().map(|(s, _)| s.clone()).collect();
    let focus = app::focus_point(app, &plain);
    let Some(screen) = crate::overlay::screen_for_point(&plain, focus).or(plain.first()) else {
        return;
    };
    let scale = list
        .iter()
        .find(|(s, _)| s.name == screen.name)
        .map_or(1.0, |(_, scale)| *scale);
    let units = if cfg!(target_os = "macos") {
        1.0
    } else {
        scale
    };
    let size = (SIZE.0 * units, SIZE.1 * units);
    let pos = crate::overlay::toast_pos(screen, size, GAP * units);
    if cfg!(target_os = "macos") {
        let _ = win.set_position(tauri::LogicalPosition::new(pos.0, pos.1));
    } else {
        let _ = win.set_position(tauri::PhysicalPosition::new(
            pos.0.round() as i32,
            pos.1.round() as i32,
        ));
    }
}

/// Offers a note for this call.
pub fn show(app: &AppHandle, call: &str) {
    let Some(win) = app.get_webview_window(TOAST).or_else(|| create(app).ok()) else {
        return;
    };
    place(app, &win);
    let _ = app.emit_to(
        TOAST,
        "toast",
        json!({ "call": call, "showMs": SHOW_MS, "theme": app::shared_of(app).config().appearance }),
    );
    let _ = win.show();
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(TOAST) {
        let _ = win.hide();
    }
}

/// Watches for calls while the app runs; each new call gets one toast unless a note is already recording.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut detector = super::Detector::default();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(super::EVERY_SECS));
            if !app::shared_of(&app).config().detect_calls {
                continue;
            }
            let calls = super::live_calls();
            let busy = crate::notes::session::active(&app).is_some();
            if busy {
                hide(&app);
            }
            if let Some(call) = detector.update(&calls, busy) {
                crate::diag::log(&format!("call detected: {call}"));
                show(&app, call);
            }
        }
    });
}
