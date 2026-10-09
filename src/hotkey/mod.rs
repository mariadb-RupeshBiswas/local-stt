//! Configurable push-to-talk combo: pure matcher plus OS hooks. Owned by Task 4.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::mpsc::Sender;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
    Fn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub modifiers: Vec<Modifier>,
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
    Cancel,
    Captured(Combo),
}

pub struct KeyState {
    pub modifiers: BTreeSet<Modifier>,
    pub key: Option<String>,
    pub escape: bool,
}

pub struct Matcher {
    _private: (),
}

impl Matcher {
    pub fn new(_combo: Combo) -> Matcher {
        todo!("Task 4")
    }

    pub fn set_combo(&mut self, _combo: Combo) {
        todo!("Task 4")
    }

    pub fn update(&mut self, _state: &KeyState) -> Option<HotkeyEvent> {
        todo!("Task 4")
    }
}

pub struct Hook {
    _private: (),
}

impl Hook {
    pub fn set_combo(&self, _combo: Combo) {
        todo!("Task 4")
    }

    pub fn set_capture(&self, _on: bool) {
        todo!("Task 4")
    }
}

pub fn default_combo() -> Combo {
    todo!("Task 4")
}

pub fn validate(_c: &Combo) -> Result<(), String> {
    todo!("Task 4")
}

pub fn display(_c: &Combo) -> String {
    todo!("Task 4")
}

pub fn start(_combo: Combo, _tx: Sender<HotkeyEvent>) -> Result<Hook, String> {
    todo!("Task 4")
}

pub fn accessibility_ok(_prompt: bool) -> bool {
    todo!("Task 4")
}
