//! Meeting notes: one file per note, lines appended while recording, tidied into one file on Stop.

pub mod segmenter;
pub mod session;
pub mod tidy;

use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

pub const ME: &str = "me";
pub const OTHERS: &str = "others";
const MAX_TITLE_CHARS: usize = 200;
const FIRST_LINE_CHARS: usize = 120;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Recording,
    Done,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Note {
    pub id: String,
    /// Empty means the app shows a default made from the source and start time.
    #[serde(default)]
    pub title: String,
    pub started_ms: u64,
    #[serde(default)]
    pub ended_ms: Option<u64>,
    /// The call app, when one was detected ("Zoom").
    #[serde(default)]
    pub source: Option<String>,
    pub status: Status,
    #[serde(default)]
    pub segments: Vec<Segment>,
}

/// One row of the notes list.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub id: String,
    pub title: String,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
    pub source: Option<String>,
    pub status: Status,
    pub first_line: String,
}

// The worker appends while the UI renames or deletes; one lock keeps every file whole.
static FILES: Mutex<()> = Mutex::new(());

fn files() -> MutexGuard<'static, ()> {
    FILES.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn dir() -> PathBuf {
    paths::data_dir().join("notes")
}

fn saved_path(id: &str) -> PathBuf {
    dir().join(format!("{id}.json"))
}

fn live_path(id: &str) -> PathBuf {
    dir().join(format!("{id}.jsonl"))
}

/// Ids are 16 lowercase hex characters, so one can never name a path outside the notes folder.
pub fn valid_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn new_id() -> String {
    use std::hash::BuildHasher;
    static COUNT: AtomicU64 = AtomicU64::new(0);
    // RandomState is keyed from the OS's randomness, so ids do not repeat across runs.
    let seed = (crate::app::now_ms(), COUNT.fetch_add(1, Ordering::Relaxed));
    format!(
        "{:016x}",
        std::collections::hash_map::RandomState::new().hash_one(seed)
    )
}

/// Trims, drops control characters and caps the length; empty means "use the default title".
pub fn clean_title(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect()
}

/// Starts the live file: the note's header on the first line, then one line per segment.
pub fn begin(note: &Note) -> std::io::Result<()> {
    let _guard = files();
    paths::private_dir(&dir())?;
    let mut file = crate::config::private_options()
        .write(true)
        .create_new(true)
        .open(live_path(&note.id))?;
    let mut line = serde_json::to_vec(note).map_err(std::io::Error::other)?;
    line.push(b'\n');
    file.write_all(&line)
}

pub fn append(id: &str, seg: &Segment) -> std::io::Result<()> {
    let _guard = files();
    let mut line = serde_json::to_vec(seg).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut file = crate::config::private_options()
        .append(true)
        .open(live_path(id))?;
    file.write_all(&line)
}

// A torn last line from a crash is skipped; everything before it is kept.
fn read_live(id: &str) -> std::io::Result<Note> {
    let text = std::fs::read_to_string(live_path(id))?;
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    let mut note: Note = serde_json::from_str(header).map_err(std::io::Error::other)?;
    for line in lines {
        if let Ok(seg) = serde_json::from_str::<Segment>(line) {
            note.segments.push(seg);
        }
    }
    Ok(note)
}

/// Tidies the live note into its saved file and removes the live one.
pub fn finish(id: &str, ended_ms: u64, smart_format: bool, english: bool) -> std::io::Result<Note> {
    let _guard = files();
    finish_locked(id, ended_ms, smart_format, english)
}

fn finish_locked(
    id: &str,
    ended_ms: u64,
    smart_format: bool,
    english: bool,
) -> std::io::Result<Note> {
    let mut note = read_live(id)?;
    note.segments = tidy::tidy(note.segments, smart_format, english);
    note.ended_ms = Some(ended_ms.max(note.started_ms));
    note.status = Status::Done;
    save(&note)?;
    std::fs::remove_file(live_path(id))?;
    Ok(note)
}

fn save(note: &Note) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(note).map_err(std::io::Error::other)?;
    crate::history::replace(&saved_path(&note.id), &bytes)
}

