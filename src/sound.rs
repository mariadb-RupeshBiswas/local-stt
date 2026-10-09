//! Short start/stop/error cues, synthesized so no audio assets ship.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, I24, U24};
use std::time::Duration;

// -18 dBFS: audible over speech without startling anyone.
const AMPLITUDE: f32 = 0.125;
const FADE_MS: u32 = 10;
// Extra time after the last sample so the device can drain its buffer before the stream drops.
const DRAIN: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
    Error,
}

pub fn play(cue: Cue) {
    // A failed spawn only loses a beep, so the result is ignored.
    let _ = std::thread::Builder::new()
        .name("local-stt-cue".into())
        .spawn(move || {
            let _ = play_blocking(&cue);
        });
}

fn play_blocking(cue: &Cue) -> Option<()> {
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let config = supported.config();
    let mono = synth(cue, config.sample_rate);
    let play_time = Duration::from_secs_f64(mono.len() as f64 / config.sample_rate as f64);
    let channels = config.channels.max(1) as usize;
    let stream = match supported.sample_format() {
        SampleFormat::I8 => output::<i8>(&device, config, mono, channels),
        SampleFormat::I16 => output::<i16>(&device, config, mono, channels),
        SampleFormat::I24 => output::<I24>(&device, config, mono, channels),
        SampleFormat::I32 => output::<i32>(&device, config, mono, channels),
        SampleFormat::I64 => output::<i64>(&device, config, mono, channels),
        SampleFormat::U8 => output::<u8>(&device, config, mono, channels),
        SampleFormat::U16 => output::<u16>(&device, config, mono, channels),
        SampleFormat::U24 => output::<U24>(&device, config, mono, channels),
        SampleFormat::U32 => output::<u32>(&device, config, mono, channels),
        SampleFormat::U64 => output::<u64>(&device, config, mono, channels),
        SampleFormat::F32 => output::<f32>(&device, config, mono, channels),
        SampleFormat::F64 => output::<f64>(&device, config, mono, channels),
        _ => None,
    }?;
    stream.play().ok()?;
    std::thread::sleep(play_time + DRAIN);
    Some(())
}

fn output<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mono: Vec<f32>,
    channels: usize,
) -> Option<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let mut pos = 0usize;
    let on_data = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        for frame in data.chunks_mut(channels) {
            let value = match mono.get(pos) {
                Some(v) => T::from_sample(*v),
                None => T::EQUILIBRIUM,
            };
            pos += 1;
            frame.fill(value);
        }
    };
    // A dead output device only costs the cue, so errors are dropped.
    device
        .build_output_stream(config, on_data, |_: cpal::Error| {}, None)
        .ok()
}

fn ms_to_samples(ms: u32, rate: u32) -> usize {
    (rate as u64 * ms as u64 / 1000) as usize
}

// Sine pip with linear fades on both edges so it starts and ends without a click.
fn pip(freq_hz: f32, ms: u32, rate: u32) -> Vec<f32> {
    let n = ms_to_samples(ms, rate);
    let fade = ms_to_samples(FADE_MS, rate).max(1) as f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let phase = std::f32::consts::TAU * freq_hz * i as f32 / rate as f32;
        let edge = (i as f32).min((n - 1 - i) as f32);
        let gain = (edge / fade).min(1.0);
        out.push(phase.sin() * AMPLITUDE * gain);
    }
    out
}

fn synth(cue: &Cue, rate: u32) -> Vec<f32> {
    match cue {
        Cue::Start => pip(880.0, 60, rate),
        Cue::Stop => pip(660.0, 60, rate),
        Cue::Error => {
            let mut out = pip(330.0, 50, rate);
            out.resize(out.len() + ms_to_samples(50, rate), 0.0);
            out.extend(pip(330.0, 50, rate));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    #[test]
    fn start_is_60ms() {
        assert_eq!(synth(&Cue::Start, 48_000).len(), 2_880);
    }

    #[test]
    fn stop_is_60ms() {
        assert_eq!(synth(&Cue::Stop, 44_100).len(), 2_646);
    }

    #[test]
    fn error_is_two_pips_with_a_gap() {
        let s = synth(&Cue::Error, 48_000);
        // 50 ms pip + 50 ms gap + 50 ms pip
        assert_eq!(s.len(), 7_200);
        assert_eq!(s[2_400..4_800].iter().filter(|v| **v != 0.0).count(), 0);
        assert!(peak(&s[4_800..]) > 0.05);
    }

    #[test]
    fn peaks_stay_below_minus_18_dbfs_plus_margin() {
        for cue in [Cue::Start, Cue::Stop, Cue::Error] {
            let p = peak(&synth(&cue, 48_000));
            assert!(p > 0.05 && p < 0.13, "{cue:?} peak {p}");
        }
    }

    #[test]
    fn edges_fade_to_silence() {
        let s = synth(&Cue::Start, 48_000);
        assert_eq!(s[0], 0.0);
        assert!(s[s.len() - 1].abs() < 0.01);
    }
}
