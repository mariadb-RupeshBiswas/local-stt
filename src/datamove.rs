//! One-time move of the Windows data folder from Roaming to Local app data.
//! Roaming is copied to a file server under roaming profiles, so models and private notes must not live there.
use std::ffi::OsStr;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Nothing,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    Renamed,
    Copied { failed: usize },
    Failed,
}

static LAST: OnceLock<Outcome> = OnceLock::new();

pub fn decide(override_set: bool, old_exists: bool, new_exists: bool) -> Action {
    if override_set || !old_exists || new_exists {
        Action::Nothing
    } else {
        Action::Move
    }
}

/// Re-downloadable models are left behind; the lock files belong to a running copy.
pub fn skip_on_copy(name: &OsStr) -> bool {
    ["models", "app.lock", "show.request"]
        .iter()
        .any(|n| name == OsStr::new(n))
}

pub fn describe(outcome: Outcome) -> Option<&'static str> {
    match outcome {
        Outcome::Nothing => None,
        Outcome::Renamed => Some("data folder moved from Roaming to Local app data"),
        Outcome::Copied { failed: 0 } => Some(
            "data folder copied from Roaming to Local app data; models are not copied and the old folder stays",
        ),
        Outcome::Copied { .. } => Some(
            "data folder copied from Roaming to Local app data with some files skipped; the old folder stays",
        ),
        Outcome::Failed => Some("data folder could not be moved from Roaming to Local app data"),
    }
}

/// Runs first thing in main, before anything reads or writes the data folder. Windows only.
pub fn run() {
    if !cfg!(windows) {
        return;
    }
    let override_set = std::env::var_os("LOCAL_STT_DATA_DIR").is_some();
    let old = crate::paths::windows_legacy_dir(std::env::var_os("APPDATA"), &crate::paths::home());
    let new = crate::paths::data_dir();
    let outcome = migrate(&old, &new, override_set, |a, b| std::fs::rename(a, b));
    let _ = LAST.set(outcome);
}

/// Logs the move once the log setting is known (main runs before the config is loaded).
pub fn log_outcome() {
    if let Some(line) = LAST.get().copied().and_then(describe) {
        crate::diag::log(line);
    }
}

fn migrate(
    old: &Path,
    new: &Path,
    override_set: bool,
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Outcome {
    match decide(override_set, old.is_dir(), new.exists()) {
        Action::Nothing => Outcome::Nothing,
        Action::Move => {
            if rename(old, new).is_ok() {
                return Outcome::Renamed;
            }
            match copy_fallback(old, new) {
                Ok(failed) => Outcome::Copied { failed },
                Err(_) => Outcome::Failed,
            }
        }
    }
}

// Copies into a sibling folder first, so a crash mid-copy never leaves a half folder that blocks a retry.
fn copy_fallback(old: &Path, new: &Path) -> std::io::Result<usize> {
    let mut staging = new.as_os_str().to_owned();
    staging.push(".moving");
    let staging = std::path::PathBuf::from(staging);
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    let mut failed = 0;
    copy_tree(old, &staging, true, &mut failed)?;
    std::fs::rename(&staging, new)?;
    Ok(failed)
}

fn copy_tree(from: &Path, to: &Path, top: bool, failed: &mut usize) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        if top && skip_on_copy(&entry.file_name()) {
            continue;
        }
        let target = to.join(entry.file_name());
        let copied = match entry.file_type() {
            Ok(t) if t.is_dir() => copy_tree(&entry.path(), &target, false, failed).is_ok(),
            Ok(t) if t.is_file() => std::fs::copy(entry.path(), &target).is_ok(),
            _ => true,
        };
        if !copied {
            *failed += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lstt-move-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn fill_old(root: &Path) -> PathBuf {
        let old = root.join("Roaming").join("local-stt");
        std::fs::create_dir_all(old.join("notes")).unwrap();
        std::fs::create_dir_all(old.join("models")).unwrap();
        std::fs::write(old.join("config.json"), "{}").unwrap();
        std::fs::write(old.join("notes").join("a.json"), "n").unwrap();
        std::fs::write(old.join("models").join("big.bin"), "m").unwrap();
        std::fs::write(old.join("app.lock"), "1").unwrap();
        old
    }

    #[test]
    fn decide_moves_only_when_old_exists_new_does_not_and_no_override() {
        assert_eq!(decide(false, true, false), Action::Move);
        assert_eq!(decide(true, true, false), Action::Nothing);
        assert_eq!(decide(false, false, false), Action::Nothing);
        assert_eq!(decide(false, true, true), Action::Nothing);
    }

    #[test]
    fn copy_skips_models_and_lock_files_only_at_top_level() {
        for name in ["models", "app.lock", "show.request"] {
            assert!(skip_on_copy(OsStr::new(name)));
        }
        for name in [
            "config.json",
            "notes",
            "history.jsonl",
            "logs",
            "voice.json",
        ] {
            assert!(!skip_on_copy(OsStr::new(name)));
        }
    }

    #[test]
    fn describe_is_silent_only_for_nothing() {
        assert!(describe(Outcome::Nothing).is_none());
        for o in [
            Outcome::Renamed,
            Outcome::Copied { failed: 0 },
            Outcome::Copied { failed: 2 },
            Outcome::Failed,
        ] {
            let line = describe(o).unwrap();
            assert!(!line.contains('\\') && !line.contains('/'));
        }
    }

    #[test]
    fn rename_success_moves_the_folder() {
        let root = temp("rename");
        let old = fill_old(&root);
        let new = root.join("Local").join("local-stt");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        let out = migrate(&old, &new, false, |a, b| std::fs::rename(a, b));
        assert_eq!(out, Outcome::Renamed);
        assert!(new.join("models").join("big.bin").exists());
        assert!(!old.exists());
    }

    #[test]
    fn failed_rename_falls_back_to_copy_without_models_and_keeps_old() {
        let root = temp("copy");
        let old = fill_old(&root);
        let new = root.join("Local").join("local-stt");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        let deny =
            |_: &Path, _: &Path| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let out = migrate(&old, &new, false, deny);
        assert_eq!(out, Outcome::Copied { failed: 0 });
        assert_eq!(
            std::fs::read_to_string(new.join("config.json")).unwrap(),
            "{}"
        );
        assert!(new.join("notes").join("a.json").exists());
        assert!(!new.join("models").exists());
        assert!(!new.join("app.lock").exists());
        assert!(!new.with_file_name("local-stt.moving").exists());
        assert!(old.join("models").join("big.bin").exists());
    }

    #[test]
    fn existing_new_folder_or_override_leaves_everything_alone() {
        let root = temp("noop");
        let old = fill_old(&root);
        let new = root.join("Local").join("local-stt");
        std::fs::create_dir_all(&new).unwrap();
        let boom = |_: &Path, _: &Path| panic!("must not touch the disk");
        assert_eq!(migrate(&old, &new, false, boom), Outcome::Nothing);
        let gone = root.join("Local2");
        assert_eq!(migrate(&old, &gone, true, boom), Outcome::Nothing);
        assert!(old.join("config.json").exists() && !gone.exists());
    }
}
