//! Model catalog and hardware fit.

use crate::hwprobe::Hardware;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelId {
    Small,
    Medium,
    LargeV3,
}

pub struct ModelInfo {
    pub id: ModelId,
    pub label: &'static str,
    pub file: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub min_ram_gb: u64,
    pub min_free_disk_gb: f64,
    pub needs_gpu: bool,
    pub min_threads_without_gpu: u32,
}

pub const CATALOG: [ModelInfo; 3] = [
    ModelInfo {
        id: ModelId::Small,
        label: "Whisper Small",
        file: "ggml-small-q5_1.bin",
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        size_bytes: 190_085_487,
        min_ram_gb: 4,
        min_free_disk_gb: 0.5,
        needs_gpu: false,
        min_threads_without_gpu: 1,
    },
    ModelInfo {
        id: ModelId::Medium,
        label: "Whisper Medium",
        file: "ggml-medium-q5_0.bin",
        sha256: "19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f",
        size_bytes: 539_212_467,
        min_ram_gb: 8,
        min_free_disk_gb: 1.5,
        needs_gpu: false,
        min_threads_without_gpu: 8,
    },
    ModelInfo {
        id: ModelId::LargeV3,
        label: "Whisper Large v3",
        file: "ggml-large-v3-q5_0.bin",
        sha256: "d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1",
        size_bytes: 1_081_140_203,
        min_ram_gb: 16,
        min_free_disk_gb: 2.5,
        needs_gpu: true,
        min_threads_without_gpu: u32::MAX,
    },
];

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Fit {
    pub id: ModelId,
    pub label: String,
    pub size_mb: u64,
    pub supported: bool,
    pub recommended: bool,
    pub downloaded: bool,
    pub reasons: Vec<String>,
}

pub fn info(id: ModelId) -> &'static ModelInfo {
    match id {
        ModelId::Small => &CATALOG[0],
        ModelId::Medium => &CATALOG[1],
        ModelId::LargeV3 => &CATALOG[2],
    }
}

pub fn url(id: ModelId) -> String {
    format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        info(id).file
    )
}

pub fn path(models_dir: &Path, id: ModelId) -> PathBuf {
    models_dir.join(info(id).file)
}

// Reasons a model does not fit; empty means supported.
fn reasons_against(m: &ModelInfo, hw: &Hardware, downloaded: bool) -> Vec<String> {
    let mut reasons = Vec::new();
    // Unknown RAM or disk blocks everything except the smallest model.
    let unknown_ok = m.id == ModelId::Small;
    match hw.ram_gb {
        Some(ram) if ram < m.min_ram_gb => {
            reasons.push(format!(
                "Needs {} GB RAM, this PC has {ram} GB",
                m.min_ram_gb
            ));
        }
        None if !unknown_ok => {
            reasons.push(format!(
                "Needs {} GB RAM, this PC's RAM could not be read",
                m.min_ram_gb
            ));
        }
        _ => {}
    }
    if !downloaded {
        match hw.free_disk_gb {
            Some(disk) if disk < m.min_free_disk_gb => {
                reasons.push(format!(
                    "Needs {} GB free disk, this PC has {disk:.1} GB",
                    m.min_free_disk_gb
                ));
            }
            None if !unknown_ok => {
                reasons.push(format!(
                    "Needs {} GB free disk, free space could not be read",
                    m.min_free_disk_gb
                ));
            }
            _ => {}
        }
    }
    if m.needs_gpu && !hw.gpu_accel {
        let mut reason = String::from("Needs a GPU this app can use");
        if hw.os.to_lowercase().contains("windows") {
            reason.push_str("; Windows build runs on CPU");
        }
        reasons.push(reason);
    } else if !m.needs_gpu && !hw.gpu_accel && hw.threads < m.min_threads_without_gpu {
        reasons.push(format!(
            "Needs Apple Silicon or at least {} CPU threads, this PC has {}",
            m.min_threads_without_gpu, hw.threads
        ));
    }
    reasons
}