/// At start: a live file left by a crash becomes a saved note with what reached disk.
pub fn recover(smart_format: bool, english: bool) -> usize {
    let _guard = files();
    let mut recovered = 0;
    for id in ids_with("jsonl") {
        let ended = read_live(&id)
            .map(|n| n.segments.iter().map(|s| s.end_ms).max().unwrap_or(0) + n.started_ms)
            .unwrap_or(0);
        if finish_locked(&id, ended, smart_format, english).is_ok() {
            recovered += 1;
        }
    }
    recovered
}

fn ids_with(ext: &str) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    read.flatten()
        .filter_map(|e| {
            let path = e.path();
            let stem = path.file_stem()?.to_str()?.to_string();
            (path.extension()? == ext && valid_id(&stem)).then_some(stem)
        })
        .collect()
}

/// The saved note, or the live one while it is being recorded.
pub fn get(id: &str) -> std::io::Result<Note> {
    let _guard = files();
    get_locked(id)
}

fn get_locked(id: &str) -> std::io::Result<Note> {
    match std::fs::read(saved_path(id)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(std::io::Error::other),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => read_live(id),
        Err(e) => Err(e),
    }
}

/// Every note, newest first. ponytail: reads each file; fine for hundreds of notes, index them past that.
pub fn list() -> Vec<Listed> {
    let _guard = files();
    let mut ids = ids_with("json");
    for live in ids_with("jsonl") {
        if !ids.contains(&live) {
            ids.push(live);
        }
    }
    let mut rows: Vec<Listed> = ids
        .iter()
        .filter_map(|id| get_locked(id).ok())
        .map(|n| Listed {
            first_line: n
                .segments
                .first()
                .map(|s| s.text.chars().take(FIRST_LINE_CHARS).collect())
                .unwrap_or_default(),
            id: n.id,
            title: n.title,
            started_ms: n.started_ms,
            ended_ms: n.ended_ms,
            source: n.source,
            status: n.status,
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.started_ms));
    rows
}

pub fn rename(id: &str, title: &str) -> std::io::Result<Note> {
    let _guard = files();
    let mut note = get_locked(id)?;
    note.title = clean_title(title);
    if note.status == Status::Done {
        save(&note)?;
    } else {
        // While recording only the header line changes; the segment lines stay as they are.
        let text = std::fs::read_to_string(live_path(id))?;
        let rest = text.split_once('\n').map(|(_, r)| r).unwrap_or("");
        let header = Note {
            segments: Vec::new(),
            ..note.clone()
        };
        let mut bytes = serde_json::to_vec(&header).map_err(std::io::Error::other)?;
        bytes.push(b'\n');
        bytes.extend_from_slice(rest.as_bytes());
        crate::history::replace(&live_path(id), &bytes)?;
    }
    Ok(note)
}

