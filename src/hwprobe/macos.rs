//! macOS probe via sysctl and df.

use super::{existing_ancestor, parse_df_free_gb, run_capture, Facts};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(3);

pub(super) fn facts(dir: &Path) -> Facts {
    let cpu = sysctl("machdep.cpu.brand_string");
    let ram_bytes = sysctl("hw.memsize").and_then(|s| s.parse().ok());
    let gpu_accel = std::env::consts::ARCH == "aarch64";
    // Apple Silicon shares one chip, so the CPU name is the GPU name.
    let gpu = if gpu_accel {
        cpu.as_ref().map(|c| format!("{c} GPU (Metal)"))
    } else {
        None
    };
    Facts {
        cpu,
        ram_bytes,
        gpu,
        gpu_accel,
        free_disk_gb: free_disk_gb(dir),
    }
}

fn sysctl(key: &str) -> Option<String> {
    let out = run_capture(Command::new("/usr/sbin/sysctl").args(["-n", key]), TIMEOUT)?;
    let value = out.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn free_disk_gb(dir: &Path) -> Option<f64> {
    let dir = existing_ancestor(dir)?;
    parse_df_free_gb(&run_capture(
        Command::new("/bin/df").arg("-k").arg("--").arg(dir),
        TIMEOUT,
    )?)
}
