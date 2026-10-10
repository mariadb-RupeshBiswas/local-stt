//! Microphone capture on a dedicated thread.

use crate::audio::{resample_linear, to_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, SampleFormat, SizedSample, I24, U24};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const TARGET_HZ: u32 = 16_000;
// Level callbacks fire roughly every 33 ms of captured audio.
const LEVELS_PER_SEC: u32 = 30;
// Raw mic RMS is small; the gain makes normal speech fill most of the meter.
const LEVEL_GAIN: f32 = 4.0;

pub struct Recording {
    pub samples_16k: Vec<f32>,
    pub duration_ms: u64,
}

type Shared = Arc<Mutex<Vec<f32>>>;
type Ready = Result<(u16, u32), String>;

pub struct Recorder {
    stop_tx: Sender<()>,
    thread: JoinHandle<()>,
    samples: Shared,
    channels: u16,
    rate: u32,
}

impl Recorder {
    pub fn list_inputs() -> Vec<String> {
        let Ok(devices) = cpal::default_host().input_devices() else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for device in devices {
            if let Ok(desc) = device.description() {
                names.push(desc.name().to_string());
            }
        }
        names
    }

    pub fn start(
        device: Option<&str>,
        on_level: Box<dyn Fn(f32) + Send + 'static>,
    ) -> Result<Recorder, String> {
        let samples: Shared = Arc::new(Mutex::new(Vec::new()));
        let (ready_tx, ready_rx) = channel::<Ready>();
        let (stop_tx, stop_rx) = channel::<()>();
        let device_name = device.map(str::to_string);
        let shared = samples.clone();
        let thread = std::thread::Builder::new()
            .name("local-stt-capture".into())
            .spawn(move || capture_thread(device_name, shared, on_level, ready_tx, stop_rx))
            .map_err(|e| format!("Could not start audio thread: {e}"))?;
        // Blocks through the first-run permission prompt, which can take a while.
        let (channels, rate) = match ready_rx.recv() {
            Ok(Ok(fmt)) => fmt,
            Ok(Err(msg)) => {
                let _ = thread.join();
                return Err(msg);
            }
            Err(_) => return Err("Audio thread stopped unexpectedly".into()),
        };
        Ok(Recorder {
            stop_tx,
            thread,
            samples,
            channels,
            rate,
        })
    }

    pub fn stop(self) -> Recording {
        let (channels, rate) = (self.channels, self.rate);
        let interleaved = self.end();
        finish(interleaved, channels, rate)
    }

    /// The last `secs` seconds as 16 kHz mono, copied without stopping the capture.
    pub fn snapshot_tail(&self, secs: u32) -> Vec<f32> {
        let copied: Vec<f32> = {
            let guard = self.samples.lock().unwrap_or_else(|e| e.into_inner());
            tail(&guard, self.channels, self.rate, secs).to_vec()
        };
        finish(copied, self.channels, self.rate).samples_16k
    }

    pub fn cancel(self) {
        drop(self.end());
    }

    // Joining first guarantees the stream is dropped, so no callback touches the buffer after.
    fn end(self) -> Vec<f32> {
        let _ = self.stop_tx.send(());
        let _ = self.thread.join();
        let mut guard = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *guard)
    }
}

// Whole frames only, so channels never shift when the tail is downmixed.
fn tail(buf: &[f32], channels: u16, rate: u32, secs: u32) -> &[f32] {
    let channels = channels.max(1) as usize;
    let want = rate as usize * secs as usize * channels;
    let mut start = buf.len().saturating_sub(want);
    start -= start % channels;
    &buf[start..]
}

fn finish(interleaved: Vec<f32>, channels: u16, rate: u32) -> Recording {
    let mono = to_mono(&interleaved, channels);
    let duration_ms = mono.len() as u64 * 1000 / rate.max(1) as u64;
    let samples_16k = resample_linear(&mono, rate, TARGET_HZ);
    Recording {
        samples_16k,
        duration_ms,
    }
}

fn capture_thread(
    device_name: Option<String>,
    samples: Shared,
    on_level: Box<dyn Fn(f32) + Send + 'static>,
    ready: Sender<Ready>,
    stop: Receiver<()>,
) {
    let (stream, channels, rate) = match open(device_name.as_deref(), samples, on_level) {
        Ok(opened) => opened,
        Err(msg) => {
            let _ = ready.send(Err(msg));
            return;
        }
    };
    let _ = ready.send(Ok((channels, rate)));
    // Returns on stop, cancel, or when the Recorder is dropped without either.
    let _ = stop.recv();
    drop(stream);
}

