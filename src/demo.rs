//! `local-stt demo`: a scripted tour with sample data, for screenshots and first looks.

use crate::app;
use crate::{history, paths};
use serde_json::json;
use std::io::Write;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const ENV: &str = "LOCAL_STT_DEMO";

pub fn active() -> bool {
    std::env::var_os(ENV).is_some()
}

fn demo_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("local-stt-demo")
}

/// The tour's sample data never outlives it.
pub fn cleanup() {
    if active() {
        let _ = std::fs::remove_dir_all(demo_dir());
    }
}

const SAMPLE_HISTORY: &[(&str, u64)] = &[
    ("The weather is really nice today, let's take the afternoon off.", 2_400),
    ("Remind me to renew the domain before the end of the month.", 2_900),
    ("Here are the notes from the planning call:\n1. Make the pricing page compare the monthly and annual plans.\n2. Mention the free trial limit in the first paragraph of the onboarding email.", 21_600),
    ("Please check the September numbers before the meeting tomorrow.", 3_200),
];

const LIVE_WORDS: &str = "Here are the notes from the planning call point one make the pricing page compare the monthly and annual plans";

/// Points the app at a throwaway data folder with sample history and, when present, the real small model.
pub fn prepare() -> Result<(), String> {
    let real_models = paths::models_dir();
    let dir = demo_dir();
    let _ = std::fs::remove_dir_all(&dir);
    std::env::set_var("LOCAL_STT_DATA_DIR", &dir);
    std::env::set_var(ENV, "1");
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let now = app::now_ms();
    for (i, (text, duration_ms)) in SAMPLE_HISTORY.iter().enumerate() {
        let entry = history::Entry {
            ts_ms: now - (SAMPLE_HISTORY.len() - i) as u64 * 47 * 60 * 1000,
            text: text.to_string(),
            duration_ms: *duration_ms,
            model: "Whisper Small".into(),
            ok: true,
        };
        history::append(&paths::history_path(), &entry).map_err(|e| e.to_string())?;
    }
    let small = crate::models::path(&real_models, crate::models::ModelId::Small);
    #[cfg(unix)]
    if small.is_file() {
        let _ = std::os::unix::fs::symlink(
            &small,
            crate::models::path(&paths::models_dir(), crate::models::ModelId::Small),
        );
    }
    #[cfg(not(unix))]
    let _ = small;
    Ok(())
}

// Each step is announced on stdout so a screenshot script can capture it at the right moment.
fn step(name: &str) {
    println!("DEMO {name}");
    let _ = std::io::stdout().flush();
}

fn wait(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn levels(app: &AppHandle, ms: u64) {
    let frames = ms / 33;
    for i in 0..frames {
        let t = i as f32 / 30.0;
        let level = 0.35 + 0.3 * (t * 7.3).sin().abs() + 0.15 * (t * 2.1).cos().abs();
        let _ = app.emit("overlay-level", json!({ "level": level.min(1.0) }));
        wait(33);
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        wait(1_500);
        app::show_main(&app);
        for tab in ["history", "model", "settings"] {
            wait(1_500);
            let _ = app.emit("show-tab", json!({ "tab": tab }));
            wait(2_500);
            step(tab);
        }
        if let Some(main) = tauri::Manager::get_webview_window(&app, app::MAIN) {
            let _ = main.hide();
        }
        let started = app::now_ms();
        let _ = app.emit("live-reset", json!({}));
        app::show_overlay(&app, "recording", "Recording", Some(started));
        let words: Vec<&str> = LIVE_WORDS.split(' ').collect();
        for n in 1..=words.len() {
            let text = words[..n].join(" ");
            let _ = app.emit("live-text", json!({ "text": text, "formatted": true }));
            levels(&app, 220);
        }
        step("recording");
        levels(&app, 2_000);
        app::show_overlay(&app, "recording", "Hands-free", Some(started));
        levels(&app, 1_500);
        step("hands-free");
        levels(&app, 1_500);
        app::show_overlay(&app, "transcribing", "Transcribing", None);
        wait(1_200);
        step("transcribing");
        app::show_overlay(&app, "done", "Pasted", None);
        // The pill fades itself about 450 ms after "done", so the capture cue goes out early.
        wait(120);
        step("done");
        app::hide_overlay_after(&app, 600);
        wait(1_500);
        step("end");
        app.exit(0);
    });
}
