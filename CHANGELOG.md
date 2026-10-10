# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Push-to-talk dictation for macOS and Windows with a configurable shortcut
  (default Fn + Shift on macOS, Ctrl + Alt on Windows), Esc to cancel.
- Local speech recognition with whisper.cpp compiled in; English output by default with
  optional keep-spoken-language mode.
- Model picker with hardware check (RAM, CPU, GPU, free disk): Whisper Small (default),
  Medium and Large v3, downloaded on demand and verified by SHA-256.
- Glass recording pill with live microphone level, three styles (Pill, Waveform, Minimal),
  drag anywhere on any monitor with edge and corner snapping, reduced-motion support.
- Main window with searchable history, model tab and settings.
- Clipboard copy and automatic paste, optional clipboard restore, start and stop sounds.
- CLI: `doctor` (hardware report), `fetch-model`, `install`, `uninstall`, `check-update`,
  `demo` (a scripted tour with sample data), `--autostart on|off`.
- Hands-free shortcut: press once to start listening and again to stop, set in Settings
  next to Push to talk (adding its extra key while holding push to talk switches over).
  Hands-free can be turned off.
- Install row in Settings (Applications or Start menu, plus the `local-stt` terminal command),
  and a notice when turning on Start at login installs the app first.
- Smart formatting setting: spoken "point one, point two" becomes a numbered list, "uh" and
  "um" are removed, and spacing is fixed before pasting.
- Live transcription popover above the pill (below it near the top of the screen) that shows
  your latest words while you speak, with a Live transcription setting.
- Update banner at the top of the main window (Install and Restart, Release Notes, Later,
  Skip This Version), plus Check for Updates and Check automatically in Settings.
- Daily update check against PyPI (can be turned off), shown as a tray menu item, never a pop-up.
- Windows end-to-end CI job: installs the wheel, checks the Start menu shortcut and terminal
  command, downloads the model, holds Ctrl + Alt and checks the pill appears, then uninstalls.
- Hardened CI: pinned actions, cargo-deny, CodeQL, zizmor, headless UI checks, gated
  Trusted Publishing release flow with provenance.

### Fixed

- Quitting no longer aborts with a Metal assertion: the speech engine is freed before exit.
- Words from consecutive speech segments no longer run together ("hereSo").
- A transcript of only "uh" or "um" is reported as no speech instead of pasting nothing.

### Security

- Clipboard writes are marked private (skipped by clipboard managers, Windows clipboard
  history and cloud clipboard).
- Models are re-verified against their pinned checksum when first loaded.
- Each window can call only the commands it needs.
- History is capped at 2,000 entries and deleted when history is turned off.
