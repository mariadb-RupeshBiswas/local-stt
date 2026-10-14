//! Names from the call app: who the call app showed as speaking, matched to the note's lines and voices at Stop.

use super::{Segment, ME, OTHERS};
use std::collections::{BTreeMap, HashMap, HashSet};

/// One look at the call app while the note records.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Milliseconds since the note started, on the same clock as the segments.
    pub at_ms: u64,
    /// The person the call app marks as speaking, if any.
    pub speaking: Option<String>,
}

/// A line's speaker needs this many samples during it, all but a few naming the same person.
const MIN_VOTES: usize = 2;
const AGREE: f32 = 0.6;

// The name most samples during this line agree on; None when too few or split.
fn line_name<'a>(seg: &Segment, samples: &'a [Sample], me: &HashSet<String>) -> Option<&'a str> {
    let mut votes: HashMap<&str, usize> = HashMap::new();
    let mut total = 0;
    for s in samples
        .iter()
        .filter(|s| s.at_ms >= seg.start_ms && s.at_ms <= seg.end_ms)
    {
        let Some(name) = s.speaking.as_deref() else {
            continue;
        };
        total += 1;
        if !me.contains(&name.to_lowercase()) {
            *votes.entry(name).or_default() += 1;
        }
    }
    let (name, count) = votes
        .into_iter()
        .max_by_key(|(n, c)| (*c, std::cmp::Reverse(*n)))?;
    (count >= MIN_VOTES && count as f32 >= AGREE * total as f32).then_some(name)
}

