//! Echo cancellation for notes: the call audio is the reference, and its sound is taken out of the
//! microphone before the microphone is transcribed. WebRTC's AEC3 through the pure-Rust `sonora`.

use sonora::config::EchoCanceller;
use sonora::{AudioProcessing, Config, StreamConfig};
use std::collections::VecDeque;

/// 10 ms at 16 kHz, the frame size AEC3 works in.
const FRAME: usize = 160;
/// More than this much call audio waiting for the microphone means the clocks drifted; the oldest goes.
const MAX_RENDER: usize = 16_000;
// Tiny noise on the reference keeps AEC3's filters out of slow denormal numbers while the call is quiet (sonora issue #34).
const DITHER: f32 = 1.0e-6;

pub struct Echo {
    apm: AudioProcessing,
    render: VecDeque<f32>,
    mic: Vec<f32>,
    // Call audio still to drop so both clocks line up.
    skip: usize,
    seed: u32,
}

impl Default for Echo {
    fn default() -> Self {
        Self::new()
    }
}

impl Echo {
    pub fn new() -> Self {
        let config = Config {
            echo_canceller: Some(EchoCanceller::default()),
            ..Default::default()
        };
        let apm = AudioProcessing::builder()
            .config(config)
            .capture_config(StreamConfig::new(16_000, 1))
            .render_config(StreamConfig::new(16_000, 1))
            .build();
        Echo {
            apm,
            render: VecDeque::new(),
            mic: Vec::new(),
            skip: 0,
            seed: 0x1234_5678,
        }
    }

    /// Call audio as it was played, on the note's clock (quiet stretches as zeros).
    pub fn render(&mut self, audio: &[f32]) {
        let dropped = self.skip.min(audio.len());
        self.skip -= dropped;
        self.render.extend(&audio[dropped..]);
        let extra = self.render.len().saturating_sub(MAX_RENDER);
        self.render.drain(..extra);
    }

    /// Lines the two clocks up once: call audio that started `lead_ms` before the microphone is skipped.
    pub fn align(&mut self, lead_ms: i64) {
        let samples = (lead_ms.unsigned_abs() as usize).saturating_mul(16);
        if lead_ms > 0 {
            let n = samples.min(self.render.len());
            self.render.drain(..n);
            self.skip += samples - n;
        } else {
            for _ in 0..samples {
                self.render.push_front(0.0);
            }
        }
    }

    fn dither(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 * DITHER - DITHER
    }

    /// The microphone with the call's echo taken out; whole 10 ms frames only, the rest waits for the next call.
    pub fn capture(&mut self, audio: &[f32]) -> Vec<f32> {
        self.mic.extend_from_slice(audio);
        let frames = self.mic.len() / FRAME;
        let mut out = Vec::with_capacity(frames * FRAME);
        let mut far = vec![0.0f32; FRAME];
        let mut far_out = vec![0.0f32; FRAME];
        let mut clean = vec![0.0f32; FRAME];
        for f in 0..frames {
            for slot in far.iter_mut() {
                let played = self.render.pop_front().unwrap_or(0.0);
                *slot = played + self.dither();
            }
            let near = &self.mic[f * FRAME..(f + 1) * FRAME];
            let rendered = self.apm.process_render_f32(&[&far], &mut [&mut far_out]);
            let captured = self.apm.process_capture_f32(&[near], &mut [&mut clean]);
            // A rejected frame passes through untouched rather than dropping the user's words.
            if rendered.is_ok() && captured.is_ok() {
                out.extend_from_slice(&clean);
            } else {
                out.extend_from_slice(near);
            }
        }
        self.mic.drain(..frames * FRAME);
        out
    }

    /// How much echo AEC3 reports removing, in dB; None until it has measured.
    pub fn erle_db(&self) -> Option<f64> {
        self.apm.statistics().echo_return_loss_enhancement
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Speech-like noise: random samples in bursts, so the canceller sees talk and pauses.
    fn far_end(seconds: usize) -> Vec<f32> {
        let mut seed = 42u32;
        (0..seconds * 16_000)
            .map(|i| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let noise = ((seed >> 9) as f32 / (1u32 << 23) as f32) * 2.0 - 1.0;
                let talking = (i / 8_000) % 3 != 2;
                if talking {
                    noise * 0.3
                } else {
                    0.0
                }
            })
            .collect()
    }

    fn energy(x: &[f32]) -> f64 {
        x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64
    }

    #[test]
    fn the_calls_sound_is_taken_out_of_the_microphone() {
        let far = far_end(12);
        // The microphone hears the speakers 60 ms later at half volume.
        let delay = 60 * 16;
        let mic: Vec<f32> = (0..far.len())
            .map(|i| {
                if i >= delay {
                    far[i - delay] * 0.5
                } else {
                    0.0
                }
            })
            .collect();
        let mut echo = Echo::new();
        let mut out = Vec::new();
        // Feed in 32 ms pieces like the note loop, call audio first.
        for (f, m) in far.chunks(512).zip(mic.chunks(512)) {
            echo.render(f);
            out.extend(echo.capture(m));
        }
        let tail = 8 * 16_000;
        let before = energy(&mic[tail..out.len()]);
        let after = energy(&out[tail..]);
        let removed_db = 10.0 * (before / after.max(1e-12)).log10();
        eprintln!(
            "echo removed: {removed_db:.1} dB, AEC3 reports {:?} dB",
            echo.erle_db()
        );
        assert!(removed_db >= 15.0, "removed {removed_db:.1} dB");
    }

