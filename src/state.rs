//! Pure recording state machine: hold, toggle, cancel, short taps, max length.

use crate::config::Mode;

pub const MIN_HOLD_MS: u64 = 300;
pub const MAX_RECORDING_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Pressed,
    Released,
    Cancel,
    Timeout,
    TranscribeDone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    StopAndTranscribe,
    CancelRecording,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Recording { since_ms: u64 },
    Transcribing,
}

pub struct Machine {
    mode: Mode,
    phase: Phase,
}

impl Machine {
    pub fn new(mode: Mode) -> Machine {
        Machine {
            mode,
            phase: Phase::Idle,
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.phase, Phase::Recording { .. })
    }

    pub fn recording_since(&self) -> Option<u64> {
        match self.phase {
            Phase::Recording { since_ms } => Some(since_ms),
            _ => None,
        }
    }

    pub fn on(&mut self, input: Input, now_ms: u64) -> Action {
        match (self.phase, input) {
            (Phase::Idle, Input::Pressed) => {
                self.phase = Phase::Recording { since_ms: now_ms };
                Action::StartRecording
            }
            (Phase::Recording { since_ms }, Input::Released) if self.mode == Mode::Hold => {
                self.finish(now_ms.saturating_sub(since_ms))
            }
            (Phase::Recording { since_ms }, Input::Pressed) if self.mode == Mode::Toggle => {
                self.finish(now_ms.saturating_sub(since_ms))
            }
            (Phase::Recording { .. }, Input::Timeout) => {
                self.phase = Phase::Transcribing;
                Action::StopAndTranscribe
            }
            (Phase::Recording { .. }, Input::Cancel) => {
                self.phase = Phase::Idle;
                Action::CancelRecording
            }
            (Phase::Transcribing, Input::TranscribeDone) => {
                self.phase = Phase::Idle;
                Action::Idle
            }
            _ => Action::None,
        }
    }

    fn finish(&mut self, held_ms: u64) -> Action {
        if held_ms < MIN_HOLD_MS {
            self.phase = Phase::Idle;
            return Action::CancelRecording;
        }
        self.phase = Phase::Transcribing;
        Action::StopAndTranscribe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_flow() {
        let mut s = Machine::new(Mode::Hold);
        assert_eq!(s.on(Input::Pressed, 0), Action::StartRecording);
        assert_eq!(s.on(Input::Released, 2_000), Action::StopAndTranscribe);
        assert_eq!(s.on(Input::TranscribeDone, 2_600), Action::Idle);
    }

    #[test]
    fn short_tap_ignored() {
        let mut s = Machine::new(Mode::Hold);
        s.on(Input::Pressed, 0);
        assert_eq!(s.on(Input::Released, 200), Action::CancelRecording);
        assert_eq!(s.on(Input::Pressed, 500), Action::StartRecording);
    }

    #[test]
    fn press_during_transcribing_is_ignored() {
        let mut s = Machine::new(Mode::Hold);
        s.on(Input::Pressed, 0);
        s.on(Input::Released, 1_000);
        assert_eq!(s.on(Input::Pressed, 1_100), Action::None);
        assert_eq!(s.on(Input::Released, 1_200), Action::None);
        assert_eq!(s.on(Input::TranscribeDone, 1_500), Action::Idle);
    }

    #[test]
    fn toggle_flow() {
        let mut s = Machine::new(Mode::Toggle);
        assert_eq!(s.on(Input::Pressed, 0), Action::StartRecording);
        assert_eq!(s.on(Input::Released, 100), Action::None);
        assert_eq!(s.on(Input::Pressed, 3_000), Action::StopAndTranscribe);
        assert_eq!(s.on(Input::Released, 3_100), Action::None);
    }

    #[test]
    fn cancel_while_recording() {
        let mut s = Machine::new(Mode::Hold);
        s.on(Input::Pressed, 0);
        assert_eq!(s.on(Input::Cancel, 500), Action::CancelRecording);
        assert!(!s.is_recording());
    }

    #[test]
    fn cancel_when_idle_does_nothing() {
        let mut s = Machine::new(Mode::Hold);
        assert_eq!(s.on(Input::Cancel, 0), Action::None);
    }

    #[test]
    fn timeout_stops_long_recording() {
        let mut s = Machine::new(Mode::Toggle);
        s.on(Input::Pressed, 0);
        assert_eq!(
            s.on(Input::Timeout, MAX_RECORDING_MS),
            Action::StopAndTranscribe
        );
    }
}
