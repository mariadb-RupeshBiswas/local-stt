//! Cuts one 16 kHz mono channel into speech segments at pauses, so each can be transcribed on its own.

use crate::audio::rms;

const RATE: u64 = 16_000;
// 30 ms frames, the same window the dictation silence check uses.
const FRAME: usize = 480;
const SPEECH_RMS: f32 = 0.01;
// A pause this long ends a segment.
const PAUSE_FRAMES: usize = 20;
// Whisper works on up to 30 s at a time; cut long monologues a little before that.
const MAX_FRAMES: usize = 833;
// Keeps the start of the first word, which is often quieter than the threshold.
const PREROLL_FRAMES: usize = 10;
// Clicks and coughs shorter than this are not worth a transcription pass.
const MIN_SPEECH_FRAMES: usize = 10;

pub struct Chunk {
    /// Milliseconds from the start of the note.
    pub start_ms: u64,
    pub pcm: Vec<f32>,
}

#[derive(Default)]
pub struct Segmenter {
    partial: Vec<f32>,
    preroll: Vec<f32>,
    current: Vec<f32>,
    start_sample: u64,
    consumed: u64,
    in_speech: bool,
    quiet_run: usize,
    speech_frames: usize,
}

impl Segmenter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the next samples and returns any segments they completed.
    pub fn feed(&mut self, samples: &[f32]) -> Vec<Chunk> {
        self.partial.extend_from_slice(samples);
        let mut done = Vec::new();
        let whole = self.partial.len() / FRAME * FRAME;
        let frames: Vec<f32> = self.partial.drain(..whole).collect();
        for frame in frames.chunks(FRAME) {
            if let Some(chunk) = self.frame(frame) {
                done.push(chunk);
            }
        }
        done
    }

    /// Ends the channel and returns the segment still open, if it held speech.
    pub fn flush(&mut self) -> Option<Chunk> {
        let rest = std::mem::take(&mut self.partial);
        if self.in_speech {
            self.current.extend_from_slice(&rest);
            return self.close();
        }
        None
    }

    fn frame(&mut self, frame: &[f32]) -> Option<Chunk> {
        let frame_start = self.consumed;
        self.consumed += frame.len() as u64;
        let speech = rms(frame) >= SPEECH_RMS;
        if !self.in_speech {
            if speech {
                self.in_speech = true;
                self.start_sample = frame_start - self.preroll.len() as u64;
                self.current = std::mem::take(&mut self.preroll);
                self.current.extend_from_slice(frame);
                self.quiet_run = 0;
                self.speech_frames = 1;
            } else {
                self.preroll.extend_from_slice(frame);
                let extra = self.preroll.len().saturating_sub(PREROLL_FRAMES * FRAME);
                self.preroll.drain(..extra);
            }
            return None;
        }
        self.current.extend_from_slice(frame);
        if speech {
            self.quiet_run = 0;
            self.speech_frames += 1;
        } else {
            self.quiet_run += 1;
        }
        if self.quiet_run >= PAUSE_FRAMES {
            return self.close();
        }
        if self.current.len() >= MAX_FRAMES * FRAME {
            // Still talking: this piece ends here and the next starts at once.
            let chunk = self.close();
            self.in_speech = true;
            self.start_sample = self.consumed;
            return chunk;
        }
        None
    }

    fn close(&mut self) -> Option<Chunk> {
        self.in_speech = false;
        self.quiet_run = 0;
        let pcm = std::mem::take(&mut self.current);
        // The pause that ended this segment is the lead-in for the next one.
        self.preroll = pcm[pcm.len().saturating_sub(PREROLL_FRAMES * FRAME)..].to_vec();
        let enough = self.speech_frames >= MIN_SPEECH_FRAMES;
        self.speech_frames = 0;
        enough.then(|| Chunk {
            start_ms: self.start_sample * 1000 / RATE,
            pcm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: u64) -> Vec<f32> {
        let n = (ms * RATE / 1000) as usize;
        (0..n).map(|i| 0.2 * ((i as f32) * 0.05).sin()).collect()
    }

    fn quiet(ms: u64) -> Vec<f32> {
        vec![0.0; (ms * RATE / 1000) as usize]
    }

    #[test]
    fn a_pause_ends_a_segment() {
        let mut s = Segmenter::new();
        let mut audio = quiet(1_000);
        audio.extend(tone(2_000));
        audio.extend(quiet(1_000));
        let chunks = s.feed(&audio);
        assert_eq!(chunks.len(), 1);
        // starts at about 1 s, minus the 300 ms kept before the first word
        assert!(
            (690..=720).contains(&chunks[0].start_ms),
            "{}",
            chunks[0].start_ms
        );
        assert!(s.flush().is_none());
    }

    #[test]
    fn two_utterances_are_two_segments_with_their_own_times() {
        let mut s = Segmenter::new();
        let mut audio = tone(1_000);
        audio.extend(quiet(800));
        audio.extend(tone(1_000));
        audio.extend(quiet(800));
        let starts: Vec<u64> = s.feed(&audio).iter().map(|c| c.start_ms).collect();
        assert_eq!(starts.len(), 2, "{starts:?}");
        assert_eq!(starts[0], 0);
        assert!((1_480..=1_520).contains(&starts[1]), "{starts:?}");
    }

    #[test]
    fn a_long_monologue_is_cut_under_thirty_seconds() {
        let mut s = Segmenter::new();
        let chunks = s.feed(&tone(40_000));
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].pcm.len() <= 30 * RATE as usize);
        let rest = s.flush().expect("the rest of the monologue");
        assert!(rest.start_ms >= 24_000);
    }

    #[test]
    fn clicks_and_silence_make_nothing() {
        let mut s = Segmenter::new();
        let mut audio = quiet(500);
        audio.extend(tone(60));
        audio.extend(quiet(1_000));
        assert!(s.feed(&audio).is_empty());
        assert!(s.flush().is_none());
    }

    #[test]
    fn feeding_in_odd_sized_pieces_gives_the_same_result() {
        let mut audio = quiet(300);
        audio.extend(tone(1_500));
        audio.extend(quiet(900));
        let mut s = Segmenter::new();
        let mut chunks = Vec::new();
        for piece in audio.chunks(337) {
            chunks.extend(s.feed(piece));
        }
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_ms, 0);
    }

    #[test]
    fn flush_returns_speech_still_open() {
        let mut s = Segmenter::new();
        assert!(s.feed(&tone(1_000)).is_empty());
        assert!(s.flush().is_some());
    }
}
