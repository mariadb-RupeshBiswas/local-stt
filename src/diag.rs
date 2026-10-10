//! Troubleshooting: a small local event log, and a report the user can choose to share.
//! Never log dictated text, audio, key presses, clipboard contents or device names.

use crate::config::Config;
use crate::hwprobe::Hardware;
use crate::{history, models, paths};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, TryLockError};

const LOG: &str = "local-stt.log";
const LOG_OLD: &str = "local-stt.1.log";
const ERR: &str = "stderr.log";
const ERR_OLD: &str = "stderr.1.log";
const ROTATE_AT: u64 = 512 * 1024;
// Native code can write to stderr all session; past this the file starts over.
const ERR_CAP: u64 = 1024 * 1024;
const LOG_LINES: usize = 400;
const ERR_LINES: usize = 150;
const REPORT_CAP: usize = 200 * 1024;
const KEEP_REPORTS: usize = 5;
#[cfg(target_os = "macos")]
const CRASH_DAYS: u64 = 30;
const CRASH_FRAMES: usize = 40;

static ENABLED: AtomicBool = AtomicBool::new(true);
static CAPTURING: AtomicBool = AtomicBool::new(false);
static FILE: Mutex<()> = Mutex::new(());

pub fn logs_dir() -> PathBuf {
    paths::data_dir().join("logs")
}

fn reports_dir() -> PathBuf {
    paths::data_dir().join("diagnostics")
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Appends one line to the local log, with the home folder and user name masked.
pub fn log(message: &str) {
    write_line(message, FILE.lock().unwrap_or_else(|e| e.into_inner()));
}

// A panic raised while this thread holds the log lock must not deadlock, so the hook skips a busy log.
fn try_log(message: &str) {
    match FILE.try_lock() {
        Ok(guard) => write_line(message, guard),
        Err(TryLockError::Poisoned(p)) => write_line(message, p.into_inner()),
        Err(TryLockError::WouldBlock) => {}
    }
}

fn write_line(message: &str, _guard: MutexGuard<'_, ()>) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let dir = logs_dir();
    if paths::private_dir(&dir).is_err() {
        return;
    }
    let path = dir.join(LOG);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > ROTATE_AT) {
        let _ = std::fs::rename(&path, dir.join(LOG_OLD));
    }
    let err = dir.join(ERR);
    if CAPTURING.load(Ordering::Relaxed) && std::fs::metadata(&err).is_ok_and(|m| m.len() > ERR_CAP)
    {
        // Writes are append-only, so the next one lands at the new end.
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .open(&err)
            .and_then(|f| f.set_len(0));
    }
    let flat = message.replace(['\r', '\n'], " ");
    let line = format!("{} {}\n", utc_stamp(crate::app::now_ms()), redact(&flat));
    if let Ok(mut f) = crate::config::private_options()
        .append(true)
        .create(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Deletes the logs and saved reports, for when the user turns the log off.
pub fn clear() {
    let _guard = FILE.lock().unwrap_or_else(|e| e.into_inner());
    // Point stderr away from the file first, so it can be deleted on Windows too and stays gone.
    if CAPTURING.swap(false, Ordering::Relaxed) {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let _ = crate::engine::redirect_stderr(Path::new(null));
    }
    for name in [LOG, LOG_OLD, ERR, ERR_OLD] {
        let _ = std::fs::remove_file(logs_dir().join(name));
    }
    let _ = std::fs::remove_dir_all(reports_dir());
}

/// Writes panics to the log before the default handler runs.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        try_log(&format!("panic: {info}"));
        let trace = std::backtrace::Backtrace::force_capture().to_string();
        for line in trace.lines().take(60) {
            try_log(&format!("  {line}"));
        }
        default(info);
    }));
}

