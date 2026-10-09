//! Start at login without a plugin: a LaunchAgent on macOS, a Run key on Windows.

use std::path::{Path, PathBuf};

const LABEL: &str = "io.github.localstt";

/// uvx runs binaries from a throwaway cache, so a login item would point at a vanishing path.
pub fn is_ephemeral(exe: &Path) -> bool {
    let p = exe.to_string_lossy().replace('\\', "/").to_lowercase();
    p.contains("/uv/archive-") || p.contains("/uv/cache/") || p.contains("/.cache/uv/")
}

pub fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("cannot locate the local-stt binary: {e}"))
}

pub fn set(enabled: bool) -> Result<(), String> {
    let exe = current_exe()?;
    if enabled && is_ephemeral(&exe) {
        return Err("local-stt is running from the uvx cache. Run `uv tool install local-stt`, then turn on start at login.".into());
    }
    platform::set(enabled, &exe)
}

pub fn is_enabled() -> bool {
    platform::is_enabled()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::LABEL;
    use std::path::{Path, PathBuf};

    fn plist_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
    }

    pub fn plist(exe: &Path) -> String {
        let exe = xml_escape(&exe.to_string_lossy());
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{LABEL}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{exe}</string>\n  </array>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>ProcessType</key>\n  <string>Interactive</string>\n</dict>\n</plist>\n"
        )
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    pub fn set(enabled: bool, exe: &Path) -> Result<(), String> {
        let path = plist_path().ok_or("HOME is not set")?;
        if !enabled {
            return match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
            };
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        std::fs::write(&path, plist(exe)).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    pub fn is_enabled() -> bool {
        plist_path().is_some_and(|p| p.exists())
    }
}

#[cfg(windows)]
mod platform {
    use std::path::Path;
    use std::process::Command;

    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "local-stt";

    pub fn set(enabled: bool, exe: &Path) -> Result<(), String> {
        let mut cmd = Command::new("reg");
        if enabled {
            let data = format!("\"{}\"", exe.display());
            cmd.args(["add", KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &data, "/f"]);
        } else {
            cmd.args(["delete", KEY, "/v", VALUE, "/f"]);
        }
        let out = cmd.output().map_err(|e| format!("cannot run reg: {e}"))?;
        if out.status.success() || !enabled {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    pub fn is_enabled() -> bool {
        Command::new("reg")
            .args(["query", KEY, "/v", VALUE])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    pub fn set(_enabled: bool, _exe: &std::path::Path) -> Result<(), String> {
        Err("start at login is supported on macOS and Windows only".into())
    }

    pub fn is_enabled() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uvx_cache_paths_are_ephemeral() {
        assert!(is_ephemeral(Path::new("/Users/a/.cache/uv/archive-v0/abc/bin/local-stt")));
        assert!(is_ephemeral(Path::new(r"C:\Users\a\AppData\Local\uv\cache\archive-v0\x\Scripts\local-stt.exe")));
        assert!(!is_ephemeral(Path::new("/Users/a/.local/bin/local-stt")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plist_escapes_path_and_names_label() {
        let p = platform::plist(Path::new("/Users/a&b/bin/local-stt"));
        assert!(p.contains("<string>/Users/a&amp;b/bin/local-stt</string>"));
        assert!(p.contains("<string>io.github.localstt</string>"));
    }
}
