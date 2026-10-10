//! `local-stt demo`: a scripted tour with sample data, for screenshots and first looks.

use crate::app;
use crate::{history, paths};
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

// Set only by prepare() in this process, so no inherited variable can turn a real launch into a tour.
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// A made-up machine for the Model tab, so screenshots never show the real one.
pub fn sample_hardware() -> crate::hwprobe::Hardware {
    let windows = cfg!(windows);
    crate::hwprobe::Hardware {
        os: if windows { "Windows 11" } else { "macOS 15.1" }.into(),
        arch: if windows { "x86_64" } else { "aarch64" }.into(),
        cpu: if windows {
            "Intel Core i7-1260P"
        } else {
            "Apple M2"
        }
        .into(),
        threads: if windows { 16 } else { 8 },
        ram_gb: Some(16),
        gpu: if windows {
            "Intel Iris Xe Graphics"
        } else {
            "Apple M2 GPU (Metal)"
        }
        .into(),
        gpu_accel: !windows,
        free_disk_gb: Some(212.4),
    }
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
    ACTIVE.store(true, Ordering::Relaxed);
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

// Each cue goes out once its state has settled, then the state holds while the script captures it.
const HOLD_MS: u64 = 2_000;

pub fn spawn(app: AppHandle) {
    let shared = app.state::<std::sync::Arc<app::Shared>>().inner().clone();
    *app::lock(&shared.active_model) = Some(shared.config().model);
    std::thread::spawn(move || {
        step(&format!("pid {}", std::process::id()));
        wait(1_500);
        app::show_main(&app);
        for tab in ["history", "model", "settings"] {
            let _ = app.emit("show-tab", json!({ "tab": tab }));
            wait(1_500);
            step(tab);
            wait(HOLD_MS);
        }
        if let Some(main) = app.get_webview_window(app::MAIN) {
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
        levels(&app, HOLD_MS);
        app::show_overlay(&app, "recording", "Hands-free", Some(started));
        levels(&app, 600);
        step("hands-free");
        levels(&app, HOLD_MS);
        app::show_overlay(&app, "transcribing", "Transcribing", None);
        wait(600);
        step("transcribing");
        wait(HOLD_MS);
        // The pill keeps "done" on screen in the tour (get_state says demo), so it holds like the rest.
        app::show_overlay(&app, "done", "Pasted", None);
        wait(600);
        step("done");
        wait(HOLD_MS);
        app::hide_overlay_after(&app, 0);
        wait(800);
        step("end");
        app.exit(0);
    });
}
