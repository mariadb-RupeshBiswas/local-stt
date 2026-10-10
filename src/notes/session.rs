//! The note being recorded: two captures, their segmenters, and the hand-off to the speech worker.

use super::segmenter::Segmenter;
use super::{Note, Segment, Status, ME, OTHERS};
use crate::app::{self, lock, Job};
use crate::diag;
use crate::recorder::{Recorder, Source};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const DRAIN_EVERY: Duration = Duration::from_millis(250);
// Meetings past this length stop on their own, so a forgotten note cannot run for days.
const MAX_NOTE: Duration = Duration::from_secs(4 * 3600);

pub struct Running {
    pub id: String,
    pub started_ms: u64,
    stop: Sender<()>,
}

/// Why notes cannot run here, or Ok. System audio needs macOS 14.6 (cpal's floor for its tap).
pub fn support() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let out = crate::proc::command("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .map_err(|_| "Could not read the macOS version.".to_string())?;
        let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !at_least(&version, 14, 6) {
            return Err("Notes need macOS 14.6 or later.".into());
        }
    }
    Ok(())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn at_least(version: &str, major: u32, minor: u32) -> bool {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (a, b) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    (a, b) >= (major, minor)
}

/// The note now recording, as (id, start time).
pub fn active(app: &AppHandle) -> Option<(String, u64)> {
    let shared = app::shared_of(app);
    let running = lock(&shared.note);
    running.as_ref().map(|r| (r.id.clone(), r.started_ms))
}

/// Starts a note at once and returns its id; capture opens on its own thread (it can wait on a permission prompt).
pub fn start(app: &AppHandle) -> Result<String, String> {
    support()?;
    let shared = app::shared_of(app);
    let mut running = lock(&shared.note);
    if running.is_some() {
        return Err("A note is already recording.".into());
    }
    let started_ms = app::now_ms();
    let id = super::new_id();
    let note = Note {
        id: id.clone(),
        title: String::new(),
        started_ms,
        ended_ms: None,
        source: None,
        status: Status::Recording,
        segments: Vec::new(),
    };
    super::begin(&note).map_err(|e| format!("Could not start the note: {e}"))?;
    let (stop_tx, stop_rx) = channel::<()>();
    *running = Some(Running {
        id: id.clone(),
        started_ms,
        stop: stop_tx,
    });
    drop(running);
    diag::log("note started");
    let thread_app = app.clone();
    let thread_id = id.clone();
    std::thread::Builder::new()
        .name("local-stt-note".into())
        .spawn(move || capture(thread_app, thread_id, started_ms, stop_rx))
        .map_err(|e| format!("Could not start the note: {e}"))?;
    app::show_notes_pill(app);
    app::refresh_update_menu(app);
    let _ = app.emit_to(
        app::MAIN,
        "note-state",
        json!({ "id": id, "status": "recording", "startedMs": started_ms }),
    );
    Ok(id)
}

/// Stops the running note; the worker tidies and saves it once its last segments are in.
pub fn stop(app: &AppHandle) -> Result<(), String> {
    let shared = app::shared_of(app);
    let Some(running) = lock(&shared.note).take() else {
        return Err("No note is recording.".into());
    };
    let _ = running.stop.send(());
    shared.notes_pill.store(false, Ordering::SeqCst);
    app::hide_overlay_after(app, 0);
    app::refresh_update_menu(app);
    let _ = app.emit_to(
        app::MAIN,
        "note-state",
        json!({ "id": running.id, "status": "tidying" }),
    );
    Ok(())
}

struct Channel {
    rec: Recorder,
    segmenter: Segmenter,
    speaker: &'static str,
    // Each capture opens a moment after the note starts; segment times count from the note's start.
    offset_ms: u64,
}

fn open(
    app: &AppHandle,
    source: Source,
    speaker: &'static str,
    started_ms: u64,
) -> Option<Channel> {
    let level_app = app.clone();
    let on_level: Box<dyn Fn(f32) + Send> = if speaker == ME {
        // The pill meters the microphone while it shows Notes, never during a dictation.
        Box::new(move |level| {
            if app::shared_of(&level_app)
                .notes_pill
                .load(Ordering::Relaxed)
            {
                let _ = level_app.emit("overlay-level", json!({ "level": level }));
            }
        })
    } else {
        Box::new(|_| {})
    };
    match Recorder::start_from(source, on_level) {
        Ok(rec) => Some(Channel {
            rec,
            segmenter: Segmenter::new(),
            speaker,
            offset_ms: app::now_ms().saturating_sub(started_ms),
        }),
        Err(e) => {
            // The error can name the chosen microphone, which stays out of the log.
            let named = app::shared_of(app).config().microphone;
            let shown = match named.as_deref() {
                Some(name) if !name.is_empty() => e.replace(name, "<microphone>"),
                _ => e.clone(),
            };
            diag::log(&format!("note: {speaker} capture failed: {shown}"));
            let message = if speaker == ME {
                format!("{e} Only the other side is being recorded.")
            } else {
                format!("{e} Only your microphone is being recorded.")
            };
            let _ = app.emit("notice", json!({ "message": message }));
            None
        }
    }
}

