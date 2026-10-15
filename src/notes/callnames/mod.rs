//! Names from the call app: who is in the call and who is talking, read from Zoom's window while a note records.
//! Each other voice takes the name Zoom showed talking during its lines; a 1:1 call names its one other voice.

use super::{Segment, ME, OTHERS};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Seconds between full looks at the call app; a participant list changes slowly.
pub const EVERY_SECS: u64 = 5;
/// Zoom moves its speaker tile about a second after someone starts talking.
const LAG_MS: u64 = 1_500;
/// Seconds of a voice's lines that must show one name before that voice takes it.
const MIN_VOTES: usize = 3;
/// Share of those seconds the winning name needs; a voice Zoom showed as two people stays unnamed.
const MIN_SHARE: f64 = 0.6;

/// What one look at the call app found.
#[derive(Default, Debug)]
pub struct Look {
    /// Participant rows and video tile labels, as their accessibility text.
    pub rows: Vec<String>,
    /// The tile label of whoever the app shows talking right now.
    pub speaking: Option<String>,
    /// A meeting is open, whether or not anything in it was readable.
    pub in_meeting: bool,
}
/// A display name longer than this is not a name (a chat line, a banner).
const MAX_NAME_CHARS: usize = 60;

/// Looks at the call app now; `full` also reads the participant list, which costs more. None when no call app is readable.
pub fn look(full: bool) -> Option<Look> {
    #[cfg(target_os = "macos")]
    return macos::look(full);
    #[cfg(windows)]
    return windows::rows().map(|rows| Look {
        rows,
        speaking: None,
        in_meeting: true,
    });
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = full;
        None
    }
}

/// The active speaker from one window's tiles (label, area): a mini window's only tile, or a Speaker view's big tile.
/// Gallery tiles are all one size and show nobody.
pub fn active_tile(tiles: &[(String, f64)], main_window: bool) -> Option<String> {
    if !main_window {
        let [(label, _)] = tiles else {
            return None;
        };
        return parse_row(label).map(|(name, _)| name);
    }
    let mut sized: Vec<&(String, f64)> = tiles.iter().collect();
    sized.sort_by(|a, b| b.1.total_cmp(&a.1));
    let [(label, big), (_, next), ..] = sized.as_slice() else {
        return None;
    };
    if *big < next * 4.0 {
        return None;
    }
    parse_row(label).map(|(name, _)| name)
}

/// What one note saw of the call: rows, and who was shown talking at which millisecond of the note.
#[derive(Default, Debug)]
pub struct Seen {
    pub rows: Vec<String>,
    pub talk: Vec<(u64, String)>,
}

#[derive(Default)]
struct Heard {
    rows: BTreeSet<String>,
    talk: Vec<(u64, String)>,
}

// Per note while it records; names stay in memory until Stop puts them in the note.
static SEEN: Mutex<BTreeMap<String, Heard>> = Mutex::new(BTreeMap::new());

pub fn remember(note_id: &str, rows: Vec<String>) {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.entry(note_id.to_string())
        .or_default()
        .rows
        .extend(rows);
}

/// Who the call app showed talking `at_ms` into the note.
pub fn heard_speaking(note_id: &str, at_ms: u64, name: String) {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.entry(note_id.to_string())
        .or_default()
        .talk
        .push((at_ms, name));
}

/// Everything seen during this note, forgotten here.
pub fn take(note_id: &str) -> Seen {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(heard) = seen.remove(note_id) else {
        return Seen::default();
    };
    Seen {
        rows: heard.rows.into_iter().collect(),
        talk: heard.talk,
    }
}

