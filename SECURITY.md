# Security Policy

## Supported versions

Only the latest release receives security fixes.

## Reporting a vulnerability

Please report vulnerabilities privately through
[GitHub private vulnerability reporting](https://github.com/mariadb-RupeshBiswas/local-stt/security/advisories/new).
Do not open a public issue for a security problem.

You can expect an acknowledgement within 7 days. Please include the version, your OS,
steps to reproduce and the impact you see.

## What the app can access

| Resource | Why | When |
|---|---|---|
| Microphone | Record your speech | Only while the push-to-talk shortcut is held, or between the two presses of the hands-free shortcut |
| Keyboard events | Detect the shortcut | Always while running; only the keys currently held are kept in memory to match the shortcut, never recorded, stored or sent. One key is held back from the focused app: the hands-free key (Space by default), and only while its exact modifiers are down |
| Synthetic paste keystroke | Type the text for you | After each dictation, when Paste is on |
| Clipboard (write, marked private) | Hand over the text; clipboard managers, Windows clipboard history and cloud clipboard skip it. macOS Universal Clipboard may still share it with your own nearby devices when Handoff is on | After each dictation |
| Network | Download a model from `huggingface.co` over HTTPS | First run and when you choose another model |
| Network | Ask `pypi.org` for the latest version number (nothing else is sent) | 30 s after start, then once a day; Settings > Check automatically turns it off |
| Running uv | Install a newer release you chose to install | Only when you click Install and Restart |
| Files in the app data folder | Settings, history, models | Always; created readable only by your user |
| Install locations | `~/Applications/local-stt.app` and `~/.local/bin/local-stt` (macOS); `%LOCALAPPDATA%\Programs\local-stt`, a Start menu shortcut and `~\.local\bin\local-stt.cmd` (Windows) | When you install, or the first time you start it from a terminal; uninstall removes exactly these and never a file it did not create |
| Troubleshooting log | App events such as starts, errors, timings and paste results in `logs/` inside the app data folder, never dictated text, audio, keys, clipboard contents or device names; home folder and user name masked; two files of 512 KB at most | While running, when Troubleshooting log is on (default); off deletes it |
| Error output | This run's stderr in `logs/stderr.log`, so a native crash's reason survives a launch from Finder, the Start menu or login | Same as the log |
| macOS crash reports | Reads local-stt's own `.ips` files in `~/Library/Logs/DiagnosticReports` and keeps only the cause and crashed thread | Only when you export a report |
| Diagnostic report | A text file in `diagnostics/` inside the app data folder, the five newest kept; never sent anywhere | Only when you click Export Report or run `local-stt diagnostics` |
| Login item | `~/Library/LaunchAgents/io.github.localstt.plist` (macOS) or a `HKCU\...\Run` value (Windows) | Only when Start at login is on |

The app sends no telemetry and no crash reports, and has no account.

## Supply chain

- Every GitHub Action is pinned to a full commit SHA; Dependabot keeps pins current.
- Workflows run with read-only tokens; write scopes exist only on the jobs that need them.
- `cargo deny` checks advisories, licenses and crate sources on every pull request.
- CodeQL and zizmor scan the code and the workflows.
- Release wheels are published with PyPI Trusted Publishing (no stored tokens) and carry
  build provenance attestations.
- whisper.cpp is vendored as a git submodule pinned to a release commit.
- Models are pinned by SHA-256, verified on download and again the first time they load in each run.
- Each app window can call only the commands it needs (Tauri capabilities); the recording pill can only read state.