/// Sends this run's error output (where native crashes print their reason) to logs/stderr.log.
pub fn capture_stderr() {
    use std::io::IsTerminal;
    // A terminal shows errors live; only a launch from Finder, Start or login loses them.
    if !ENABLED.load(Ordering::Relaxed) || std::io::stderr().is_terminal() {
        return;
    }
    let dir = logs_dir();
    if paths::private_dir(&dir).is_err() {
        return;
    }
    let path = dir.join(ERR);
    let _ = std::fs::rename(&path, dir.join(ERR_OLD));
    if crate::engine::redirect_stderr(&path) {
        CAPTURING.store(true, Ordering::Relaxed);
    } else {
        log("stderr capture: could not open the file");
    }
}

// ---------- redaction ----------

fn home() -> String {
    paths::home().display().to_string()
}

fn user_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}

/// Masks the home folder as ~ and the user name as <user>, ignoring letter case as macOS and Windows paths do.
pub fn redact(text: &str) -> String {
    redact_with(text, &home(), &user_name())
}

fn redact_with(text: &str, home: &str, user: &str) -> String {
    let mut out = text.to_string();
    if home.len() > 1 {
        out = replace_ci(&out, home, "~", false);
    }
    // Names under 3 letters would mostly hit ordinary words; whole words only, so "art" leaves "start" alone.
    if user.chars().count() >= 3 {
        out = replace_ci(&out, user, "<user>", true);
    }
    out
}

