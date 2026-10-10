// Manual end-to-end check of the note pipeline on real audio: records the mic as Me and system
// audio as Others for N seconds, transcribes each segment with the small model, then tidies.
// Run: cargo run --release --example note_probe -- 20   (play a video or `say` something meanwhile)
use local_stt::engine::{Engine, Opts};
use local_stt::notes::segmenter::Segmenter;
use local_stt::notes::{tidy::tidy, Segment, ME, OTHERS};
use local_stt::recorder::{Recorder, Source};
use local_stt::{models, paths};
use std::time::{Duration, Instant};

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let model = models::path(&paths::models_dir(), models::ModelId::Small);
    let engine = Engine::load(&model).expect("the small model is downloaded");
    let opts = Opts { translate: true, language: "auto".into(), threads: 8 };
    let channels = [(Source::System, OTHERS), (Source::Mic(None), ME)];
    let mut open = Vec::new();
    for (source, speaker) in channels {
        match Recorder::start_from(source, Box::new(|_| {})) {
            Ok(rec) => open.push((rec, Segmenter::new(), speaker)),
            Err(e) => println!("{speaker}: {e}"),
        }
    }
    let began = Instant::now();
    let mut segments: Vec<Segment> = Vec::new();
    let handle = |speaker: &str, start_ms: u64, pcm: Vec<f32>, segments: &mut Vec<Segment>| {
        let len = pcm.len();
        let mut padded = pcm;
        if padded.len() < 17_600 {
            padded.resize(17_600, 0.0);
        }
        let text = engine.transcribe(&padded, &opts).unwrap_or_default();
        println!("{:>6} ms {speaker:<6} {}", start_ms, text.trim());
        segments.push(Segment {
            start_ms,
            end_ms: start_ms + len as u64 / 16,
            speaker: speaker.into(),
            text,
        });
    };
    while began.elapsed() < Duration::from_secs(secs) {
        std::thread::sleep(Duration::from_millis(250));
        for (rec, seg, speaker) in open.iter_mut() {
            for chunk in seg.feed(&rec.drain()) {
                handle(speaker, chunk.start_ms, chunk.pcm, &mut segments);
            }
        }
    }
    for (rec, mut seg, speaker) in open {
        let mut chunks = seg.feed(&rec.drain());
        chunks.extend(seg.flush());
        for chunk in chunks {
            handle(speaker, chunk.start_ms, chunk.pcm, &mut segments);
        }
        rec.cancel();
    }
    println!("--- tidied");
    for s in tidy(segments, true, true) {
        println!("{:>6} ms {:<6} {}", s.start_ms, s.speaker, s.text);
    }
}
