//! The user's own voice fingerprint, learned from their dictations; other people's voices are never stored.

use super::voices::{cosine, MODEL_SHA256};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A microphone line this alike or less is not the user. Measured: own lines scored 0.70 or more, others 0.59 or less.
pub const SOMEONE_ELSE: f32 = 0.45;
/// Dictations learned before the profile may call a voice someone else's.
pub const MIN_COUNT: u32 = 10;
/// A dictation teaches from its first 30 s; more costs time and adds little.
pub const MAX_CLIP: usize = 480_000;
/// Past this a new dictation still moves the mean, slowly.
const MAX_COUNT: u32 = 200;

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Profile {
    model_sha256: String,
    count: u32,
    mean: Vec<f32>,
}

impl Profile {
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Folds one dictation's fingerprint into the running mean.
    pub fn learn(&mut self, voice: &[f32]) {
        if voice.is_empty() || voice.iter().any(|v| !v.is_finite()) {
            return;
        }
        if self.mean.len() != voice.len() {
            self.mean = voice.to_vec();
            self.count = 1;
        } else {
            self.count = (self.count + 1).min(MAX_COUNT);
            let n = self.count as f32;
            for (m, v) in self.mean.iter_mut().zip(voice) {
                *m += (v - *m) / n;
            }
        }
        self.model_sha256 = MODEL_SHA256.to_string();
    }

    /// True when the profile is trusted and this voice is clearly not the user's.
    pub fn is_someone_else(&self, voice: &[f32]) -> bool {
        self.count >= MIN_COUNT && cosine(voice, &self.mean) < SOMEONE_ELSE
    }
}

pub fn path() -> PathBuf {
    crate::paths::data_dir().join("voice.json")
}

/// The saved profile, or an empty one when none is saved, it is damaged, or the voice model changed.
pub fn load_from(path: &Path) -> Profile {
    let parsed = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Profile>(&bytes).ok());
    match parsed {
        Some(p) if p.model_sha256 == MODEL_SHA256 && p.count > 0 && !p.mean.is_empty() => Profile {
            count: p.count.min(MAX_COUNT),
            ..p
        },
        _ => Profile::default(),
    }
}

pub fn save_to(path: &Path, profile: &Profile) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(profile).map_err(std::io::Error::other)?;
    crate::history::replace(path, &bytes)
}

/// "Learn my voice" off: the fingerprint is deleted.
pub fn clear() {
    let _ = std::fs::remove_file(path());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lstt-myvoice-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join("voice.json")
    }

    fn unit(axis: usize, len: usize) -> Vec<f32> {
        let mut v = vec![0.0; len];
        v[axis] = 1.0;
        v
    }

    #[test]
    fn the_mean_follows_each_dictation_and_adapts_slowly_past_the_cap() {
        let mut p = Profile::default();
        p.learn(&[2.0, 0.0]);
        p.learn(&[0.0, 2.0]);
        assert_eq!((p.count, p.mean.clone()), (2, vec![1.0, 1.0]));
        for _ in 0..500 {
            p.learn(&[1.0, 1.0]);
        }
        assert_eq!(p.count, MAX_COUNT);
        p.learn(&[201.0, 1.0]);
        assert!((p.mean[0] - 2.0).abs() < 1e-3, "{:?}", p.mean);
    }

    #[test]
    fn a_bad_or_odd_sized_fingerprint_never_corrupts_the_mean() {
        let mut p = Profile::default();
        p.learn(&[1.0, 0.0]);
        p.learn(&[f32::NAN, 0.0]);
        p.learn(&[]);
        assert_eq!((p.count, p.mean.clone()), (1, vec![1.0, 0.0]));
        p.learn(&[0.0, 1.0, 0.0]);
        assert_eq!((p.count, p.mean.len()), (1, 3));
    }

    #[test]
    fn only_a_trusted_profile_calls_a_clearly_different_voice_someone_else() {
        let mine = unit(0, 4);
        let other = unit(1, 4);
        let mut p = Profile::default();
        for _ in 0..MIN_COUNT - 1 {
            p.learn(&mine);
        }
        assert!(!p.is_someone_else(&other), "too few dictations to judge");
        p.learn(&mine);
        assert!(p.is_someone_else(&other));
        assert!(!p.is_someone_else(&mine));
        let near = [0.8, 0.6, 0.0, 0.0];
        assert!(!p.is_someone_else(&near), "0.8 alike is still the user");
        let far = [0.4, 0.9165, 0.0, 0.0];
        assert!(p.is_someone_else(&far), "0.4 alike is not");
    }

    #[test]
    fn a_saved_profile_loads_back_and_a_different_voice_model_discards_it() {
        let file = tmp("rt");
        let mut p = Profile::default();
        p.learn(&[0.5, 0.25]);
        save_to(&file, &p).unwrap();
        assert_eq!(load_from(&file), p);
        let stale = serde_json::json!({ "model_sha256": "other", "count": 50, "mean": [1.0] });
        std::fs::write(&file, stale.to_string()).unwrap();
        assert_eq!(load_from(&file), Profile::default());
        std::fs::write(&file, "{not json").unwrap();
        assert_eq!(load_from(&file), Profile::default());
        assert_eq!(
            load_from(&file.with_extension("missing")),
            Profile::default()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_saved_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let file = tmp("perm");
        let _ = std::fs::remove_file(&file);
        save_to(&file, &Profile::default()).unwrap();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
