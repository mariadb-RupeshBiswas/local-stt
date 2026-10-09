use local_stt::engine::{Engine, Opts};
use std::path::PathBuf;

fn model() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("LOCAL_STT_TEST_MODEL").ok()?);
    p.exists().then_some(p)
}

// Walks RIFF chunks to "data"; headers from `say` are not a fixed 44 bytes.
fn wav_data_to_f32(bytes: &[u8]) -> Vec<f32> {
    assert!(
        bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "not a RIFF/WAVE file"
    );
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = pos + 8;
        if id == b"data" {
            // Streamed WAVs may carry a placeholder size larger than the file.
            let end = body.saturating_add(size).min(bytes.len());
            return bytes[body..end]
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect();
        }
        // Chunks are padded to an even length.
        pos = body.saturating_add(size).saturating_add(size & 1);
    }
    panic!("no data chunk in WAV");
}

#[cfg(target_os = "macos")]
fn speak_to_pcm(text: &str, voice: Option<&str>) -> Vec<f32> {
    let wav = std::env::temp_dir().join(format!("lstt-{}-{}.wav", std::process::id(), text.len()));
    let mut cmd = std::process::Command::new("say");
    if let Some(v) = voice {
        cmd.args(["-v", v]);
    }
    let status = cmd
        .arg("-o")
        .arg(&wav)
        .args(["--data-format=LEI16@16000", text])
        .status()
        .unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&wav).unwrap();
    let _ = std::fs::remove_file(&wav);
    wav_data_to_f32(&bytes)
}

// Headless CI voices sometimes render silence; that is an OS limitation, not an engine bug.
#[cfg(target_os = "macos")]
fn usable(pcm: &[f32]) -> bool {
    let ok = !local_stt::audio::is_silent(pcm, local_stt::audio::SILENCE_RMS);
    if !ok {
        eprintln!(
            "skip: the OS voice produced silence ({} samples)",
            pcm.len()
        );
    }
    ok
}

#[test]
fn wav_parser_skips_extra_chunks() {
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF\0\0\0\0WAVE");
    w.extend_from_slice(b"fmt \x10\0\0\0");
    w.extend_from_slice(&[0u8; 16]);
    w.extend_from_slice(b"FLLR\x03\0\0\0abc\0");
    w.extend_from_slice(b"data\x04\0\0\0");
    w.extend_from_slice(&[0x00, 0x40, 0x00, 0xC0]);
    assert_eq!(wav_data_to_f32(&w), vec![0.5, -0.5]);
}

#[test]
#[cfg(target_os = "macos")]
fn english_passes_through() {
    let Some(m) = model() else {
        eprintln!("skip: LOCAL_STT_TEST_MODEL not set");
        return;
    };
    let e = Engine::load(&m).unwrap();
    let pcm = speak_to_pcm(
        "Please check the September billing numbers before the meeting tomorrow.",
        None,
    );
    if !usable(&pcm) {
        return;
    }
    let out = e
        .transcribe(
            &pcm,
            &Opts {
                translate: true,
                language: "auto".into(),
                threads: 4,
            },
        )
        .unwrap();
    assert!(
        out.to_lowercase().contains("september billing numbers"),
        "transcript {out:?} from {} samples (rms {})",
        pcm.len(),
        local_stt::audio::rms(&pcm)
    );
}

#[test]
#[cfg(target_os = "macos")]
fn hindi_is_translated() {
    let Some(m) = model() else {
        eprintln!("skip: LOCAL_STT_TEST_MODEL not set");
        return;
    };
    // CI runners may not ship the Hindi voice; skip rather than fail on a missing OS asset.
    let voices = std::process::Command::new("say")
        .args(["-v", "?"])
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    if !String::from_utf8_lossy(&voices).contains("Lekha") {
        eprintln!("skip: Hindi voice Lekha not installed");
        return;
    }
    let e = Engine::load(&m).unwrap();
    let pcm = speak_to_pcm(
        "\u{0906}\u{091c} \u{092e}\u{094c}\u{0938}\u{092e} \u{092c}\u{0939}\u{0941}\u{0924} \u{0905}\u{091a}\u{094d}\u{091b}\u{093e} \u{0939}\u{0948}",
        Some("Lekha"),
    );
    if !usable(&pcm) {
        return;
    }
    let out = e
        .transcribe(
            &pcm,
            &Opts {
                translate: true,
                language: "auto".into(),
                threads: 4,
            },
        )
        .unwrap();
    assert!(out.to_lowercase().contains("weather"), "{out}");
}

#[test]
fn missing_model_is_an_error_not_a_panic() {
    assert!(Engine::load(std::path::Path::new("/nonexistent/model.bin")).is_err());
}

#[test]
fn empty_audio_returns_empty_text() {
    let Some(m) = model() else {
        eprintln!("skip: LOCAL_STT_TEST_MODEL not set");
        return;
    };
    let e = Engine::load(&m).unwrap();
    let out = e
        .transcribe(
            &[],
            &Opts {
                translate: true,
                language: "auto".into(),
                threads: 2,
            },
        )
        .unwrap();
    assert_eq!(out, "");
}

#[test]
fn interior_nul_in_language_is_an_error() {
    let Some(m) = model() else {
        eprintln!("skip: LOCAL_STT_TEST_MODEL not set");
        return;
    };
    let e = Engine::load(&m).unwrap();
    let r = e.transcribe(
        &[0.0; 16000],
        &Opts {
            translate: false,
            language: "e\0n".into(),
            threads: 2,
        },
    );
    assert!(r.is_err());
}
