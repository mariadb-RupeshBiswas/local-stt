//! Telling voices apart on real audio, through the linked sherpa-onnx library.
//! Set LOCAL_STT_TEST_VOICE_MODEL to the voice model; LOCAL_STT_TEST_VOICE_CLIPS to a folder of <speaker>-<n>.wav
//! (16 kHz mono, 16-bit) adds a run on recorded speech.

use local_stt::engine::VoiceEmbedder;
use local_stt::notes::voices;
use std::collections::HashMap;
use std::path::PathBuf;

fn embedder() -> Option<VoiceEmbedder> {
    let path = PathBuf::from(std::env::var("LOCAL_STT_TEST_VOICE_MODEL").ok()?);
    Some(VoiceEmbedder::load(&path).expect("the voice model loads"))
}

// Walks RIFF chunks to the 16-bit "data" chunk.
fn wav_to_f32(bytes: &[u8]) -> Vec<f32> {
    assert!(bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE");
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = pos + 8;
        if &bytes[pos..pos + 4] == b"data" {
            let end = body.saturating_add(size).min(bytes.len());
            return bytes[body..end]
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect();
        }
        pos = body.saturating_add(size).saturating_add(size & 1);
    }
    panic!("no data chunk in WAV");
}

// Every pair of lines shares a group exactly when the same person said them.
fn assert_grouped_by_speaker(ex: &VoiceEmbedder, lines: &[(String, Vec<f32>)]) {
    let mut voiced = Vec::new();
    for (i, (_, pcm)) in lines.iter().enumerate() {
        let started = std::time::Instant::now();
        let v = ex.embed(pcm).expect("a fingerprint for every line");
        eprintln!(
            "line {i}: {} samples, fingerprint in {} ms",
            pcm.len(),
            started.elapsed().as_millis()
        );
        voiced.push((i as u64 * 10_000, pcm.len(), v));
    }
    let keys = voices::speakers(&voiced);
    let people: std::collections::HashSet<&str> =
        lines.iter().map(|(who, _)| who.as_str()).collect();
    let groups: std::collections::HashSet<&String> = keys.values().collect();
    assert_eq!(groups.len(), people.len(), "{keys:?}");
    for (i, (a, _)) in lines.iter().enumerate() {
        for (j, (b, _)) in lines.iter().enumerate() {
            let same = keys[&(i as u64 * 10_000)] == keys[&(j as u64 * 10_000)];
            assert_eq!(same, a == b, "lines {i} ({a}) and {j} ({b})");
        }
    }
}

#[cfg(target_os = "macos")]
fn say(voice: &str, text: &str) -> Vec<f32> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wav = std::env::temp_dir().join(format!("lstt-voice-{}-{n}.wav", std::process::id()));
    let status = std::process::Command::new("say")
        .args(["-v", voice, "-o"])
        .arg(&wav)
        .args(["--data-format=LEI16@16000", text])
        .status()
        .unwrap();
    assert!(status.success(), "say -v {voice}");
    let bytes = std::fs::read(&wav).unwrap();
    let _ = std::fs::remove_file(&wav);
    wav_to_f32(&bytes)
}

#[cfg(target_os = "macos")]
#[test]
fn two_voices_taking_turns_become_two_speakers() {
    let Some(ex) = embedder() else {
        eprintln!("skipped: LOCAL_STT_TEST_VOICE_MODEL is not set");
        return;
    };
    let lines = [
        "Let's review the quarterly numbers before the meeting tomorrow morning.",
        "I think the pricing page needs a clearer comparison between plans.",
        "Can you send the updated slide deck to the whole team today?",
    ];
    let mut turns = Vec::new();
    for text in lines {
        turns.push(("Samantha".to_string(), say("Samantha", text)));
        turns.push(("Daniel".to_string(), say("Daniel", text)));
    }
    assert_grouped_by_speaker(&ex, &turns);
}

#[test]
fn recorded_speakers_fall_into_their_own_groups() {
    let (Some(ex), Ok(dir)) = (embedder(), std::env::var("LOCAL_STT_TEST_VOICE_CLIPS")) else {
        eprintln!("skipped: LOCAL_STT_TEST_VOICE_MODEL or LOCAL_STT_TEST_VOICE_CLIPS is not set");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    files.sort();
    let mut by_speaker: HashMap<String, usize> = HashMap::new();
    let mut lines = Vec::new();
    for path in files {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let who = name.split('-').next().unwrap().to_string();
        *by_speaker.entry(who.clone()).or_default() += 1;
        lines.push((who, wav_to_f32(&std::fs::read(&path).unwrap())));
    }
    eprintln!("{} lines from {:?}", lines.len(), by_speaker);
    assert_grouped_by_speaker(&ex, &lines);
}
