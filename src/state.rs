//! Pure recording state machine: push-to-talk, hands-free, cancel, short taps, max length.

pub const MIN_HOLD_MS: u64 = 300;
pub const MAX_RECORDING_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Pressed,
    Released,
    Toggle,
    Cancel,
    Timeout,
    TranscribeDone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    /// A push-to-talk hold became hands-free; keep recording.
    HandsFree,
    StopAndTranscribe,
    CancelRecording,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Recording { since_ms: u64, hands_free: bool },
    Transcribing,
}

pub struct Machine {
    phase: Phase,
}

impl Default for Machine {
    fn default() -> Self {
        Machine::new()
    }
}

impl Machine {
    pub fn new() -> Machine {
        Machine { phase: Phase::Idle }
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.phase, Phase::Recording { .. })
    }

    pub fn is_hands_free(&self) -> bool {
        matches!(
            self.phase,
            Phase::Recording {
                hands_free: true,
                ..
            }
        )
    }

    pub fn recording_since(&self) -> Option<u64> {
        match self.phase {
            Phase::Recording { since_ms, .. } => Some(since_ms),
            _ => None,
        }
    }

    pub fn on(&mut self, input: Input, now_ms: u64) -> Action {
        match (self.phase, input) {
            (Phase::Idle, Input::Pressed) => self.start(now_ms, false),
            (Phase::Idle, Input::Toggle) => self.start(now_ms, true),
            (
                Phase::Recording {
                    since_ms,
                    hands_free: false,
                },
                Input::Toggle,
            ) => {
                self.phase = Phase::Recording {
                    since_ms,
                    hands_free: true,
                };
                Action::HandsFree
            }
            (
                Phase::Recording {
                    since_ms,
                    hands_free: false,
                },
                Input::Released,
            ) => self.finish(now_ms.saturating_sub(since_ms)),
            (
                Phase::Recording {
                    since_ms,
                    hands_free: true,
                },
                Input::Toggle,
            ) => self.finish(now_ms.saturating_sub(since_ms)),
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

    fn start(&mut self, now_ms: u64, hands_free: bool) -> Action {
        self.phase = Phase::Recording {
            since_ms: now_ms,
            hands_free,
        };
        Action::StartRecording
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
        let mut s = Machine::new();
        assert_eq!(s.on(Input::Pressed, 0), Action::StartRecording);
        assert_eq!(s.on(Input::Released, 2_000), Action::StopAndTranscribe);
        assert_eq!(s.on(Input::TranscribeDone, 2_600), Action::Idle);
    }

    #[test]
    fn short_tap_ignored() {
        let mut s = Machine::new();
        s.on(Input::Pressed, 0);
        assert_eq!(s.on(Input::Released, 200), Action::CancelRecording);
        assert_eq!(s.on(Input::Pressed, 500), Action::StartRecording);
    }

    #[test]
    fn press_during_transcribing_is_ignored() {
        let mut s = Machine::new();
        s.on(Input::Pressed, 0);
        s.on(Input::Released, 1_000);
        assert_eq!(s.on(Input::Pressed, 1_100), Action::None);
        assert_eq!(s.on(Input::Toggle, 1_150), Action::None);
        assert_eq!(s.on(Input::Released, 1_200), Action::None);
        assert_eq!(s.on(Input::TranscribeDone, 1_500), Action::Idle);
    }

    #[test]
    fn hands_free_flow() {
        let mut s = Machine::new();
        assert_eq!(s.on(Input::Toggle, 0), Action::StartRecording);
        assert_eq!(s.on(Input::Released, 100), Action::None);
        assert_eq!(s.on(Input::Pressed, 2_900), Action::None);
        assert_eq!(s.on(Input::Toggle, 3_000), Action::StopAndTranscribe);
    }

    #[test]
    fn hold_grows_into_hands_free_and_keeps_recording() {
        let mut s = Machine::new();
        assert_eq!(s.on(Input::Pressed, 0), Action::StartRecording);
        // Space joins Fn+Shift: the hotkey layer sends Toggle before Released.
        assert_eq!(s.on(Input::Toggle, 150), Action::HandsFree);
        assert_eq!(s.on(Input::Released, 150), Action::None);
        assert!(s.is_recording());
        assert_eq!(s.on(Input::Pressed, 4_000), Action::None);
        assert_eq!(s.on(Input::Toggle, 4_050), Action::StopAndTranscribe);
    }

    #[test]
    fn quick_double_toggle_is_a_tap() {
        let mut s = Machine::new();
        s.on(Input::Toggle, 0);
        assert_eq!(s.on(Input::Toggle, 120), Action::CancelRecording);
    }

    #[test]
    fn cancel_while_recording() {
        let mut s = Machine::new();
        s.on(Input::Pressed, 0);
        assert_eq!(s.on(Input::Cancel, 500), Action::CancelRecording);
        assert!(!s.is_recording());
        s.on(Input::Toggle, 600);
        assert_eq!(s.on(Input::Cancel, 900), Action::CancelRecording);
    }

    #[test]
    fn cancel_when_idle_does_nothing() {
        let mut s = Machine::new();
        assert_eq!(s.on(Input::Cancel, 0), Action::None);
    }

    #[test]
    fn timeout_stops_long_recording() {
        let mut s = Machine::new();
        s.on(Input::Toggle, 0);
        assert_eq!(
            s.on(Input::Timeout, MAX_RECORDING_MS),
            Action::StopAndTranscribe
        );
    }
}