/// Lines with no voice split ("Others") take a speaker per name; returns the names for every other-side speaker key.
/// The user's own names (`me`, lowercase) never name another speaker; a name the user already gave is kept.
pub fn apply(
    segments: &mut [Segment],
    names: &mut BTreeMap<String, String>,
    samples: &[Sample],
    me: &HashSet<String>,
) -> usize {
    if samples.iter().all(|s| s.speaking.is_none()) {
        return 0;
    }
    let found: Vec<Option<String>> = segments
        .iter()
        .map(|seg| {
            if seg.speaker == ME {
                None
            } else {
                line_name(seg, samples, me).map(str::to_string)
            }
        })
        .collect();
    // Without a voice split, the call app's speaker is the only way to tell the other side apart.
    let mut keys_for: HashMap<String, String> = HashMap::new();
    let mut next = 1 + segments
        .iter()
        .filter_map(|s| {
            s.speaker
                .strip_prefix('s')
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    for (seg, name) in segments.iter_mut().zip(&found) {
        if seg.speaker != OTHERS {
            continue;
        }
        if let Some(name) = name {
            let key = keys_for.entry(name.clone()).or_insert_with(|| {
                let k = format!("s{next}");
                next += 1;
                k
            });
            seg.speaker = key.clone();
        }
    }
    // Each voice group takes the name most of its lines carry; a name goes to one group only.
    let mut tally: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for (seg, name) in segments.iter().zip(&found) {
        if seg.speaker == ME || seg.speaker == OTHERS {
            continue;
        }
        if let Some(name) = name {
            *tally
                .entry(seg.speaker.clone())
                .or_default()
                .entry(name.clone())
                .or_default() += 1;
        }
    }
    let mut picks: Vec<(usize, String, String)> = Vec::new();
    for (key, counts) in &tally {
        let total: usize = counts.values().sum();
        if let Some((name, n)) = counts
            .iter()
            .max_by_key(|(n, c)| (**c, std::cmp::Reverse((*n).clone())))
        {
            if *n as f32 >= AGREE * total as f32 {
                picks.push((*n, key.clone(), name.clone()));
            }
        }
    }
    picks.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut used: HashSet<String> = names.values().map(|n| n.to_lowercase()).collect();
    let mut named = 0;
    for (_, key, name) in picks {
        if names.contains_key(&key) || used.contains(&name.to_lowercase()) {
            continue;
        }
        used.insert(name.to_lowercase());
        names.insert(key, name);
        named += 1;
    }
    named
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(who: &str, start: u64, end: u64) -> Segment {
        Segment {
            start_ms: start,
            end_ms: end,
            speaker: who.into(),
            text: "words".into(),
        }
    }

    // Samples twice a second naming `who` from `from` to `to`.
    fn speaking(who: &str, from: u64, to: u64) -> Vec<Sample> {
        (from..to)
            .step_by(500)
            .map(|t| Sample {
                at_ms: t,
                speaking: Some(who.into()),
            })
            .collect()
    }

    fn me() -> HashSet<String> {
        ["rupesh".to_string()].into_iter().collect()
    }

    #[test]
    fn voice_groups_take_the_name_the_call_app_showed() {
        let mut segs = vec![
            seg("s1", 0, 4_000),
            seg("me", 4_000, 6_000),
            seg("s2", 6_000, 10_000),
            seg("s1", 10_000, 13_000),
        ];
        let mut samples = speaking("Simon", 0, 4_000);
        samples.extend(speaking("Rupesh", 4_000, 6_000));
        samples.extend(speaking("Deepshiksha", 6_000, 10_000));
        samples.extend(speaking("Simon", 10_000, 13_000));
        let mut names = BTreeMap::new();
        assert_eq!(apply(&mut segs, &mut names, &samples, &me()), 2);
        assert_eq!(names.get("s1").map(String::as_str), Some("Simon"));
        assert_eq!(names.get("s2").map(String::as_str), Some("Deepshiksha"));
        assert!(!names.contains_key("me"));
    }

    #[test]
    fn without_a_voice_split_the_call_app_splits_the_other_side() {
        let mut segs = vec![
            seg("others", 0, 3_000),
            seg("others", 3_000, 6_000),
            seg("others", 6_000, 9_000),
            seg("others", 9_000, 9_400),
        ];
        let mut samples = speaking("Simon", 0, 3_000);
        samples.extend(speaking("Priya", 3_000, 6_000));
        samples.extend(speaking("Simon", 6_000, 9_000));
        let mut names = BTreeMap::new();
        apply(&mut segs, &mut names, &samples, &me());
        let who: Vec<&str> = segs.iter().map(|s| s.speaker.as_str()).collect();
        assert_eq!(
            who,
            vec!["s1", "s2", "s1", "others"],
            "a line with too few samples stays Others"
        );
        assert_eq!(names.get("s1").map(String::as_str), Some("Simon"));
        assert_eq!(names.get("s2").map(String::as_str), Some("Priya"));
    }

    #[test]
    fn the_users_own_name_never_names_someone_else() {
        let mut segs = vec![seg("s1", 0, 4_000)];
        let samples = speaking("Rupesh", 0, 4_000);
        let mut names = BTreeMap::new();
        assert_eq!(apply(&mut segs, &mut names, &samples, &me()), 0);
        assert!(names.is_empty());
    }

    #[test]
    fn a_name_the_user_gave_is_kept_and_a_split_vote_names_nobody() {
        let mut segs = vec![seg("s1", 0, 4_000), seg("s2", 4_000, 8_000)];
        let mut samples = speaking("Simon", 0, 2_000);
        samples.extend(speaking("Priya", 2_000, 4_000));
        samples.extend(speaking("Ana", 4_000, 8_000));
        let mut names: BTreeMap<String, String> = [("s2".to_string(), "Ana B.".to_string())]
            .into_iter()
            .collect();
        assert_eq!(apply(&mut segs, &mut names, &samples, &me()), 0);
        assert_eq!(names.get("s2").map(String::as_str), Some("Ana B."));
        assert!(
            !names.contains_key("s1"),
            "half Simon, half Priya is not enough"
        );
    }

    #[test]
    fn one_name_goes_to_one_group_only() {
        let mut segs = vec![seg("s1", 0, 6_000), seg("s2", 6_000, 8_000)];
        let mut samples = speaking("Simon", 0, 8_000);
        samples.push(Sample {
            at_ms: 9_000,
            speaking: None,
        });
        let mut names = BTreeMap::new();
        apply(&mut segs, &mut names, &samples, &me());
        assert_eq!(names.len(), 1);
        assert_eq!(
            names.get("s1").map(String::as_str),
            Some("Simon"),
            "the group with more lines wins"
        );
    }

    #[test]
    fn no_samples_change_nothing() {
        let mut segs = vec![seg("others", 0, 3_000)];
        let mut names = BTreeMap::new();
        assert_eq!(apply(&mut segs, &mut names, &[], &me()), 0);
        assert_eq!(segs[0].speaker, "others");
    }
}
