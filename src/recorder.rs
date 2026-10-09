//! Microphone capture on a dedicated thread. Owned by Task 3.

pub struct Recording {
    pub samples_16k: Vec<f32>,
    pub duration_ms: u64,
}

pub struct Recorder {
    _private: (),
}

impl Recorder {
    pub fn list_inputs() -> Vec<String> {
        todo!("Task 3")
    }

    pub fn start(
        _device: Option<&str>,
        _on_level: Box<dyn Fn(f32) + Send + 'static>,
    ) -> Result<Recorder, String> {
        todo!("Task 3")
    }

    pub fn stop(self) -> Recording {
        todo!("Task 3")
    }

    pub fn cancel(self) {
        todo!("Task 3")
    }
}
