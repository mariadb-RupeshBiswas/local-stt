use std::path::PathBuf;

const APP_DIR: &str = "local-stt";

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LOCAL_STT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    platform_data_root().join(APP_DIR)
}

pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn history_path() -> PathBuf {
    data_dir().join("history.jsonl")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    let data = data_dir();
    std::fs::create_dir_all(models_dir())?;
    restrict_dir(&data)?;
    restrict_dir(&models_dir())
}

/// Creates a folder only this user can open (Unix mode 0700).
pub(crate) fn private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    restrict_dir(dir)
}

#[cfg(unix)]
fn restrict_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn restrict_dir(_dir: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_data_root() -> PathBuf {
    home().join("Library").join("Application Support")
}

#[cfg(windows)]
fn platform_data_root() -> PathBuf {
    let local = windows_data_root(std::env::var_os("LOCALAPPDATA"), &home());
    let roaming = windows_legacy_dir(std::env::var_os("APPDATA"), &home());
    if still_roaming(local.join(APP_DIR).exists(), roaming.is_dir()) {
        return roaming.parent().map(PathBuf::from).unwrap_or(local);
    }
    local
}

/// Until the one-time move succeeds (a running older copy can block it), the Roaming folder stays the data folder.
#[cfg_attr(not(windows), allow(dead_code))]
fn still_roaming(local_exists: bool, roaming_exists: bool) -> bool {
    roaming_exists && !local_exists
}

/// The Windows data folder once moved; the one-time move renames the Roaming folder to this.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_local_dir() -> PathBuf {
    windows_data_root(std::env::var_os("LOCALAPPDATA"), &home()).join(APP_DIR)
}

/// Local, not Roaming: roaming profiles copy Roaming to a file server, and this folder holds models and private notes.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_data_root(
    local_app_data: Option<std::ffi::OsString>,
    home: &std::path::Path,
) -> PathBuf {
    match local_app_data {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.join("AppData").join("Local"),
    }
}

/// Where older Windows versions kept the data folder (Roaming), for the one-time move.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_legacy_dir(
    app_data: Option<std::ffi::OsString>,
    home: &std::path::Path,
) -> PathBuf {
    let root = match app_data {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.join("AppData").join("Roaming"),
    };
    root.join(APP_DIR)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_data_root() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("share"))
}

pub(crate) fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Tests that point LOCAL_STT_DATA_DIR somewhere hold this, since the variable is process-wide.
#[cfg(test)]
pub(crate) static TEST_DATA_DIR: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_roots_use_local_then_roaming_with_home_fallbacks() {
        let home = std::path::Path::new("H");
        let local = Some(std::ffi::OsString::from("L"));
        assert_eq!(windows_data_root(local, home), PathBuf::from("L"));
        let fallback = home.join("AppData").join("Local");
        assert_eq!(windows_data_root(None, home), fallback);
        assert_eq!(windows_data_root(Some("".into()), home), fallback);
        let old = windows_legacy_dir(Some("R".into()), home);
        assert_eq!(old, PathBuf::from("R").join(APP_DIR));
        let old_fallback = home.join("AppData").join("Roaming").join(APP_DIR);
        assert_eq!(windows_legacy_dir(None, home), old_fallback);
    }

    #[test]
    fn roaming_stays_the_data_folder_until_the_move_happens() {
        assert!(still_roaming(false, true));
        assert!(!still_roaming(true, true));
        assert!(!still_roaming(false, false));
        assert!(!still_roaming(true, false));
    }

    #[test]
    fn override_env_wins() {
        let _env = TEST_DATA_DIR.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join("lstt-paths-test");
        std::env::set_var("LOCAL_STT_DATA_DIR", &dir);
        assert_eq!(data_dir(), dir);
        assert_eq!(models_dir(), dir.join("models"));
        assert_eq!(config_path(), dir.join("config.json"));
        assert_eq!(history_path(), dir.join("history.jsonl"));
        ensure_dirs().unwrap();
        assert!(models_dir().is_dir());
        std::env::remove_var("LOCAL_STT_DATA_DIR");
    }
}
