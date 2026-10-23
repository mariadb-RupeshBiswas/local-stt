//! One-time move of the Windows data folder from Roaming to Local app data.
//! Roaming is copied to a file server under roaming profiles, so models and private notes must not live there.
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    Moved,
    Waiting,
}

static LAST: OnceLock<Outcome> = OnceLock::new();

/// Runs first thing in main, before anything reads or writes the data folder. Windows only.
pub fn run() {
    if !cfg!(windows) || std::env::var_os("LOCAL_STT_DATA_DIR").is_some() {
        return;
    }
    let old = crate::paths::windows_legacy_dir(std::env::var_os("APPDATA"), &crate::paths::home());
    let new = crate::paths::windows_local_dir();
    let _ = LAST.set(migrate(&old, &new, |a, b| std::fs::rename(a, b)));
}

/// Logs the move once the log setting is known (main runs before the config is loaded).
pub fn log_outcome() {
    if let Some(line) = LAST.get().copied().and_then(describe) {
        crate::diag::log(line);
    }
}

pub fn describe(outcome: Outcome) -> Option<&'static str> {
    match outcome {
        Outcome::Nothing => None,
        Outcome::Moved => Some("data folder moved from Roaming to Local app data"),
        Outcome::Waiting => Some(
            "data folder stays in Roaming for now: a running copy holds files in it, or it is on another drive",
        ),
    }
}

// Only a rename: a running older copy holds the model open, and a copy would leave models behind, so a later start retries.
fn migrate(
    old: &Path,
    new: &Path,
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Outcome {
    if !old.is_dir() || new.exists() {
        return Outcome::Nothing;
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if rename(old, new).is_ok() {
        Outcome::Moved
    } else {
        Outcome::Waiting
    }
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
        std::fs::create_dir_all(old.join("models")).unwrap();
        std::fs::write(old.join("config.json"), "{}").unwrap();
        std::fs::write(old.join("models").join("big.bin"), "m").unwrap();
        old
    }

    #[test]
    fn a_free_folder_is_renamed_with_its_models() {
        let root = temp("rename");
        let old = fill_old(&root);
        let new = root.join("Local").join("local-stt");
        assert_eq!(
            migrate(&old, &new, |a, b| std::fs::rename(a, b)),
            Outcome::Moved
        );
        assert!(new.join("models").join("big.bin").exists());
        assert!(!old.exists());
    }

    #[test]
    fn a_blocked_rename_leaves_everything_in_roaming_for_a_later_start() {
        let root = temp("blocked");
        let old = fill_old(&root);
        let new = root.join("Local").join("local-stt");
        let deny =
            |_: &Path, _: &Path| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(migrate(&old, &new, deny), Outcome::Waiting);
        assert!(old.join("models").join("big.bin").exists());
        assert!(!new.exists(), "no half folder that would end the retries");
    }

    #[test]
    fn nothing_to_move_or_already_moved_touches_nothing() {
        let root = temp("noop");
        let boom = |_: &Path, _: &Path| panic!("must not touch the disk");
        let missing = root.join("Roaming").join("local-stt");
        assert_eq!(
            migrate(&missing, &root.join("Local"), boom),
            Outcome::Nothing
        );
        let old = fill_old(&root);
        let new = root.join("Local").join("local-stt");
        std::fs::create_dir_all(&new).unwrap();
        assert_eq!(migrate(&old, &new, boom), Outcome::Nothing);
    }

    #[test]
    fn log_lines_hold_no_paths() {
        assert!(describe(Outcome::Nothing).is_none());
        for o in [Outcome::Moved, Outcome::Waiting] {
            let line = describe(o).unwrap();
            assert!(!line.contains('\\') && !line.contains('/'));
        }
    }
}
