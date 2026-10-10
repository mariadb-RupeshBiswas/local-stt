//! One running copy per data folder: a lock file naming the owner's PID, checked against the live process.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOCK: &str = "app.lock";
const SHOW: &str = "show.request";

pub enum Claim {
    /// This process runs the app; remove the lock at exit.
    Owner(Option<PathBuf>),
    /// Another copy is running and has been asked to show its window.
    Other,
}

pub fn claim(dir: &Path) -> Claim {
    let path = dir.join(LOCK);
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                let _ = write!(f, "{}", std::process::id());
                return Claim::Owner(Some(path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok());
                if pid.is_some_and(is_local_stt) {
                    let _ = std::fs::write(dir.join(SHOW), b"");
                    return Claim::Other;
                }
                // Left behind by a crash: take it over.
                let _ = std::fs::remove_file(&path);
            }
            // A read-only or odd folder must not stop the app from starting.
            Err(_) => return Claim::Owner(None),
        }
    }
    Claim::Owner(None)
}

pub fn release(lock: &Option<PathBuf>) {
    if let Some(path) = lock {
        let _ = std::fs::remove_file(path);
    }
}

/// True once per "please show your window" request from a second launch.
pub fn take_show_request(dir: &Path) -> bool {
    std::fs::remove_file(dir.join(SHOW)).is_ok()
}

// A PID can be reused, so the process must also be local-stt to count.
fn is_local_stt(pid: u32) -> bool {
    if pid == std::process::id() {
        return false;
    }
    let out = if cfg!(windows) {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
    } else {
        Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
    };
    out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("local-stt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lstt-inst1-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn first_claim_owns_and_release_frees() {
        let d = scratch("own");
        let Claim::Owner(lock) = claim(&d) else {
            panic!("expected to own the lock");
        };
        assert!(d.join(LOCK).exists());
        release(&lock);
        assert!(!d.join(LOCK).exists());
    }

    #[test]
    fn stale_lock_from_a_dead_process_is_taken_over() {
        let d = scratch("stale");
        std::fs::write(d.join(LOCK), "999999").unwrap();
        assert!(matches!(claim(&d), Claim::Owner(Some(_))));
    }

    #[test]
    fn lock_naming_another_program_is_taken_over() {
        let d = scratch("other-prog");
        // PID 1 is launchd/init, alive but not local-stt.
        std::fs::write(d.join(LOCK), "1").unwrap();
        assert!(matches!(claim(&d), Claim::Owner(Some(_))));
    }

    #[test]
    fn show_request_is_taken_once() {
        let d = scratch("show");
        std::fs::write(d.join(SHOW), b"").unwrap();
        assert!(take_show_request(&d));
        assert!(!take_show_request(&d));
    }
}