fn capture(app: AppHandle, id: String, started_ms: u64, stop: std::sync::mpsc::Receiver<()>) {
    let mut channels: Vec<Channel> = [(Source::System, OTHERS), (mic_source(&app), ME)]
        .into_iter()
        .filter_map(|(source, speaker)| open(&app, source, speaker, started_ms))
        .collect();
    let began = Instant::now();
    let mut segments = 0usize;
    if channels.is_empty() {
        let _ = app.emit(
            "notice",
            json!({ "message": "Could not record anything for this note." }),
        );
    }
    loop {
        match stop.recv_timeout(DRAIN_EVERY) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        for ch in channels.iter_mut() {
            let audio = ch.rec.drain();
            for chunk in ch.segmenter.feed(&audio) {
                segments += 1;
                send(
                    &app,
                    &id,
                    ch.speaker,
                    ch.offset_ms + chunk.start_ms,
                    chunk.pcm,
                );
            }
        }
        if began.elapsed() >= MAX_NOTE {
            let _ = app.emit(
                "notice",
                json!({ "message": "Notes stop after 4 hours. This note is saved." }),
            );
            let _ = stop_from_capture(&app, &id);
            break;
        }
    }
    for mut ch in channels.drain(..) {
        let audio = ch.rec.drain();
        let mut chunks = ch.segmenter.feed(&audio);
        chunks.extend(ch.segmenter.flush());
        for chunk in chunks {
            segments += 1;
            send(
                &app,
                &id,
                ch.speaker,
                ch.offset_ms + chunk.start_ms,
                chunk.pcm,
            );
        }
        ch.rec.cancel();
    }
    diag::log(&format!(
        "note stopped: {:.1} min, {segments} segments queued",
        began.elapsed().as_secs_f64() / 60.0
    ));
    let finish = Job::NoteFinish {
        id,
        ended_ms: app::now_ms(),
    };
    let _ = lock(&app::shared_of(&app).worker).send(finish);
}

// The 4-hour stop comes from the capture thread itself, which only needs the shared state cleared.
fn stop_from_capture(app: &AppHandle, id: &str) -> Result<(), String> {
    let shared = app::shared_of(app);
    let mine = lock(&shared.note).as_ref().is_some_and(|r| r.id == id);
    if mine {
        stop(app)
    } else {
        Ok(())
    }
}

fn mic_source(app: &AppHandle) -> Source {
    Source::Mic(app::shared_of(app).config().microphone)
}

fn send(app: &AppHandle, id: &str, speaker: &'static str, start_ms: u64, pcm: Vec<f32>) {
    let opts = app::transcribe_opts(&app::shared_of(app).config());
    let job = Job::NoteSegment {
        id: id.to_string(),
        speaker,
        start_ms,
        pcm,
        opts,
    };
    let _ = lock(&app::shared_of(app).worker).send(job);
}

/// Worker side: a transcribed segment joins the note on disk and in the main window (only it shows notes).
pub fn add_segment(
    app: &AppHandle,
    id: &str,
    speaker: &str,
    start_ms: u64,
    samples: usize,
    text: &str,
) {
    let text = text.trim();
    if !text.chars().any(char::is_alphanumeric) {
        return;
    }
    let seg = Segment {
        start_ms,
        end_ms: start_ms + samples as u64 * 1000 / 16_000,
        speaker: speaker.to_string(),
        text: text.to_string(),
    };
    if let Err(e) = super::append(id, &seg) {
        diag::log(&format!("note: could not save a segment: {e}"));
        return;
    }
    let _ = app.emit_to(
        app::MAIN,
        "note-segment",
        json!({ "id": id, "segment": seg }),
    );
}

/// Worker side, after the note's last segment: tidy it and save it whole.
pub fn finish(app: &AppHandle, id: &str, ended_ms: u64) {
    let cfg = app::shared_of(app).config();
    match super::finish(id, ended_ms, cfg.smart_format, app::english_output(&cfg)) {
        Ok(note) => {
            diag::log(&format!("note saved: {} lines", note.segments.len()));
            let _ = app.emit_to(
                app::MAIN,
                "note-state",
                json!({ "id": id, "status": "done" }),
            );
        }
        Err(e) => {
            diag::log(&format!("note: could not save: {e}"));
            let _ = app.emit(
                "notice",
                json!({ "message": format!("Could not save the note: {e}") }),
            );
        }
    }
    let _ = app.emit_to(app::MAIN, "notes-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_version_floor() {
        assert!(at_least("14.6", 14, 6));
        assert!(at_least("15.0.1", 14, 6));
        assert!(at_least("27.0.1", 14, 6));
        assert!(!at_least("14.5", 14, 6));
        assert!(!at_least("13.7.2", 14, 6));
        assert!(!at_least("", 14, 6));
    }
}
