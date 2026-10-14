//! Note summaries on this computer: which language models fit this hardware, and where they live.

pub mod run;
pub mod write;

use crate::hwprobe::Hardware;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum SummaryModelId {
    GraniteMicro,
    Gemma4E4b,
}

/// How the model wants a chat laid out; the C API's template list has no Gemma 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Granite,
    Gemma,
}

pub struct Info {
    pub id: SummaryModelId,
    pub label: &'static str,
    pub maker: &'static str,
    pub about: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub min_ram_gb: u64,
    /// Recommended only where the GPU runs it; on CPU it takes minutes.
    pub wants_gpu: bool,
    pub format: Format,
}

pub const CATALOG: [Info; 2] = [
    Info {
        id: SummaryModelId::GraniteMicro,
        label: "Granite 4.0 Micro",
        maker: "IBM",
        about: "Fastest and smallest. Good summaries; may guess who owns a task.",
        repo: "ibm-granite/granite-4.0-micro-GGUF",
        file: "granite-4.0-micro-Q4_K_M.gguf",
        sha256: "97c417dcc0534b0737c74016fb2af083cb17c3b51eaac621192d23961b7024eb",
        size_bytes: 2_099_502_528,
        min_ram_gb: 8,
        wants_gpu: false,
        format: Format::Granite,
    },
    Info {
        id: SummaryModelId::Gemma4E4b,
        label: "Gemma 4 E4B",
        maker: "Google",
        about: "Best summaries: catches deadlines and says when a task has no owner. Slower and larger.",
        repo: "google/gemma-4-E4B-it-qat-q4_0-gguf",
        file: "gemma-4-E4B_q4_0-it.gguf",
        sha256: "676c35070db6dbe52f93e9c864ee0fba4eddea94b9c875d9cb10daff453fbaee",
        size_bytes: 5_154_941_280,
        min_ram_gb: 16,
        wants_gpu: true,
        format: Format::Gemma,
    },
];

pub fn info(id: SummaryModelId) -> &'static Info {
    match id {
        SummaryModelId::GraniteMicro => &CATALOG[0],
        SummaryModelId::Gemma4E4b => &CATALOG[1],
    }
}

pub fn url(id: SummaryModelId) -> String {
    let m = info(id);
    format!("https://huggingface.co/{}/resolve/main/{}", m.repo, m.file)
}

/// The file's Hugging Face page, which shows the SHA-256 the download is checked against.
pub fn page_url(id: SummaryModelId) -> String {
    let m = info(id);
    format!("https://huggingface.co/{}/blob/main/{}", m.repo, m.file)
}

pub fn path(models_dir: &Path, id: SummaryModelId) -> PathBuf {
    models_dir.join(info(id).file)
}