    // A small room: the speaker's sound arrives late, bounces for ~250 ms, and the speaker distorts loud parts.
    fn through_a_room(far: &[f32]) -> Vec<f32> {
        let mut seed = 99u32;
        let taps: Vec<f32> = (0..4_000)
            .map(|k| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let n = ((seed >> 9) as f32 / (1u32 << 23) as f32) * 2.0 - 1.0;
                n * 0.08 * (-(k as f32) / 900.0).exp()
            })
            .collect();
        let delay = 45 * 16;
        let mut out = vec![0.0f32; far.len()];
        for (i, o) in out.iter_mut().enumerate() {
            let mut acc = if i >= delay {
                far[i - delay] * 0.6
            } else {
                0.0
            };
            for (k, t) in taps.iter().enumerate().step_by(8) {
                if i >= delay + k {
                    acc += far[i - delay - k] * t;
                }
            }
            *o = (acc * 1.5).tanh() / 1.5;
        }
        out
    }

    #[test]
    fn echo_through_a_room_is_cut_while_both_people_talk() {
        let far = far_end(14);
        let room = through_a_room(&far);
        let mut seed = 5u32;
        // The user talks from 9 s to 11 s, over the call.
        let near: Vec<f32> = (0..far.len())
            .map(|i| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let n = ((seed >> 9) as f32 / (1u32 << 23) as f32) * 2.0 - 1.0;
                if (9 * 16_000..11 * 16_000).contains(&i) {
                    n * 0.2
                } else {
                    0.0
                }
            })
            .collect();
        let mic: Vec<f32> = room.iter().zip(&near).map(|(r, n)| r + n).collect();
        let mut echo = Echo::new();
        let mut out = Vec::new();
        for (f, m) in far.chunks(512).zip(mic.chunks(512)) {
            echo.render(f);
            out.extend(echo.capture(m));
        }
        let echo_only = 6 * 16_000..9 * 16_000;
        let removed_db =
            10.0 * (energy(&room[echo_only.clone()]) / energy(&out[echo_only]).max(1e-12)).log10();
        let talk = 9 * 16_000 + 4_000..11 * 16_000;
        let kept_db = 10.0 * (energy(&out[talk.clone()]) / energy(&near[talk])).log10();
        eprintln!(
            "room echo removed: {removed_db:.1} dB; your voice during double talk: {kept_db:.1} dB"
        );
        assert!(
            removed_db >= 10.0,
            "room echo removed only {removed_db:.1} dB"
        );
        // AEC3 tones down overlap (about 7 dB here, noise on noise is its worst case); the words must still count as speech.
        let kept_rms = energy(&out[9 * 16_000 + 4_000..11 * 16_000]).sqrt() as f32;
        assert!(
            kept_rms >= 0.02,
            "your voice fell to rms {kept_rms:.3}, near the 0.01 speech line ({kept_db:.1} dB)"
        );
    }

    #[test]
    fn your_own_voice_stays_when_the_call_is_quiet() {
        let mut echo = Echo::new();
        let mut seed = 7u32;
        let voice: Vec<f32> = (0..6 * 16_000)
            .map(|i| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (((seed >> 9) as f32 / (1u32 << 23) as f32) * 2.0 - 1.0)
                    * 0.2
                    * ((i as f32) * 0.01).sin().abs()
            })
            .collect();
        let mut out = Vec::new();
        for v in voice.chunks(512) {
            echo.render(&vec![0.0; v.len()]);
            out.extend(echo.capture(v));
        }
        let kept_db = 10.0 * (energy(&out[16_000..]) / energy(&voice[16_000..out.len()])).log10();
        eprintln!("own voice change: {kept_db:.1} dB");
        assert!(kept_db > -3.0, "own voice lost {kept_db:.1} dB");
    }

    #[test]
    fn frames_wait_for_ten_milliseconds_and_alignment_skips_lead() {
        let mut echo = Echo::new();
        assert!(echo.capture(&[0.1; 100]).is_empty());
        assert_eq!(echo.capture(&[0.1; 100]).len(), FRAME);
        echo.render(&[0.5; 1_600]);
        echo.align(50);
        assert_eq!(echo.render.len(), 800);
        echo.align(-10);
        assert_eq!(echo.render.len(), 960);
        echo.render(&vec![0.0; 40_000]);
        assert_eq!(echo.render.len(), MAX_RENDER);
        let mut late = Echo::new();
        late.align(20);
        late.render(&[0.5; 500]);
        assert_eq!(
            late.render.len(),
            180,
            "call audio before the microphone's start is dropped as it arrives"
        );
    }
}
