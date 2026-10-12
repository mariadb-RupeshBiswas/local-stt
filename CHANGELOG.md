# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-10-12

### Added

- Notes: name the speakers. Type a name next to Me or Others above the transcript and every
  line, and the copied note, uses it; clear it to go back. Works while recording too.

## [0.5.0] - 2026-10-12

### Added

- CI rebuilds each speech model from OpenAI's own Whisper checkpoint and checks it is the exact
  file the app downloads, apart from a version number in its header.
- The pill names the model in use ("Whisper Medium") under "Recording" and "Transcribing".
- After an update, the new version opens its window with confetti and "Updated to local-stt ...",
  with What's New and Close. No confetti when Reduce Motion is on.

### Changed

- Release Notes opens this version's notes when no newer version is known, not the list of
  releases.

## [0.4.1] - 2026-10-12

### Fixed

- macOS: the 0.4.0 wheel on PyPI was a plain build, not the one signed with the release
  certificate, so installing it resets permissions. Skip 0.4.0: updating from 0.3.0 to 0.4.1
  keeps them; from 0.4.0 they are asked for once more.
- Releases publish only the signed macOS wheel and the Windows wheel, and stop if the macOS
  wheel is not the signed one.

## [0.4.0] - 2026-10-12

### Added

- Model tab: each model shows its SHA-256 checksum and a View on Hugging Face button, so you
  can check the file the app downloads against its source.
- Model tab: Delete removes a downloaded model you no longer use and frees its disk space; the
  model in use is kept.

## [0.3.0] - 2026-10-12

### Added

- macOS: releases are signed with local-stt's own certificate, so permissions stay allowed
  across updates, as with Zoom or Wispr Flow. Updating to 0.3.0 asks for them one last time.

### Changed

- Releases build the wheels while the full CI runs on the merge commit, about 6 minutes
  faster; nothing is published until both pass.
- macOS: Reinstall opens setup again after resetting permissions, so you can allow the
  microphone and Accessibility in one place; your name, settings, history and insights stay.

### Fixed

- macOS: after an update, Accessibility read "Not allowed" and the shortcut did nothing, though
  local-stt's switch was still on. macOS ties permissions to how each build is signed, and every
  build was a new app to it. Installing a build that macOS sees as new now resets local-stt's
  stale permissions and opens setup again (name, microphone test, Accessibility, model), keeping
  your settings, history, notes and insights.

### Security

- The macOS signing key lives only in the GitHub release environment; a release without it fails
  instead of shipping an ad-hoc app. Every pull request checks the signing path with a throwaway
  certificate.

## [0.2.0] - 2026-10-12

### Added

- While you talk, the pill follows focus: switch to a window on another screen and the pill
  (and the live text) moves there within a third of a second. A pill you dragged stays put
  until focus changes screen.
- Insights: an info button next to Freezes explains how freezes are earned and used, the
  countdown reads "+1 freeze after 3 more active workdays", and full freezes say so.

### Fixed

- macOS: the pill showed on another screen while you dictated into local-stt's own window,
  such as setup's Try it box, because the app's own windows never counted as focused.

## [0.1.1] - 2026-10-12

### Fixed

- `local-stt uninstall` reports only what was there: no login-item line when start at login
  was off, no "settings are kept" line when there is no data folder, and a plain "nothing to
  remove" on a clean machine.

## [0.1.0] - 2026-10-11

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
- Smart formatting setting: spoken lists ("point one ... point two", "number one ...",
  "first, ... second, ...", "the first point is ...") become a numbered list, "uh", "um" and
  "er" are removed, and spacing is fixed before pasting. Markers count mid-sentence too, unless
  they follow words like "this", "my" or "at"; "point one is how ..." lists "How ..."; "to
  points" before a two-item list reads "two points"; a closing remark stays out of the last item.
  Forms can mix ("first point ... point two ... number three"), and a bare "So" or "Okay"
  before the list is dropped.
- Live transcription popover above the pill (below it near the top of the screen) that shows
  your latest words while you speak, with a Live transcription setting.
- Update banner at the top of the main window (Install and Restart, Release Notes, Later,
  Skip This Version), plus Check for Updates and Check automatically in Settings.
- Daily update check against PyPI (can be turned off), shown as a tray menu item, never a pop-up.
- Windows end-to-end CI job: installs the wheel, checks the Start menu shortcut and terminal
  command, downloads the model, holds Ctrl + Alt and checks the pill appears, then uninstalls.