pub fn delete(models_dir: &Path, id: SummaryModelId) -> Result<(), String> {
    std::fs::remove_file(path(models_dir, id))
        .map_err(|e| format!("cannot delete {}: {e}", info(id).label))
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fit {
    pub id: SummaryModelId,
    pub label: String,
    pub maker: String,
    pub about: String,
    pub size_mb: u64,
    pub supported: bool,
    pub recommended: bool,
    pub downloaded: bool,
    pub sha256: &'static str,
    pub reasons: Vec<String>,
    /// A warning that does not block: "slow on this computer".
    pub hint: Option<String>,
}

const DISK_SPARE_GB: f64 = 0.5;

// Reasons a model cannot run here; empty means supported.
fn reasons_against(m: &Info, hw: &Hardware, downloaded: bool) -> Vec<String> {
    let mut reasons = Vec::new();
    // Unknown RAM blocks the large model only.
    match hw.ram_gb {
        Some(ram) if ram < m.min_ram_gb => {
            reasons.push(format!(
                "Needs {} GB RAM, this computer has {ram} GB",
                m.min_ram_gb
            ));
        }
        None if m.wants_gpu => {
            reasons.push(format!(
                "Needs {} GB RAM, this computer's RAM could not be read",
                m.min_ram_gb
            ));
        }
        _ => {}
    }
    if !downloaded {
        let need = m.size_bytes as f64 / 1_073_741_824.0 + DISK_SPARE_GB;
        match hw.free_disk_gb {
            Some(disk) if disk < need => {
                reasons.push(format!(
                    "Needs {need:.1} GB free disk, this computer has {disk:.1} GB"
                ));
            }
            None if m.wants_gpu => {
                reasons.push(format!(
                    "Needs {need:.1} GB free disk, free space could not be read"
                ));
            }
            _ => {}
        }
    }
    reasons
}

pub fn evaluate(hw: &Hardware, models_dir: &Path) -> Vec<Fit> {
    let mut fits: Vec<Fit> = CATALOG
        .iter()
        .map(|m| {
            let downloaded = path(models_dir, m.id).is_file();
            let reasons = reasons_against(m, hw, downloaded);
            let hint = (m.wants_gpu && !hw.gpu_accel)
                .then(|| "Runs on the CPU here: several minutes per call".to_string());
            Fit {
                id: m.id,
                label: m.label.to_string(),
                maker: m.maker.to_string(),
                about: m.about.to_string(),
                size_mb: m.size_bytes / (1024 * 1024),
                supported: reasons.is_empty(),
                recommended: false,
                downloaded,
                sha256: m.sha256,
                reasons,
                hint,
            }
        })
        .collect();
    // The best model this computer runs well: the large one only where the GPU runs it.
    let best = fits
        .iter()
        .rposition(|f| f.supported && f.hint.is_none())
        .or_else(|| fits.iter().position(|f| f.supported));
    if let Some(i) = best {
        fits[i].recommended = true;
    }
    fits
}

/// The model a Summarize click uses: the chosen one if it is downloaded, else any downloaded one, best first.
pub fn pick(fits: &[Fit], chosen: Option<SummaryModelId>) -> Option<SummaryModelId> {
    if let Some(id) = chosen {
        if fits.iter().any(|f| f.id == id && f.downloaded) {
            return Some(id);
        }
    }
    let downloaded = |f: &&Fit| f.downloaded && f.supported;
    fits.iter()
        .filter(downloaded)
        .find(|f| f.recommended)
        .or_else(|| fits.iter().rev().find(downloaded))
        .map(|f| f.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(ram: Option<u64>, gpu: bool, disk: Option<f64>) -> Hardware {
        Hardware {
            os: "x".into(),
            arch: "x".into(),
            cpu: "c".into(),
            threads: 8,
            ram_gb: ram,
            gpu: "-".into(),
            gpu_accel: gpu,
            free_disk_gb: disk,
        }
    }

    fn fits(h: &Hardware) -> Vec<Fit> {
        evaluate(h, &std::env::temp_dir().join("lstt-no-summary-models"))
    }

    fn recommended(f: &[Fit]) -> Vec<SummaryModelId> {
        f.iter().filter(|x| x.recommended).map(|x| x.id).collect()
    }

    #[test]
    fn a_mac_with_16_gb_gets_gemma_recommended() {
        let f = fits(&hw(Some(24), true, Some(200.0)));
        assert!(f.iter().all(|x| x.supported && x.hint.is_none()));
        assert_eq!(recommended(&f), vec![SummaryModelId::Gemma4E4b]);
    }

    #[test]
    fn a_pc_without_gpu_gets_granite_and_a_slow_hint_on_gemma() {
        let f = fits(&hw(Some(32), false, Some(200.0)));
        assert_eq!(recommended(&f), vec![SummaryModelId::GraniteMicro]);
        assert!(f[1].supported && f[1].hint.is_some());
    }

    #[test]
    fn an_8_gb_mac_gets_granite_only() {
        let f = fits(&hw(Some(8), true, Some(200.0)));
        assert_eq!(recommended(&f), vec![SummaryModelId::GraniteMicro]);
        assert!(!f[1].supported && f[1].reasons[0].contains("16 GB RAM"));
    }

    #[test]
    fn too_little_disk_or_ram_recommends_nothing() {
        let f = fits(&hw(Some(4), true, Some(200.0)));
        assert!(recommended(&f).is_empty());
        let f = fits(&hw(Some(32), true, Some(1.0)));
        assert!(f.iter().all(|x| !x.supported));
    }

    #[test]
    fn unknown_ram_keeps_granite_usable() {
        let f = fits(&hw(None, true, Some(200.0)));
        assert!(f[0].supported && !f[1].supported);
        assert_eq!(recommended(&f), vec![SummaryModelId::GraniteMicro]);
    }

    #[test]
    fn urls_are_pinned_https_huggingface_pages() {
        for m in CATALOG.iter() {
            assert!(url(m.id).starts_with("https://huggingface.co/"));
            assert_eq!(url(m.id).replace("/resolve/", "/blob/"), page_url(m.id));
            assert_eq!(m.sha256.len(), 64);
        }
    }

    #[test]
    fn pick_prefers_the_chosen_downloaded_model_then_the_recommended_one() {
        let mut f = fits(&hw(Some(24), true, Some(200.0)));
        assert_eq!(pick(&f, None), None);
        f[0].downloaded = true;
        assert_eq!(pick(&f, None), Some(SummaryModelId::GraniteMicro));
        assert_eq!(
            pick(&f, Some(SummaryModelId::Gemma4E4b)),
            Some(SummaryModelId::GraniteMicro)
        );
        f[1].downloaded = true;
        assert_eq!(pick(&f, None), Some(SummaryModelId::Gemma4E4b));
        assert_eq!(
            pick(&f, Some(SummaryModelId::GraniteMicro)),
            Some(SummaryModelId::GraniteMicro)
        );
    }
}