// ASCII-case-insensitive replace; ASCII lowercasing keeps byte offsets, so they map back onto `text`.
fn replace_ci(text: &str, find: &str, with: &str, whole_word: bool) -> String {
    let hay = text.to_ascii_lowercase();
    let needle = find.to_ascii_lowercase();
    let is_word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    while let Some(found) = hay[from..].find(&needle) {
        let at = from + found;
        let end = at + needle.len();
        let bounded = !whole_word
            || (!is_word(text[..at].chars().next_back()) && !is_word(text[end..].chars().next()));
        out.push_str(&text[from..at]);
        out.push_str(if bounded { with } else { &text[at..end] });
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

// ---------- time ----------

/// "2026-10-10 09:30:00 UTC" from Unix milliseconds.
pub fn utc_stamp(ms: u64) -> String {
    let secs = ms / 1000;
    let (y, m, d) = civil((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

// ---------- report ----------

/// Builds the report, saves it in the data folder and returns its path.
pub fn export(cfg: &Config, hw: &Hardware) -> Result<PathBuf, String> {
    let text = report(cfg, hw);
    let dir = reports_dir();
    paths::private_dir(&dir).map_err(|e| format!("cannot create the report folder: {e}"))?;
    let stamp = utc_stamp(crate::app::now_ms())
        .replace(" UTC", "")
        .replace([' ', ':'], "-");
    let path = dir.join(format!("local-stt-diagnostics-{stamp}.txt"));
    let mut file = crate::config::private_options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| format!("cannot save the report: {e}"))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("cannot save the report: {e}"))?;
    prune_reports(&dir);
    log("diagnostic report saved");
    Ok(path)
}

/// Opens the folder with the file selected, so it can be dragged into an email or chat.
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        // explorer wants /select,"path" as one raw token; Command::arg would quote the comma part too.
        let _ = crate::proc::command(PathBuf::from(root).join("explorer.exe"))
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
}

fn prune_reports(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    // The stamp in the name sorts by time.
    names.sort();
    let extra = names.len().saturating_sub(KEEP_REPORTS);
    for old in names.iter().take(extra) {
        let _ = std::fs::remove_file(old);
    }
}

/// Plain text an LLM or a person can read; built from an allowlist, then masked as a whole.
pub fn report(cfg: &Config, hw: &Hardware) -> String {
    let mut r = String::new();
    r.push_str("local-stt diagnostic report\n");
    r.push_str(&format!("Created {}\n", utc_stamp(crate::app::now_ms())));
    r.push_str("Includes app, system and settings details and recent app logs. Never includes audio, dictated text, key presses, clipboard contents or microphone names.\n");
    r.push_str("Your home folder is shown as ~ and your user name as <user>. Read it before you share it.\n");

    section(&mut r, "App");
    r.push_str(&format!("Version: {}\n", env!("CARGO_PKG_VERSION")));
    r.push_str(&format!(
        "Build: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let installed = crate::install::installed_exe()
        .zip(std::env::current_exe().ok())
        .is_some_and(|(a, b)| a == b);
    r.push_str(&format!(
        "Running from: {}\n",
        if installed {
            "the installed copy"
        } else {
            "another location (uvx or a build)"
        }
    ));
    r.push_str(&format!("Data folder: {}\n", paths::data_dir().display()));

    section(&mut r, "System");
    r.push_str(&format!("OS: {} {} ({})\n", hw.os, os_version(), hw.arch));
    r.push_str(&format!("CPU: {} ({} threads)\n", hw.cpu, hw.threads));
    r.push_str(&format!("Memory: {} GB\n", dash(hw.ram_gb)));
    r.push_str(&format!(
        "GPU: {} ({})\n",
        hw.gpu,
        if hw.gpu_accel { "used" } else { "not used" }
    ));
    r.push_str(&format!("Free disk: {} GB\n", dash(hw.free_disk_gb)));

    section(&mut r, "Settings");
    r.push_str(&settings_json(cfg));
    r.push('\n');

    section(&mut r, "Models");
    for fit in models::evaluate(hw, &paths::models_dir()) {
        r.push_str(&format!(
            "{}: {}{}\n",
            fit.label,
            if fit.downloaded {
                "downloaded"
            } else {
                "not downloaded"
            },
            if fit.supported {
                ""
            } else {
                ", not supported here"
            }
        ));
    }

    section(&mut r, "History");
    let entries = history::read_newest_first(&paths::history_path(), usize::MAX)
        .map(|e| e.len().to_string())
        .unwrap_or_else(|_| "-".into());
    let bytes = std::fs::metadata(paths::history_path())
        .map(|m| format!("{} KB", m.len().div_ceil(1024)))
        .unwrap_or_else(|_| "-".into());
    r.push_str(&format!("Entries: {entries} ({bytes})\n"));

    section(&mut r, "Microphones");
    r.push_str(&format!(
        "Inputs found: {}\n",
        crate::recorder::Recorder::list_inputs().len()
    ));

    section(&mut r, "Crash reports");
    let crashes = crash_reports();
    if crashes.is_empty() {
        r.push_str("None in the last 30 days.\n");
    }
    for c in crashes {
        r.push_str(&c);
    }

    section(&mut r, "Error output, latest app run");
    r.push_str(&tail(&logs_dir().join(ERR), ERR_LINES));
    section(&mut r, "Error output, the run before");
    r.push_str(&tail(&logs_dir().join(ERR_OLD), ERR_LINES));

    section(&mut r, "App log");
    let mut log_text = tail(&logs_dir().join(LOG_OLD), LOG_LINES);
    log_text.push_str(&tail(&logs_dir().join(LOG), LOG_LINES));
    r.push_str(&last_lines(&log_text, LOG_LINES));

    let mut r = redact(&r);
    if r.len() > REPORT_CAP {
        let mut cut = REPORT_CAP;
        while !r.is_char_boundary(cut) {
            cut -= 1;
        }
        r.truncate(cut);
        r.push_str("\n[report cut at 200 KB]\n");
    }
    r
}

// The exact OS build matters for permission and audio behaviour; "-" when it cannot be read.
fn os_version() -> String {
    let out = if cfg!(target_os = "macos") {
        std::process::Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
    } else if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        crate::proc::command(PathBuf::from(root).join("System32").join("cmd.exe"))
            .args(["/c", "ver"])
            .output()
    } else {
        return "-".into();
    };
    out.ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "-".into())
}

fn section(r: &mut String, title: &str) {
    r.push_str(&format!("\n== {title} ==\n"));
}

fn dash<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "-".into())
}