- Meeting notes: a Notes tab that records your microphone as Me and system audio as Others,
  transcribes both locally while the call runs, and tidies the transcript on Stop. Notes list
  with search, rename, copy and select-to-delete; a one-time consent reminder with a line to
  paste in the meeting chat; a Notes state on the pill; New Note and Stop Notes in the tray.
  Audio is never saved. macOS 14.6+ or Windows 10/11.
- Insights tab: a workday streak (weekends never count or break it) with streak freezes (5 to
  start, one more per 10 active workdays, 10 at most, spent automatically), words today, this
  week and in all, speaking speed and estimated time saved, words by hour, the nouns and verbs
  you use most (English, approximate, can be turned off), and word milestones. Totals only,
  never sentences; Reset Insights deletes them.
- Troubleshooting: a local event log (no words, audio, keys or device names; home folder and
  user name masked), error output kept for launches outside a terminal, and Export Report in
  Settings or `local-stt diagnostics`, a plain-text report with app, system, settings, macOS
  crash summaries and recent logs, saved locally and shown in Finder or Explorer.
- History select mode: Select, then pick dictations with checkboxes (a whole day at once,
  Shift-click for a run, Select All, Cmd or Ctrl + A) and delete just those.
- First-run setup: an optional name, a live system check (computer, model fit, microphones,
  permissions, model download with real progress), Recommended or Customize, a microphone test
  and Accessibility step, and a first dictation to try. Skippable at every step; Run Setup Again
  in Settings. People updating from an earlier version are not sent through it.
- Settings > General > Accessibility (macOS): shows whether the shortcut and paste are
  allowed, with Open Settings and a tip for a switch that stayed on from an older copy.
- Reinstall in Settings > Troubleshooting and `local-stt reinstall [--clean]`: installs again
  (the newest version when one is out), resets local-stt's macOS permissions, reopens, and can
  also delete settings, history, notes, insights and logs. Models stay.
- Hardened CI: pinned actions, cargo-deny, CodeQL, zizmor, headless UI checks, gated
  Trusted Publishing release flow with provenance.
- Releases publish on merge: when main carries a version that is neither tagged nor on PyPI,
  `release.yml` runs the full CI on that commit, builds the wheels, publishes to PyPI after your
  approval, then tags the commit `v<version>` and creates the GitHub release. CI checks on every
  pull request that `Cargo.toml`, `Cargo.lock` and `pyproject.toml` carry the same version.

### Fixed

- Quitting no longer aborts with a Metal assertion: the speech engine is freed before exit.
- Words from consecutive speech segments no longer run together ("hereSo").
- A transcript of only "uh" or "um" is reported as no speech instead of pasting nothing.
- Check for Updates stays in its Checking state until the check itself answers.
- The live popover's "Formats on paste" tag starts correct when the output is not English.
- History rewrites are flushed to disk before they replace the file, and keep lines they cannot read.
- The demo shows a sample machine on the Model tab and holds each state while it is captured.
- Windows: launching from the Start menu or at login no longer leaves a console window open.
- Started from a terminal (`uvx local-stt` or `local-stt`), the app installs or refreshes its
  standalone copy and opens that instead: macOS asks for permissions as local-stt, not as the
  terminal, and closing the terminal no longer quits it. Opening it again shows its window.
- Windows: background tools (download, update check, Start at login, install) no longer open console windows.
- The pill shows on the screen you are typing on (the focused window's screen, or the pointer's
  when no window has focus), keeping its spot in proportion when it was placed on another screen.
- Install and Restart starts the new version reliably: it waits for the old copy to quit instead of handing back to it.
- macOS: the Accessibility prompt shows once per build instead of at every start, and the
  shortcut starts working once Accessibility is allowed, without restarting the app.
- The main window reads the app's state off the window thread, so listing microphones and
  checking start at login (reg.exe on Windows) never hold up the window.
- Smart formatting no longer crashes on list items holding characters such as "€" or "ë"
  ("5€ is the budget", "Zoë books the room"). Markers with nothing after them ("point one,
  point two") stay as spoken, and a period after a marker ("Point one. Buy milk.") no longer
  pastes a stray dot.
- Windows exe file properties take their version from `Cargo.toml`, like `local-stt --version`.

### Security

- Clipboard writes are marked private (skipped by clipboard managers, Windows clipboard
  history and cloud clipboard).
- Models are re-verified against their pinned checksum when first loaded.
- Each window can call only the commands it needs.
- History is capped at 2,000 entries and deleted when history is turned off.
- README screenshots are captured window by window (never a screen region), land in a
  gitignored review folder, and are stripped of PNG metadata; a check rejects metadata chunks.
