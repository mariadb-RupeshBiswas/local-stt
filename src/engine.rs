//! whisper.cpp through the C shim. Owned by Task 2.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

pub struct Opts {
    pub translate: bool,
    pub language: String,
    pub threads: u32,
}

pub struct Engine {
    ctx: *mut c_void,
}

// SAFETY: the whisper context is only used from one thread at a time.
unsafe impl Send for Engine {}

const OUT_LEN: usize = 64 * 1024;

extern "C" {
    fn lstt_load(path: *const c_char) -> *mut c_void;
    fn lstt_transcribe(
        ctx: *mut c_void,
        pcm: *const f32,
        n: c_int,
        translate: c_int,
        lang: *const c_char,
        threads: c_int,
        abortable: c_int,
        out: *mut c_char,
        out_len: c_int,
    ) -> c_int;
    fn lstt_free(ctx: *mut c_void);
    fn lstt_set_abort(on: c_int);
}

/// Stops any running preview pass (and refuses new ones) until called with false.
pub fn abort_previews(on: bool) {
    // SAFETY: the shim only stores the flag in an atomic.
    unsafe { lstt_set_abort(on as c_int) }
}

fn load_raw(path: &CStr) -> *mut c_void {
    // SAFETY: path is a valid NUL-terminated string that outlives the call.
    unsafe { lstt_load(path.as_ptr()) }
}

fn transcribe_raw(
    ctx: *mut c_void,
    pcm: &[f32],
    translate: bool,
    lang: &CStr,
    threads: i32,
    abortable: bool,
) -> Result<String, String> {
    let n = c_int::try_from(pcm.len()).map_err(|_| "audio is too long".to_string())?;
    let mut out = vec![0u8; OUT_LEN];
    // SAFETY: ctx is a live context, pcm and out are valid for the lengths passed, lang is NUL-terminated.
    let rc = unsafe {
        lstt_transcribe(
            ctx,
            pcm.as_ptr(),
            n,
            translate as c_int,
            lang.as_ptr(),
            threads,
            abortable as c_int,
            out.as_mut_ptr() as *mut c_char,
            OUT_LEN as c_int,
        )
    };
    match rc {
        0 => {
            let len = out.iter().position(|&b| b == 0).unwrap_or(out.len());
            Ok(clean(&String::from_utf8_lossy(&out[..len])))
        }
        -1 => Err("engine is not ready".to_string()),
        -2 => Err("transcription failed".to_string()),
        -3 => Err("transcript is too long".to_string()),
        -4 => Err("speech engine hit an internal error".to_string()),
        -5 => Err("preview stopped".to_string()),
        other => Err(format!("engine error {other}")),
    }
}

// Whisper emits these for non-speech; strip them so silence pastes nothing.
const NOISE_WORDS: [&str; 8] = [
    "silence",
    "music",
    "applause",
    "laughter",
    "noise",
    "blank_audio",
    "inaudible",
    "typing",
];

fn clean(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(['[', '(']) {
        let close = if rest[start..].starts_with('[') {
            ']'
        } else {
            ')'
        };
        let Some(len) = rest[start + 1..].find(close) else {
            break;
        };
        let inner = &rest[start + 1..start + 1 + len];
        let is_noise = close == ']' || NOISE_WORDS.contains(&inner.trim().to_lowercase().as_str());
        kept.push_str(&rest[..start]);
        if !is_noise {
            kept.push_str(&rest[start..start + len + 2]);
        }
        rest = &rest[start + len + 2..];
    }
    kept.push_str(rest);
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Engine {
    pub fn load(model: &Path) -> Result<Engine, String> {
        if !model.is_file() {
            return Err(format!("model file not found: {}", model.display()));
        }
        let path = model.to_str().ok_or("model path is not valid UTF-8")?;
        let cpath = CString::new(path).map_err(|_| "model path contains a NUL byte".to_string())?;
        let ctx = load_raw(&cpath);
        if ctx.is_null() {
            return Err(format!("could not load model: {}", model.display()));
        }
        Ok(Engine { ctx })
    }

    pub fn transcribe(&self, pcm_16k: &[f32], opts: &Opts) -> Result<String, String> {
        self.run(pcm_16k, opts, false)
    }

    /// A live preview pass that stops early when `abort_previews(true)` is called.
    pub fn transcribe_preview(&self, pcm_16k: &[f32], opts: &Opts) -> Result<String, String> {
        self.run(pcm_16k, opts, true)
    }

    fn run(&self, pcm_16k: &[f32], opts: &Opts, abortable: bool) -> Result<String, String> {
        let lang = CString::new(opts.language.as_str())
            .map_err(|_| "language contains a NUL byte".to_string())?;
        let threads = i32::try_from(opts.threads).unwrap_or(i32::MAX);
        transcribe_raw(self.ctx, pcm_16k, opts.translate, &lang, threads, abortable)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: ctx came from lstt_load, is non-null, and is freed exactly once here.
        unsafe { lstt_free(self.ctx) }
    }
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn strips_noise_tokens() {
        assert_eq!(clean(" [BLANK_AUDIO]"), "");
        assert_eq!(clean("(silence)"), "");
        assert_eq!(clean("[Music] hello  world (Applause)"), "hello world");
    }

    #[test]
    fn keeps_real_parentheses() {
        assert_eq!(
            clean("see the note (page two) now"),
            "see the note (page two) now"
        );
        assert_eq!(clean("open (unclosed"), "open (unclosed");
    }
}
