//! local-stt: free, local push-to-talk speech-to-text.
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod audio;
pub mod autostart;
pub mod commands;
pub mod config;
pub mod download;
pub mod engine;
pub mod history;
pub mod hotkey;
pub mod hwprobe;
pub mod models;
pub mod output;
pub mod overlay;
pub mod paths;
pub mod recorder;
pub mod sound;
pub mod state;