// Settings as JSON, with the chosen microphone's name replaced, since device names often carry a person's name.
fn settings_json(cfg: &Config) -> String {
    let mut v = serde_json::to_value(cfg).unwrap_or(Value::Null);
    if let Some(map) = v.as_object_mut() {
        let mic = if cfg.microphone.is_some() {
            "a chosen device"
        } else {
            "system default"
        };
        map.insert("microphone".into(), Value::String(mic.into()));
        // The saved pill position names a screen, and a screen can be "Jane's iPad".
        if cfg.overlay_pos.is_some() {
            map.insert("overlay_pos".into(), Value::String("saved".into()));
        }
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

fn tail(path: &Path, lines: usize) -> String {
    match std::fs::read(path) {
        Ok(bytes) => last_lines(&String::from_utf8_lossy(&bytes), lines),
        Err(_) => "-\n".into(),
    }
}

fn last_lines(text: &str, n: usize) -> String {
    let all: Vec<&str> = text.lines().filter(|l| *l != "-").collect();
    if all.is_empty() {
        return "-\n".into();
    }
    let mut out = all[all.len().saturating_sub(n)..].join("\n");
    out.push('\n');
    out
}

// ---------- macOS crash reports ----------

#[cfg(target_os = "macos")]
fn crash_reports() -> Vec<String> {
    let dir = paths::home().join("Library/Logs/DiagnosticReports");
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(CRASH_DAYS * 86_400);
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("local-stt") || !name.ends_with(".ips") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if modified >= cutoff {
            found.push((modified, entry.path()));
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut out = Vec::new();
    for (_, path) in found.into_iter().take(3) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(summary) = summarize_ips(&text) {
            out.push(summary);
        }
    }
    out
}

#[cfg(not(target_os = "macos"))]
fn crash_reports() -> Vec<String> {
    vec!["Not collected on this system; native crashes print their reason in the error output below.\n".into()]
}

/// Picks only what explains a crash from a macOS .ips report; device ids, paths and the model code stay out.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn summarize_ips(text: &str) -> Option<String> {
    let (head, body) = text.split_once('\n')?;
    let head: Value = serde_json::from_str(head).ok()?;
    if head.get("app_name").and_then(Value::as_str) != Some("local-stt") {
        return None;
    }
    let body: Value = serde_json::from_str(body).ok()?;
    let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("-").to_string();
    let mut out = format!(
        "Crash at {} (version {}, {})\n",
        s(head.get("timestamp")),
        s(head.get("app_version")),
        s(head.get("os_version"))
    );
    let exception = body.get("exception");
    out.push_str(&format!(
        "Exception: {} {}\n",
        s(exception.and_then(|e| e.get("type"))),
        s(exception.and_then(|e| e.get("signal")))
    ));
    out.push_str(&format!(
        "Termination: {}\n",
        s(body.get("termination").and_then(|t| t.get("indicator")))
    ));
    if let Some(asi) = body.get("asi").and_then(Value::as_object) {
        for (lib, lines) in asi {
            for line in lines.as_array().into_iter().flatten() {
                out.push_str(&format!(
                    "Note from {}: {}\n",
                    lib,
                    line.as_str().unwrap_or("-")
                ));
            }
        }
    }
    let images = body.get("usedImages").and_then(Value::as_array);
    let image_name = |index: Option<u64>| -> String {
        let image = index.and_then(|i| images.and_then(|all| all.get(i as usize)));
        let named = image.and_then(|im| im.get("name").and_then(Value::as_str));
        let from_path = image
            .and_then(|im| im.get("path").and_then(Value::as_str))
            .and_then(|p| p.rsplit('/').next());
        named.or(from_path).unwrap_or("?").to_string()
    };
    let faulting = body.get("faultingThread").and_then(Value::as_u64)?;
    let frames = body
        .get("threads")
        .and_then(|t| t.get(faulting as usize))
        .and_then(|t| t.get("frames"))
        .and_then(Value::as_array)?;
    out.push_str(&format!("Crashed thread {faulting}:\n"));
    for (n, frame) in frames.iter().take(CRASH_FRAMES).enumerate() {
        let image = image_name(frame.get("imageIndex").and_then(Value::as_u64));
        let place = match frame.get("symbol").and_then(Value::as_str) {
            Some(sym) => format!(
                "{sym} + {}",
                frame
                    .get("symbolLocation")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            ),
            None => format!(
                "0x{:x}",
                frame
                    .get("imageOffset")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            ),
        };
        out.push_str(&format!("  {n:>2} {image} {place}\n"));
    }
    out.push('\n');
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_stamp_matches_known_dates() {
        assert_eq!(utc_stamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_stamp(951_782_400_000), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc_stamp(1_791_624_255_000), "2026-10-10 09:24:15 UTC");
    }

    #[test]
    fn redaction_masks_home_and_user_but_spares_short_names() {
        let text = "/Users/jane.doe/Library/x failed for jane.doe";
        assert_eq!(
            redact_with(text, "/Users/jane.doe", "jane.doe"),
            "~/Library/x failed for <user>"
        );
        assert_eq!(
            redact_with("an ox in a box", "/home/ox", "ox"),
            "an ox in a box"
        );
    }

    #[test]
    fn redaction_takes_whole_names_in_any_case() {
        assert_eq!(
            redact_with("start: recording started by art", "/Users/art", "art"),
            "start: recording started by <user>"
        );
        assert_eq!(
            redact_with(r"C:\Users\Jane\AppData and JANE", r"c:\users\jane", "jane"),
            r"~\AppData and <user>"
        );
    }

    #[test]
    fn settings_hide_the_microphone_name() {
        let cfg = Config {
            microphone: Some("Priya's AirPods".into()),
            ..Config::default()
        };
        let cfg = Config {
            overlay_pos: Some(crate::config::OverlayPos {
                monitor: "Priya's iPad".into(),
                x: 1.0,
                y: 2.0,
            }),
            ..cfg
        };
        let json = settings_json(&cfg);
        assert!(!json.contains("Priya"), "{json}");
        assert!(json.contains("a chosen device"));
    }

    #[test]
    fn last_lines_keeps_the_newest() {
        assert_eq!(last_lines("a\nb\nc\n", 2), "b\nc\n");
        assert_eq!(last_lines("", 2), "-\n");
    }

    const IPS: &str = concat!(
        r#"{"app_name":"local-stt","timestamp":"2026-10-10 13:11:54.00 +0530","app_version":"0.1.0","os_version":"macOS 27.0.1 (26A434)","incident_id":"SECRET-INCIDENT"}"#,
        "\n",
        r#"{"crashReporterKey":"SECRET-KEY","sleepWakeUUID":"SECRET-WAKE","modelCode":"Mac16,8","procPath":"/Users/USER/*/local-stt","faultingThread":1,
           "exception":{"type":"EXC_CRASH","signal":"SIGABRT"},"termination":{"indicator":"Abort trap: 6"},
           "asi":{"libsystem_c.dylib":["abort() called"]},
           "usedImages":[{"name":"local-stt","path":"/Users/someone/bin/local-stt"},{"path":"/usr/lib/system/libsystem_kernel.dylib"}],
           "threads":[{"frames":[]},{"frames":[{"imageIndex":1,"symbol":"__pthread_kill","symbolLocation":8},{"imageIndex":0,"imageOffset":4096}]}]}"#
    );

    #[test]
    fn crash_summary_keeps_the_cause_and_drops_identifiers() {
        let s = summarize_ips(IPS).unwrap();
        assert!(s.contains("EXC_CRASH SIGABRT"), "{s}");
        assert!(s.contains("abort() called"));
        assert!(s.contains("libsystem_kernel.dylib __pthread_kill + 8"));
        assert!(s.contains("local-stt 0x1000"));
        for secret in ["SECRET", "Mac16,8", "/Users/", "someone", "procPath"] {
            assert!(!s.contains(secret), "{secret} leaked: {s}");
        }
    }

    #[test]
    fn crash_summary_ignores_other_apps() {
        let other = IPS.replace(r#""app_name":"local-stt""#, r#""app_name":"other""#);
        assert!(summarize_ips(&other).is_none());
    }
}