pub fn evaluate(hw: &Hardware, models_dir: &Path) -> Vec<Fit> {
    let mut fits = Vec::new();
    for m in CATALOG.iter() {
        let downloaded = path(models_dir, m.id).is_file();
        let reasons = reasons_against(m, hw, downloaded);
        fits.push(Fit {
            id: m.id,
            label: m.label.to_string(),
            // MiB, which matches the 181 / 514 / 1031 labels in the spec.
            size_mb: m.size_bytes / (1024 * 1024),
            supported: reasons.is_empty(),
            recommended: false,
            downloaded,
            reasons,
        });
    }
    // Catalog is ordered small to large, so the last supported model is the largest.
    if let Some(best) = fits.iter_mut().rev().find(|f| f.supported) {
        best.recommended = true;
    }
    fits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hwprobe::Hardware;
    fn hw(ram: Option<u64>, threads: u32, gpu: bool, disk: Option<f64>) -> Hardware {
        Hardware {
            os: "x".into(),
            arch: if gpu {
                "aarch64".into()
            } else {
                "x86_64".into()
            },
            cpu: "c".into(),
            threads,
            ram_gb: ram,
            gpu: "-".into(),
            gpu_accel: gpu,
            free_disk_gb: disk,
        }
    }
    fn fits(h: &Hardware) -> Vec<Fit> {
        evaluate(h, &std::env::temp_dir().join("lstt-no-models"))
    }
    #[test]
    fn m4_pro_gets_large_recommended() {
        let f = fits(&hw(Some(24), 12, true, Some(400.0)));
        assert!(f.iter().all(|x| x.supported));
        assert_eq!(
            f.iter()
                .filter(|x| x.recommended)
                .map(|x| x.id)
                .collect::<Vec<_>>(),
            vec![ModelId::LargeV3]
        );
    }
    #[test]
    fn windows_8gb_quad_core_gets_small_only() {
        let f = fits(&hw(Some(8), 4, false, Some(100.0)));
        assert!(f[0].supported && f[0].recommended);
        assert!(!f[1].supported && f[1].reasons.iter().any(|r| r.contains("8 CPU threads")));
        assert!(!f[2].supported && f[2].reasons.iter().any(|r| r.contains("16 GB RAM")));
        assert!(f[2].reasons.iter().any(|r| r.contains("GPU")));
    }
    #[test]
    fn low_disk_blocks_download() {
        let f = fits(&hw(Some(32), 16, true, Some(1.0)));
        assert!(!f[2].supported && f[2].reasons.iter().any(|r| r.contains("free disk")));
    }
    #[test]
    fn unknown_ram_is_not_treated_as_zero() {
        let f = fits(&hw(None, 8, true, Some(100.0)));
        assert!(
            f[0].supported,
            "small must stay usable when RAM could not be read"
        );
    }
    #[test]
    fn urls_are_pinned_https_huggingface() {
        for m in CATALOG.iter() {
            assert!(
                url(m.id).starts_with("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/")
            );
            assert_eq!(m.sha256.len(), 64);
        }
    }
    #[test]
    fn unknown_ram_and_disk_block_medium_and_large() {
        let f = fits(&hw(None, 12, true, None));
        assert!(f[0].supported && !f[1].supported && !f[2].supported);
    }
    #[test]
    fn downloaded_model_skips_disk_check_and_is_flagged() {
        let dir = std::env::temp_dir().join(format!("lstt-have-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path(&dir, ModelId::Small), b"x").unwrap();
        let f = evaluate(&hw(Some(8), 4, false, Some(0.1)), &dir);
        assert!(f[0].downloaded && f[0].supported);
        assert!(!f[1].downloaded);
    }
    #[test]
    fn size_mb_matches_spec_labels() {
        let f = fits(&hw(Some(24), 12, true, Some(400.0)));
        assert_eq!(
            f.iter().map(|x| x.size_mb).collect::<Vec<_>>(),
            vec![181, 514, 1031]
        );
    }
}