// Splits on commas outside parentheses, so "Rupesh (Host, me), audio muted" keeps its tag with the name.
fn top_level_parts(raw: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in raw.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth <= 0 => {
                parts.push(&raw[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&raw[start..]);
    parts
}

/// One participant row ("Priya (Host), audio muted" or "Rupesh (Host, me)") as (display name, is the user).
/// Headers like "In the meeting (3)" and anything that does not read as a name give None.
pub fn parse_row(raw: &str) -> Option<(String, bool)> {
    let parts = top_level_parts(raw);
    let mut name = parts.first()?.trim().to_string();
    let mut tags: Vec<String> = Vec::new();
    // Trailing "(Host, me)" style tags; a bare "(3)" makes the row a section header.
    while let Some(open) = name.rfind('(') {
        if !name.ends_with(')') {
            break;
        }
        let inner = name[open + 1..name.len() - 1].trim().to_string();
        if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        tags.extend(inner.split(',').map(|t| t.trim().to_lowercase()));
        name = name[..open].trim_end().to_string();
    }
    let rest_me = parts
        .iter()
        .skip(1)
        .any(|t| t.trim().eq_ignore_ascii_case("me"));
    let me = rest_me || tags.iter().any(|t| t == "me");
    let ok = !name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.contains('\n')
        && name.chars().any(char::is_alphabetic);
    ok.then_some((name, me))
}

/// Tells the user's own names apart: rows marked "me", and the setup name as a whole name or a first name.
pub struct User {
    marked: Vec<String>,
    setup: Option<String>,
}

impl User {
    /// Nobody is the user: for a note with no call.
    pub const fn nobody() -> Self {
        User {
            marked: Vec::new(),
            setup: None,
        }
    }

    pub fn new(rows: &[String], user_name: Option<&str>) -> Self {
        let marked = rows
            .iter()
            .filter_map(|r| parse_row(r))
            .filter(|(_, me)| *me)
            .map(|(name, _)| name.to_lowercase())
            .collect();
        let setup = user_name
            .map(|n| n.trim().to_lowercase())
            .filter(|n| !n.is_empty());
        User { marked, setup }
    }

    pub fn is(&self, name: &str) -> bool {
        let low = name.trim().to_lowercase();
        if self.marked.contains(&low) {
            return true;
        }
        let Some(setup) = self.setup.as_deref() else {
            return false;
        };
        let first = low.split_whitespace().next().unwrap_or_default();
        low == setup || first == setup
    }
}

/// The other people in the call, from every row seen: without the user, each once.
pub fn others(rows: &[String], user_name: Option<&str>) -> Vec<String> {
    let user = User::new(rows, user_name);
    let mut found: Vec<String> = Vec::new();
    for raw in rows {
        let Some((name, me)) = parse_row(raw) else {
            continue;
        };
        if me || user.is(&name) {
            continue;
        }
        if !found.iter().any(|f| f.eq_ignore_ascii_case(&name)) {
            found.push(name);
        }
    }
    found
}

/// Each other-side voice takes the name the call app showed talking during most of its lines; returns how many were named.
/// A name the user already gave is kept, and a voice without a clear winner stays unnamed.
pub fn name_voices(
    segments: &[Segment],
    names: &mut BTreeMap<String, String>,
    talk: &[(u64, String)],
    user: &User,
) -> usize {
    let mut talk: Vec<&(u64, String)> = talk.iter().filter(|(_, who)| !user.is(who)).collect();
    talk.sort_by_key(|(at, _)| *at);
    let mut votes: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for seg in segments.iter().filter(|s| s.speaker != ME) {
        let from = talk.partition_point(|(at, _)| *at < seg.start_ms);
        let to = seg.end_ms + LAG_MS;
        for (at, who) in &talk[from..] {
            if *at > to {
                break;
            }
            *votes
                .entry(seg.speaker.as_str())
                .or_default()
                .entry(who.as_str())
                .or_default() += 1;
        }
    }
    let mut named = 0;
    for (key, counts) in votes {
        if names.contains_key(key) {
            continue;
        }
        let total: usize = counts.values().sum();
        let Some((who, n)) = counts
            .into_iter()
            .max_by_key(|(who, n)| (*n, std::cmp::Reverse(*who)))
        else {
            continue;
        };
        if n >= MIN_VOTES && n as f64 >= total as f64 * MIN_SHARE {
            names.insert(key.to_string(), who.to_string());
            named += 1;
        }
    }
    named
}

/// In a call with one other person, their voice takes their name; returns how many speakers were named.
/// A name the user already gave is kept.
pub fn apply(
    segments: &[Segment],
    names: &mut BTreeMap<String, String>,
    others: &[String],
) -> usize {
    let [only] = others else {
        return 0;
    };
    let mut keys: HashMap<&str, usize> = HashMap::new();
    for seg in segments.iter().filter(|s| s.speaker != ME) {
        *keys.entry(seg.speaker.as_str()).or_default() += 1;
    }
    // A room voice or a split voice means more than one other person spoke: no guessing which is which.
    if keys.len() != 1 {
        return 0;
    }
    let key = keys.keys().next().copied().unwrap_or(OTHERS).to_string();
    if names.contains_key(&key) {
        return 0;
    }
    names.insert(key, only.clone());
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(who: &str) -> Segment {
        Segment {
            start_ms: 0,
            end_ms: 1_000,
            speaker: who.into(),
            text: "words".into(),
        }
    }

    fn rows(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_row_gives_the_display_name_and_whether_it_is_the_user() {
        assert_eq!(
            parse_row("Priya Shah, audio muted, video off"),
            Some(("Priya Shah".into(), false))
        );
        assert_eq!(
            parse_row("Rupesh Biswas (Host, me), audio unmuted"),
            Some(("Rupesh Biswas".into(), true))
        );
        assert_eq!(parse_row("Simon (Co-host)"), Some(("Simon".into(), false)));
        assert_eq!(parse_row("Rupesh, me"), Some(("Rupesh".into(), true)));
    }

    #[test]
    fn headers_and_non_names_are_skipped() {
        assert_eq!(parse_row("In the meeting (3)"), None);
        assert_eq!(parse_row("Waiting room (2)"), None);
        assert_eq!(parse_row(""), None);
        assert_eq!(parse_row("12:30"), None);
        assert_eq!(parse_row(&"x".repeat(80)), None);
    }

    #[test]
    fn others_leaves_out_the_user_and_repeats() {
        let seen = rows(&[
            "In the meeting (3)",
            "Rupesh (Host, me)",
            "Simon",
            "Deepshiksha, audio muted",
            "simon, video off",
        ]);
        assert_eq!(others(&seen, None), vec!["Simon", "Deepshiksha"]);
        let unmarked = rows(&["Rupesh", "Simon"]);
        assert_eq!(
            others(&unmarked, Some("rupesh")),
            vec!["Simon"],
            "the setup name marks the user too"
        );
    }

    #[test]
    fn a_one_to_one_call_names_the_other_voice() {
        let segs = vec![seg("me"), seg("others"), seg("others")];
        let mut names = BTreeMap::new();
        assert_eq!(apply(&segs, &mut names, &["Simon".to_string()]), 1);
        assert_eq!(names.get("others").map(String::as_str), Some("Simon"));
        let split = vec![seg("me"), seg("s1"), seg("s1")];
        let mut names = BTreeMap::new();
        apply(&split, &mut names, &["Simon".to_string()]);
        assert_eq!(names.get("s1").map(String::as_str), Some("Simon"));
    }

    #[test]
    fn a_group_call_or_two_voices_names_nobody() {
        let segs = vec![seg("others")];
        let mut names = BTreeMap::new();
        assert_eq!(
            apply(
                &segs,
                &mut names,
                &["Simon".to_string(), "Priya".to_string()]
            ),
            0
        );
        let two = vec![seg("s1"), seg("s2")];
        assert_eq!(
            apply(&two, &mut names, &["Simon".to_string()]),
            0,
            "one participant but two voices: someone shares a room"
        );
        assert!(names.is_empty());
    }

    #[test]
    fn a_name_the_user_gave_is_kept() {
        let segs = vec![seg("others")];
        let mut names: BTreeMap<String, String> = [("others".to_string(), "Simon K.".to_string())]
            .into_iter()
            .collect();
        assert_eq!(apply(&segs, &mut names, &["Simon".to_string()]), 0);
        assert_eq!(names.get("others").map(String::as_str), Some("Simon K."));
    }

    #[test]
    fn rows_are_kept_per_note_until_taken() {
        remember("n1", rows(&["Simon"]));
        remember("n1", rows(&["Simon", "Priya"]));
        heard_speaking("n1", 2_000, "Priya".into());
        let seen = take("n1");
        assert_eq!(seen.rows, rows(&["Priya", "Simon"]));
        assert_eq!(seen.talk, vec![(2_000, "Priya".to_string())]);
        assert!(take("n1").rows.is_empty());
    }

    fn line(who: &str, start_ms: u64, end_ms: u64) -> Segment {
        Segment {
            start_ms,
            end_ms,
            speaker: who.into(),
            text: "words".into(),
        }
    }

    fn every_second(from_s: u64, to_s: u64, who: &str) -> Vec<(u64, String)> {
        (from_s..to_s)
            .map(|s| (s * 1_000, who.to_string()))
            .collect()
    }

    #[test]
    fn the_mini_window_or_speaker_view_shows_who_talks_and_gallery_shows_nobody() {
        let one = vec![(
            "Rup 2, Computer audio unmuted, Video off".to_string(),
            50_000.0,
        )];
        assert_eq!(active_tile(&one, false), Some("Rup 2".into()));
        let speaker_view = vec![
            (
                "Rup 2, Computer audio unmuted, Video off".to_string(),
                1024.0 * 576.0,
            ),
            (
                "Rupesh Biswas, Computer audio muted, Video off".to_string(),
                160.0 * 90.0,
            ),
        ];
        assert_eq!(active_tile(&speaker_view, true), Some("Rup 2".into()));
        let gallery = vec![
            (
                "Rupesh Biswas, Computer audio muted".to_string(),
                656.0 * 369.0,
            ),
            ("Rup 2, Computer audio unmuted".to_string(), 656.0 * 369.0),
        ];
        assert_eq!(active_tile(&gallery, true), None);
        assert_eq!(
            active_tile(&one, true),
            None,
            "one tile in the meeting window is just you"
        );
    }

    #[test]
    fn each_voice_takes_the_name_shown_while_it_talked() {
        let segs = vec![
            line("s1", 0, 5_000),
            line("s2", 10_000, 16_000),
            line("me", 20_000, 25_000),
            line("s1", 30_000, 34_000),
        ];
        let mut talk = every_second(0, 6, "Deepshikha");
        talk.extend(every_second(10, 17, "Gaurav"));
        talk.extend(every_second(20, 26, "Rupesh Biswas"));
        talk.extend(every_second(30, 35, "Deepshikha"));
        let user = User::new(&[], Some("Rupesh"));
        let mut names = BTreeMap::new();
        assert_eq!(name_voices(&segs, &mut names, &talk, &user), 2);
        assert_eq!(names.get("s1").map(String::as_str), Some("Deepshikha"));
        assert_eq!(names.get("s2").map(String::as_str), Some("Gaurav"));
    }

    #[test]
    fn a_voice_without_a_clear_winner_or_enough_seconds_stays_unnamed() {
        let segs = vec![line("s1", 0, 10_000), line("s2", 20_000, 21_000)];
        let mut talk = every_second(0, 5, "Ana");
        talk.extend(every_second(5, 10, "Ben"));
        talk.push((20_000, "Ben".into()));
        let mut names = BTreeMap::new();
        assert_eq!(
            name_voices(&segs, &mut names, &talk, &User::new(&[], None)),
            0
        );
        assert!(names.is_empty());
    }

    #[test]
    fn a_name_the_user_gave_beats_the_speaker_log() {
        let segs = vec![line("s1", 0, 5_000)];
        let mut names: BTreeMap<String, String> = [("s1".to_string(), "Priya S.".to_string())]
            .into_iter()
            .collect();
        let talk = every_second(0, 5, "Priya Shah");
        assert_eq!(
            name_voices(&segs, &mut names, &talk, &User::new(&[], None)),
            0
        );
        assert_eq!(names.get("s1").map(String::as_str), Some("Priya S."));
    }

    #[test]
    fn the_user_is_known_by_the_me_tag_or_the_setup_first_name() {
        let seen = rows(&["Rupesh Biswas (Host, me)", "Simon"]);
        assert!(User::new(&seen, None).is("rupesh biswas"));
        let tiles = rows(&["Rupesh Biswas, Computer audio muted", "Rup 2, Video off"]);
        let user = User::new(&tiles, Some("Rupesh"));
        assert!(user.is("Rupesh Biswas"));
        assert!(!user.is("Rup 2"));
        assert_eq!(others(&tiles, Some("Rupesh")), vec!["Rup 2"]);
    }
}
