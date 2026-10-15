//! Telling the other side of a call apart: a voice fingerprint per line, grouped into speakers at Stop.

use std::collections::HashMap;
use std::path::PathBuf;

pub const MODEL_FILE: &str = "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
// "recongition" is how sherpa-onnx spells this release tag.
pub const MODEL_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
pub const MODEL_SHA256: &str = "aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2";

/// Average voice similarity at which two groups of lines are one speaker.
pub const SAME_SPEAKER: f32 = 0.55;
/// Group centres this alike are one speaker; two different people's centres measured at most 0.63 on the fixtures.
pub const SAME_CENTRE: f32 = 0.70;
/// Past the call's head count, groups still need this much in common to be merged.
const MERGE_FLOOR: f32 = 0.30;
/// A group holding under this share of the clear lines (and at least 2) is a stray.
const STRAY_PERCENT: usize = 3;
/// Under a second of speech says too little about a voice; such a line takes its neighbour's speaker.
pub const MIN_SAMPLES: usize = 16_000;
/// Lines this long found the speakers; a shorter line (a "yeah", an "okay") only joins the closest one.
pub const CLEAR_SAMPLES: usize = 48_000;
/// Grouping is cubic in lines; past this the note keeps one Others label rather than stall the save.
const MAX_LINES: usize = 2_000;
/// Set on the start time of a microphone line that is not the user's, so it cannot collide with an Others line.
pub const ROOM: u64 = 1 << 63;

pub fn model_path() -> PathBuf {
    crate::paths::models_dir().join(MODEL_FILE)
}

/// Builds without the voice library keep one "Others" label.
pub fn supported() -> bool {
    cfg!(lstt_voices)
}

/// The voice model, loaded the first time a line needs it, once its checksum matches.
#[derive(Default)]
pub struct Lazy {
    embedder: Option<crate::engine::VoiceEmbedder>,
    failed: bool,
}

impl Lazy {
    pub fn get(&mut self) -> Option<&crate::engine::VoiceEmbedder> {
        if self.embedder.is_none() && !self.failed && supported() {
            let path = model_path();
            // Not downloaded yet: try again on a later line.
            if !path.exists() {
                return None;
            }
            let sum = crate::download::sha256_file(&path).unwrap_or_default();
            self.embedder = (sum == MODEL_SHA256)
                .then(|| crate::engine::VoiceEmbedder::load(&path))
                .flatten();
            self.failed = self.embedder.is_none();
            crate::diag::log(if self.failed {
                "voice model could not be loaded; speakers stay as Others"
            } else {
                "voice model loaded"
            });
        }
        self.embedder.as_ref()
    }
}

pub(super) fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Average-linkage grouping: merges the two closest groups until none are `threshold` alike; one group index per voice.
/// ponytail: O(n^3) in lines; fine for a few hundred lines a call, use a heap past a few thousand.
pub fn cluster(voices: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let n = voices.len();
    let mut sim = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let s = cosine(&voices[i], &voices[j]);
            sim[i][j] = s;
            sim[j][i] = s;
        }
    }
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut alive: Vec<bool> = vec![true; n];
    loop {
        let mut best: Option<(f32, usize, usize)> = None;
        for i in 0..n {
            if !alive[i] {
                continue;
            }
            for j in (i + 1)..n {
                if alive[j] && best.is_none_or(|(s, _, _)| sim[i][j] > s) {
                    best = Some((sim[i][j], i, j));
                }
            }
        }
        let Some((score, keep, gone)) = best else {
            break;
        };
        if score < threshold {
            break;
        }
        // Average linkage: the merged group's similarity to every other is the size-weighted mean.
        let (wk, wg) = (members[keep].len() as f32, members[gone].len() as f32);
        for k in 0..n {
            if alive[k] && k != keep && k != gone {
                let merged = (sim[keep][k] * wk + sim[gone][k] * wg) / (wk + wg);
                sim[keep][k] = merged;
                sim[k][keep] = merged;
            }
        }
        let moved = std::mem::take(&mut members[gone]);
        members[keep].extend(moved);
        alive[gone] = false;
    }
    let mut group = vec![0; n];
    let mut next = 0;
    for (i, list) in members.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        for &line in list {
            group[line] = next;
        }
        next += 1;
    }
    group
}

