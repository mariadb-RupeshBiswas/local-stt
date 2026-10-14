//! Telling the other side of a call apart: a voice fingerprint per line, grouped into speakers at Stop.

use std::collections::HashMap;
use std::path::PathBuf;

pub const MODEL_FILE: &str = "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
// "recongition" is how sherpa-onnx spells this release tag.
pub const MODEL_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
pub const MODEL_SHA256: &str = "aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2";

/// Average voice similarity at which two groups of lines are one speaker.
pub const SAME_SPEAKER: f32 = 0.55;
/// Under a second of speech says too little about a voice; such a line takes its neighbour's speaker.
pub const MIN_SAMPLES: usize = 16_000;
/// Lines this long found the speakers; a shorter line (a "yeah", an "okay") only joins the closest one.
pub const CLEAR_SAMPLES: usize = 48_000;
/// Grouping is cubic in lines; past this the note keeps one Others label rather than stall the save.
const MAX_LINES: usize = 2_000;

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

fn cosine(a: &[f32], b: &[f32]) -> f32 {
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

/// Speaker keys for the other side's (start ms, samples, fingerprint) lines: "s1".."sN" by first speaking; empty when one voice.
pub fn speakers(voiced: &[(u64, usize, Vec<f32>)]) -> HashMap<u64, String> {
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
    // ponytail: a voice in one clear line is a stray once two real speakers exist; a lone remark joins the closest, a head count from the call app would fix that.
    let mut kept: Vec<usize> = (0..count).filter(|&g| size[g] >= 2).collect();
    if kept.len() < 2 {
        kept = (0..count).collect();
    }
    if kept.len() < 2 {
        return HashMap::new();
    }
    let mut centre = vec![Vec::new(); kept.len()];
    let mut place: HashMap<usize, usize> = HashMap::new();
    for (k, &line) in clear.iter().enumerate() {
        if let Some(slot) = kept.iter().position(|&g| g == group[k]) {
            add_unit(&mut centre[slot], &voiced[line].2);
            place.insert(line, slot);
        }
    }
    let mut order: Vec<usize> = (0..voiced.len()).collect();
    order.sort_by_key(|&i| voiced[i].0);
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

    #[test]
    fn short_call_lines_join_the_speakers_instead_of_making_their_own() {
        let c = call();
        let voiced: Vec<(u64, usize, Vec<f32>)> = (0..c.labels.len())
            .map(|i| (i as u64, c.samples[i], c.vectors[i].clone()))
            .collect();
        let labels = speakers(&voiced);
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
            speakers(&voiced)
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
            speakers(&pair)
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
        let labels = speakers(&voiced);
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
        assert!(speakers(&one).is_empty());
        assert!(speakers(&[]).is_empty());
        assert!(speakers(&[(0, CLEAR_SAMPLES, vec![0.0; 192])]).is_empty());
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
        assert!(speakers(&many).is_empty());
    }
}
