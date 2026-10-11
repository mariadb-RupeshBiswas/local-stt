//! Update check against PyPI and a one-click upgrade through uv.

use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const PYPI_URL: &str = "https://pypi.org/pypi/local-stt/json";
pub const RELEASES_URL: &str = "https://github.com/mariadb-RupeshBiswas/local-stt/releases";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub available: bool,
    pub notes_url: String,
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Asks PyPI for the newest release. Sends nothing but the request itself: no identifiers, no usage data.
pub fn check() -> Result<UpdateInfo, String> {
    let latest = fetch_latest()?;
    Ok(info_for(&latest))
}

pub fn info_for(latest: &str) -> UpdateInfo {
    let available = is_newer(latest, current());
    UpdateInfo {
        current: current().to_string(),
        latest: Some(latest.to_string()),
        available,
        notes_url: notes_url(latest),
    }
}

pub fn notes_url(version: &str) -> String {
    if valid_version(version) {
        format!("{RELEASES_URL}/tag/v{version}")
    } else {
        RELEASES_URL.to_string()
    }
}

fn fetch_latest() -> Result<String, String> {
    let out = crate::proc::command(crate::download::curl_program())
        .args([
            "-q",
            "--fail",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
        ])
        .args([
            "--tlsv1.2",
            "--silent",
            "--show-error",
            "--max-filesize",
            "5000000",
        ])
        .args(["--connect-timeout", "10", "--max-time", "20", PYPI_URL])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !out.status.success() {
        let detail = String::from_utf8_lossy(&out.stderr);
        if detail.contains("404") {
            return Err("No release has been published yet.".into());
        }
        return Err(format!("Could not check for updates: {}", detail.trim()));
    }
    parse_latest(&out.stdout)
}

/// Reads `info.version` from PyPI's JSON and refuses anything that is not a plain version string.
pub fn parse_latest(body: &[u8]) -> Result<String, String> {
    let json: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("unexpected reply from PyPI: {e}"))?;
    let version = json
        .get("info")
        .and_then(|i| i.get("version"))
        .and_then(|v| v.as_str())
        .ok_or("PyPI reply has no version")?;
    if !valid_version(version) {
        return Err("PyPI reply has an invalid version".into());
    }
    Ok(version.to_string())
}

// The version goes to uv as an argument, so only digits, letters, dots and dashes are allowed.
pub fn valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 32
        && v.chars().next().is_some_and(|c| c.is_ascii_digit())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

// PEP 440 pre-releases: 1.0a1, 1.0b2, 1.0rc1, 1.0.dev3, or a dashed suffix; "+local" metadata does not count.
fn is_pre_release(v: &str) -> bool {
    let public = v.split('+').next().unwrap_or("");
    if public.contains('-') || public.contains("rc") || public.contains("dev") {
        return true;
    }
    let chars: Vec<char> = public.chars().collect();
    chars
        .windows(2)
        .any(|w| w[0].is_ascii_digit() && matches!(w[1], 'a' | 'b'))
}

fn numeric_parts(v: &str) -> Vec<u64> {
    let core = v.split(['-', '+']).next().unwrap_or("");
    let mut parts = Vec::new();
    for part in core.split('.') {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        parts.push(digits.parse().unwrap_or(0));
    }
    parts
}

/// True when `latest` is a higher release than `current`; pre-releases never count as an update.
pub fn is_newer(latest: &str, current: &str) -> bool {
    if is_pre_release(latest) {
        return false;
    }
    let (mut l, mut c) = (numeric_parts(latest), numeric_parts(current));
    let len = l.len().max(c.len());
    l.resize(len, 0);
    c.resize(len, 0);
    l > c
}

/// Finds uv even when the app was started from Spotlight or the Start menu with a minimal PATH.
pub fn find_uv() -> Option<PathBuf> {
    let name = if cfg!(windows) { "uv.exe" } else { "uv" };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    let home = PathBuf::from(home);
    let mut known = vec![
        home.join(".local").join("bin").join(name),
        home.join(".cargo").join("bin").join(name),
    ];
    if cfg!(target_os = "macos") {
        known.push(PathBuf::from("/opt/homebrew/bin/uv"));
        known.push(PathBuf::from("/usr/local/bin/uv"));
    }
    known.into_iter().find(|p| p.is_file())
}

