//! Pure audio helpers. Owned by Task 3.

pub const SILENCE_RMS: f32 = 0.01;

pub fn to_mono(_interleaved: &[f32], _channels: u16) -> Vec<f32> {
    todo!("Task 3")
}

pub fn resample_linear(_input: &[f32], _from_hz: u32, _to_hz: u32) -> Vec<f32> {
    todo!("Task 3")
}

pub fn rms(_frame: &[f32]) -> f32 {
    todo!("Task 3")
}

pub fn is_silent(_samples: &[f32], _threshold: f32) -> bool {
    todo!("Task 3")
}
