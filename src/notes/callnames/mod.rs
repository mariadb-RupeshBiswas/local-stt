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

/// A call app whose page marks the talking tile with a class, read through its web accessibility tree.
/// The class names are the ones the apps ship today; they change when the apps do.
pub struct WebCall {
    pub source: &'static str,
    /// Process names that can host it.
    pub processes: &'static [&'static str],
    /// A window title start that shows the call is in this window; empty reads every window.
    pub title_prefix: &'static str,
    /// Class on each participant tile; empty when tiles have no class and the marker's ancestors carry the name.
    pub tile: &'static str,
    /// Class present inside the tile of whoever is talking.
    pub marker: &'static str,
}

pub const WEB_CALLS: [WebCall; 3] = [
    WebCall {
        source: "Google Meet",
        processes: &["Google Chrome", "Microsoft Edge", "Brave Browser", "Arc"],
        title_prefix: "Meet - ",
        tile: "oZRSLe",
        marker: "kssMZb",
    },
    WebCall {
        source: "Microsoft Teams",
        processes: &["MSTeams", "Microsoft Teams"],
        title_prefix: "",
        tile: "",
        marker: "vdi-frame-occlusion",
    },
    WebCall {
        source: "Slack",
        processes: &["Slack"],
        title_prefix: "",
        tile: "p-huddle_peer_tile",
        marker: "p-huddle_peer_tile__overlay--active_speaker",
    },
];

/// A tile or roster label as a person's name: Teams' role suffix goes, and "+3 more" or "You" are not names.
pub fn web_name(label: &str) -> Option<String> {
    let mut name = label.trim();
    for role in [
        "Organizer",
        "Co-organizer",
        "Meeting guest",
        "Presenter",
        "Attendee",
    ] {
        if let Some(at) = name.find(&format!(", {role}")) {
            name = &name[..at];
        }
    }
    let lower = name.to_lowercase();
    let counted = lower.starts_with(|c: char| c == '+' || c.is_ascii_digit());
    let last = lower.rsplit(' ').next().unwrap_or_default();
    let overflow =
        counted && ["more", "other", "others", "participant", "participants"].contains(&last);
    if overflow || lower == "you" {
        return None;
    }
    parse_row(name).map(|(n, _)| n)
}

/// What one look at the call app found.
#[derive(Default, Debug)]
pub struct Look {
    /// Participant rows and video tile labels, as their accessibility text.
    pub rows: Vec<String>,
    /// The tile label of whoever the app shows talking right now.
    pub speaking: Option<String>,
    /// A meeting is open, whether or not anything in it was readable.
    pub in_meeting: bool,
    /// A web call's tiles were found, so the next looks read its page every second too.
    pub web_live: bool,
    /// The call app that showed anything ("Zoom", "Google Meet").
    pub source: Option<&'static str>,
}
/// A display name longer than this is not a name (a chat line, a banner).
const MAX_NAME_CHARS: usize = 60;

/// Looks at the call apps now; `full` also reads participant lists, `web` also reads web calls' pages, both costing more.
/// None when no call app is readable.
pub fn look(full: bool, web: bool, border: bool) -> Option<Look> {
    #[cfg(target_os = "macos")]
    return macos::look(full, web, border);
    #[cfg(windows)]
    return windows::look(full, web, border);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (full, web, border);
        None
    }
}

/// Window titles of running apps with these process names; empty where this is not read.
pub fn window_titles(processes: &[&str]) -> Vec<String> {
    #[cfg(target_os = "macos")]
    return macos::window_titles(processes);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = processes;
        Vec::new()
    }
}

