//! User settings in config.json. Owned by Task 6.

use crate::hotkey::Combo;
use crate::models::ModelId;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Hold,
    Toggle,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Pill,
    Waveform,
    Minimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OverlayPos {
    pub monitor: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub hotkey: Combo,
    pub mode: Mode,
    pub translate: bool,
    pub language: String,
    pub paste: bool,
    pub restore_clipboard: bool,
    pub microphone: Option<String>,
    pub theme: Theme,
    pub show_overlay: bool,
    pub overlay_pos: Option<OverlayPos>,
    pub sounds: bool,
    pub save_history: bool,
    pub autostart: bool,
    pub model: ModelId,
}

impl Default for Config {
    fn default() -> Self {
        todo!("Task 6")
    }
}

pub fn load(_path: &Path) -> Config {
    todo!("Task 6")
}

pub fn save(_path: &Path, _cfg: &Config) -> std::io::Result<()> {
    todo!("Task 6")
}
