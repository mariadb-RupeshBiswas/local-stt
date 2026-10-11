//! Insights: streak, totals, speed and word habits, kept as aggregates only (never sentences).

pub mod streak;
pub mod words;

use crate::localtime::{Day, LocalNow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use streak::{Streak, WeekState};
use words::Tag;

const KEEP_DAYS: i64 = 400;
const KEEP_WPM: usize = 100;
const TYPING_WPM: u32 = 40;
const MILESTONES: [u64; 6] = [1_000, 10_000, 50_000, 100_000, 250_000, 1_000_000];
const HOURS_WINDOW_DAYS: i64 = 30;
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

// Recording runs on the worker while the UI reads and resets; one lock keeps load-modify-save whole.
static FILE: Mutex<()> = Mutex::new(());

fn file_lock() -> std::sync::MutexGuard<'static, ()> {
    FILE.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Row {
    words: u64,
    ms: u64,
    dictations: u64,
    hours: [u64; 24],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Folded {
    words: u64,
    ms: u64,
    dictations: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WordStat {
    count: u64,
    tag: Tag,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct PosTotals {
    noun: u64,
    verb: u64,
    adjective: u64,
    adverb: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Reached {
    words: u64,
    on: Day,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    rows: BTreeMap<Day, Row>,
    folded: Folded,
    streak: Streak,
    wpm: Vec<f64>,
    milestones: Vec<Reached>,
    words: BTreeMap<String, WordStat>,
    pos: PosTotals,
}

fn path() -> PathBuf {
    crate::paths::data_dir().join("insights.json")
}

// A missing or unreadable file starts fresh; insights are never worth blocking a dictation.
fn load() -> Store {
    std::fs::read(path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(store: &Store) {
    let file = path();
    if let Some(dir) = file.parent() {
        let _ = crate::paths::private_dir(dir);
    }
    if let Ok(bytes) = serde_json::to_vec(store) {
        let _ = crate::history::replace(&file, &bytes);
    }
}

impl Store {
    fn active(&self, day: Day) -> bool {
        self.rows.get(&day).is_some_and(|r| r.dictations > 0)
    }

    fn words_all(&self) -> u64 {
        self.folded.words + self.rows.values().map(|r| r.words).sum::<u64>()
    }

    fn dictations_all(&self) -> u64 {
        self.folded.dictations + self.rows.values().map(|r| r.dictations).sum::<u64>()
    }

    fn ms_all(&self) -> u64 {
        self.folded.ms + self.rows.values().map(|r| r.ms).sum::<u64>()
    }

    fn evaluate(&mut self, today: Day) {
        let rows = &self.rows;
        streak::advance(
            &mut self.streak,
            &|d| rows.get(&d).is_some_and(|r| r.dictations > 0),
            today,
        );
    }

    // Rows past the window fold into all-time totals so the file stays small.
    fn fold_old_rows(&mut self, today: Day) {
        let old: Vec<Day> = self
            .rows
            .keys()
            .copied()
            .take_while(|d| today.days_since(*d) > KEEP_DAYS)
            .collect();
        for day in old {
            if let Some(row) = self.rows.remove(&day) {
                self.folded.words += row.words;
                self.folded.ms += row.ms;
                self.folded.dictations += row.dictations;
            }
        }
    }

    fn update_milestones(&mut self, today: Day) {
        let total = self.words_all();
        for goal in MILESTONES {
            let seen = self.milestones.iter().any(|m| m.words == goal);
            if total >= goal && !seen {
                self.milestones.push(Reached {
                    words: goal,
                    on: today,
                });
            }
        }
    }

    fn count_words(&mut self, text: &str) {
        for (word, tag) in words::extract(text) {
            let stat = self.words.entry(word).or_insert(WordStat { count: 0, tag });
            stat.count += 1;
            stat.tag = tag;
            match tag {
                Tag::Noun => self.pos.noun += 1,
                Tag::Verb => self.pos.verb += 1,
                Tag::Adjective => self.pos.adjective += 1,
                Tag::Adverb => self.pos.adverb += 1,
                Tag::Other => {}
            }
        }
        // ponytail: a new word at the cap starts at 1 and is the first to go; fine for a top list.
        if self.words.len() > words::MAX_WORDS {
            let mut ranked: Vec<(u64, String)> = self
                .words
                .iter()
                .map(|(w, s)| (s.count, w.clone()))
                .collect();
            ranked.sort();
            let excess = self.words.len() - words::MAX_WORDS;
            for (_, word) in ranked.into_iter().take(excess) {
                self.words.remove(&word);
            }
        }
    }
}

/// Adds one dictation. Errors are swallowed: insights never block or break a dictation.
pub fn record(now: &LocalNow, text: &str, duration_ms: u64, count_words: bool) {
    let word_total = words::count(text);
    if word_total == 0 {
        return;
    }
    let _guard = file_lock();
    let mut store = load();
    let today = now.date;
    store.evaluate(today);
    if store.streak.first_day.is_none() {
        store.streak.first_day = Some(today);
    }
    let row = store.rows.entry(today).or_default();
    row.words += word_total;
    row.ms += duration_ms;
    row.dictations += 1;
    row.hours[usize::from(now.hour.min(23))] += word_total;
    if duration_ms >= 1_000 {
        let minutes = duration_ms as f64 / 60_000.0;
        store.wpm.push(word_total as f64 / minutes);
        let extra = store.wpm.len().saturating_sub(KEEP_WPM);
        store.wpm.drain(..extra);
    }
    store.update_milestones(today);
    if count_words {
        store.count_words(text);
    }
    store.fold_old_rows(today);
    save(&store);
}

/// Deletes everything Insights knows.
pub fn reset() {
    let _guard = file_lock();
    let _ = std::fs::remove_file(path());
}

/// Drops the word counts and part-of-speech totals, keeping the rest.
pub fn clear_words() {
    let _guard = file_lock();
    if !path().exists() {
        return;
    }
    let mut store = load();
    store.words.clear();
    store.pos = PosTotals::default();
    save(&store);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub empty: bool,
    pub today: Day,
    pub streak: StreakView,
    pub week: Vec<WeekDay>,
    pub totals: Totals,
    pub speed: Speed,
    pub hours: [u64; 24],
    pub words: Option<WordsView>,
    pub milestones: Vec<MilestoneView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreakView {
    pub current: u32,
    pub longest: u32,
    pub freezes: u32,
    pub freeze_cap: u32,
    pub next_freeze_in: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekDay {
    pub date: Day,
    pub weekday: &'static str,
    pub state: WeekState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub words_today: u64,
    pub words_week: u64,
    pub words_all: u64,
    pub dictations_all: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    pub wpm: Option<u32>,
    pub typing_wpm: u32,
    pub minutes_saved_week: Option<u64>,
    pub minutes_saved_all: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordCount {
    pub word: String,
    pub count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    pub noun: f64,
    pub verb: f64,
    pub adjective: f64,
    pub adverb: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordsView {
    pub top: Vec<WordCount>,
    pub nouns: Vec<WordCount>,
    pub verbs: Vec<WordCount>,
    pub mix: Mix,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneView {
    pub words: u64,
    pub reached_on: Option<Day>,
}

/// The tab's data. Also settles missed days, so freezes spend themselves without a new dictation.
pub fn view(now: &LocalNow, word_insights: bool) -> View {
    let _guard = file_lock();
    let mut store = load();
    let before = store.streak.clone();
    if store.streak.first_day.is_some() {
        store.evaluate(now.date);
    }
    if store.streak != before {
        save(&store);
    }
    build_view(&store, now.date, word_insights)
}

// Typing minutes avoided, less the minutes spent speaking; never negative.
fn minutes_saved(words: u64, ms: u64) -> u64 {
    let typing = words as f64 / f64::from(TYPING_WPM);
    let speaking = ms as f64 / 60_000.0;
    (typing - speaking).max(0.0).round() as u64
}

fn top_words(store: &Store, tag: Option<Tag>, limit: usize) -> Vec<WordCount> {
    let mut list: Vec<WordCount> = store
        .words
        .iter()
        .filter(|(_, s)| tag.is_none_or(|t| s.tag == t))
        .map(|(w, s)| WordCount {
            word: w.clone(),
            count: s.count,
        })
        .collect();
    list.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));
    list.truncate(limit);
    list
}

fn words_view(store: &Store) -> Option<WordsView> {
    let p = &store.pos;
    let total = (p.noun + p.verb + p.adjective + p.adverb) as f64;
    if store.words.is_empty() || total == 0.0 {
        return None;
    }
    Some(WordsView {
        top: top_words(store, None, 15),
        nouns: top_words(store, Some(Tag::Noun), 10),
        verbs: top_words(store, Some(Tag::Verb), 10),
        mix: Mix {
            noun: p.noun as f64 / total,
            verb: p.verb as f64 / total,
            adjective: p.adjective as f64 / total,
            adverb: p.adverb as f64 / total,
        },
    })
}

fn build_view(store: &Store, today: Day, word_insights: bool) -> View {
    let empty = store.dictations_all() == 0;
    let shown = streak::with_today(&store.streak, store.active(today), today);
    let week = streak::week(&store.streak, &|d| store.active(d), today);
    let (monday, sunday) = (week[0].0, week[6].0);
    let words_in = |from: Day, to: Day| -> (u64, u64) {
        store
            .rows
            .range(from..=to)
            .fold((0, 0), |(w, ms), (_, r)| (w + r.words, ms + r.ms))
    };
    let (words_week, ms_week) = words_in(monday, sunday);
    let (words_today, _) = words_in(today, today);
    let mut hours = [0u64; 24];
    let window_start = today.plus_days(1 - HOURS_WINDOW_DAYS);
    for (_, row) in store.rows.range(window_start..=today) {
        for (total, add) in hours.iter_mut().zip(row.hours) {
            *total += add;
        }
    }
    let wpm = (!store.wpm.is_empty())
        .then(|| (store.wpm.iter().sum::<f64>() / store.wpm.len() as f64).round() as u32);
    let next_freeze_in = (shown.freezes < streak::FREEZE_CAP)
        .then(|| streak::WORKDAYS_PER_FREEZE - shown.active_workdays % streak::WORKDAYS_PER_FREEZE);
    View {
        empty,
        today,
        streak: StreakView {
            current: shown.current,
            longest: shown.longest,
            freezes: shown.freezes,
            freeze_cap: streak::FREEZE_CAP,
            next_freeze_in,
        },
        week: week
            .iter()
            .map(|(date, state)| WeekDay {
                date: *date,
                weekday: WEEKDAYS[usize::from(date.weekday())],
                state: *state,
            })
            .collect(),
        totals: Totals {
            words_today,
            words_week,
            words_all: store.words_all(),
            dictations_all: store.dictations_all(),
        },
        speed: Speed {
            wpm,
            typing_wpm: TYPING_WPM,
            minutes_saved_week: (!empty).then(|| minutes_saved(words_week, ms_week)),
            minutes_saved_all: (!empty).then(|| minutes_saved(store.words_all(), store.ms_all())),
        },
        hours,
        words: if word_insights {
            words_view(store)
        } else {
            None
        },
        milestones: MILESTONES
            .iter()
            .map(|goal| MilestoneView {
                words: *goal,
                reached_on: store
                    .milestones
                    .iter()
                    .find(|m| m.words == *goal)
                    .map(|m| m.on),
            })
            .collect(),
    }
}

/// Writes believable sample data for the demo: three weeks of workdays, a 6-day streak, 4 freezes.
pub fn seed_demo(now: &LocalNow) {
    let _guard = file_lock();
    save(&demo_store(now.date));
}

fn demo_store(today: Day) -> Store {
    let mut store = Store::default();
    // Workdays before today, newest first; the newest six are the streak, the seventh is frozen.
    let mut workdays = Vec::new();
    let mut day = today.prev();
    while today.days_since(day) <= 21 {
        if day.is_workday() {
            workdays.push(day);
        }
        day = day.prev();
    }
    let frozen = workdays.get(6).copied();
    for (i, day) in workdays.iter().enumerate() {
        if Some(*day) == frozen {
            continue;
        }
        let words = 380 + (i as u64 * 97) % 520;
        let mut row = Row {
            words,
            ms: words * 1_000 * 60 / 128,
            dictations: 6 + (i as u64 % 5),
            ..Row::default()
        };
        // Spread over the working day, heaviest mid-morning and mid-afternoon.
        for (hour, share) in [
            (8, 4),
            (9, 12),
            (10, 20),
            (11, 14),
            (13, 8),
            (14, 16),
            (15, 14),
            (16, 8),
            (17, 4),
        ] {
            row.hours[hour] = words * share / 100;
        }
        store.rows.insert(*day, row);
    }
    store.streak = Streak {
        current: 6,
        longest: 12,
        freezes: 4,
        active_workdays: 47,
        last_evaluated: Some(today.prev()),
        first_day: workdays.last().copied(),
        frozen: frozen.into_iter().collect(),
    };
    store.folded = Folded {
        words: 5_800,
        ms: 5_800 * 1_000 * 60 / 125,
        dictations: 190,
    };
    store.wpm = (0..KEEP_WPM)
        .map(|i| 112.0 + ((i * 7) % 34) as f64)
        .collect();
    let total = store.words_all();
    for (goal, ago) in [(1_000, 40), (10_000, 3)] {
        if total >= goal {
            store.milestones.push(Reached {
                words: goal,
                on: today.plus_days(-ago),
            });
        }
    }
    let sample: [(&str, Tag, u64); 24] = [
        ("pricing", Tag::Noun, 42),
        ("meeting", Tag::Noun, 37),
        ("release", Tag::Noun, 31),
        ("roadmap", Tag::Noun, 26),
        ("customer", Tag::Noun, 24),
        ("budget", Tag::Noun, 19),
        ("feedback", Tag::Noun, 15),
        ("ship", Tag::Verb, 29),
        ("review", Tag::Verb, 27),
        ("schedule", Tag::Verb, 21),
        ("update", Tag::Verb, 18),
        ("discuss", Tag::Verb, 14),
        ("share", Tag::Verb, 12),
        ("draft", Tag::Verb, 10),
        ("quickly", Tag::Adverb, 16),
        ("probably", Tag::Adverb, 11),
        ("simple", Tag::Adjective, 20),
        ("important", Tag::Adjective, 17),
        ("clear", Tag::Adjective, 13),
        ("useful", Tag::Adjective, 9),
        ("project", Tag::Noun, 33),
        ("deadline", Tag::Noun, 12),
        ("email", Tag::Noun, 22),
        ("finish", Tag::Verb, 8),
    ];
    for (word, tag, count) in sample {
        store
            .words
            .insert(word.to_string(), WordStat { count, tag });
        match tag {
            Tag::Noun => store.pos.noun += count,
            Tag::Verb => store.pos.verb += count,
            Tag::Adjective => store.pos.adjective += count,
            Tag::Adverb => store.pos.adverb += count,
            Tag::Other => {}
        }
    }
    store
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox {
        _lock: std::sync::MutexGuard<'static, ()>,
        dir: PathBuf,
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn sandbox() -> Sandbox {
        let lock = crate::paths::TEST_DATA_DIR
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("lstt-insights-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("LOCAL_STT_DATA_DIR", &dir);
        Sandbox { _lock: lock, dir }
    }

    fn day(y: i32, m: u32, d: u32) -> Day {
        Day::from_ymd(y, m, d).unwrap()
    }

    // 2026-10-12 is a Monday.
    fn at(date: Day, hour: u8) -> LocalNow {
        LocalNow { date, hour }
    }

    fn monday() -> Day {
        day(2026, 10, 12)
    }

    #[test]
    fn view_json_has_the_contract_keys() {
        let _s = sandbox();
        record(
            &at(monday(), 9),
            "Pricing review for the roadmap",
            4_000,
            true,
        );
        let v = serde_json::to_value(view(&at(monday(), 9), true)).unwrap();
        let keys = |value: &serde_json::Value| -> Vec<String> {
            let mut k: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(
            keys(&v),
            [
                "empty",
                "hours",
                "milestones",
                "speed",
                "streak",
                "today",
                "totals",
                "week",
                "words"
            ]
        );
        assert_eq!(
            keys(&v["streak"]),
            ["current", "freezeCap", "freezes", "longest", "nextFreezeIn"]
        );
        assert_eq!(
            keys(&v["totals"]),
            ["dictationsAll", "wordsAll", "wordsToday", "wordsWeek"]
        );
        assert_eq!(
            keys(&v["speed"]),
            ["minutesSavedAll", "minutesSavedWeek", "typingWpm", "wpm"]
        );
        assert_eq!(keys(&v["words"]), ["mix", "nouns", "top", "verbs"]);
        assert_eq!(
            keys(&v["words"]["mix"]),
            ["adjective", "adverb", "noun", "verb"]
        );
        assert_eq!(keys(&v["week"][0]), ["date", "state", "weekday"]);
        assert_eq!(keys(&v["milestones"][0]), ["reachedOn", "words"]);
        assert_eq!(v["today"], "2026-10-12");
        assert_eq!(v["week"].as_array().unwrap().len(), 7);
        assert_eq!(v["week"][0]["weekday"], "Mon");
        assert_eq!(v["week"][0]["state"], "active");
        assert_eq!(v["hours"].as_array().unwrap().len(), 24);
        assert_eq!(v["milestones"].as_array().unwrap().len(), 6);
        assert!(v["milestones"][1]["reachedOn"].is_null());
        assert_eq!(v["streak"]["current"], 1);
        assert_eq!(v["streak"]["freezeCap"], 10);
    }

    #[test]
    fn empty_view_uses_nulls_not_zeros() {
        let _s = sandbox();
        let v = serde_json::to_value(view(&at(monday(), 9), true)).unwrap();
        assert_eq!(v["empty"], true);
        assert!(v["words"].is_null());
        assert!(v["speed"]["wpm"].is_null());
        assert!(v["speed"]["minutesSavedAll"].is_null());
        assert!(
            !path().exists(),
            "viewing an empty store must not create the file"
        );
    }

    #[test]
    fn word_insights_off_hides_words_and_clear_words_drops_them() {
        let _s = sandbox();
        record(
            &at(monday(), 9),
            "Pricing review for the roadmap",
            4_000,
            true,
        );
        assert!(view(&at(monday(), 9), false).words.is_none());
        assert!(view(&at(monday(), 9), true).words.is_some());
        clear_words();
        assert!(view(&at(monday(), 9), true).words.is_none());
        assert_eq!(view(&at(monday(), 9), true).totals.words_all, 5);
        reset();
        assert!(view(&at(monday(), 9), true).empty);
    }

    #[test]
    fn record_without_word_counting_keeps_totals_only() {
        let _s = sandbox();
        record(&at(monday(), 9), "pricing review roadmap", 3_000, false);
        let v = view(&at(monday(), 9), true);
        assert!(v.words.is_none());
        assert_eq!(v.totals.words_today, 3);
    }

    #[test]
    fn file_is_private() {
        let _s = sandbox();
        record(&at(monday(), 9), "hello there world", 2_000, false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(path().exists());
    }

    #[test]
    fn hours_wpm_and_totals() {
        let _s = sandbox();
        record(
            &at(monday(), 9),
            "one two three four five six",
            3_000,
            false,
        );
        record(&at(monday(), 14), "seven eight", 500, false);
        let v = view(&at(monday(), 15), false);
        assert_eq!(v.hours[9], 6);
        assert_eq!(v.hours[14], 2);
        // 6 words in 3 s is 120 wpm; the 0.5 s dictation is too short to rate.
        assert_eq!(v.speed.wpm, Some(120));
        assert_eq!(v.totals.dictations_all, 2);
        assert_eq!(v.totals.words_week, 8);
    }

    #[test]
    fn wpm_list_keeps_the_last_hundred() {
        let _s = sandbox();
        for _ in 0..105 {
            record(&at(monday(), 9), "one two", 2_000, false);
        }
        assert_eq!(load().wpm.len(), KEEP_WPM);
    }

    #[test]
    fn minutes_saved_math() {
        // 4000 words is 100 typing minutes; 20 spoken minutes leaves 80.
        assert_eq!(minutes_saved(4_000, 20 * 60_000), 80);
        assert_eq!(minutes_saved(100, 10 * 60_000), 0);
        assert_eq!(minutes_saved(70, 0), 2);
        assert_eq!(minutes_saved(0, 0), 0);
    }

    #[test]
    fn milestones_record_the_first_day_reached() {
        let _s = sandbox();
        let text = vec!["word"; 600].join(" ");
        record(&at(monday(), 9), &text, 60_000, false);
        assert!(load().milestones.is_empty());
        record(&at(monday().next(), 9), &text, 60_000, false);
        record(&at(monday().plus_days(2), 9), &text, 60_000, false);
        let v = view(&at(monday().plus_days(2), 9), false);
        assert_eq!(v.milestones[0].reached_on, Some(monday().next()));
        assert!(v.milestones[1].reached_on.is_none());
    }

    #[test]
    fn rows_older_than_400_days_fold_into_totals() {
        let _s = sandbox();
        let old = monday().plus_days(-500);
        record(&at(old, 9), "one two three", 3_000, false);
        record(&at(monday(), 9), "four five", 2_000, false);
        let store = load();
        assert_eq!(store.rows.len(), 1);
        assert_eq!(store.folded.words, 3);
        let v = view(&at(monday(), 9), false);
        assert_eq!(v.totals.words_all, 5);
        assert_eq!(v.totals.dictations_all, 2);
    }

    #[test]
    fn view_freezes_missed_days_and_saves() {
        let _s = sandbox();
        record(&at(monday(), 9), "one two", 2_000, false);
        record(&at(monday().next(), 9), "one two", 2_000, false);
        // Thursday: Wednesday was missed, so one freeze is spent just by looking.
        let thursday = monday().plus_days(3);
        let v = view(&at(thursday, 9), false);
        assert_eq!(v.streak.freezes, streak::START_FREEZES - 1);
        assert_eq!(v.streak.current, 2);
        assert_eq!(v.week[2].state, WeekState::Frozen);
        assert_eq!(v.week[3].state, WeekState::Today);
        assert_eq!(load().streak.freezes, streak::START_FREEZES - 1);
        assert_eq!(
            view(&at(thursday, 9), false).streak.freezes,
            streak::START_FREEZES - 1
        );
    }

    #[test]
    fn weekend_dictation_is_a_bonus_that_adds_nothing() {
        let _s = sandbox();
        record(&at(monday(), 9), "one two", 2_000, false);
        let saturday = monday().plus_days(5);
        record(&at(saturday, 9), "one two", 2_000, false);
        let v = view(&at(saturday, 9), false);
        assert_eq!(v.week[5].state, WeekState::Bonus);
        assert_eq!(v.streak.current, 1);
    }

    #[test]
    fn next_freeze_countdown_and_cap() {
        let mut store = Store::default();
        store.streak.active_workdays = 7;
        let v = build_view(&store, monday(), false);
        assert_eq!(v.streak.next_freeze_in, Some(3));
        store.streak.freezes = streak::FREEZE_CAP;
        assert_eq!(
            build_view(&store, monday(), false).streak.next_freeze_in,
            None
        );
    }

    #[test]
    fn word_cap_drops_the_least_used() {
        let mut store = Store::default();
        let list: Vec<String> = (0..words::MAX_WORDS + 30)
            .map(|i| {
                format!("{}{}", "word", (b'a' + (i % 26) as u8) as char).repeat(1) + &i.to_string()
            })
            .collect();
        for (i, w) in list.iter().enumerate() {
            store.words.insert(
                w.clone(),
                WordStat {
                    count: 1 + i as u64,
                    tag: Tag::Noun,
                },
            );
        }
        store.count_words("pricing pricing");
        assert_eq!(store.words.len(), words::MAX_WORDS);
        assert!(store.words.contains_key(list.last().unwrap()));
        assert!(!store.words.contains_key(&list[0]));
    }

    #[test]
    fn top_lists_are_capped_and_sorted() {
        let mut store = Store::default();
        for i in 0..30u64 {
            let tag = if i % 2 == 0 { Tag::Noun } else { Tag::Verb };
            store.words.insert(
                format!("w{i:02}"),
                WordStat {
                    count: 100 - i,
                    tag,
                },
            );
        }
        store.pos.noun = 10;
        store.pos.verb = 10;
        let view = words_view(&store).unwrap();
        assert_eq!(view.top.len(), 15);
        assert_eq!(view.nouns.len(), 10);
        assert_eq!(view.verbs.len(), 10);
        assert_eq!(view.top[0].word, "w00");
        assert!((view.mix.noun - 0.5).abs() < 1e-9);
    }

    #[test]
    fn demo_seed_looks_believable() {
        let _s = sandbox();
        seed_demo(&at(monday(), 10));
        let v = view(&at(monday(), 10), true);
        assert_eq!(v.streak.current, 6);
        assert_eq!(v.streak.longest, 12);
        assert_eq!(v.streak.freezes, 4);
        assert!(!v.empty);
        let words = v.words.unwrap();
        assert_eq!(words.top[0].word, "pricing");
        assert!(words.verbs.iter().any(|w| w.word == "ship"));
        assert!(v.milestones[0].reached_on.is_some());
        assert_eq!(v.week[0].state, WeekState::Today);
    }
}