/// The active speaker from one window's tiles (label, area): a mini window's only tile, or a Speaker view's big tile.
/// Gallery tiles are all one size and show nobody.
pub fn active_tile(tiles: &[(String, f64)], main_window: bool) -> Option<String> {
    // Zoom can name the talker in the tile's own label ("Priya, active speaker, ..."), in any view.
    let marked = tiles
        .iter()
        .find(|(label, _)| label.to_lowercase().contains("active speaker"));
    if let Some((label, _)) = marked {
        return parse_row(label).map(|(name, _)| name);
    }
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

/// Speaker view with its thumbnails floated out: the meeting window's one big tile is the talker,
/// when the floating strip shows that person too (a lone tile with no strip is just you).
pub fn strip_talker(main: &[(String, f64)], strip: &[(String, f64)]) -> Option<String> {
    let [(label, big)] = main else {
        return None;
    };
    let name = parse_row(label)?.0;
    let in_strip = strip.iter().any(|(other, area)| {
        *big >= area * 4.0 && parse_row(other).is_some_and(|(n, _)| n == name)
    });
    in_strip.then_some(name)
}

/// Zoom's speaking border: a bright green, far greener than skin, video or the dark tile background.
pub fn border_green(r: u8, g: u8, b: u8) -> bool {
    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
    g > 128 && g - r > 64 && g - b > 50
}

/// The talker from each tile's share of border-green pixels in a thin band around it:
/// exactly one tile clearly lit while every other tile stays dark, else nobody.
pub fn border_talker(shares: &[(String, f64)]) -> Option<String> {
    let lit: Vec<&(String, f64)> = shares.iter().filter(|(_, s)| *s >= 0.15).collect();
    let [(label, _)] = lit.as_slice() else {
        return None;
    };
    let dark = shares.iter().all(|(other, s)| other == label || *s < 0.03);
    if !dark {
        return None;
    }
    parse_row(label).map(|(name, _)| name)
}

/// Asks the OS to allow reading the call app's pixels, for the speaking border; true when already allowed.
pub fn ask_screen_access() -> bool {
    #[cfg(target_os = "macos")]
    return macos::ask_screen_access();
    #[cfg(not(target_os = "macos"))]
    true
}

/// The name in a mini window's "Talking: Priya" line.
pub fn talking_label(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix("Talking:")?;
    parse_row(rest).map(|(name, _)| name)
}

/// What one note saw of the call: rows, and who was shown talking at which millisecond of the note.
#[derive(Default, Debug)]
pub struct Seen {
    pub rows: Vec<String>,
    pub talk: Vec<(u64, String)>,
    pub source: Option<&'static str>,
}

#[derive(Default)]
struct Heard {
    rows: BTreeSet<String>,
    talk: Vec<(u64, String)>,
    source: Option<&'static str>,
}

// Per note while it records; names stay in memory until Stop puts them in the note.
static SEEN: Mutex<BTreeMap<String, Heard>> = Mutex::new(BTreeMap::new());

pub fn remember(note_id: &str, rows: Vec<String>, source: Option<&'static str>) {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let heard = seen.entry(note_id.to_string()).or_default();
    if !rows.is_empty() {
        heard.source = heard.source.or(source);
    }
    heard.rows.extend(rows);
}

/// Who the call app showed talking `at_ms` into the note.
pub fn heard_speaking(note_id: &str, at_ms: u64, name: String) {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.entry(note_id.to_string())
        .or_default()
        .talk
        .push((at_ms, name));
}

/// Who the call app showed talking during most of a line, while the note still records; None without a clear winner.
pub fn live_name(
    note_id: &str,
    start_ms: u64,
    end_ms: u64,
    user_name: Option<&str>,
) -> Option<String> {
    let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let heard = seen.get(note_id)?;
    let rows: Vec<String> = heard.rows.iter().cloned().collect();
    let user = User::new(&rows, user_name);
    let window = start_ms..=end_ms + LAG_MS;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (at, who) in &heard.talk {
        if window.contains(at) && !user.is(who) {
            *counts.entry(who.as_str()).or_default() += 1;
        }
    }
    let total: usize = counts.values().sum();
    let (who, n) = counts
        .into_iter()
        .max_by_key(|(who, n)| (*n, std::cmp::Reverse(*who)))?;
    (n as f64 >= total as f64 * MIN_SHARE).then(|| who.to_string())
}

/// How many other people the call app has shown during this note so far; 0 when none were read.
pub fn head_count(note_id: &str, user_name: Option<&str>) -> usize {
    let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(heard) = seen.get(note_id) else {
        return 0;
    };
    let rows: Vec<String> = heard.rows.iter().cloned().collect();
    others(&rows, user_name).len()
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
        source: heard.source,
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

// Words that put the next name in the vocative: "Hi Priya", "Thanks, Priya", "over to you Priya".
const CUE_WORDS: [&str; 6] = ["hi", "hello", "hey", "thanks", "you", "morning"];
// A reply this long after the line that named someone still counts as theirs.
const REPLY_WITHIN_MS: u64 = 20_000;
// Votes a voice needs from the conversation before it takes a name.
const MIN_MENTIONS: usize = 2;

// Where `name` is spoken to in `line` ("Hello, Gaurav." / "Gaurav, can you" / "what about you, Gaurav?").
fn addressed(line: &str, name: &str) -> bool {
    let line = line.to_lowercase();
    let name = name.to_lowercase();
    let mut from = 0;
    while let Some(at) = line[from..].find(&name) {
        let start = from + at;
        let end = start + name.len();
        from = end;
        let before = line[..start].trim_end();
        let after = line[end..].chars().next();
        let whole_word = !line[..start].ends_with(char::is_alphanumeric)
            && !after.is_some_and(char::is_alphanumeric);
        if !whole_word {
            continue;
        }
        let asked = after == Some('?');
        let set_off = before.is_empty()
            || before.ends_with(',')
            || CUE_WORDS.iter().any(|w| before.ends_with(w));
        let closes = matches!(after, None | Some(',' | '.' | '!' | '?'));
        if asked || (set_off && closes) {
            return true;
        }
    }
    false
}

// Where a voice names itself: "this is Priya", "I'm Priya", "my name is Priya".
fn introduces(line: &str, name: &str) -> bool {
    let line = line.to_lowercase();
    let name = name.to_lowercase();
    ["this is ", "i'm ", "i am ", "my name is "]
        .iter()
        .any(|lead| line.contains(&format!("{lead}{name}")))
}

/// Names voices from the conversation: the voice that answers right after someone is spoken to by name,
/// or that introduces itself, takes that name once it has two such cues and most of its cues agree.
/// Only names the call app listed count, so a person merely talked about is never assigned.
pub fn name_by_mentions(
    segments: &[Segment],
    names: &mut BTreeMap<String, String>,
    others: &[String],
) -> usize {
    let mut votes: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for (i, seg) in segments.iter().enumerate() {
        for person in others {
            let first = person.split_whitespace().next().unwrap_or(person);
            if seg.speaker != ME && (introduces(&seg.text, first) || introduces(&seg.text, person))
            {
                *votes
                    .entry(&seg.speaker)
                    .or_default()
                    .entry(person)
                    .or_default() += 2;
            }
            if !(addressed(&seg.text, first) || addressed(&seg.text, person)) {
                continue;
            }
            let reply = segments[i + 1..]
                .iter()
                .find(|next| next.speaker != seg.speaker && next.speaker != ME);
            let Some(reply) = reply else {
                continue;
            };
            if reply.start_ms <= seg.end_ms + REPLY_WITHIN_MS {
                *votes
                    .entry(&reply.speaker)
                    .or_default()
                    .entry(person)
                    .or_default() += 1;
            }
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
        let fresh = !names.values().any(|t| t == who);
        if fresh && n >= MIN_MENTIONS && n as f64 >= total as f64 * MIN_SHARE {
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

    fn said(who: &str, start_s: u64, text: &str) -> Segment {
        Segment {
            start_ms: start_s * 1_000,
            end_ms: start_s * 1_000 + 2_000,
            speaker: who.into(),
            text: text.into(),
        }
    }

    #[test]
    fn spoken_to_by_name_means_the_next_voice_is_that_person() {
        assert!(addressed("Hello, Gaurav. Is everything alright?", "Gaurav"));
        assert!(addressed("Are you watching Gaurav?", "gaurav"));
        assert!(addressed("Gaurav, can you share?", "Gaurav"));
        assert!(addressed("Thanks Priya", "Priya"));
        assert!(!addressed("Simon said the deck is late.", "Simon"));
        assert!(!addressed("I asked Rupesh ji to check", "Rupesh"));
        assert!(!addressed("Priyanka will join later.", "Priya"));
        assert!(introduces("Hi all, this is Priya from finance", "Priya"));
    }

    #[test]
    fn two_cues_name_a_voice_and_a_talked_about_person_is_never_assigned() {
        let others = vec!["Gaurav Kumar".to_string(), "Deepshikha".to_string()];
        let segs = vec![
            said("s2", 0, "Hello, Gaurav. Is everything alright at home?"),
            said("s3", 3, "Yes, all good."),
            said("s2", 10, "Simon said we need five filters."),
            said("s1", 13, "That makes no sense."),
            said("me", 20, "What do you think, Gaurav?"),
            said("s3", 23, "I think four is enough."),
        ];
        let mut names = BTreeMap::new();
        assert_eq!(name_by_mentions(&segs, &mut names, &others), 1);
        assert_eq!(names.get("s3").map(String::as_str), Some("Gaurav Kumar"));
        assert!(!names.contains_key("s1"), "Simon is not in the call");
    }

    #[test]
    fn one_cue_or_a_name_already_given_is_not_enough() {
        let others = vec!["Priya".to_string()];
        let segs = vec![said("me", 0, "Thanks, Priya."), said("s1", 3, "Sure.")];
        let mut names = BTreeMap::new();
        assert_eq!(name_by_mentions(&segs, &mut names, &others), 0);
        let twice = vec![
            said("me", 0, "Thanks, Priya."),
            said("s1", 3, "Sure."),
            said("me", 10, "Priya, your turn."),
            said("s1", 12, "Okay."),
        ];
        let mut names: BTreeMap<String, String> = [("s2".to_string(), "Priya".to_string())]
            .into_iter()
            .collect();
        assert_eq!(
            name_by_mentions(&twice, &mut names, &others),
            0,
            "Priya already names another voice"
        );
    }

    #[test]
    fn a_floated_strip_makes_the_lone_big_tile_the_talker() {
        let tile = |name: &str, w: f64, h: f64| (format!("{name}, Computer audio unmuted"), w * h);
        let big = vec![tile("Simon K", 1360.0, 765.0)];
        let strip = vec![
            tile("Naman S", 240.0, 135.0),
            tile("Simon K", 240.0, 135.0),
            tile("Adam L", 240.0, 135.0),
        ];
        assert_eq!(strip_talker(&big, &strip), Some("Simon K".into()));
        assert_eq!(strip_talker(&big, &[]), None, "alone in the meeting");
        let elsewhere = vec![tile("Naman S", 240.0, 135.0)];
        assert_eq!(
            strip_talker(&big, &elsewhere),
            None,
            "big tile not in the strip"
        );
        let two = vec![big[0].clone(), tile("Adam L", 1360.0, 765.0)];
        assert_eq!(strip_talker(&two, &strip), None);
        let same_size = vec![tile("Simon K", 240.0, 135.0)];
        assert_eq!(strip_talker(&same_size, &strip), None);
    }

    #[test]
    fn the_one_green_bordered_tile_is_the_talker() {
        assert!(border_green(72, 205, 70), "Zoom's border green");
        assert!(!border_green(230, 190, 160), "skin");
        assert!(!border_green(28, 28, 30), "tile background");
        let tile =
            |name: &str, share: f64| (format!("{name}, Computer audio unmuted, Video off"), share);
        assert_eq!(
            border_talker(&[
                tile("R2", 0.0),
                tile("Rupesh Biswas", 0.0),
                tile("R3", 0.39)
            ]),
            Some("R3".into())
        );
        assert_eq!(
            border_talker(&[tile("R2", 0.0), tile("R3", 0.01)]),
            None,
            "nobody lit"
        );
        assert_eq!(
            border_talker(&[tile("R2", 0.30), tile("R3", 0.25)]),
            None,
            "two lit tiles are a green background, not a border"
        );
        assert_eq!(
            border_talker(&[tile("R2", 0.30), tile("R3", 0.08)]),
            None,
            "green spilling into another tile is not trusted"
        );
    }

    #[test]
    fn web_labels_lose_role_suffixes_and_overflow_tiles_are_not_names() {
        assert_eq!(web_name("Priya Shah, Organizer"), Some("Priya Shah".into()));
        assert_eq!(
            web_name("Simon, Meeting guest, external"),
            Some("Simon".into())
        );
        assert_eq!(web_name("+3 more"), None);
        assert_eq!(web_name("12 participants"), None);
        assert_eq!(web_name("You"), None);
        assert_eq!(web_name("Gaurav"), Some("Gaurav".into()));
    }

    #[test]
    fn rows_are_kept_per_note_until_taken() {
        remember("n1", rows(&["Simon"]), Some("Zoom"));
        remember("n1", rows(&["Simon", "Priya"]), None);
        heard_speaking("n1", 2_000, "Priya".into());
        let seen = take("n1");
        assert_eq!(seen.rows, rows(&["Priya", "Simon"]));
        assert_eq!(seen.talk, vec![(2_000, "Priya".to_string())]);
        assert_eq!(seen.source, Some("Zoom"));
        assert!(take("n1").rows.is_empty());
    }

    #[test]
    fn a_live_line_takes_the_name_shown_while_it_was_said() {
        remember(
            "n2",
            rows(&[
                "Rupesh Biswas, Computer audio unmuted",
                "Sam, Computer audio unmuted",
            ]),
            Some("Zoom"),
        );
        for s in 0..4 {
            heard_speaking("n2", s * 1_000, "Sam".into());
        }
        heard_speaking("n2", 9_000, "Rupesh Biswas".into());
        assert_eq!(
            live_name("n2", 0, 3_000, Some("Rupesh")),
            Some("Sam".into())
        );
        assert_eq!(
            live_name("n2", 8_500, 9_500, Some("Rupesh")),
            None,
            "the user is never another voice"
        );
        assert_eq!(
            live_name("n2", 20_000, 22_000, Some("Rupesh")),
            None,
            "nobody shown then"
        );
        assert_eq!(live_name("missing", 0, 1_000, None), None);
        take("n2");
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
        let marked = vec![
            (
                "Rupesh Biswas, Computer audio muted".to_string(),
                656.0 * 369.0,
            ),
            (
                "Priya, active speaker, Computer audio unmuted".to_string(),
                656.0 * 369.0,
            ),
        ];
        assert_eq!(
            active_tile(&marked, true),
            Some("Priya".into()),
            "a gallery tile Zoom marks"
        );
        assert_eq!(
            talking_label("Talking: Gaurav Kumar"),
            Some("Gaurav Kumar".into())
        );
        assert_eq!(talking_label("Recording"), None);
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