fn open(
    name: Option<&str>,
    samples: Shared,
    on_level: Box<dyn Fn(f32) + Send + 'static>,
) -> Result<(cpal::Stream, u16, u32), String> {
    let host = cpal::default_host();
    let device = pick_input(&host, name)?;
    let supported = device.default_input_config().map_err(describe)?;
    let config = supported.config();
    let format = supported.sample_format();
    let stream = match format {
        SampleFormat::I8 => build::<i8>(&device, config, samples, on_level),
        SampleFormat::I16 => build::<i16>(&device, config, samples, on_level),
        SampleFormat::I24 => build::<I24>(&device, config, samples, on_level),
        SampleFormat::I32 => build::<i32>(&device, config, samples, on_level),
        SampleFormat::I64 => build::<i64>(&device, config, samples, on_level),
        SampleFormat::U8 => build::<u8>(&device, config, samples, on_level),
        SampleFormat::U16 => build::<u16>(&device, config, samples, on_level),
        SampleFormat::U24 => build::<U24>(&device, config, samples, on_level),
        SampleFormat::U32 => build::<u32>(&device, config, samples, on_level),
        SampleFormat::U64 => build::<u64>(&device, config, samples, on_level),
        SampleFormat::F32 => build::<f32>(&device, config, samples, on_level),
        SampleFormat::F64 => build::<f64>(&device, config, samples, on_level),
        other => return Err(format!("Unsupported microphone sample format: {other}")),
    }
    .map_err(describe)?;
    stream.play().map_err(describe)?;
    Ok((stream, supported.channels(), supported.sample_rate()))
}

// An unknown or unplugged device name falls back to the system default.
// A chosen microphone that is gone is an error, never a silent switch to another mic.
fn pick_input(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device, String> {
    let Some(wanted) = name else {
        return host
            .default_input_device()
            .ok_or_else(|| "No microphone found".to_string());
    };
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if device.description().is_ok_and(|d| d.name() == wanted) {
                return Ok(device);
            }
        }
    }
    Err(format!(
        "{wanted} is not connected. Pick another microphone in Settings."
    ))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    samples: Shared,
    on_level: Box<dyn Fn(f32) + Send + 'static>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let frames_per_level = (config.sample_rate / LEVELS_PER_SEC).max(1) as usize;
    let mut sum_sq = 0.0f32;
    let mut frames = 0usize;
    let on_data = move |data: &[T], _: &cpal::InputCallbackInfo| {
        {
            let mut buf = samples.lock().unwrap_or_else(|e| e.into_inner());
            buf.extend(data.iter().map(|s| s.to_sample::<f32>()));
        }
        // Level is metered on the mono mix, outside the lock so the callback never blocks on it.
        for frame in data.chunks(channels) {
            let mut mono = 0.0f32;
            for s in frame {
                mono += s.to_sample::<f32>();
            }
            mono /= frame.len() as f32;
            sum_sq += mono * mono;
            frames += 1;
            if frames >= frames_per_level {
                let level = (sum_sq / frames as f32).sqrt() * LEVEL_GAIN;
                on_level(level.min(1.0));
                sum_sq = 0.0;
                frames = 0;
            }
        }
    };
    let on_error = |err: cpal::Error| match err.kind() {
        ErrorKind::DeviceChanged | ErrorKind::Xrun | ErrorKind::RealtimeDenied => {}
        _ => eprintln!("local-stt: microphone stream error: {err}"),
    };
    device.build_input_stream(config, on_data, on_error, None)
}

fn describe(err: cpal::Error) -> String {
    match err.kind() {
        ErrorKind::PermissionDenied => {
            "Microphone access denied. Allow it in system privacy settings.".into()
        }
        ErrorKind::DeviceNotAvailable => "Microphone not available".into(),
        ErrorKind::DeviceBusy => "Microphone is busy in another app".into(),
        _ => format!("Microphone error: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_mono_one_second() {
        let rec = finish(vec![0.0; 48_000], 1, 48_000);
        assert_eq!(rec.duration_ms, 1000);
        assert_eq!(rec.samples_16k.len(), 16_000);
    }

    #[test]
    fn finish_stereo_downmixes() {
        let rec = finish(vec![0.5; 96_000], 2, 48_000);
        assert_eq!(rec.duration_ms, 1000);
        assert_eq!(rec.samples_16k.len(), 16_000);
        assert!((rec.samples_16k[100] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn tail_keeps_whole_frames_of_the_last_seconds() {
        let buf: Vec<f32> = (0..10).map(|i| i as f32).collect();
        assert_eq!(tail(&buf, 2, 2, 1), &[6.0, 7.0, 8.0, 9.0]);
        assert_eq!(tail(&buf, 1, 100, 1), &buf[..]);
        assert_eq!(tail(&buf[..9], 2, 2, 1), &[4.0, 5.0, 6.0, 7.0, 8.0]);
        assert!(tail(&[], 2, 48_000, 18).is_empty());
    }

    #[test]
    fn finish_empty() {
        let rec = finish(Vec::new(), 1, 44_100);
        assert_eq!(rec.duration_ms, 0);
        assert!(rec.samples_16k.is_empty());
    }
}
