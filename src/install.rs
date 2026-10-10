//! Installs a lasting copy: Spotlight-visible app on macOS, Start menu entry on Windows, terminal command on both.

use std::path::{Path, PathBuf};

const EXE_NAME: &str = if cfg!(windows) {
    "local-stt.exe"
} else {
    "local-stt"
};
// Names the build an installed copy came from; macOS re-signs the copy, so its own hash never matches the build's.
const STAMP: &str = "local-stt.source";
/// Set on a copy started by the hand-off, so it runs where it is instead of handing off again.
pub const IN_PROCESS: &str = "LOCAL_STT_IN_PROCESS";

/// Where the installed binary lives on this platform.
pub fn installed_exe() -> Option<PathBuf> {
    platform::installed_exe()
}

pub fn is_installed() -> bool {
    installed_exe().is_some_and(|p| p.is_file())
}

/// Installs from `exe` (usually the running binary) and returns the installed binary plus notes.
pub fn install(exe: &Path) -> Result<(PathBuf, Vec<String>), String> {
    let source = crate::download::sha256_file(exe).unwrap_or_default();
    let target = platform::install(exe, &source)?;
    let mut notes = vec![platform::installed_note(&target)];
    if let Some(bin) = user_bin_dir() {
        let path_env = std::env::var_os("PATH").unwrap_or_default();
        match link_into(&target, &bin, &path_env) {
            Ok(note) => notes.push(note),
            Err(e) => notes.push(format!("The terminal command was not added: {e}")),
        }
    }
    Ok((target, notes))
}

fn stamp_path(installed: &Path) -> Option<PathBuf> {
    let dir = installed.parent()?;
    if cfg!(target_os = "macos") {
        Some(dir.parent()?.join("Resources").join(STAMP))
    } else {
        Some(dir.join(STAMP))
    }
}

/// True when the installed copy was made from this very build.
fn installed_from(exe: &Path, installed: &Path) -> bool {
    if same_file(exe, installed) {
        return true;
    }
    let stamp = stamp_path(installed).and_then(|p| std::fs::read_to_string(p).ok());
    let source = crate::download::sha256_file(exe).ok();
    installed.is_file() && stamp.is_some() && stamp == source
}

/// Typed in a terminal, `local-stt` hands over to the installed app so it runs on its own: macOS then
/// asks for permissions as local-stt, not as the terminal, and closing the terminal leaves it running.
/// Returns what to tell the user, or None to run right here (already standalone, a test run, or the hand-off failed).
pub fn hand_off() -> Option<String> {
    let test_run = [
        IN_PROCESS,
        "LOCAL_STT_EXIT_AFTER_MS",
        "LOCAL_STT_DATA_DIR",
        "LOCAL_STT_NO_PERMISSION_PROMPTS",
    ]
    .iter()
    .any(|v| std::env::var_os(v).is_some());
    if test_run || !platform::started_by_a_terminal() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let installed = match installed_exe() {
        Some(p) if installed_from(&exe, &p) => p,
        _ => match install(&exe) {
            Ok((p, _)) => p,
            Err(e) => {
                eprintln!(
                    "local-stt: could not install the app, so it runs from this terminal: {e}"
                );
                return None;
            }
        },
    };
    match platform::open_app(&installed) {
        Ok(()) => Some(platform::handed_off_note()),
        Err(e) => {
            eprintln!(
                "local-stt: could not open the installed app, so it runs from this terminal: {e}"
            );
            None
        }
    }
}

/// Removes everything `install` created, but never a command file it did not create.
pub fn uninstall() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if let (Some(bin), Some(target)) = (user_bin_dir(), installed_exe()) {
        if remove_command(&target, &bin)? {
            notes.push(format!(
                "Removed the local-stt command from {}.",
                bin.display()
            ));
        }
    }
    notes.extend(platform::uninstall()?);
    Ok(notes)
}

fn user_bin_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".local").join("bin"))
}

