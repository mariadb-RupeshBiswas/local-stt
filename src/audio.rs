//! Pure audio helpers: downmix, resample, level and silence checks.

pub const SILENCE_RMS: f32 = 0.01;

// 30 ms at 16 kHz, the rate every clip is resampled to before the silence check.
const WINDOW: usize = 480;

pub fn to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let n = channels as usize;
    let mut mono = Vec::with_capacity(interleaved.len() / n);
    for frame in interleaved.chunks_exact(n) {
        mono.push(frame.iter().sum::<f32>() / n as f32);
    }
    mono
}

pub fn resample_linear(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if input.is_empty() || from_hz == 0 || to_hz == 0 || from_hz == to_hz {
        return input.to_vec();
    }
    let ratio = from_hz as f64 / to_hz as f64;
    let out_len = (input.len() as f64 * to_hz as f64 / from_hz as f64).round() as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = (pos as usize).min(last);
        let frac = (pos - idx as f64) as f32;
        let next = input[(idx + 1).min(last)];
        out.push(input[idx] + (next - input[idx]) * frac);
    }
    out
}

pub fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = frame.iter().map(|s| s * s).sum();
    (sum_sq / frame.len() as f32).sqrt()
}

// Expects 16 kHz input; any window at or above the threshold counts as speech.
pub fn is_silent(samples: &[f32], threshold: f32) -> bool {
    samples.chunks(WINDOW).all(|w| rms(w) < threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mono_averages_channels() {
        assert_eq!(to_mono(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }
    #[test]
    fn mono_passthrough() {
        assert_eq!(to_mono(&[0.1, 0.2], 1), vec![0.1, 0.2]);
    }
    #[test]
    fn resample_48k_to_16k_length() {
        assert_eq!(
            resample_linear(&vec![0.0; 48_000], 48_000, 16_000).len(),
            16_000
        );
    }
    #[test]
    fn resample_same_rate_is_identity() {
        let v = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&v, 16_000, 16_000), v);
    }
    #[test]
    fn resample_empty() {
        assert!(resample_linear(&[], 44_100, 16_000).is_empty());
    }
    #[test]
    fn rms_of_constant() {
        assert!((rms(&[0.5; 100]) - 0.5).abs() < 1e-6);
    }
    #[test]
    fn rms_empty_is_zero() {
        assert_eq!(rms(&[]), 0.0);
    }
    #[test]
    fn silence_detected() {
        assert!(is_silent(&vec![0.001; 16_000], SILENCE_RMS));
    }
    #[test]
    fn speech_not_silent() {
        let tone: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        assert!(!is_silent(&tone, SILENCE_RMS));
    }
}
