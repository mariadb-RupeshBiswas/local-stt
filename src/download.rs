//! Verified model download through curl.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const ALLOWED_PREFIX: &str = "https://huggingface.co/";
const POLL: Duration = Duration::from_millis(250);
// Largest pinned model is about 1 GB; the cap only stops a runaway response before the hash check.
const MAX_BYTES: &str = "2147483648";

// System curl by absolute path, so PATH cannot swap in another binary.
fn curl_program() -> std::path::PathBuf {
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
    if !url.starts_with(ALLOWED_PREFIX) {
        return Err(format!(
            "refusing {url}: only {ALLOWED_PREFIX} over https is allowed"
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
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn fetch_and_verify(
    url: &str,
    sha256_hex: &str,
    dest: &Path,
    part: &Path,
    progress: &dyn Fn(u64),
    allow_file_scheme_for_tests: bool,
) -> Result<(), String> {
    // Pre-create the part file owner-only; curl truncates it and keeps the mode.
    let _ = std::fs::remove_file(part);
    crate::config::private_options()
        .write(true)
        .create_new(true)
        .open(part)
        .map_err(|e| format!("cannot create {}: {e}", part.display()))?;

    let protos = if allow_file_scheme_for_tests {
        "=https,file"
    } else {
        "=https"
    };
    let mut child = Command::new(curl_program())
        // -q must come first: it stops curl reading a user .curlrc.
        .args(["-q", "--max-filesize", MAX_BYTES, "--connect-timeout", "20"])
        .args(["--speed-limit", "1024", "--speed-time", "60"])
        .args([
            "--fail",
            "--location",
            "--proto",
            protos,
            "--proto-redir",
            "=https",
        ])
        .args(["--tlsv1.2", "--silent", "--show-error", "--output"])
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
    if !status.success() {
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        return Err(format!("download failed ({status}): {}", stderr.trim()));
    }

    let actual = sha256_file(part).map_err(|e| format!("cannot hash download: {e}"))?;
    if !actual.eq_ignore_ascii_case(sha256_hex) {
        return Err(format!(
            "checksum mismatch: expected {sha256_hex}, got {actual}"
        ));
    }
    std::fs::rename(part, dest).map_err(|e| format!("cannot move download into place: {e}"))
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
}