/// Deletes these notes, never the one being recorded, and returns how many went.
pub fn delete(ids: &HashSet<String>, recording: Option<&str>) -> std::io::Result<usize> {
    let _guard = files();
    let mut removed = 0;
    for id in ids {
        if !valid_id(id) || Some(id.as_str()) == recording {
            continue;
        }
        let mut gone = false;
        for path in [saved_path(id), live_path(id)] {
            match std::fs::remove_file(&path) {
                Ok(()) => gone = true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        if gone {
            removed += 1;
        }
    }
    Ok(removed)
}

/// How many notes exist, for the diagnostic report (never their content).
pub fn count() -> usize {
    let _guard = files();
    ids_with("json").len() + ids_with("jsonl").len()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Notes live under LOCAL_STT_DATA_DIR, which is process-wide, so these tests share one folder and one lock.
    fn sandbox() -> MutexGuard<'static, ()> {
        let guard = crate::paths::TEST_DATA_DIR
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let d = std::env::temp_dir().join(format!("lstt-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::env::set_var("LOCAL_STT_DATA_DIR", &d);
        guard
    }

    fn header(id: &str, started: u64) -> Note {
        Note {
            id: id.into(),
            title: String::new(),
            started_ms: started,
            ended_ms: None,
            source: Some("Zoom".into()),
            status: Status::Recording,
            segments: Vec::new(),
        }
    }

    fn seg(speaker: &str, start: u64, text: &str) -> Segment {
        Segment {
            start_ms: start,
            end_ms: start + 1_000,
            speaker: speaker.into(),
            text: text.into(),
        }
    }

    #[test]
    fn ids_are_hex_and_paths_cannot_escape() {
        let id = new_id();
        assert!(valid_id(&id), "{id}");
        assert_ne!(id, new_id());
        for bad in [
            "../../etc/passwd",
            "0123456789ABCDEF",
            "0123456789abcde",
            "0123456789abcdeg",
        ] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn a_note_records_lines_then_finishes_tidied() {
        let _g = sandbox();
        let id = "00000000000000a1";
        begin(&header(id, 1_000)).unwrap();
        append(id, &seg(ME, 0, "First point.")).unwrap();
        append(id, &seg(ME, 1_500, "Second point.")).unwrap();
        append(id, &seg(OTHERS, 5_000, "Agreed.")).unwrap();
        assert_eq!(get(id).unwrap().status, Status::Recording);
        assert_eq!(list()[0].status, Status::Recording);
        let note = finish(id, 9_000, false, true).unwrap();
        assert_eq!(note.status, Status::Done);
        assert_eq!(note.segments.len(), 2);
        assert_eq!(note.segments[0].text, "First point. Second point.");
        assert!(!live_path(id).exists());
        assert_eq!(get(id).unwrap().ended_ms, Some(9_000));
    }

    #[cfg(unix)]
    #[test]
    fn note_files_and_folder_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let _g = sandbox();
        let id = "00000000000000b2";
        begin(&header(id, 1)).unwrap();
        let mode = |p: PathBuf| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(live_path(id)), 0o600);
        assert_eq!(mode(dir()), 0o700);
        finish(id, 2, false, true).unwrap();
        assert_eq!(mode(saved_path(id)), 0o600);
    }

    #[test]
    fn a_crash_mid_call_keeps_what_reached_disk() {
        let _g = sandbox();
        let id = "00000000000000c3";
        begin(&header(id, 10_000)).unwrap();
        append(id, &seg(OTHERS, 0, "Hello there.")).unwrap();
        // a torn last line, as a crash mid-write leaves
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(live_path(id))
            .unwrap();
        write!(f, "{{\"start_ms\":5").unwrap();
        assert_eq!(recover(false, true), 1);
        let note = get(id).unwrap();
        assert_eq!(note.status, Status::Done);
        assert_eq!(note.segments.len(), 1);
        assert_eq!(note.ended_ms, Some(11_000));
    }

    #[test]
    fn rename_works_while_recording_and_after() {
        let _g = sandbox();
        let id = "00000000000000d4";
        begin(&header(id, 1)).unwrap();
        append(id, &seg(ME, 0, "Line.")).unwrap();
        rename(id, "  Weekly\u{7} sync  ").unwrap();
        let live = get(id).unwrap();
        assert_eq!(live.title, "Weekly sync");
        assert_eq!(live.segments.len(), 1);
        finish(id, 5, false, true).unwrap();
        rename(id, &"x".repeat(500)).unwrap();
        assert_eq!(get(id).unwrap().title.chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn delete_skips_the_note_being_recorded_and_bad_ids() {
        let _g = sandbox();
        let (a, b) = ("00000000000000e5", "00000000000000e6");
        begin(&header(a, 1)).unwrap();
        finish(a, 2, false, true).unwrap();
        begin(&header(b, 3)).unwrap();
        let ids: HashSet<String> = [a, b, "../config"].iter().map(|s| s.to_string()).collect();
        assert_eq!(delete(&ids, Some(b)).unwrap(), 1);
        assert!(get(a).is_err());
        assert!(get(b).is_ok());
        assert_eq!(count(), 1);
    }

    #[test]
    fn list_is_newest_first_with_a_first_line() {
        let _g = sandbox();
        for (id, at) in [("00000000000000f1", 100), ("00000000000000f2", 300)] {
            begin(&header(id, at)).unwrap();
            append(id, &seg(ME, 0, "Opening line.")).unwrap();
            finish(id, at + 10, false, true).unwrap();
        }
        let rows = list();
        assert_eq!(rows[0].id, "00000000000000f2");
        assert_eq!(rows[0].first_line, "Opening line.");
    }
}
