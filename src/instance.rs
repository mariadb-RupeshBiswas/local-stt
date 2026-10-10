//! One running copy per data folder: a lock file naming the owner's PID, checked against the live process.

use std::io::Write;
use std::path::{Path, PathBuf};

const LOCK: &str = "app.lock";
const SHOW: &str = "show.request";
/// Set by a copy that installed an update on the copy it starts: its own PID, which is about to quit.
pub const REPLACES: &str = "LOCAL_STT_REPLACES";
// Covers the old copy's engine shutdown (up to 10 s) with room to spare.
const HANDOVER_WAIT_MS: u64 = 15_000;

pub enum Claim {
    /// This process runs the app; remove the lock at exit. `unclean` means the last run never removed its lock.
    Owner {
        lock: Option<PathBuf>,
        unclean: bool,
    },
    /// Another copy is running and has been asked to show its window.
    Other,
}

pub fn claim(dir: &Path) -> Claim {
    let replaces = std::env::var(REPLACES)
        .ok()
        .and_then(|v| v.parse::<u32>().ok());
    // Read once; children of this copy must not inherit it.
    std::env::remove_var(REPLACES);
    claim_with(dir, replaces, is_local_stt)
}

fn claim_with(dir: &Path, replaces: Option<u32>, running: fn(u32) -> bool) -> Claim {
    let path = dir.join(LOCK);
    let mut unclean = false;
    for _ in 0..3 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                let _ = write!(f, "{}", std::process::id());
                return Claim::Owner {
                    lock: Some(path),
                    unclean,
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok());
                if pid.is_some_and(running) {
                    // An update's handover: wait for the old copy to quit, then take its place.
                    let old = pid.filter(|p| Some(*p) == replaces);
                    if old.is_some_and(|p| wait_for_exit(p, running)) {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    let _ = std::fs::write(dir.join(SHOW), b"");
                    return Claim::Other;
                }
                // Left behind by a crash or a forced quit: take it over.
                unclean = true;
                let _ = std::fs::remove_file(&path);
            }
            // A read-only or odd folder must not stop the app from starting.
            Err(_) => {
                return Claim::Owner {
                    lock: None,
                    unclean,
                }
            }
        }
    }
    Claim::Owner {
        lock: None,
        unclean,
    }
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

fn wait_for_exit(pid: u32, running: fn(u32) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(HANDOVER_WAIT_MS);
    while std::time::Instant::now() < deadline {
        if !running(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

// A PID can be reused, so the process must also be local-stt to count.
fn is_local_stt(pid: u32) -> bool {
    if pid == std::process::id() {
        return false;
    }
    let out = if cfg!(windows) {
        crate::proc::command("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
    } else {
        crate::proc::command("/bin/ps")
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
        let Claim::Owner { lock, unclean } = claim(&d) else {
            panic!("expected to own the lock");
        };
        assert!(!unclean);
        assert!(d.join(LOCK).exists());
        release(&lock);
        assert!(!d.join(LOCK).exists());
    }

    #[test]
    fn stale_lock_from_a_dead_process_is_taken_over() {
        let d = scratch("stale");
        std::fs::write(d.join(LOCK), "999999").unwrap();
        assert!(matches!(
            claim(&d),
            Claim::Owner {
                lock: Some(_),
                unclean: true
            }
        ));
    }

    #[test]
    fn lock_naming_another_program_is_taken_over() {
        let d = scratch("other-prog");
        // PID 1 is launchd/init, alive but not local-stt.
        std::fs::write(d.join(LOCK), "1").unwrap();
        assert!(matches!(claim(&d), Claim::Owner { lock: Some(_), .. }));
    }

    fn always(_: u32) -> bool {
        true
    }

    // Alive for the first few checks, then gone, like an old copy finishing its shutdown.
    fn quits_soon(_: u32) -> bool {
        use std::sync::atomic::{AtomicU32, Ordering};
        static CALLS: AtomicU32 = AtomicU32::new(0);
        CALLS.fetch_add(1, Ordering::SeqCst) < 3
    }

    #[test]
    fn a_running_copy_is_asked_to_show_itself() {
        let d = scratch("other");
        std::fs::write(d.join(LOCK), "4242").unwrap();
        assert!(matches!(claim_with(&d, None, always), Claim::Other));
        assert!(take_show_request(&d));
    }

    #[test]
    fn an_update_waits_for_the_copy_it_replaces() {
        let d = scratch("handover");
        std::fs::write(d.join(LOCK), "4242").unwrap();
        let claim = claim_with(&d, Some(4242), quits_soon);
        assert!(matches!(
            claim,
            Claim::Owner {
                lock: Some(_),
                unclean: false
            }
        ));
        assert!(!take_show_request(&d));
    }

    #[test]
    fn show_request_is_taken_once() {
        let d = scratch("show");
        std::fs::write(d.join(SHOW), b"").unwrap();
        assert!(take_show_request(&d));
        assert!(!take_show_request(&d));
    }
}
