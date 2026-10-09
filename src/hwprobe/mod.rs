//! Hardware facts that decide which models fit. Owned by Task 5.

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

pub fn probe(_dir: &std::path::Path) -> Hardware {
    todo!("Task 5")
}
