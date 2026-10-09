//! Windows probe via windows-sys.

use super::{existing_ancestor, run_capture, Facts, GIB};
use std::os::windows::{ffi::OsStrExt, process::CommandExt};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

const GPU_TIMEOUT: Duration = Duration::from_secs(3);
const CREATE_NO_WINDOW: u32 = 0x0800_0000; // no console flash from a GUI app

pub(super) fn facts(dir: &Path) -> Facts {
    Facts {
        cpu: std::env::var("PROCESSOR_IDENTIFIER")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        ram_bytes: ram_bytes(),
        gpu: gpu_name(),
        // The Windows build runs whisper.cpp on the CPU only.
        gpu_accel: false,
        free_disk_gb: free_disk_gb(dir),
    }
}

fn ram_bytes() -> Option<u64> {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    // SAFETY: status is a live MEMORYSTATUSEX with dwLength set, as the API requires.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 || status.ullTotalPhys == 0 {
        None
    } else {
        Some(status.ullTotalPhys)
    }
}

fn free_disk_gb(dir: &Path) -> Option<f64> {
    let dir = existing_ancestor(dir)?;
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_to_caller: u64 = 0;
    // SAFETY: wide is NUL-terminated and outlives the call; the two unused out-pointers may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_to_caller,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    Some((free_to_caller as f64 / GIB * 10.0).round() / 10.0)
}

fn gpu_name() -> Option<String> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let exe = Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let mut cmd = Command::new(exe);
    cmd.args([
        "-NoProfile",
        "-Command",
        "(Get-CimInstance Win32_VideoController | Select-Object -First 1).Name",
    ])
    .creation_flags(CREATE_NO_WINDOW);
    let out = run_capture(&mut cmd, GPU_TIMEOUT)?;
    let name = out.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