/// Speaker keys by line start (start ms, samples, fingerprint): "s1".."sN" by first speaking; empty when one voice and no room voice.
/// `heads` is how many other people the call app showed (0 when unknown): more groups than that plus one get merged down.
pub fn speakers(voiced: &[(u64, usize, Vec<f32>)], heads: usize) -> HashMap<u64, String> {
    if voiced.len() > MAX_LINES {
        return HashMap::new();
    }
    let mut clear: Vec<usize> = (0..voiced.len())
        .filter(|&i| voiced[i].1 >= CLEAR_SAMPLES)
        .collect();
    if clear.len() < 2 {
        clear = (0..voiced.len()).collect();
    }
    let vectors: Vec<Vec<f32>> = clear.iter().map(|&i| voiced[i].2.clone()).collect();
    let group = cluster(&vectors, SAME_SPEAKER);
    let count = group.iter().max().map_or(0, |m| m + 1);
    let mut size = vec![0; count];
    for &g in &group {
        size[g] += 1;
    }
    // A group this small is a stray (a cough, two people at once) once real speakers exist; its lines join the closest.
    let stray = (clear.len() * STRAY_PERCENT).div_ceil(100).max(2);
    let mut kept: Vec<usize> = (0..count).filter(|&g| size[g] >= stray).collect();
    if kept.len() < 2 {
        kept = (0..count).filter(|&g| size[g] >= 2).collect();
    }
    if kept.len() < 2 {
        kept = (0..count).collect();
    }
    let mut parts: Vec<Vec<usize>> = Vec::new();
    for &g in &kept {
        parts.push((0..clear.len()).filter(|&k| group[k] == g).collect());
    }
    merge_parts(&mut parts, &vectors, heads);
    let room = voiced.iter().any(|(k, _, _)| k & ROOM != 0);
    if parts.is_empty() || (parts.len() < 2 && !room) {
        return HashMap::new();
    }
    let mut centre = vec![Vec::new(); parts.len()];
    let mut place: HashMap<usize, usize> = HashMap::new();
    for (slot, part) in parts.iter().enumerate() {
        for &k in part {
            add_unit(&mut centre[slot], &voiced[clear[k]].2);
            place.insert(clear[k], slot);
        }
    }
    let mut order: Vec<usize> = (0..voiced.len()).collect();
    order.sort_by_key(|&i| voiced[i].0 & !ROOM);
    let mut number: HashMap<usize, usize> = HashMap::new();
    let mut labels = HashMap::new();
    for i in order {
        let slot = match place.get(&i) {
            Some(&slot) => slot,
            None => closest(&centre, &voiced[i].2),
        };
        let next = number.len() + 1;
        let n = *number.entry(slot).or_insert(next);
        labels.insert(voiced[i].0, format!("s{n}"));
    }
    labels
}

// Pairs of lines from one person on a call often score under SAME_SPEAKER while their groups' centres agree:
// joins groups whose centres are SAME_CENTRE alike, and past the call's head count (plus one for a room voice) the closest pair above MERGE_FLOOR.
fn merge_parts(parts: &mut Vec<Vec<usize>>, vectors: &[Vec<f32>], heads: usize) {
    let cap = if heads > 0 { heads + 1 } else { usize::MAX };
    while parts.len() > 1 {
        let mut centres = Vec::new();
        for part in parts.iter() {
            let mut c = Vec::new();
            for &k in part {
                add_unit(&mut c, &vectors[k]);
            }
            centres.push(c);
        }
        let mut best = (f32::MIN, 0, 0);
        for a in 0..parts.len() {
            for b in (a + 1)..parts.len() {
                let s = cosine(&centres[a], &centres[b]);
                if s > best.0 {
                    best = (s, a, b);
                }
            }
        }
        let (score, a, b) = best;
        let over = parts.len() > cap;
        if score < SAME_CENTRE && !(over && score >= MERGE_FLOOR) {
            return;
        }
        let gone = parts.remove(b);
        parts[a].extend(gone);
    }
}

