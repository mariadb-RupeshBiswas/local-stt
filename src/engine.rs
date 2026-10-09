//! whisper.cpp through the C shim. Owned by Task 2.

pub struct Opts {
    pub translate: bool,
    pub language: String,
    pub threads: u32,
}

pub struct Engine {
    _private: (),
}

// SAFETY: placeholder until Task 2; the whisper context is only used from one thread at a time.
unsafe impl Send for Engine {}

impl Engine {
    pub fn load(_model: &std::path::Path) -> Result<Engine, String> {
        todo!("Task 2")
    }

    pub fn transcribe(&self, _pcm_16k: &[f32], _opts: &Opts) -> Result<String, String> {
        todo!("Task 2")
    }
}
