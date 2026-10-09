//! Dictation history as JSON Lines. Owned by Task 6.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub ts_ms: u64,
    pub text: String,
    pub duration_ms: u64,
    pub model: String,
    pub ok: bool,
}

pub fn append(_path: &Path, _e: &Entry) -> std::io::Result<()> {
    todo!("Task 6")
}

pub fn read_newest_first(_path: &Path, _limit: usize) -> std::io::Result<Vec<Entry>> {
    todo!("Task 6")
}

pub fn clear(_path: &Path) -> std::io::Result<()> {
    todo!("Task 6")
}
