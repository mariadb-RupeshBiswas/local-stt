//! Meeting notes: one file per note, lines appended while recording, tidied into one file on Stop.

pub mod callnames;
pub mod myvoice;
pub mod segmenter;
pub mod session;
pub mod tidy;
pub mod voices;

use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

pub const ME: &str = "me";
pub const OTHERS: &str = "others";
const MAX_TITLE_CHARS: usize = 200;
const MAX_SPEAKER_CHARS: usize = 60;
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
    /// Names the user gave a speaker ("others" -> "Priya"); a speaker without one shows its default.
    #[serde(default)]
    pub speakers: BTreeMap<String, String>,
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
    clean(raw, MAX_TITLE_CHARS)
}

fn clean(raw: &str, max: usize) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(max)
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

/// Tidies the live note into its saved file and removes the live one; `voices` names the other side's speakers by line start.
pub fn finish(
    id: &str,
    ended_ms: u64,
    smart_format: bool,
    english: bool,
    voices: &HashMap<u64, String>,
) -> std::io::Result<Note> {
    let _guard = files();
    finish_locked(id, ended_ms, smart_format, english, voices)
}

fn finish_locked(
    id: &str,
    ended_ms: u64,
    smart_format: bool,
    english: bool,
    voices: &HashMap<u64, String>,
) -> std::io::Result<Note> {
    let mut note = read_live(id)?;
    // Before relabelling: only a microphone line can be the speakers' echo.
    note.segments = tidy::drop_echo(note.segments);
    assign_speakers(&mut note.segments, voices);
    keep_others_name(&mut note);
    note.segments = tidy::tidy(note.segments, smart_format, english);
    note.ended_ms = Some(ended_ms.max(note.started_ms));
    note.status = Status::Done;
    save(&note)?;
    std::fs::remove_file(live_path(id))?;
    Ok(note)
}

// Others lines take the speaker their voice was grouped into; a line too short to tell takes the nearest grouped line's.
// A microphone line the user's own voice did not match is a person in the room and takes its group's speaker.
fn assign_speakers(segments: &mut [Segment], voices: &HashMap<u64, String>) {
    if voices.is_empty() {
        return;
    }
    for seg in segments.iter_mut().filter(|s| s.speaker == ME) {
        if let Some(key) = voices.get(&(seg.start_ms | voices::ROOM)) {
            seg.speaker = key.clone();
        }
    }
    let grouped: Vec<(u64, String)> = segments
        .iter()
        .filter(|s| s.speaker == OTHERS)
        .filter_map(|s| voices.get(&s.start_ms).map(|k| (s.start_ms, k.clone())))
        .collect();
    for seg in segments.iter_mut().filter(|s| s.speaker == OTHERS) {
        let nearest = grouped
            .iter()
            .min_by_key(|(start, _)| start.abs_diff(seg.start_ms));
        if let Some((_, key)) = nearest {
            seg.speaker = key.clone();
        }
    }
}

// A name given to Others while recording goes to the voice that said the most once the call is split.
fn keep_others_name(note: &mut Note) {
    if note.segments.iter().any(|s| s.speaker == OTHERS) {
        return;
    }
    let Some(name) = note.speakers.remove(OTHERS) else {
        return;
    };
    let mut lines: HashMap<&str, usize> = HashMap::new();
    for seg in note.segments.iter().filter(|s| s.speaker != ME) {
        *lines.entry(seg.speaker.as_str()).or_default() += 1;
    }
    let busiest = lines
        .into_iter()
        .max_by_key(|(key, count)| (*count, std::cmp::Reverse(*key)))
        .map(|(key, _)| key.to_string());
    if let Some(key) = busiest {
        note.speakers.entry(key).or_insert(name);
    }
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
        if finish_locked(&id, ended, smart_format, english, &HashMap::new()).is_ok() {
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
    let title = clean_title(title);
    update(id, |note| {
        note.title = title;
        Ok(())
    })
}

/// Names one speaker across the whole note; an empty name brings back the default ("Me", "Others").
pub fn rename_speaker(id: &str, speaker: &str, name: &str) -> std::io::Result<Note> {
    let name = clean(name, MAX_SPEAKER_CHARS);
    update(id, |note| {
        let known = speaker == ME
            || speaker == OTHERS
            || note.segments.iter().any(|s| s.speaker == speaker);
        if !known {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no such speaker in this note",
            ));
        }
        if name.is_empty() {
            note.speakers.remove(speaker);
        } else {
            note.speakers.insert(speaker.to_string(), name);
        }
        Ok(())
    })
}