// Sums unit-length fingerprints, so every line counts the same in its speaker's centre.
fn add_unit(sum: &mut Vec<f32>, v: &[f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return;
    }
    if sum.len() != v.len() {
        *sum = vec![0.0; v.len()];
    }
    for (s, x) in sum.iter_mut().zip(v) {
        *s += x / norm;
    }
}

fn closest(centres: &[Vec<f32>], v: &[f32]) -> usize {
    let mut best = 0;
    let mut best_score = f32::MIN;
    for (slot, c) in centres.iter().enumerate() {
        let score = cosine(v, c);
        if score > best_score {
            best = slot;
            best_score = score;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Fixture {
        labels: Vec<String>,
        vectors: Vec<Vec<f32>>,
    }

    // Fingerprints of four real speakers reading the same sentences (CMU ARCTIC), from MODEL_FILE.
    fn arctic() -> Fixture {
        serde_json::from_str(include_str!("../../tests/fixtures/voices-arctic.json")).unwrap()
    }

    #[test]
    fn the_shipped_targets_tell_voices_apart() {
        let shipped = cfg!(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "windows", target_arch = "x86_64")
        ));
        assert_eq!(supported(), shipped);
    }

    #[test]
    fn four_real_speakers_reading_the_same_lines_fall_into_four_groups() {
        let f = arctic();
        let group = cluster(&f.vectors, SAME_SPEAKER);
        let groups: std::collections::HashSet<usize> = group.iter().copied().collect();
        assert_eq!(groups.len(), 4);
        for i in 0..group.len() {
            for j in 0..group.len() {
                assert_eq!(group[i] == group[j], f.labels[i] == f.labels[j], "{i} {j}");
            }
        }
    }

    #[derive(serde::Deserialize)]
    struct Call {
        labels: Vec<String>,
        samples: Vec<usize>,
        vectors: Vec<Vec<f32>>,
    }

    // Three voices in call-like lines, most of them a word or two in a short window (see tests/fixtures/README.md).
    fn call() -> Call {
        serde_json::from_str(include_str!("../../tests/fixtures/voices-call.json")).unwrap()
    }

    type Voiced = (u64, usize, Vec<f32>);

    // Deterministic noise for the drifting call (xorshift), so the test needs no crate.
    struct Noise(u64);

    impl Noise {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }

        fn pick(&mut self, n: usize) -> usize {
            (self.next() * n as f64) as usize % n
        }

        // A random unit-length direction.
        fn direction(&mut self) -> Vec<f32> {
            let mut v = Vec::with_capacity(192);
            for _ in 0..192 {
                let (u1, u2) = (self.next().max(1e-12), self.next());
                v.push(((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32);
            }
            unit(&v)
        }
    }

    fn unit(v: &[f32]) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / n).collect()
    }

    // A long call as Zoom delivers it: each person's device and codec drift between three stretches of the call,
    // and some lines hold two people (a turn change inside one line). Built from the call fixture.
    fn drifting_call(seed: u64) -> (Vec<Voiced>, Vec<String>) {
        let c = call();
        let mut rng = Noise(seed);
        let names = ["aup", "eey", "ljm"];
        let mut pools: Vec<Vec<usize>> = Vec::new();
        for who in names {
            let lines = (0..c.labels.len())
                .filter(|&i| c.labels[i] == who && c.samples[i] >= MIN_SAMPLES)
                .collect();
            pools.push(lines);
        }
        let mut drift = Vec::new();
        for _ in 0..names.len() * 3 {
            drift.push(rng.direction());
        }
        let (mut voiced, mut truth) = (Vec::new(), Vec::new());
        for line in 0..180u64 {
            let who = rng.pick(3);
            let stretch = line as usize * 3 / 180;
            let mut base = unit(&c.vectors[pools[who][rng.pick(pools[who].len())]]);
            let mut label = names[who].to_string();
            if rng.next() < 0.15 {
                let other = (who + 1 + rng.pick(2)) % 3;
                let more = unit(&c.vectors[pools[other][rng.pick(pools[other].len())]]);
                let both: Vec<f32> = base.iter().zip(&more).map(|(a, b)| a + b).collect();
                base = unit(&both);
                label = format!("{label}+{}", names[other]);
            }
            let noise = rng.direction();
            let d = &drift[who * 3 + stretch];
            let mut v = Vec::with_capacity(192);
            for k in 0..192 {
                v.push(base[k] + 0.6 * d[k] + 0.5 * noise[k]);
            }
            let samples = [20_000, 60_000, 120_000][rng.pick(3)];
            voiced.push((line * 10_000, samples, v));
            truth.push(label);
        }
        (voiced, truth)
    }

    #[test]
    fn a_drifting_call_keeps_one_group_per_person_given_the_head_count() {
        let mut shares = Vec::new();
        for seed in [1, 2, 3, 4] {
            let (voiced, truth) = drifting_call(seed);
            let known = speakers(&voiced, 3);
            let unknown = speakers(&voiced, 0);
            let count = |l: &HashMap<u64, String>| {
                l.values().collect::<std::collections::HashSet<_>>().len()
            };
            assert!(count(&known) <= 4, "seed {seed}: {} groups", count(&known));
            assert!(
                count(&unknown) <= 12,
                "seed {seed}: {} groups",
                count(&unknown)
            );
            // No two people share their main group: the merge never joins different voices.
            let mut mains = std::collections::HashSet::new();
            for who in ["aup", "eey", "ljm"] {
                let mine: Vec<usize> = (0..truth.len()).filter(|&i| truth[i] == who).collect();
                let mut count: HashMap<&String, usize> = HashMap::new();
                for &i in &mine {
                    *count.entry(&known[&voiced[i].0]).or_default() += 1;
                }
                let (group, n) = count.into_iter().max_by_key(|(_, n)| *n).unwrap();
                assert!(
                    mains.insert(group.clone()),
                    "seed {seed}: two people in {group}"
                );
                shares.push(n as f32 / mine.len() as f32);
            }
        }
        let average = shares.iter().sum::<f32>() / shares.len() as f32;
        eprintln!("main-group share per person: {shares:.2?}, average {average:.2}");
        assert!(average >= 0.6, "average {average:.2}");
    }

    #[test]
    fn short_call_lines_join_the_speakers_instead_of_making_their_own() {
        let c = call();
        let voiced: Vec<(u64, usize, Vec<f32>)> = (0..c.labels.len())
            .map(|i| (i as u64, c.samples[i], c.vectors[i].clone()))
            .collect();
        let labels = speakers(&voiced, 0);
        let found: std::collections::HashSet<&String> = labels.values().collect();
        assert_eq!(found.len(), 3, "{labels:?}");
        let mut right = 0;
        for i in 0..voiced.len() {
            let same_as_most = (0..voiced.len())
                .filter(|&j| c.labels[j] == c.labels[i])
                .filter(|&j| labels[&(j as u64)] == labels[&(i as u64)])
                .count();
            let theirs = c.labels.iter().filter(|l| **l == c.labels[i]).count();
            if same_as_most * 2 > theirs {
                right += 1;
            }
        }
        assert!(
            right * 100 >= voiced.len() * 95,
            "{right} of {} lines",
            voiced.len()
        );
    }

    #[test]
    fn a_voice_in_one_clear_line_joins_a_speaker_but_two_voices_are_never_merged() {
        let f = arctic();
        let mut voiced: Vec<(u64, usize, Vec<f32>)> = f
            .vectors
            .iter()
            .enumerate()
            .filter(|(i, _)| ["aup", "eey"].contains(&f.labels[*i].as_str()))
            .map(|(i, v)| (i as u64, CLEAR_SAMPLES, v.clone()))
            .collect();
        let ljm = f.labels.iter().position(|l| l == "ljm").unwrap();
        voiced.push((900, CLEAR_SAMPLES, f.vectors[ljm].clone()));
        assert_eq!(
            speakers(&voiced, 0)
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2
        );
        let pair = [
            voiced[0].clone(),
            (900, CLEAR_SAMPLES, f.vectors[ljm].clone()),
        ];
        assert_eq!(
            speakers(&pair, 0)
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2,
            "two lone voices stay two speakers"
        );
    }

    #[test]
    fn speakers_are_numbered_by_who_spoke_first() {
        let f = arctic();
        // ljm speaks first, then aup, then the rest.
        let mut voiced: Vec<(u64, usize, Vec<f32>)> = Vec::new();
        for (i, (label, v)) in f.labels.iter().zip(&f.vectors).enumerate() {
            let start = match label.as_str() {
                "ljm" => i as u64,
                "aup" => 1_000 + i as u64,
                _ => 2_000 + i as u64,
            };
            voiced.push((start, CLEAR_SAMPLES, v.clone()));
        }
        let labels = speakers(&voiced, 0);
        let first_ljm = voiced.iter().find(|(s, _, _)| *s < 1_000).unwrap().0;
        let first_aup = voiced
            .iter()
            .find(|(s, _, _)| (1_000..2_000).contains(s))
            .unwrap()
            .0;
        assert_eq!(labels[&first_ljm], "s1");
        assert_eq!(labels[&first_aup], "s2");
        assert_eq!(
            labels
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
    }

    #[test]
    fn one_voice_or_none_keeps_the_others_label() {
        let f = arctic();
        let one: Vec<(u64, usize, Vec<f32>)> = f
            .labels
            .iter()
            .zip(&f.vectors)
            .filter(|(l, _)| l.as_str() == "eey")
            .enumerate()
            .map(|(i, (_, v))| (i as u64, CLEAR_SAMPLES, v.clone()))
            .collect();
        assert!(speakers(&one, 0).is_empty());
        assert!(speakers(&[], 0).is_empty());
        assert!(speakers(&[(0, CLEAR_SAMPLES, vec![0.0; 192])], 0).is_empty());
    }

    #[test]
    fn a_room_voice_on_the_microphone_is_numbered_in_time_and_never_collides() {
        let f = arctic();
        let first = |label: &str| f.labels.iter().position(|l| l == label).unwrap();
        let (a, b) = (first("ljm"), first("aup"));
        // One Others voice at 5 s and a different voice on the microphone at 1 s and at 5 s.
        let voiced = vec![
            (5_000, CLEAR_SAMPLES, f.vectors[a].clone()),
            (1_000 | ROOM, CLEAR_SAMPLES, f.vectors[b].clone()),
            (5_000 | ROOM, CLEAR_SAMPLES, f.vectors[b].clone()),
        ];
        let labels = speakers(&voiced, 0);
        assert_eq!(labels[&(1_000 | ROOM)], "s1");
        assert_eq!(labels[&(5_000 | ROOM)], "s1");
        assert_eq!(labels[&5_000], "s2");
        // One voice and nothing from the microphone keeps Others; a lone room voice is named.
        assert!(speakers(&voiced[..1], 0).is_empty());
        assert_eq!(speakers(&voiced[1..2], 0)[&(1_000 | ROOM)], "s1");
    }

    #[test]
    fn a_call_too_long_to_group_keeps_one_label() {
        let f = arctic();
        let many: Vec<(u64, usize, Vec<f32>)> = (0..=MAX_LINES)
            .map(|i| {
                (
                    i as u64,
                    CLEAR_SAMPLES,
                    f.vectors[i % f.vectors.len()].clone(),
                )
            })
            .collect();
        assert!(speakers(&many, 0).is_empty());
    }
}