/// Installs `version` with uv and returns what the new copy reported.
pub fn install(version: &str) -> Result<String, String> {
    if !valid_version(version) {
        return Err("invalid version".into());
    }
    let uv = find_uv().ok_or("uv was not found. Download the update from the releases page.")?;
    let spec = format!("local-stt=={version}");
    let mut cmd = crate::proc::command(uv);
    cmd.args(["tool", "run", "--from", &spec, "local-stt", "install"]);
    let (ok, stdout, stderr) = run_with_timeout(cmd, std::time::Duration::from_secs(600))?;
    if !ok {
        return Err(format!("update failed: {}", stderr.trim()));
    }
    Ok(stdout.trim().to_string())
}

/// Runs a command, draining its output on threads, and kills it if it runs past `limit`.
fn run_with_timeout(
    mut cmd: Command,
    limit: std::time::Duration,
) -> Result<(bool, String, String), String> {
    use std::io::Read;
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run uv: {e}"))?;
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if started.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The update took too long and was stopped.".into());
            }
            None => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    Ok((status.success(), stdout, stderr))
}

/// Opens a fixed project URL in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with(RELEASES_URL) {
        return Err("refusing to open an unexpected address".into());
    }
    let status = if cfg!(target_os = "macos") {
        Command::new("/usr/bin/open").arg(url).status()
    } else if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        crate::proc::command(std::path::PathBuf::from(root).join("explorer.exe"))
            .arg(url)
            .status()
    } else {
        Command::new("xdg-open").arg(url).status()
    };
    status
        .map(|_| ())
        .map_err(|e| format!("cannot open the browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("0.1.1", "0.1"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn pre_releases_are_not_offered() {
        assert!(!is_newer("0.2.0rc1", "0.1.0"));
        assert!(!is_newer("0.2.0-beta.1", "0.1.0"));
        assert!(!is_newer("0.2.0.dev3", "0.1.0"));
        assert!(!is_newer("0.2.0a1", "0.1.0"));
        assert!(is_newer("0.2.0+build5", "0.1.0"));
    }

    #[test]
    fn version_strings_are_checked_before_use() {
        assert!(valid_version("0.2.0"));
        assert!(valid_version("1.0.0+local"));
        assert!(!valid_version(""));
        assert!(!valid_version("latest"));
        assert!(!valid_version("1.0; rm -rf /"));
        assert!(!valid_version("1.0 --index-url http://x"));
        assert!(!valid_version(&"9".repeat(40)));
    }

    #[test]
    fn pypi_reply_is_parsed_and_validated() {
        let ok = br#"{"info":{"version":"0.3.1","name":"local-stt"},"releases":{}}"#;
        assert_eq!(parse_latest(ok).unwrap(), "0.3.1");
        assert!(parse_latest(br#"{"info":{"version":"--evil"}}"#).is_err());
        assert!(parse_latest(br#"{"info":{}}"#).is_err());
        assert!(parse_latest(b"<html>").is_err());
    }

    #[test]
    fn notes_link_points_at_the_release_tag() {
        assert_eq!(
            notes_url("0.2.0"),
            "https://github.com/mariadb-RupeshBiswas/local-stt/releases/tag/v0.2.0"
        );
        assert_eq!(notes_url("bad version"), RELEASES_URL);
    }

    #[cfg(unix)]
    #[test]
    fn slow_commands_are_stopped() {
        let mut cmd = Command::new("/bin/sleep");
        cmd.arg("5");
        let started = std::time::Instant::now();
        let result = run_with_timeout(cmd, std::time::Duration::from_millis(300));
        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn command_output_is_collected() {
        let mut cmd = Command::new("/bin/echo");
        cmd.arg("hello");
        let (ok, out, _) = run_with_timeout(cmd, std::time::Duration::from_secs(5)).unwrap();
        assert!(ok);
        assert_eq!(out.trim(), "hello");
    }

    #[test]
    fn only_project_urls_are_opened() {
        assert!(open_url("https://example.com").is_err());
    }
}