// Changes the note's header: the saved file once done, only the live file's first line while recording.
fn update(
    id: &str,
    change: impl FnOnce(&mut Note) -> std::io::Result<()>,
) -> std::io::Result<Note> {
    let _guard = files();
    let mut note = get_locked(id)?;
    change(&mut note)?;
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
            speakers: BTreeMap::new(),
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
        let note = finish(id, 9_000, false, true, &HashMap::new()).unwrap();
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
        finish(id, 2, false, true, &HashMap::new()).unwrap();
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
        finish(id, 5, false, true, &HashMap::new()).unwrap();
        rename(id, &"x".repeat(500)).unwrap();
        assert_eq!(get(id).unwrap().title.chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn a_speaker_name_holds_while_recording_after_the_tidy_and_can_be_cleared() {
        let _g = sandbox();
        let id = "00000000000000d5";
        begin(&header(id, 1)).unwrap();
        append(id, &seg(OTHERS, 0, "Hi.")).unwrap();
        rename_speaker(id, OTHERS, "  Priya\u{7} ").unwrap();
        assert_eq!(get(id).unwrap().speakers[OTHERS], "Priya");
        assert_eq!(get(id).unwrap().segments.len(), 1);
        finish(id, 5, false, true, &HashMap::new()).unwrap();
        assert_eq!(get(id).unwrap().speakers[OTHERS], "Priya");
        rename_speaker(id, ME, &"x".repeat(100)).unwrap();
        assert_eq!(
            get(id).unwrap().speakers[ME].chars().count(),
            MAX_SPEAKER_CHARS
        );
        rename_speaker(id, OTHERS, " ").unwrap();
        assert!(!get(id).unwrap().speakers.contains_key(OTHERS));
        assert!(rename_speaker(id, "s9", "Ghost").is_err());
    }

    #[test]
    fn the_other_side_takes_its_speakers_and_short_lines_follow_their_neighbours() {
        let _g = sandbox();
        let id = "00000000000000d6";
        begin(&header(id, 1)).unwrap();
        append(id, &seg(OTHERS, 0, "Hello, I'm Priya.")).unwrap();
        append(id, &seg(OTHERS, 2_500, "Hi.")).unwrap();
        append(id, &seg(ME, 20_000, "Welcome both.")).unwrap();
        append(id, &seg(OTHERS, 30_000, "Thanks, glad to join.")).unwrap();
        let voices: HashMap<u64, String> =
            [(0, "s1".to_string()), (30_000, "s2".to_string())].into();
        rename_speaker(id, OTHERS, "Priya").unwrap();
        let note = finish(id, 40_000, false, true, &voices).unwrap();
        let who: Vec<&str> = note.segments.iter().map(|s| s.speaker.as_str()).collect();
        assert_eq!(who, vec!["s1", ME, "s2"]);
        assert_eq!(note.segments[0].text, "Hello, I'm Priya. Hi.");
        // the name typed for Others during the call lands on the voice with the most lines
        assert_eq!(note.speakers.get("s1").map(String::as_str), Some("Priya"));
        assert!(!note.speakers.contains_key(OTHERS));
    }

    #[test]
    fn a_room_voice_on_the_microphone_is_a_speaker_but_a_relabelled_echo_still_goes() {
        let _g = sandbox();
        let id = "00000000000000d7";
        begin(&header(id, 1)).unwrap();
        append(
            id,
            &seg(OTHERS, 0, "Let's review the September numbers first."),
        )
        .unwrap();
        append(id, &seg(ME, 100, "let's review the September numbers")).unwrap();
        append(id, &seg(ME, 10_000, "Sorry to interrupt, quick question.")).unwrap();
        append(id, &seg(ME, 20_000, "Yes, that works for me.")).unwrap();
        // The echo and the room voice were both flagged as not the user's; only the echo must go.
        let voices: HashMap<u64, String> = [
            (0, "s1".to_string()),
            (100 | voices::ROOM, "s1".to_string()),
            (10_000 | voices::ROOM, "s2".to_string()),
        ]
        .into();
        let note = finish(id, 30_000, false, true, &voices).unwrap();
        let lines: Vec<(&str, u64)> = note
            .segments
            .iter()
            .map(|s| (s.speaker.as_str(), s.start_ms))
            .collect();
        assert_eq!(lines, vec![("s1", 0), ("s2", 10_000), (ME, 20_000)]);
    }

    #[test]
    fn delete_skips_the_note_being_recorded_and_bad_ids() {
        let _g = sandbox();
        let (a, b) = ("00000000000000e5", "00000000000000e6");
        begin(&header(a, 1)).unwrap();
        finish(a, 2, false, true, &HashMap::new()).unwrap();
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
            finish(id, at + 10, false, true, &HashMap::new()).unwrap();
        }
        let rows = list();
        assert_eq!(rows[0].id, "00000000000000f2");
        assert_eq!(rows[0].first_line, "Opening line.");
    }
}
