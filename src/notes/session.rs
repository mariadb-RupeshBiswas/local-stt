//! The note being recorded: two captures, their segmenters, and the hand-off to the speech worker.

use super::segmenter::Segmenter;
use super::{Note, Segment, Status, ME, OTHERS};
use crate::app::{self, lock, Job};
use crate::diag;
use crate::recorder::{Recorder, Source};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
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
    static ANSWER: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    ANSWER.get_or_init(check_support).clone()
}

fn check_support() -> Result<(), String> {
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

/// Where a channel's audio comes from; a trait so the loop can be tested without devices.
pub(crate) trait Feed: Send {
    fn drain(&self) -> Vec<f32>;
    fn close(self: Box<Self>);
}

impl Feed for Recorder {
    fn drain(&self) -> Vec<f32> {
        Recorder::drain(self)
    }
    fn close(self: Box<Self>) {
        self.cancel();
    }
}

pub(crate) struct Channel {
    feed: Box<dyn Feed>,
    segmenter: Segmenter,
    speaker: &'static str,
    // Each capture opens a moment after the note starts; segment times count from the note's start.
    offset_ms: u64,
    opened: Instant,
    fed: u64,
}

impl Channel {
    pub(crate) fn new(feed: Box<dyn Feed>, speaker: &'static str, offset_ms: u64) -> Self {
        Channel {
            feed,
            segmenter: Segmenter::new(),
            speaker,
            offset_ms,
            opened: Instant::now(),
            fed: 0,
        }
    }
}

// Silence longer than this with no audio delivered is filled in, since loopback sends nothing while the computer is quiet.
const GAP_FILL_MS: u64 = 300;

/// Feeds what arrived, after zeros for any stretch the device skipped, so segment times follow the clock.
fn feed(ch: &mut Channel, audio: &[f32], elapsed_ms: u64) -> Vec<super::segmenter::Chunk> {
    let expected = elapsed_ms * 16;
    let behind = expected.saturating_sub(ch.fed + audio.len() as u64);
    let mut chunks = Vec::new();
    if behind > GAP_FILL_MS * 16 {
        chunks.extend(ch.segmenter.feed(&vec![0.0; behind as usize]));
        ch.fed += behind;
    }
    chunks.extend(ch.segmenter.feed(audio));
    ch.fed += audio.len() as u64;
    chunks
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
        Ok(rec) => Some(Channel::new(
            Box::new(rec),
            speaker,
            app::now_ms().saturating_sub(started_ms),
        )),
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

/// Drains every open channel until told to stop; channels join as their devices open, so a slow
/// permission prompt on one never holds up the other or the Stop button.
pub(crate) fn run(
    stop: &Receiver<()>,
    opened: &Receiver<Channel>,
    max: Duration,
    mut emit: impl FnMut(&'static str, u64, Vec<f32>),
) -> bool {
    let began = Instant::now();
    let mut channels: Vec<Channel> = Vec::new();
    let mut hit_max = false;
    loop {
        match stop.recv_timeout(DRAIN_EVERY) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        while let Ok(ch) = opened.try_recv() {
            channels.push(ch);
        }
        for ch in channels.iter_mut() {
            let audio = ch.feed.drain();
            let elapsed = ch.opened.elapsed().as_millis() as u64;
            for chunk in feed(ch, &audio, elapsed) {
                emit(ch.speaker, ch.offset_ms + chunk.start_ms, chunk.pcm);
            }
        }
        if began.elapsed() >= max {
            hit_max = true;
            break;
        }
    }
    for mut ch in channels.drain(..) {
        let audio = ch.feed.drain();
        let elapsed = ch.opened.elapsed().as_millis() as u64;
        let mut chunks = feed(&mut ch, &audio, elapsed);
        chunks.extend(ch.segmenter.flush());
        for chunk in chunks {
            emit(ch.speaker, ch.offset_ms + chunk.start_ms, chunk.pcm);
        }
        ch.feed.close();
    }
    hit_max
}

fn capture(app: AppHandle, id: String, started_ms: u64, stop: Receiver<()>) {
    let (opened_tx, opened_rx) = channel::<Channel>();
    let mut opening = Vec::new();
    for (source, speaker) in [(Source::System, OTHERS), (mic_source(&app), ME)] {
        let app = app.clone();
        let tx = opened_tx.clone();
        // A device still opening after Stop is dropped when it arrives, which ends its stream.
        opening.push(std::thread::spawn(move || {
            if let Some(ch) = open(&app, source, speaker, started_ms) {
                let _ = tx.send(ch);
            }
        }));
    }
    drop(opened_tx);
    let began = Instant::now();
    let mut segments = 0usize;
    let hit_max = run(&stop, &opened_rx, MAX_NOTE, |speaker, start_ms, pcm| {
        segments += 1;
        send(&app, &id, speaker, start_ms, pcm);
    });
    if hit_max {
        let _ = app.emit(
            "notice",
            json!({ "message": "Notes stop after 4 hours. This note is saved." }),
        );
        let _ = stop_from_capture(&app, &id);
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
    drop(opening);
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

    struct Silent;
    impl Feed for Silent {
        fn drain(&self) -> Vec<f32> {
            Vec::new()
        }
        fn close(self: Box<Self>) {}
    }

    #[test]
    fn stop_ends_the_note_even_when_no_device_ever_opens() {
        let (stop_tx, stop_rx) = channel::<()>();
        let (_opened_tx, opened_rx) = channel::<Channel>();
        stop_tx.send(()).unwrap();
        let started = Instant::now();
        let hit_max = run(&stop_rx, &opened_rx, MAX_NOTE, |_, _, _| {});
        assert!(!hit_max);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_channel_that_opens_late_still_records_until_stop() {
        let (stop_tx, stop_rx) = channel::<()>();
        let (opened_tx, opened_rx) = channel::<Channel>();
        let mut speakers = Vec::new();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            opened_tx
                .send(Channel::new(Box::new(Silent), OTHERS, 400))
                .unwrap();
            std::thread::sleep(Duration::from_millis(400));
            stop_tx.send(()).unwrap();
        });
        run(&stop_rx, &opened_rx, MAX_NOTE, |speaker, _, _| {
            speakers.push(speaker)
        });
        assert!(speakers.is_empty(), "silence makes no segments");
    }

    fn tone(ms: u64) -> Vec<f32> {
        (0..ms * 16)
            .map(|i| 0.2 * ((i as f32) * 0.05).sin())
            .collect()
    }

    #[test]
    fn a_device_that_sends_nothing_while_quiet_keeps_the_clock() {
        let mut ch = Channel::new(Box::new(Silent), OTHERS, 0);
        // one second of speech, then nothing delivered for three seconds, then speech again
        assert!(feed(&mut ch, &tone(1_000), 1_000).is_empty());
        let closed = feed(&mut ch, &[], 4_000);
        assert_eq!(closed.len(), 1, "the quiet stretch ends the first segment");
        assert_eq!(closed[0].start_ms, 0);
        feed(&mut ch, &tone(1_000), 5_000);
        let second = ch.segmenter.flush().expect("second utterance");
        assert!(
            (3_690..=3_710).contains(&second.start_ms),
            "{}",
            second.start_ms
        );
    }

    #[test]
    fn normal_delivery_jitter_adds_no_padding() {
        let mut ch = Channel::new(Box::new(Silent), ME, 0);
        feed(&mut ch, &tone(250), 250);
        feed(&mut ch, &tone(150), 500);
        assert_eq!(ch.fed, 400 * 16);
    }

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
