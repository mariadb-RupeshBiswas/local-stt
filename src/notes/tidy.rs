//! The pass after a call: drop echo, clean each line, merge a speaker's back-to-back lines.

use super::Segment;
use std::collections::HashSet;

// A Me line sharing this much of its wording with the Others lines at the same time is the speakers' echo.
const ECHO_SHARE: f32 = 0.7;
// This many Me words in a row that Others said at the same time are echo, even inside the user's own line.
const ECHO_RUN: usize = 3;
// Lines from one speaker this close together read as one turn.
const MERGE_GAP_MS: u64 = 2_000;

pub fn tidy(mut segments: Vec<Segment>, smart_format: bool, english: bool) -> Vec<Segment> {
    segments.sort_by_key(|s| s.start_ms);
    let heard: Vec<Option<String>> = segments
        .iter()
        .map(|s| {
            if s.speaker == super::ME {
                without_echo(s, &segments)
            } else {
                Some(s.text.clone())
            }
        })
        .collect();
    let mut out: Vec<Segment> = Vec::new();
    for (seg, kept) in segments.into_iter().zip(heard) {
        let Some(raw) = kept else {
            continue;
        };
        let text = if smart_format {
            crate::format::tidy_for(&raw, english)
        } else {
            raw.trim().to_string()
        };
        if !text.chars().any(char::is_alphanumeric) {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if last.speaker == seg.speaker && seg.start_ms <= last.end_ms + MERGE_GAP_MS {
                last.text.push(' ');
                last.text.push_str(&text);
                last.end_ms = last.end_ms.max(seg.end_ms);
                continue;
            }
        }
        out.push(Segment { text, ..seg });
    }
    out
}

// The Me line without the speakers' echo of the other side; None when little of the user's own is left.
fn without_echo(me: &Segment, all: &[Segment]) -> Option<String> {
    let mut theirs: HashSet<String> = HashSet::new();
    for o in all.iter().filter(|o| o.speaker != super::ME) {
        if o.start_ms < me.end_ms && me.start_ms < o.end_ms {
            theirs.extend(words(&o.text));
        }
    }
    if theirs.is_empty() {
        return Some(me.text.clone());
    }
    let tokens: Vec<&str> = me.text.split_whitespace().collect();
    let heard: Vec<bool> = tokens
        .iter()
        .map(|t| {
            let w = words(t);
            !w.is_empty() && w.is_subset(&theirs)
        })
        .collect();
    let mut keep = vec![true; tokens.len()];
    let mut i = 0;
    while i < tokens.len() {
        let mut end = i;
        while end < tokens.len() && heard[end] {
            end += 1;
        }
        if end - i >= ECHO_RUN {
            keep[i..end].fill(false);
        }
        i = end.max(i + 1);
    }
    let left: Vec<&str> = tokens
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(t, _)| *t)
        .collect();
    if left.len() < tokens.len() && left.len() < ECHO_RUN {
        return None;
    }
    let text = left.join(" ");
    let mine = words(&text);
    if mine.is_empty() {
        return Some(text);
    }
    let shared = mine.intersection(&theirs).count();
    if shared as f32 / mine.len() as f32 >= ECHO_SHARE {
        return None;
    }
    Some(text)
}

fn words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{ME, OTHERS};
    use super::*;

    fn seg(speaker: &str, start: u64, end: u64, text: &str) -> Segment {
        Segment {
            start_ms: start,
            end_ms: end,
            speaker: speaker.into(),
            text: text.into(),
        }
    }

    #[test]
    fn echo_of_the_others_on_my_microphone_is_dropped() {
        let out = tidy(
            vec![
                seg(
                    OTHERS,
                    1_000,
                    4_000,
                    "Let's review the September numbers first.",
                ),
                seg(ME, 1_200, 4_100, "let's review the September numbers"),
            ],
            false,
            true,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].speaker, OTHERS);
    }

    #[test]
    fn echo_inside_my_own_line_is_cut_and_my_words_stay() {
        let out = tidy(
            vec![
                seg(
                    OTHERS,
                    189_000,
                    193_000,
                    "Bless you. I think we should finish the call early.",
                ),
                seg(
                    ME,
                    185_000,
                    194_000,
                    "I'm so sorry. I think we should finish the call early.",
                ),
            ],
            false,
            true,
        );
        let mine: Vec<&str> = out
            .iter()
            .filter(|s| s.speaker == ME)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(mine, vec!["I'm so sorry."]);
    }

    #[test]
    fn echo_of_two_others_lines_in_one_of_mine_is_dropped() {
        let out = tidy(
            vec![
                seg(OTHERS, 0, 2_000, "Okay, cool."),
                seg(
                    OTHERS,
                    2_000,
                    5_000,
                    "There's one more, the Apex dashboard.",
                ),
                seg(ME, 0, 5_000, "okay cool there's one more"),
            ],
            false,
            true,
        );
        assert!(out.iter().all(|s| s.speaker == OTHERS), "{out:?}");
    }

    #[test]
    fn talking_over_each_other_keeps_both() {
        let out = tidy(
            vec![
                seg(
                    OTHERS,
                    1_000,
                    4_000,
                    "Let's review the September numbers first.",
                ),
                seg(ME, 1_500, 3_000, "Sorry, can you share your screen?"),
            ],
            false,
            true,
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn back_to_back_lines_merge_and_a_gap_or_new_speaker_splits() {
        let out = tidy(
            vec![
                seg(ME, 0, 2_000, "First point."),
                seg(ME, 3_000, 5_000, "Second point."),
                seg(OTHERS, 5_500, 7_000, "Agreed."),
                seg(ME, 7_500, 8_000, "Good."),
                seg(ME, 20_000, 21_000, "Later."),
            ],
            false,
            true,
        );
        let lines: Vec<(&str, &str)> = out
            .iter()
            .map(|s| (s.speaker.as_str(), s.text.as_str()))
            .collect();
        assert_eq!(
            lines,
            vec![
                (ME, "First point. Second point."),
                (OTHERS, "Agreed."),
                (ME, "Good."),
                (ME, "Later.")
            ]
        );
        assert_eq!(out[0].end_ms, 5_000);
    }

    #[test]
    fn smart_formatting_cleans_lines_and_filler_only_lines_go() {
        let out = tidy(
            vec![
                seg(ME, 0, 1_000, "um"),
                seg(OTHERS, 2_000, 4_000, "uh so we ship on friday"),
            ],
            true,
            true,
        );
        assert_eq!(out.len(), 1);
        assert!(
            !out[0].text.to_lowercase().starts_with("uh"),
            "{}",
            out[0].text
        );
    }

    #[test]
    fn lines_come_out_in_time_order() {
        let out = tidy(
            vec![
                seg(OTHERS, 5_000, 6_000, "Second."),
                seg(ME, 0, 1_000, "First."),
            ],
            false,
            true,
        );
        assert_eq!(out[0].text, "First.");
    }
}
