//! whisper.cpp through the C shim. Owned by Task 2.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

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
    fn lstt_load(path: *const c_char, use_gpu: c_int) -> *mut c_void;
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
    fn lstt_gpu_off();
    fn lstt_gpu_name(out: *mut c_char, out_len: c_int) -> c_int;
    #[cfg(unix)]
    fn lstt_redirect_stderr(path: *const c_char) -> c_int;
    #[cfg(windows)]
    fn lstt_redirect_stderr(path: *const u16) -> c_int;
}

/// Sends error output to this file for the rest of the run; false when it could not be opened.
pub fn redirect_stderr(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `c` is a NUL-terminated path that outlives the call; the shim only opens and dup2s it.
        unsafe { lstt_redirect_stderr(c.as_ptr()) == 0 }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call; the shim only reopens stderr on it.
        unsafe { lstt_redirect_stderr(wide.as_ptr()) == 0 }
    }
}

static GPU_OFF: AtomicBool = AtomicBool::new(false);
// Set when the GPU could not load a model, so the rest of the run reports and plans for the CPU.
static GPU_FAILED: AtomicBool = AtomicBool::new(false);
static BACKENDS: Once = Once::new();

/// Keeps whisper.cpp on the CPU for this run; only works before the first model load or GPU query.
pub fn keep_off_gpu() {
    GPU_OFF.store(true, Ordering::SeqCst);
}

// ggml starts its backends once per process, so the GPU decision must be made before its first call.
fn start_backends() {
    BACKENDS.call_once(|| {
        if GPU_OFF.load(Ordering::SeqCst) || !crate::hwprobe::gpu_runtime_present() {
            // SAFETY: the shim only sets an environment variable through the C runtime.
            unsafe { lstt_gpu_off() }
        }
    });
}

// An older CPU would die on an illegal instruction inside ggml, with nothing in the log.
fn cpu_can_run() -> Result<(), String> {
    let missing = crate::hwprobe::missing_cpu_features();
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "This processor lacks {}, which the speech engine needs.",
        missing.join(", ")
    ))
}

/// The GPU whisper.cpp runs on, or None for the CPU.
pub fn gpu_device() -> Option<String> {
    cpu_can_run().ok()?;
    start_backends();
    if GPU_FAILED.load(Ordering::SeqCst) {
        return None;
    }
    let mut out = vec![0u8; 256];
    // SAFETY: out is writable for the length passed; the shim NUL-terminates within it.
    let found = unsafe { lstt_gpu_name(out.as_mut_ptr() as *mut c_char, out.len() as c_int) };
    if found == 0 {
        return None;
    }
    let len = out.iter().position(|&b| b == 0).unwrap_or(out.len());
    Some(String::from_utf8_lossy(&out[..len]).trim().to_string())
}

/// Stops any running preview pass (and refuses new ones) until called with false.
pub fn abort_previews(on: bool) {
    // SAFETY: the shim only stores the flag in an atomic.
    unsafe { lstt_set_abort(on as c_int) }
}

fn load_raw(path: &CStr) -> *mut c_void {
    let gpu = gpu_device().is_some();
    let loaded = load_falling_back(gpu, |use_gpu| {
        // SAFETY: path is a valid NUL-terminated string that outlives the call.
        let ctx = unsafe { lstt_load(path.as_ptr(), use_gpu as c_int) };
        (!ctx.is_null()).then_some(ctx)
    });
    match loaded {
        Some((ctx, used_gpu)) => {
            if gpu && !used_gpu {
                GPU_FAILED.store(true, Ordering::SeqCst);
                crate::diag::log("speech engine: the GPU could not load the model, using the CPU");
            }
            ctx
        }
        None => std::ptr::null_mut(),
    }
}

