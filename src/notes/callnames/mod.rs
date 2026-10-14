//! Names from the call app: who is in the call, read from Zoom's participant list while a note records.
//! Zoom shows no active speaker to other apps, so a 1:1 call names its one other voice and a group call offers the names.

use super::{Segment, ME, OTHERS};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Seconds between looks at the call app; a participant list changes slowly.
pub const EVERY_SECS: u64 = 5;
/// A display name longer than this is not a name (a chat line, a banner).
const MAX_NAME_CHARS: usize = 60;

/// The participant rows the call app shows right now, as its accessibility text; None when no call app list is readable.
pub fn read_rows() -> Option<Vec<String>> {
    #[cfg(target_os = "macos")]
    return macos::rows();
    #[cfg(windows)]
    return windows::rows();
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}

// Rows seen per note while it records; names stay in memory until Stop puts the others' names in the note.
static SEEN: Mutex<BTreeMap<String, BTreeSet<String>>> = Mutex::new(BTreeMap::new());

pub fn remember(note_id: &str, rows: Vec<String>) {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.entry(note_id.to_string()).or_default().extend(rows);
}

/// The rows seen during this note, forgotten here.
pub fn take(note_id: &str) -> Vec<String> {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.remove(note_id)
        .map(|s| s.into_iter().collect())
        .unwrap_or_default()
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

/// The other people in the call, from every row seen: without the user (marked "me", or named as in setup), each once.
pub fn others(rows: &[String], user_name: Option<&str>) -> Vec<String> {
    let user = user_name.map(|n| n.trim().to_lowercase());
    let mut mine: Vec<String> = Vec::new();
    let mut found: Vec<String> = Vec::new();
    for raw in rows {
        let Some((name, me)) = parse_row(raw) else {
            continue;
        };
        if me {
            mine.push(name.to_lowercase());
            continue;
        }
        if !found.iter().any(|f| f.eq_ignore_ascii_case(&name)) {
            found.push(name);
        }
    }
    found.retain(|n| {
        let low = n.to_lowercase();
        !mine.contains(&low) && user.as_deref() != Some(low.as_str())
    });
    found
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
        assert_eq!(take("n1"), rows(&["Priya", "Simon"]));
        assert!(take("n1").is_empty());
    }
}
