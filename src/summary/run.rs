//! Background work for summaries: downloading a model, and summarizing a note with it.

use super::SummaryModelId;
use crate::app::{lock, shared_of};
use crate::{diag, paths};
use serde_json::json;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub fn start_download(app: AppHandle, id: SummaryModelId) -> Result<(), String> {
    let s = shared_of(&app);
    {
        let mut busy = lock(&s.summary_downloading);
        if busy.is_some() {
            return Err("Another summary model is downloading. Try again when it finishes.".into());
        }
        *busy = Some(id);
    }
    std::thread::spawn(move || {
        let info = super::info(id);
        let dest = super::path(&paths::models_dir(), id);
        let total = info.size_bytes;
        let last = std::cell::Cell::new(Instant::now() - Duration::from_secs(1));
        let emitter = app.clone();
        let progress = move |done: u64| {
            if last.get().elapsed() >= Duration::from_millis(200) {
                last.set(Instant::now());
                let _ = emitter.emit(
                    "summary-model-progress",
                    json!({ "id": id, "downloaded": done, "total": total }),
                );
            }
        };
        diag::log(&format!("summary model {} download started", info.label));
        let result = crate::download::download(&super::url(id), info.sha256, &dest, &progress);
        let s = shared_of(&app);
        *lock(&s.summary_downloading) = None;
        diag::log(&match &result {
            Ok(()) => format!("summary model {} downloaded and verified", info.label),
            Err(e) => format!("summary model {} download failed: {e}", info.label),
        });
        if result.is_ok() {
            s.update_config(|c| {
                c.summary_model = Some(id);
                c.summaries = true;
            });
        }
        let _ = app.emit(
            "summary-model-done",
            json!({ "id": id, "ok": result.is_ok(), "error": result.err() }),
        );
    });
    Ok(())
}

pub fn start_summarize(app: AppHandle, note_id: String) -> Result<(), String> {
    let s = shared_of(&app);
    let cfg = s.config();
    if !cfg.summaries {
        return Err("Summaries are off. Turn them on in the Model tab.".into());
    }
    let fits = super::evaluate(&s.hardware, &paths::models_dir());
    let Some(model) = super::pick(&fits, cfg.summary_model) else {
        return Err("Download a summary model in the Model tab first.".into());
    };
    let note =
        crate::notes::get(&note_id).map_err(|_| "This note could not be read.".to_string())?;
    if note.status != crate::notes::Status::Done {
        return Err("Stop the note before summarizing it.".into());
    }
    {
        let mut busy = lock(&s.summarizing);
        if busy.is_some() {
            return Err("Another note is being summarized. Try again when it finishes.".into());
        }
        *busy = Some(note_id.clone());
    }
    std::thread::spawn(move || {
        let started = Instant::now();
        let result = summarize(&app, &note, model, cfg.user_name.as_deref());
        let s = shared_of(&app);
        *lock(&s.summarizing) = None;
        // Sizes and timings only; the summary's words never reach the log.
        diag::log(&match &result {
            Ok(_) => format!(
                "note summarized with {} in {:.0} s",
                super::info(model).label,
                started.elapsed().as_secs_f64()
            ),
            Err(e) => format!("note summary failed: {e}"),
        });
        let _ = app.emit(
            "note-summary",
            json!({ "id": note.id, "ok": result.is_ok(), "error": result.err() }),
        );
    });
    Ok(())
}

// Re-checks the pinned checksum once per run, so a damaged or swapped file is never parsed.
fn verify_once(model: SummaryModelId, path: &std::path::Path) -> Result<(), String> {
    static VERIFIED: std::sync::Mutex<Vec<SummaryModelId>> = std::sync::Mutex::new(Vec::new());
    let mut done = lock(&VERIFIED);
    if done.contains(&model) {
        return Ok(());
    }
    let sum =
        crate::download::sha256_file(path).map_err(|e| format!("cannot read the model: {e}"))?;
    if sum != super::info(model).sha256 {
        return Err(format!(
            "{} does not match its checksum. Delete it in the Model tab and download it again.",
            super::info(model).label
        ));
    }
    done.push(model);
    Ok(())
}

fn summarize(
    app: &AppHandle,
    note: &crate::notes::Note,
    model: SummaryModelId,
    user_name: Option<&str>,
) -> Result<(), String> {
    let path = super::path(&paths::models_dir(), model);
    verify_once(model, &path)?;
    let llm = crate::engine::Llm::load(&path)
        .ok_or_else(|| format!("{} could not be loaded.", super::info(model).label))?;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get() as u32);
    let mut generate = |prompt: &str, root: &str, max: u32| {
        llm.generate(
            prompt,
            super::write::GRAMMAR,
            root,
            super::write::N_CTX,
            max,
            threads,
        )
    };
    let id = note.id.clone();
    let emitter = app.clone();
    let mut progress = |done: usize, total: usize| {
        let _ = emitter.emit(
            "note-summary-progress",
            json!({ "id": id, "done": done, "total": total }),
        );
    };
    let made_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let summary = super::write::summarize(
        note,
        user_name,
        model,
        made_ms,
        &mut generate,
        &mut progress,
    )?;
    crate::notes::set_summary(&note.id, summary)
        .map_err(|e| format!("The summary could not be saved: {e}"))?;
    Ok(())
}
