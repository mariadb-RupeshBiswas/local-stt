//! Model catalog and hardware fit. Owned by Task 6.

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

pub fn info(_id: ModelId) -> &'static ModelInfo {
    todo!("Task 6")
}

pub fn url(_id: ModelId) -> String {
    todo!("Task 6")
}

pub fn path(_models_dir: &Path, _id: ModelId) -> PathBuf {
    todo!("Task 6")
}

pub fn evaluate(_hw: &crate::hwprobe::Hardware, _models_dir: &Path) -> Vec<Fit> {
    todo!("Task 6")
}
