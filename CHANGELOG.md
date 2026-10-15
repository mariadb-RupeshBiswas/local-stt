# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.1] - 2026-10-15

### Added

- Notes also name a voice from the conversation: when someone says "Hello, Gaurav." or "What do
  you think, Gaurav?" and the next voice answers, or a voice introduces itself ("this is Priya"),
  that voice gets the name after two such cues. Only names the call app listed count, so someone
  merely talked about is never assigned. This fills in names in Zoom's gallery view and full
  screen, where Zoom shows no talker.

## [0.11.0] - 2026-10-15

### Added

- Notes name each speaker from Zoom: local-stt reads the names on Zoom's video tiles (no
  Participants panel needed) and, every second, who Zoom shows talking in Speaker view or in the
  mini window of a minimized meeting. At Stop each voice takes the name Zoom showed while it
  talked, when one name clearly wins. A full-screen meeting behind another app cannot be read, so
  a one-time hint asks for a window instead.
- On a Mac the same naming works for Google Meet (Chrome, Edge, Brave, Arc), Microsoft Teams and
  Slack huddles, read from the page each app shows. They rely on the class names those apps ship
  today and are not yet tested in a real call; please open an issue if one stops naming speakers.
- Call detection: when Zoom, Microsoft Teams, Slack, Webex or FaceTime (or a browser window
  showing Google Meet or Microsoft Teams) starts using the microphone and no note is recording, a
  small toast at the bottom of the screen offers **Start note**. One toast per call, hidden after
  20 s unless the pointer rests on it. Only app names are read and logged; nothing is recorded
  until you click Start note. Settings > Notes > Detect calls (on by default) turns it off.

- THIRD_PARTY_NOTICES.md lists every bundled component's licence and ships in the wheel next to
  LICENSE.

### Changed

- Summaries use at most 4 CPU threads: the model runs on the GPU, and more threads only heated the
  computer without finishing sooner.

## [0.10.0] - 2026-10-14

### Added

- Echo cancellation in notes: the call's sound is taken out of your microphone before it is
  transcribed, so laptop speakers no longer repeat the other side's words under your name. It is
  WebRTC's AEC3 through the pure-Rust `sonora` crate, with the call audio as the reference.
  Settings > Notes > Echo cancellation turns it off.

## [0.9.0] - 2026-10-14

### Added

- Notes can be summarized on this computer: an overview, what each person said, and the action
  items with their owner and the moment they were said. The Model tab gains a Summary model
  section with two optional models and recommends one for this computer: Granite 4.0 Micro
  (IBM, 2.1 GB) or Gemma 4 E4B (Google, 5.2 GB, recommended with 16 GB of memory and a GPU it
  can use). Each is downloaded from its maker's Hugging Face page and checked against a pinned
  SHA-256. Summaries are off by default: a Summaries switch in the Model tab turns them on, and
  downloading a model does too.
- Settings > Appearance: System, Light or Dark for the app's window, shown as small window
  pictures like the macOS picker. System follows the OS, as before.
- Windows: notes tell the call's voices apart and Learn my voice works, as on the Mac. The voice
  library is sherpa-onnx's Windows build, pinned by checksum like the Mac one.
- Notes read who is in a Zoom call from Zoom's participants panel (macOS through Accessibility,
  Windows through UI Automation). Zoom does not tell other apps who is speaking, so a one-to-one
  call names the other voice on its own; in a group call the names are offered as one-click
  choices for each speaker. Names stay in memory until Stop and only the other people's names are
  kept in the note. `local-stt call-names` shows what is read, for checking.

## [0.8.0] - 2026-10-14

### Added

- Learn my voice (Mac with Apple silicon, on by default, Settings > General): your own voice
  fingerprint is learned from your dictations and kept in `voice.json` on this computer, readable
  only by you. Notes use it to tell a person in the room from you, giving them a Speaker label
  instead of Me. Only numbers are kept, never audio, and never anyone else's voice. Off deletes
  the file.

### Fixed

- Notes split one call into dozens of speakers (Speaker 3 to Speaker 51 for three people), because
  every short "yeah" or "okay" got a voice of its own. Speakers now come from lines of 3 seconds
  or more; shorter lines join the closest voice, and a voice heard in only one line joins the
  closest speaker.
- Notes recorded on laptop speakers repeated the other side's words under your name. Echo is now
  cut out of your lines even when it sits next to your own words or spans two of their lines.

## [0.7.1] - 2026-10-13

### Fixed

- Notes kept only your microphone once Bluetooth headphones (AirPods) switched to call mode,
  because the change of sample rate ended the call audio stream. A stream the device ends is
  now opened again within a second and the note carries on.

### Changed

- Notes show the name you gave in setup instead of "Me".

## [0.7.0] - 2026-10-12

### Added

- Notes (Mac with Apple silicon): the call's voices are told apart. At Stop, Others becomes
  Speaker 1, Speaker 2 and so on, which you can name. A voice fingerprint per line stays in
  memory until Stop and is never saved; the 28 MB voice model downloads once, checked against
  a pinned SHA-256.

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