/// Copies `exe` into `dir` as the app binary, replacing an older copy atomically.
pub fn copy_binary(exe: &Path, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let dest = dir.join(EXE_NAME);
    if same_file(exe, &dest) {
        return Ok(dest);
    }
    let tmp = dir.join(format!("{EXE_NAME}.new"));
    let _ = std::fs::remove_file(&tmp);
    std::fs::copy(exe, &tmp).map_err(|e| format!("cannot copy local-stt: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("cannot set permissions: {e}"))?;
    }
    // Windows refuses to overwrite a running exe but allows renaming it, so an update moves the old one aside.
    // Each move gets its own name, and leftovers from earlier updates are cleared when no longer running.
    sweep_old_copies(dir);
    let old = dir.join(format!("{EXE_NAME}.old-{}", std::process::id()));
    if cfg!(windows) && dest.exists() {
        std::fs::rename(&dest, &old).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("cannot replace {}: {e}", dest.display())
        })?;
    }
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::rename(&old, &dest);
        format!("cannot install local-stt to {}: {e}", dest.display())
    })?;
    Ok(dest)
}

fn sweep_old_copies(dir: &Path) {
    let prefix = format!("{EXE_NAME}.old");
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Puts a `local-stt` command in `bin` that runs `target`; never replaces a file it did not create.
pub fn link_into(target: &Path, bin: &Path, path_env: &std::ffi::OsStr) -> Result<String, String> {
    std::fs::create_dir_all(bin).map_err(|e| format!("cannot create {}: {e}", bin.display()))?;
    let on_path = std::env::split_paths(path_env).any(|p| p == bin);
    let hint = if on_path {
        String::new()
    } else {
        format!(
            " Add {} to your PATH to use it in any terminal.",
            bin.display()
        )
    };
    match command_link(target, bin)? {
        Some(link) => Ok(format!(
            "The local-stt command is at {}.{hint}",
            link.display()
        )),
        None => Ok(format!(
            "A local-stt command already exists in {}, so it was left unchanged.",
            bin.display()
        )),
    }
}

#[cfg(unix)]
fn command_link(target: &Path, bin: &Path) -> Result<Option<PathBuf>, String> {
    let link = bin.join(EXE_NAME);
    match std::fs::read_link(&link) {
        Ok(existing) if existing == target => return Ok(Some(link)),
        Ok(_) => return Ok(None),
        Err(_) if link.symlink_metadata().is_ok() => return Ok(None),
        Err(_) => {}
    }
    std::os::unix::fs::symlink(target, &link)
        .map_err(|e| format!("cannot link {}: {e}", link.display()))?;
    Ok(Some(link))
}

#[cfg(unix)]
fn remove_command(target: &Path, bin: &Path) -> Result<bool, String> {
    let link = bin.join(EXE_NAME);
    match std::fs::read_link(&link) {
        Ok(existing) if existing == target => std::fs::remove_file(&link)
            .map(|()| true)
            .map_err(|e| format!("cannot remove {}: {e}", link.display())),
        _ => Ok(false),
    }
}

// Marks a Windows command shim as ours, so it may be refreshed or removed but a foreign file never is.
#[cfg(windows)]
const SHIM_MARKER: &str = "rem local-stt launcher";

#[cfg(windows)]
fn command_link(target: &Path, bin: &Path) -> Result<Option<PathBuf>, String> {
    if bin.join("local-stt.exe").exists() {
        return Ok(None);
    }
    let shim = bin.join("local-stt.cmd");
    if let Ok(existing) = std::fs::read_to_string(&shim) {
        if !existing.contains(SHIM_MARKER) {
            return Ok(None);
        }
    }
    let body = format!(
        "@echo off\r\n{SHIM_MARKER}\r\n\"{}\" %*\r\n",
        target.display()
    );
    std::fs::write(&shim, body).map_err(|e| format!("cannot write {}: {e}", shim.display()))?;
    Ok(Some(shim))
}

#[cfg(windows)]
fn remove_command(_target: &Path, bin: &Path) -> Result<bool, String> {
    let shim = bin.join("local-stt.cmd");
    match std::fs::read_to_string(&shim) {
        Ok(existing) if existing.contains(SHIM_MARKER) => std::fs::remove_file(&shim)
            .map(|()| true)
            .map_err(|e| format!("cannot remove {}: {e}", shim.display())),
        _ => Ok(false),
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::{Path, PathBuf};

    const ICON: &[u8] = include_bytes!("../icons/icon.icns");

    fn app_dir() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Applications")
                .join("local-stt.app"),
        )
    }

    pub fn installed_exe() -> Option<PathBuf> {
        app_dir().map(|a| a.join("Contents").join("MacOS").join(super::EXE_NAME))
    }

    pub fn bundle_plist(version: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>CFBundleExecutable</key><string>local-stt</string>\n  <key>CFBundleIdentifier</key><string>io.github.localstt</string>\n  <key>CFBundleName</key><string>local-stt</string>\n  <key>CFBundleDisplayName</key><string>local-stt</string>\n  <key>CFBundleIconFile</key><string>icon</string>\n  <key>CFBundlePackageType</key><string>APPL</string>\n  <key>CFBundleShortVersionString</key><string>{version}</string>\n  <key>CFBundleVersion</key><string>{version}</string>\n  <key>LSMinimumSystemVersion</key><string>11.0</string>\n  <key>LSUIElement</key><true/>\n  <key>NSHighResolutionCapable</key><true/>\n  <key>NSMicrophoneUsageDescription</key><string>local-stt listens only while you hold your shortcut, and turns your speech into text on this computer. Audio never leaves your device.</string>\n</dict>\n</plist>\n"
        )
    }

    /// An existing bundle is ours only if its Info.plist names our identifier.
    pub(super) fn is_ours(app: &Path) -> bool {
        std::fs::read_to_string(app.join("Contents").join("Info.plist"))
            .is_ok_and(|plist| plist.contains("<string>io.github.localstt</string>"))
    }

    // Launched by LaunchServices or launchd (Finder, Spotlight, login item, `open`), the parent is launchd.
    pub fn started_by_a_terminal() -> bool {
        std::os::unix::process::parent_id() != 1
    }

    // Through LaunchServices the app is its own process, so macOS shows its name in prompts and indicators.
    pub fn open_app(_installed: &Path) -> Result<(), String> {
        let app = app_dir().ok_or("HOME is not set")?;
        let status = crate::proc::command("/usr/bin/open")
            .arg(&app)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("open exited with {status}"))
        }
    }

    pub fn handed_off_note() -> String {
        "local-stt is running from your Applications folder; look for it in the menu bar. You can close this terminal.".into()
    }

    pub fn install(exe: &Path, source: &str) -> Result<PathBuf, String> {
        let app = app_dir().ok_or("HOME is not set")?;
        if app.exists() && !is_ours(&app) {
            return Err(format!(
                "{} belongs to another app, so it was left alone.",
                app.display()
            ));
        }
        let contents = app.join("Contents");
        let resources = contents.join("Resources");
        std::fs::create_dir_all(&resources)
            .map_err(|e| format!("cannot create {}: {e}", resources.display()))?;
        std::fs::write(
            contents.join("Info.plist"),
            bundle_plist(env!("CARGO_PKG_VERSION")),
        )
        .map_err(|e| format!("cannot write Info.plist: {e}"))?;
        std::fs::write(resources.join("icon.icns"), ICON)
            .map_err(|e| format!("cannot write the app icon: {e}"))?;
        // Written before signing, so the seal covers it.
        std::fs::write(resources.join(super::STAMP), source)
            .map_err(|e| format!("cannot write the build stamp: {e}"))?;
        let exe = super::copy_binary(exe, &contents.join("MacOS"))?;
        // Ad-hoc signing binds Info.plist to the binary so macOS treats it as one app.
        let _ = std::process::Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-"])
            .arg(&app)
            .output();
        // Ask Spotlight to index it now rather than at its next pass.
        let _ = std::process::Command::new("/usr/bin/mdimport")
            .arg(&app)
            .output();
        Ok(exe)
    }

    pub fn installed_note(_exe: &Path) -> String {
        "Installed local-stt in your Applications folder; Spotlight can find it.".into()
    }

    pub fn uninstall() -> Result<Vec<String>, String> {
        let app = app_dir().ok_or("HOME is not set")?;
        if !app.exists() {
            return Ok(vec![]);
        }
        if !is_ours(&app) {
            return Ok(vec![format!(
                "{} is not local-stt's, so it was left alone.",
                app.display()
            )]);
        }
        std::fs::remove_dir_all(&app)
            .map_err(|e| format!("cannot remove {}: {e}", app.display()))?;
        Ok(vec![format!("Removed {}.", app.display())])
    }
}

