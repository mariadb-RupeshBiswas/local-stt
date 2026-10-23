//! Verified model download through curl.

use crate::diag;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

// Whisper and summary models from Hugging Face; the voice model for notes from sherpa-onnx's own releases.
const ALLOWED_PREFIXES: [&str; 2] = [
    "https://huggingface.co/",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/",
];
const POLL: Duration = Duration::from_millis(250);
// Largest pinned model is the 5.2 GB summary model; the cap only stops a runaway response before the hash check.
const MAX_BYTES: u64 = 6_442_450_944;

// System curl by absolute path, so PATH cannot swap in another binary.
pub(crate) fn curl_program() -> std::path::PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        std::path::PathBuf::from(root)
            .join("System32")
            .join("curl.exe")
    } else {
        std::path::PathBuf::from("/usr/bin/curl")
    }
}

pub fn download(
    url: &str,
    sha256_hex: &str,
    dest: &Path,
    progress: &dyn Fn(u64),
) -> Result<(), String> {
    if !ALLOWED_PREFIXES.iter().any(|p| url.starts_with(p)) {
        return Err(format!(
            "refusing {url}: only the pinned model sources over https are allowed"
        ));
    }
    download_with(url, sha256_hex, dest, progress, false)
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize().iter() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn download_with(
    url: &str,
    sha256_hex: &str,
    dest: &Path,
    progress: &dyn Fn(u64),
    allow_file_scheme_for_tests: bool,
) -> Result<(), String> {
    let part = part_path(dest);
    let result = fetch_and_verify(
        url,
        sha256_hex,
        dest,
        &part,
        progress,
        allow_file_scheme_for_tests,
    );
    match result {
        Ok(()) => Ok(()),
        Err(fail) => {
            // A network failure keeps the bytes so the next attempt resumes; an empty file is no use.
            let has_bytes = std::fs::metadata(&part).is_ok_and(|m| m.len() > 0);
            if !(fail.keep_part && has_bytes) {
                let _ = std::fs::remove_file(&part);
            }
            Err(fail.message)
        }
    }
}

struct Failure {
    message: String,
    keep_part: bool,
}

impl Failure {
    fn discard(message: String) -> Failure {
        Failure {
            message,
            keep_part: false,
        }
    }
}

// The cap applies to the whole file, and curl checks it against the bytes still to come.
fn remaining_cap(already_have: u64) -> u64 {
    MAX_BYTES.saturating_sub(already_have)
}

// curl exit 33: the server ignored the Range header, so the saved bytes cannot be extended.
fn resume_unsupported(exit_code: Option<i32>) -> bool {
    exit_code == Some(33)
}

// A 416 on resume means the saved bytes already span the whole file; the hash decides if they are right.
fn range_past_end(stderr: &str) -> bool {
    stderr.contains("error: 416")
}

// Schannel cannot reach a revocation server (offline, captive network, strict proxy); curl >= 7.70 can ignore that.
fn wants_revoke_retry(windows: bool, stderr: &str, already_retried: bool) -> bool {
    windows
        && !already_retried
        && (stderr.contains("CRYPT_E_NO_REVOCATION_CHECK")
            || stderr.contains("CRYPT_E_REVOCATION_OFFLINE"))
}

struct CurlRun {
    status: std::process::ExitStatus,
    stderr: String,
}

fn run_curl(
    url: &str,
    part: &Path,
    protos: &str,
    best_effort_revocation: bool,
    progress: &dyn Fn(u64),
) -> Result<CurlRun, String> {
    let have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let mut command = crate::proc::command(curl_program());
    // -q must come first: it stops curl reading a user .curlrc.
    command
        .args(["-q", "--max-filesize"])
        .arg(remaining_cap(have).to_string())
        .args(["--connect-timeout", "20"])
        .args(["--speed-limit", "1024", "--speed-time", "60"])
        .args([
            "--fail",
            "--location",
            "--proto",
            protos,
            "--proto-redir",
            "=https",
        ])
        .args(["--tlsv1.2", "--silent", "--show-error"])
        // -C - appends to the part file from its current size.
        .args(["-C", "-"]);
    if best_effort_revocation {
        command.arg("--ssl-revoke-best-effort");
    }
    let mut child = command
        .arg("--output")
        .arg(part)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run curl: {e}"))?;

    let status = loop {
        match child
            .try_wait()
            .map_err(|e| format!("curl wait failed: {e}"))?
        {
            Some(status) => break status,
            None => {
                if let Ok(meta) = std::fs::metadata(part) {
                    progress(meta.len());
                }
                std::thread::sleep(POLL);
            }
        }
    };
    if let Ok(meta) = std::fs::metadata(part) {
        progress(meta.len());
    }
    let mut stderr = String::new();
    if let Some(pipe) = child.stderr.take() {
        // Bounded: curl prints one line per failure, and a runaway message must not fill memory.
        let _ = pipe.take(16 * 1024).read_to_string(&mut stderr);
    }
    Ok(CurlRun { status, stderr })
}

fn fetch_and_verify(
    url: &str,
    sha256_hex: &str,
    dest: &Path,
    part: &Path,
    progress: &dyn Fn(u64),
    allow_file_scheme_for_tests: bool,
) -> Result<(), Failure> {
    // Keep an earlier part file to resume from; a new one is created owner-only and curl appends to it.
    crate::config::private_options()
        .append(true)
        .create(true)
        .open(part)
        .map_err(|e| Failure::discard(format!("cannot create {}: {e}", part.display())))?;
    let have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have >= MAX_BYTES {
        let _ = std::fs::remove_file(part);
        return fetch_and_verify(
            url,
            sha256_hex,
            dest,
            part,
            progress,
            allow_file_scheme_for_tests,
        );
    }
    if have > 0 {
        diag::log("download: resuming a partial file");
        progress(have);
    }

    let protos = if allow_file_scheme_for_tests {
        "=https,file"
    } else {
        "=https"
    };
    let mut run = run_curl(url, part, protos, false, progress).map_err(Failure::discard)?;
    if !run.status.success() && wants_revoke_retry(cfg!(windows), &run.stderr, false) {
        diag::log("download: certificate revocation check unavailable, retrying with best-effort revocation");
        run = run_curl(url, part, protos, true, progress).map_err(Failure::discard)?;
    }
    let status = run.status;
    if !status.success() {
        let complete = have > 0 && range_past_end(&run.stderr);
        if !complete {
            return Err(Failure {
                message: format!("download failed ({status}): {}", run.stderr.trim()),
                keep_part: !resume_unsupported(status.code()),
            });
        }
    }

    let actual =
        sha256_file(part).map_err(|e| Failure::discard(format!("cannot hash download: {e}")))?;
    if !actual.eq_ignore_ascii_case(sha256_hex) {
        return Err(Failure::discard(format!(
            "checksum mismatch: expected {sha256_hex}, got {actual}"
        )));
    }
    std::fs::rename(part, dest)
        .map_err(|e| Failure::discard(format!("cannot move download into place: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Windows paths need file:///C:/... with forward slashes.
    fn file_url(p: &Path) -> String {
        let s = p.display().to_string().replace('\\', "/");
        if s.starts_with('/') {
            format!("file://{s}")
        } else {
            format!("file:///{s}")
        }
    }
    #[test]
    fn sha256_known_value() {
        let p = std::env::temp_dir().join(format!("lstt-sha-{}", std::process::id()));
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn checksum_mismatch_deletes_file() {
        let dir = std::env::temp_dir().join(format!("lstt-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.bin");
        std::fs::write(&src, b"hello").unwrap();
        let dest = dir.join("dest.bin");
        let url = file_url(&src);
        let err = download_with(&url, &"0".repeat(64), &dest, &|_| {}, true).unwrap_err();
        assert!(err.contains("checksum"));
        assert!(!dest.exists());
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")));
    }
    #[test]
    fn non_https_url_rejected() {
        let err = download(
            "http://example.com/x",
            &"0".repeat(64),
            &std::env::temp_dir().join("never"),
            &|_| {},
        )
        .unwrap_err();
        assert!(err.contains("https"));
    }
    #[test]
    fn matching_checksum_installs_file_owner_only() {
        let dir = std::env::temp_dir().join(format!("lstt-dl-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.bin");
        std::fs::write(&src, b"abc").unwrap();
        let dest = dir.join("dest.bin");
        let url = file_url(&src);
        let sha = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        download_with(&url, sha, &dest, &|_| {}, true).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"abc");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn missing_source_fails_and_leaves_nothing() {
        let dir = std::env::temp_dir().join(format!("lstt-dl-miss-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("dest.bin");
        let url = file_url(&dir.join("nope.bin"));
        assert!(download_with(&url, &"0".repeat(64), &dest, &|_| {}, true).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }
    fn sha_of(bytes: &[u8]) -> String {
        let p = std::env::temp_dir().join(format!(
            "lstt-sha-of-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&p, bytes).unwrap();
        let hex = sha256_file(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        hex
    }
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lstt-dl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    #[test]
    fn partial_file_is_resumed_and_progress_counts_it() {
        let dir = scratch("resume");
        let src = dir.join("src.bin");
        std::fs::write(&src, b"0123456789").unwrap();
        let dest = dir.join("dest.bin");
        std::fs::write(part_path(&dest), b"0123").unwrap();
        let seen = std::cell::RefCell::new(Vec::new());
        let progress = |n: u64| seen.borrow_mut().push(n);
        let sha = sha_of(b"0123456789");
        download_with(&file_url(&src), &sha, &dest, &progress, true).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"0123456789");
        assert_eq!(seen.borrow().first(), Some(&4));
        assert_eq!(seen.borrow().last(), Some(&10));
        assert!(!part_path(&dest).exists());
    }
    #[test]
    fn failed_network_keeps_the_partial_file() {
        let dir = scratch("keep");
        let dest = dir.join("dest.bin");
        std::fs::write(part_path(&dest), b"0123").unwrap();
        let url = file_url(&dir.join("nope.bin"));
        assert!(download_with(&url, &"0".repeat(64), &dest, &|_| {}, true).is_err());
        assert_eq!(std::fs::read(part_path(&dest)).unwrap(), b"0123");
    }
    #[test]
    fn mismatch_after_resume_deletes_the_partial_file() {
        let dir = scratch("stale");
        let src = dir.join("src.bin");
        std::fs::write(&src, b"0123456789").unwrap();
        let dest = dir.join("dest.bin");
        std::fs::write(part_path(&dest), b"XXXX").unwrap();
        let sha = sha_of(b"0123456789");
        let err = download_with(&file_url(&src), &sha, &dest, &|_| {}, true).unwrap_err();
        assert!(err.contains("checksum"));
        assert!(!part_path(&dest).exists() && !dest.exists());
    }
    #[test]
    fn cap_shrinks_by_the_bytes_already_saved() {
        assert_eq!(remaining_cap(0), MAX_BYTES);
        assert_eq!(remaining_cap(1_000), MAX_BYTES - 1_000);
        assert_eq!(remaining_cap(MAX_BYTES + 1), 0);
    }
    #[test]
    fn resume_signals_read_from_curl_output() {
        assert!(resume_unsupported(Some(33)));
        assert!(!resume_unsupported(Some(22)));
        assert!(!resume_unsupported(None));
        assert!(range_past_end(
            "curl: (22) The requested URL returned error: 416"
        ));
        assert!(!range_past_end(
            "curl: (22) The requested URL returned error: 404"
        ));
    }
    #[test]
    fn revocation_retry_only_on_windows_once_for_those_errors() {
        let offline = "curl: (35) schannel: next InitializeSecurityContext failed: CRYPT_E_REVOCATION_OFFLINE (0x80092013)";
        let no_check = "schannel: ... CRYPT_E_NO_REVOCATION_CHECK (0x80092012)";
        assert!(wants_revoke_retry(true, offline, false));
        assert!(wants_revoke_retry(true, no_check, false));
        assert!(!wants_revoke_retry(false, offline, false));
        assert!(!wants_revoke_retry(true, offline, true));
        assert!(!wants_revoke_retry(
            true,
            "curl: (6) Could not resolve host",
            false
        ));
        assert!(!wants_revoke_retry(
            true,
            "CRYPT_E_REVOKED: the certificate is revoked",
            false
        ));
    }
}