// A GPU the driver lists but ggml cannot run (no 16-bit storage, a broken driver) must not stop dictation.
fn load_falling_back<T>(gpu: bool, load: impl Fn(bool) -> Option<T>) -> Option<(T, bool)> {
    if gpu {
        if let Some(loaded) = load(true) {
            return Some((loaded, true));
        }
    }
    load(false).map(|loaded| (loaded, false))
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
        cpu_can_run()?;
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

#[cfg(lstt_voices)]
extern "C" {
    fn lstt_voice_new(model: *const c_char, threads: c_int) -> *const c_void;
    fn lstt_voice_dim(ex: *const c_void) -> c_int;
    fn lstt_voice_embed(
        ex: *const c_void,
        samples: *const f32,
        n: c_int,
        out: *mut f32,
        dim: c_int,
    ) -> c_int;
    fn lstt_voice_free(ex: *const c_void);
}

/// Voice fingerprints for telling the other side of a call apart; only on builds that link sherpa-onnx.
pub struct VoiceEmbedder {
    #[cfg_attr(not(lstt_voices), allow(dead_code))]
    ex: *const c_void,
    #[cfg_attr(not(lstt_voices), allow(dead_code))]
    dim: usize,
}

// SAFETY: the extractor is only used from the worker thread that owns it.
unsafe impl Send for VoiceEmbedder {}

impl VoiceEmbedder {
    /// None when this build has no voice support or the model cannot be loaded.
    pub fn load(model: &Path) -> Option<Self> {
        #[cfg(lstt_voices)]
        {
            let path = CString::new(model.to_str()?).ok()?;
            // SAFETY: path is NUL-terminated and outlives the call; the shim copies what it needs.
            let ex = unsafe { lstt_voice_new(path.as_ptr(), 1) };
            if ex.is_null() {
                return None;
            }
            // SAFETY: ex is the non-null extractor just created.
            let dim = usize::try_from(unsafe { lstt_voice_dim(ex) }).ok()?;
            Some(Self { ex, dim })
        }
        #[cfg(not(lstt_voices))]
        {
            let _ = model;
            None
        }
    }

    /// The fingerprint of 16 kHz mono audio; None when it is too short to tell.
    pub fn embed(&self, pcm_16k: &[f32]) -> Option<Vec<f32>> {
        #[cfg(lstt_voices)]
        {
            let n = c_int::try_from(pcm_16k.len()).ok()?;
            let mut out = vec![0.0f32; self.dim];
            let dim = c_int::try_from(self.dim).ok()?;
            // SAFETY: ex is live; pcm holds n samples; out holds dim floats, the size the shim writes.
            let rc =
                unsafe { lstt_voice_embed(self.ex, pcm_16k.as_ptr(), n, out.as_mut_ptr(), dim) };
            (rc == 0 && out.iter().all(|v| v.is_finite())).then_some(out)
        }
        #[cfg(not(lstt_voices))]
        {
            let _ = pcm_16k;
            None
        }
    }
}

impl Drop for VoiceEmbedder {
    fn drop(&mut self) {
        #[cfg(lstt_voices)]
        // SAFETY: ex came from lstt_voice_new, is non-null, and is freed exactly once here.
        unsafe {
            lstt_voice_free(self.ex)
        }
    }
}

extern "C" {
    fn lstt_llm_load(path: *const c_char, gpu_layers: c_int) -> *mut c_void;
    fn lstt_llm_free(model: *mut c_void);
    fn lstt_llm_set_abort(on: c_int);
    fn lstt_llm_generate(
        model: *mut c_void,
        prompt: *const c_char,
        grammar: *const c_char,
        root: *const c_char,
        n_ctx: c_int,
        max_tokens: c_int,
        threads: c_int,
        out: *mut c_char,
        out_len: c_int,
    ) -> c_int;
}

/// A language model for note summaries, through llama.cpp.
pub struct Llm {
    model: *mut c_void,
}

// SAFETY: the model is only used from the summary thread that owns it.
unsafe impl Send for Llm {}

const LLM_OUT_LEN: usize = 256 * 1024;

impl Llm {
    /// None when the file cannot be loaded; Apple silicon runs every layer on the GPU.
    pub fn load(path: &Path) -> Option<Self> {
        let c = CString::new(path.to_str()?).ok()?;
        let gpu_layers = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            999
        } else {
            0
        };
        cpu_can_run().ok()?;
        start_backends();
        // SAFETY: c is NUL-terminated and outlives the call; the shim copies what it needs.
        let model = unsafe { lstt_llm_load(c.as_ptr(), gpu_layers) };
        (!model.is_null()).then_some(Self { model })
    }

    /// The reply to one prompt, held to the grammar's `root` rule.
    pub fn generate(
        &self,
        prompt: &str,
        grammar: &str,
        root: &str,
        n_ctx: u32,
        max_tokens: u32,
        threads: u32,
    ) -> Result<String, String> {
        let prompt = CString::new(prompt).map_err(|_| "prompt holds a NUL byte")?;
        let grammar = CString::new(grammar).map_err(|_| "grammar holds a NUL byte")?;
        let root = CString::new(root).map_err(|_| "root holds a NUL byte")?;
        let mut out = vec![0 as c_char; LLM_OUT_LEN];
        // SAFETY: model is live; the strings are NUL-terminated; out holds LLM_OUT_LEN bytes, the length passed.
        let rc = unsafe {
            lstt_llm_generate(
                self.model,
                prompt.as_ptr(),
                grammar.as_ptr(),
                root.as_ptr(),
                n_ctx as c_int,
                max_tokens as c_int,
                threads.max(1) as c_int,
                out.as_mut_ptr(),
                LLM_OUT_LEN as c_int,
            )
        };
        match rc {
            n if n >= 0 => {
                // SAFETY: on success the shim wrote a NUL-terminated string into out.
                let text = unsafe { CStr::from_ptr(out.as_ptr()) };
                Ok(text.to_string_lossy().into_owned())
            }
            -2 => Err("the note part is too long for the model".into()),
            -3 => Err("the reply is too long".into()),
            -4 => Err("stopped".into()),
            _ => Err("the model could not write a reply".into()),
        }
    }
}

/// Stops a running summary at its next token (on quit).
pub fn abort_llm(on: bool) {
    // SAFETY: sets an atomic flag in the shim; no pointers involved.
    unsafe { lstt_llm_set_abort(c_int::from(on)) }
}

impl Drop for Llm {
    fn drop(&mut self) {
        // SAFETY: model came from lstt_llm_load, is non-null, and is freed exactly once here.
        unsafe { lstt_llm_free(self.model) }
    }
}

#[cfg(test)]
mod tests {
    use super::{clean, load_falling_back};
    use std::cell::RefCell;

    #[test]
    fn a_gpu_that_cannot_load_falls_back_to_the_cpu() {
        let tried = RefCell::new(Vec::new());
        let loaded = load_falling_back(true, |gpu| {
            tried.borrow_mut().push(gpu);
            (!gpu).then_some("ctx")
        });
        assert_eq!(loaded, Some(("ctx", false)));
        assert_eq!(*tried.borrow(), vec![true, false]);
    }

    #[test]
    fn a_working_gpu_loads_once_and_no_gpu_tries_only_the_cpu() {
        assert_eq!(load_falling_back(true, |_| Some(1)), Some((1, true)));
        let tried = RefCell::new(Vec::new());
        let none: Option<(u8, bool)> = load_falling_back(false, |gpu| {
            tried.borrow_mut().push(gpu);
            None
        });
        assert_eq!(none, None);
        assert_eq!(*tried.borrow(), vec![false]);
    }

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