#[cfg(windows)]
mod platform {
    use std::path::{Path, PathBuf};

    fn program_dir() -> Option<PathBuf> {
        let local = std::env::var_os("LOCALAPPDATA")?;
        Some(PathBuf::from(local).join("Programs").join("local-stt"))
    }

    fn shortcut() -> Option<PathBuf> {
        let roaming = std::env::var_os("APPDATA")?;
        Some(
            PathBuf::from(roaming)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("local-stt.lnk"),
        )
    }

    pub fn installed_exe() -> Option<PathBuf> {
        program_dir().map(|d| d.join(super::EXE_NAME))
    }

    fn powershell() -> PathBuf {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        PathBuf::from(root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    }

    // A marker file proves the folder was made by install, so uninstall may remove it.
    const MARKER: &str = ".installed-by-local-stt";

    fn run_powershell(script: &str, env: &[(&str, &Path)]) -> Result<String, String> {
        let mut cmd = crate::proc::command(powershell());
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ]);
        for (key, value) in env {
            cmd.env(key, value);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("cannot run PowerShell: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// The shortcut is ours when it points at our installed exe.
    fn shortcut_is_ours(lnk: &Path, target: &Path) -> bool {
        let script =
            "(New-Object -ComObject WScript.Shell).CreateShortcut($env:LSTT_LNK).TargetPath";
        run_powershell(script, &[("LSTT_LNK", lnk)])
            .is_ok_and(|found| std::path::Path::new(&found) == target)
    }

    // A Start menu or login launch gets a console of its own; a terminal shares one with the shell.
    pub fn started_by_a_terminal() -> bool {
        crate::output::console_is_shared()
    }

    // The installed copy starts with no console, so it does not belong to this terminal.
    pub fn open_app(installed: &Path) -> Result<(), String> {
        crate::proc::command(installed)
            .env(super::IN_PROCESS, "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn handed_off_note() -> String {
        "local-stt is running from the Start menu copy; look for it in the system tray. You can close this terminal.".into()
    }

    pub fn install(exe: &Path, source: &str) -> Result<PathBuf, String> {
        let dir = program_dir().ok_or("LOCALAPPDATA is not set")?;
        if dir.exists()
            && !dir.join(MARKER).exists()
            && dir.read_dir().is_ok_and(|mut d| d.next().is_some())
        {
            return Err(format!(
                "{} holds files local-stt did not create, so it was left alone.",
                dir.display()
            ));
        }
        let target = super::copy_binary(exe, &dir)?;
        std::fs::write(dir.join(MARKER), b"")
            .map_err(|e| format!("cannot mark the install folder: {e}"))?;
        std::fs::write(dir.join(super::STAMP), source)
            .map_err(|e| format!("cannot write the build stamp: {e}"))?;
        let lnk = shortcut().ok_or("APPDATA is not set")?;
        if lnk.exists() && !shortcut_is_ours(&lnk, &target) {
            return Ok(target);
        }
        // Paths travel as environment variables, never spliced into the script, so no quoting can break out.
        let script = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:LSTT_LNK); $s.TargetPath = $env:LSTT_TARGET; $s.WorkingDirectory = $env:LSTT_DIR; $s.Description = 'Free, local push-to-talk speech-to-text'; $s.Save()";
        run_powershell(
            script,
            &[
                ("LSTT_LNK", &lnk),
                ("LSTT_TARGET", &target),
                ("LSTT_DIR", &dir),
            ],
        )
        .map_err(|e| format!("cannot create the Start menu shortcut: {e}"))?;
        Ok(target)
    }

    pub fn installed_note(_exe: &Path) -> String {
        "Installed local-stt and added it to the Start menu, so Windows search can find it.".into()
    }

    pub fn uninstall() -> Result<Vec<String>, String> {
        let mut notes = Vec::new();
        let target = installed_exe().unwrap_or_default();
        if let Some(lnk) = shortcut().filter(|p| p.exists()) {
            if shortcut_is_ours(&lnk, &target) {
                std::fs::remove_file(&lnk)
                    .map_err(|e| format!("cannot remove {}: {e}", lnk.display()))?;
                notes.push("Removed the Start menu shortcut.".to_string());
            }
        }
        if let Some(dir) = program_dir().filter(|p| p.join(MARKER).exists()) {
            // Windows cannot delete a running program, so the installed copy cannot remove its own folder.
            let running_from_dir = std::env::current_exe()
                .ok()
                .and_then(|e| e.canonicalize().ok())
                .zip(dir.canonicalize().ok())
                .is_some_and(|(exe, d)| exe.starts_with(d));
            if running_from_dir {
                // Let a detached PowerShell remove the folder once this process has exited.
                let script = "Start-Sleep -Seconds 3; Remove-Item -LiteralPath $env:LSTT_DIR -Recurse -Force";
                let spawned = crate::proc::command(powershell())
                    .args([
                        "-NoProfile",
                        "-NonInteractive",
                        "-WindowStyle",
                        "Hidden",
                        "-Command",
                        script,
                    ])
                    .env("LSTT_DIR", &dir)
                    .spawn();
                notes.push(match spawned {
                    Ok(_) => format!("{} will be removed in a few seconds.", dir.display()),
                    Err(_) => format!("Close local-stt, then delete {} to finish.", dir.display()),
                });
            } else {
                std::fs::remove_dir_all(&dir)
                    .map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
                notes.push(format!("Removed {}.", dir.display()));
            }
        }
        Ok(notes)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use std::path::{Path, PathBuf};

    pub fn installed_exe() -> Option<PathBuf> {
        Some(crate::paths::data_dir().join("bin").join(super::EXE_NAME))
    }

    pub fn started_by_a_terminal() -> bool {
        false
    }

    pub fn open_app(_installed: &Path) -> Result<(), String> {
        Err("not supported here".into())
    }

    pub fn handed_off_note() -> String {
        String::new()
    }

    pub fn install(exe: &Path, source: &str) -> Result<PathBuf, String> {
        let target = super::copy_binary(exe, &crate::paths::data_dir().join("bin"))?;
        let _ = std::fs::write(target.with_file_name(super::STAMP), source);
        Ok(target)
    }

    pub fn installed_note(exe: &Path) -> String {
        format!("Installed local-stt to {}.", exe.display())
    }

    pub fn uninstall() -> Result<Vec<String>, String> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installed_copy_counts_as_this_build_only_by_its_stamp() {
        let root = std::env::temp_dir().join(format!("lstt-stamp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let bin_dir = if cfg!(target_os = "macos") {
            root.join("App").join("Contents").join("MacOS")
        } else {
            root.join("App")
        };
        std::fs::create_dir_all(&bin_dir).unwrap();
        let installed = bin_dir.join(EXE_NAME);
        std::fs::write(&installed, b"signed copy").unwrap();
        let source = root.join("source-build");
        std::fs::write(&source, b"the build").unwrap();
        assert!(!installed_from(&source, &installed), "no stamp yet");
        let stamp = stamp_path(&installed).unwrap();
        std::fs::create_dir_all(stamp.parent().unwrap()).unwrap();
        std::fs::write(&stamp, crate::download::sha256_file(&source).unwrap()).unwrap();
        assert!(installed_from(&source, &installed));
        std::fs::write(&source, b"a newer build").unwrap();
        assert!(
            !installed_from(&source, &installed),
            "a new build reinstalls"
        );
        assert!(
            installed_from(&installed, &installed),
            "the installed copy itself"
        );
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lstt-inst-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn copy_replaces_older_binary() {
        let d = scratch("copy");
        let exe = d.join("source-bin");
        std::fs::write(&exe, b"v1").unwrap();
        let dest = copy_binary(&exe, &d.join("bin")).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"v1");
        std::fs::write(&exe, b"v2").unwrap();
        assert_eq!(copy_binary(&exe, &d.join("bin")).unwrap(), dest);
        assert_eq!(std::fs::read(&dest).unwrap(), b"v2");
    }

    #[test]
    fn copying_onto_itself_is_a_no_op() {
        let d = scratch("self");
        let dir = d.join("bin");
        std::fs::create_dir_all(&dir).unwrap();
        let installed = dir.join(EXE_NAME);
        std::fs::write(&installed, b"same").unwrap();
        assert_eq!(copy_binary(&installed, &dir).unwrap(), installed);
        assert_eq!(std::fs::read(&installed).unwrap(), b"same");
    }

    #[test]
    fn link_is_created_reused_and_removed() {
        let d = scratch("link");
        let target = d.join("installed");
        std::fs::write(&target, b"x").unwrap();
        let bin = d.join("bin");
        let path_env = std::env::join_paths([bin.clone()]).unwrap();
        let first = link_into(&target, &bin, &path_env).unwrap();
        assert!(first.contains("command is at"), "{first}");
        assert!(!first.contains("Add "), "{first}");
        let again = link_into(&target, &bin, &path_env).unwrap();
        assert!(again.contains("command is at"), "{again}");
        assert!(remove_command(&target, &bin).unwrap());
        assert!(!remove_command(&target, &bin).unwrap());
    }

    #[test]
    fn foreign_command_is_never_replaced_or_removed() {
        let d = scratch("foreign");
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let foreign = bin.join(EXE_NAME);
        std::fs::write(&foreign, b"someone else").unwrap();
        let target = d.join("installed");
        std::fs::write(&target, b"x").unwrap();
        let note = link_into(&target, &bin, std::ffi::OsStr::new("")).unwrap();
        assert!(note.contains("left unchanged"), "{note}");
        assert!(!remove_command(&target, &bin).unwrap());
        assert_eq!(std::fs::read(&foreign).unwrap(), b"someone else");
    }

    #[test]
    fn missing_path_entry_is_pointed_out() {
        let d = scratch("path");
        let target = d.join("installed");
        std::fs::write(&target, b"x").unwrap();
        let note = link_into(&target, &d.join("bin"), std::ffi::OsStr::new("")).unwrap();
        assert!(note.contains("to your PATH"), "{note}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_a_bundle_with_our_identifier_counts_as_ours() {
        let d = scratch("bundle");
        let ours = d.join("ours.app");
        let foreign = d.join("foreign.app");
        std::fs::create_dir_all(ours.join("Contents")).unwrap();
        std::fs::create_dir_all(foreign.join("Contents")).unwrap();
        std::fs::write(
            ours.join("Contents/Info.plist"),
            platform::bundle_plist("1.0"),
        )
        .unwrap();
        std::fs::write(
            foreign.join("Contents/Info.plist"),
            "<plist><dict><key>CFBundleIdentifier</key><string>com.example.other</string></dict></plist>",
        )
        .unwrap();
        assert!(platform::is_ours(&ours));
        assert!(!platform::is_ours(&foreign));
        assert!(!platform::is_ours(&d.join("missing.app")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundle_plist_names_the_app_for_spotlight() {
        let p = platform::bundle_plist("9.9.9");
        for needle in [
            "<key>CFBundleExecutable</key><string>local-stt</string>",
            "<key>CFBundleIdentifier</key><string>io.github.localstt</string>",
            "<key>CFBundleIconFile</key><string>icon</string>",
            "<key>CFBundlePackageType</key><string>APPL</string>",
            "<string>9.9.9</string>",
            "<key>LSUIElement</key><true/>",
            "NSMicrophoneUsageDescription",
        ] {
            assert!(p.contains(needle), "missing {needle}");
        }
    }
}
