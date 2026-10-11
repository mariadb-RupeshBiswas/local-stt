//! Hardware facts that decide which models fit.

use serde::Serialize;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hardware {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub threads: u32,
    pub ram_gb: Option<u64>,
    pub gpu: String,
    pub gpu_accel: bool,
    pub free_disk_gb: Option<f64>,
}

// What only the OS can tell us; any field that could not be read stays None.
#[derive(Default)]
struct Facts {
    cpu: Option<String>,
    ram_bytes: Option<u64>,
    gpu: Option<String>,
    gpu_accel: bool,
    free_disk_gb: Option<f64>,
}

#[cfg(target_os = "macos")]
use macos::facts;
#[cfg(windows)]
use windows::facts;
#[cfg(not(any(target_os = "macos", windows)))]
fn facts(_dir: &std::path::Path) -> Facts {
    Facts::default()
}

pub fn probe(dir: &std::path::Path) -> Hardware {
    let facts = facts(dir);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
    Hardware {
        os: os_name().to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu: facts.cpu.unwrap_or_else(dash),
        threads,
        ram_gb: facts.ram_bytes.map(bytes_to_gb),
        gpu: facts.gpu.unwrap_or_else(dash),
        gpu_accel: facts.gpu_accel,
        free_disk_gb: facts.free_disk_gb,
    }
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    }
}

fn dash() -> String {
    "-".to_string()
}

const GIB: f64 = 1_073_741_824.0;

fn bytes_to_gb(bytes: u64) -> u64 {
    (bytes as f64 / GIB).round() as u64
}

// Last line of `df -k`: the 4th column is the free space in KiB.
#[cfg(any(target_os = "macos", test))]
fn parse_df_free_gb(out: &str) -> Option<f64> {
    let line = out.lines().rev().find(|l| !l.trim().is_empty())?;
    let kib: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    let gib = kib as f64 * 1024.0 / GIB;
    Some((gib * 10.0).round() / 10.0)
}

// The models folder may not exist yet; ask about the closest folder that does.
#[cfg(any(target_os = "macos", windows))]
fn existing_ancestor(dir: &std::path::Path) -> Option<&std::path::Path> {
    dir.ancestors().find(|p| p.exists())
}

// Runs argv without a shell and gives up after `timeout`; None on any failure.
#[cfg(any(target_os = "macos", windows, test))]
fn run_capture(cmd: &mut std::process::Command, timeout: std::time::Duration) -> Option<String> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::Instant;

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    if !status.success() {
        return None;
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bytes_to_gb_rounds() {
        assert_eq!(bytes_to_gb(25_769_803_776), 24);
        assert_eq!(bytes_to_gb(8_589_934_592), 8);
    }
    #[test]
    fn df_line_parses_free_gb() {
        let out = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s5 971350180 512000000 459350180 53% /System/Volumes/Data\n";
        assert_eq!(parse_df_free_gb(out), Some(438.1));
    }
    #[test]
    fn df_garbage_is_none() {
        assert_eq!(parse_df_free_gb("nonsense"), None);
    }
    #[test]
    fn probe_fills_basics() {
        let hw = probe(&std::env::temp_dir());
        assert!(!hw.os.is_empty() && !hw.arch.is_empty() && hw.threads >= 1);
    }
    #[test]
    fn df_header_only_is_none() {
        assert_eq!(
            parse_df_free_gb("Filesystem 1024-blocks Used Available Capacity Mounted on\n"),
            None
        );
    }
    #[cfg(unix)]
    #[test]
    fn run_capture_reads_stdout() {
        let mut cmd = std::process::Command::new("/bin/echo");
        cmd.arg("hi");
        assert_eq!(
            run_capture(&mut cmd, std::time::Duration::from_secs(3)).as_deref(),
            Some("hi\n")
        );
    }
    #[cfg(unix)]
    #[test]
    fn run_capture_kills_on_timeout() {
        let mut cmd = std::process::Command::new("/bin/sleep");
        cmd.arg("5");
        let start = std::time::Instant::now();
        assert_eq!(
            run_capture(&mut cmd, std::time::Duration::from_millis(200)),
            None
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn probe_reads_real_mac_values() {
        let hw = probe(&std::env::temp_dir().join("lstt-not-created-yet"));
        assert!(hw.ram_gb.is_some_and(|g| g >= 1));
        assert!(hw.free_disk_gb.is_some_and(|g| g > 0.0));
        assert_ne!(hw.cpu, "-");
        assert_eq!(hw.gpu_accel, hw.arch == "aarch64");
    }
}
