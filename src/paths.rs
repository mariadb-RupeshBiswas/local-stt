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
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("AppData").join("Roaming"))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_data_root() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("share"))
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_env_wins() {
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
