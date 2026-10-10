# local-stt: design

Date: 2026-10-09. Status: draft for review.

## Goal

Free, local speech-to-text for macOS and Windows laptops, using a small Whisper model.
Hold a hotkey, speak English or Hindi, release: English text is copied to the
clipboard and pasted into the focused app. Past dictations are kept in a history.
No audio leaves the machine; no account, no API key.

Install and run with uv, like ruff or uv itself:

```
uvx local-stt                 # try it
uv tool install local-stt     # keep it, needed for start-at-login
local-stt --autostart on      # start at login (off to undo)
local-stt doctor              # print hardware specs and which models fit
```

## Decisions

| Topic | Decision |
|---|---|
| License | MIT, copyright "local-stt contributors" |
| Name | `local-stt` (free on PyPI, crates.io, GitHub on 2026-10-09) |
| Repo | `mariadb-RupeshBiswas/local-stt` |
| Platforms | macOS (Apple Silicon) built and tested first; Windows x64 from the same code, tested by the user |
| Stack | Rust + Tauri 2, shipped as a Python wheel through maturin `bindings = "bin"` |
| Speech engine | whisper.cpp v1.9.5 compiled into the binary (Metal on macOS, CPU on Windows) |
| Default model | Whisper small q5_1, 181 MB, downloaded on first run |
| Other models | Medium q5_0 (514 MB) and large-v3 q5_0 (1031 MB), offered by hardware fit |
| Hotkey | macOS: hold Fn+Shift. Windows: hold Ctrl+Alt |
| Task | Translate on: Hindi becomes English, English passes through |
| Output | Private clipboard write (arboard), then synthetic paste (Cmd+V / Ctrl+V) |
| Indicator | Floating overlay, bottom-right by default, draggable, live mic level |

Measured on an Apple M4 Pro (24 GB), file test, wall time including model load:
small q5_1 0.45 s, large-v3 q5_0 2.63 s. Both transcribed the English clip exactly
and translated the Hindi clip correctly.

## Components

One Rust module per job.

| Module | Job | Platform code |
|---|---|---|
| `hotkey` | Emit `Pressed` / `Released` for the configured combo (modifier-only or modifiers + one key); Esc cancels | macOS `CGEventTap` on flags-changed; Windows `WH_KEYBOARD_LL` hook. macOS hand-written FFI, Windows `windows-sys` |
| `recorder` | Capture default mic while held; RMS level every ~50 ms; return mono f32 samples | cpal |
| `audio` | Downmix and linear-resample to 16 kHz | none |
| `engine` | Load model once, keep it loaded, transcribe a buffer | whisper.cpp via a ~40-line C shim (`shim.cpp`) |
| `output` | Clipboard, then paste keystroke | `arboard` (private markers); `CGEventPost` FFI / `SendInput` via `windows-sys` |
| `history` | Append `{ts, text, duration_ms, model, ok}` to `history.jsonl`; read newest first | none |
| `hwprobe` | Read RAM, CPU model and thread count, architecture, GPU, free disk | macOS `sysctl` + `statvfs`; Windows `GlobalMemoryStatusEx`, `GetSystemInfo`, `GetDiskFreeSpaceExW` via `windows-sys`, GPU name via PowerShell CIM |
| `models` | Catalog of 3 models, fit rules, download with pinned SHA-256, switch active model | `curl` (ships with macOS and Windows 10+) |
| `config` | `config.json` in the app data dir: active model, overlay position | none |
| `autostart` | Start at login: LaunchAgent (macOS), Run key (Windows) | `reg` via argv on Windows |
| `state` | Pure recording state machine (hold, toggle, cancel, 300 ms tap guard, 5 min cap) | none |
| overlay window | Always-on-top, borderless, non-focusable: red dot, level bar, state text | Tauri |
| main window | Model dropdown + history list | Tauri, plain HTML/JS, no npm |
| tray | Open, Quit; no Dock / taskbar icon | Tauri |

The C shim keeps Rust away from whisper.cpp's large params struct. It exposes
`lstt_load(path)`, `lstt_transcribe(ctx, pcm, n, translate, out, out_len)` and
`lstt_free(ctx)`, built with the `cc` crate after `cmake` builds whisper.cpp
(`GGML_METAL_EMBED_LIBRARY=ON` on macOS so the binary is self-contained).

## Data flow

```
hold   -> hotkey Pressed  -> recorder.start -> overlay "Recording" + level bar
release-> hotkey Released -> recorder.stop  -> audio.to_16k -> overlay "Transcribing"
       -> engine.transcribe (model already loaded) -> output.copy + paste
       -> history.append -> overlay hides
```

- Holds under 300 ms are ignored.
- One dictation at a time; a hold during transcription is ignored.
- Audio stays in memory; nothing is written to disk.

## Models and hardware fit

Three models are always listed. Each is either supported or greyed out with the
reason. The largest supported model is marked Recommended.

| Model | Size | Needs (supported only if all true) |
|---|---|---|
| Small q5_1 | 181 MB | 4 GB RAM, 0.5 GB free disk |
| Medium q5_0 | 514 MB | 8 GB RAM, 1.5 GB free disk, Apple Silicon or at least 8 CPU threads |
| Large-v3 q5_0 | 1031 MB | 16 GB RAM, 2.5 GB free disk, Apple Silicon (GPU via Metal) |

- The thresholds are estimates. Model size plus whisper.cpp buffers measured at about
  1.8 GB for large-v3 on the M4 Pro; the rest is headroom. They are calibrated in testing.
- Large-v3 needs a GPU because CPU-only runs are expected to take several seconds per
  dictation. The Windows build is CPU-only, so large-v3 is greyed out on Windows.
- A greyed-out row says what is missing, for example:
  "Needs 16 GB RAM, this PC has 8 GB" or "Needs a GPU this app can use; Windows build runs on CPU".
- Choosing a supported model downloads it (progress shown from file size), verifies the
  SHA-256, loads it, and makes it active. Downloaded models stay on disk for quick switching.
- `local-stt doctor` prints the same specs and table in the terminal.

Pinned checksum for the default model (from Hugging Face `ggerganov/whisper.cpp`):
`ggml-small-q5_1.bin` = `ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb`.
The other two are pinned the same way at implementation time.

## Overlay

Visual design follows Apple's Human Interface Guidelines for floating controls: a small
glass capsule that sits above content, system font, system colours, quiet motion.

- Shape: capsule about 220 x 44 pt (Minimal theme: 44 pt circle), fully rounded, native
  window shadow, a 0.5 pt light inner border for edge definition on any wallpaper.
- Material: real window translucency, not a CSS imitation. macOS 26+ uses Tauri's
  `LiquidGlassRegular` effect, older macOS `HudWindow`; Windows 11 `Mica`, Windows 10
  `Acrylic`; an opaque dark fallback where none is available. Window is `transparent`
  (needs Tauri's `macos-private-api` feature).
- Type: `system-ui` (SF Pro on macOS, Segoe UI Variable on Windows), 13 pt medium,
  tabular digits for the timer so it does not jitter.
- Colour: macOS system red (#FF3B30 light, #FF453A dark) for the recording dot, system
  orange while transcribing, system green for done. Text in label colour at 85 % opacity.
- Level meter: 7 rounded bars driven by mic RMS with fast attack and slow release, so it
  reads as calm rather than twitchy. Normal speech fills about half the height.
- Motion: appears with a 180 ms scale-and-fade, state changes cross-fade in 200 ms,
  disappears with a 150 ms fade. With "reduce motion" on, everything is a plain fade.
- Position: drag the whole capsule anywhere on any monitor. Within 16 pt of a screen edge
  or corner it snaps to a 24 pt margin, so corners are easy to hit. The position is saved
  with the monitor's name and scale; if that monitor is gone at next start, the capsule
  goes to the bottom-right of the screen with the mouse pointer. First run uses that same
  default. Positions are clamped to the visible work area (not under the menu bar, Dock
  or taskbar).
- Hidden between dictations. Settings has a "Move pill" button that shows it in a
  "Drag me anywhere" state so it can be placed without recording.
- Never takes focus. Spike 0 (2026-10-09) proved it: created hidden with `focusable(false)`
  and `focused(false)`, then shown later, the frontmost app kept focus. A window that is
  visible at creation did steal focus, so the overlay is always created hidden.
- States, each with a text label so colour is never the only signal:

| State | Look | Label |
|---|---|---|
| Recording | Red dot, live level | "Recording" plus elapsed seconds |
| Transcribing | Amber dot, level frozen | "Transcribing" |
| Done | Green check, 600 ms | "Pasted" or "Copied" |
| Error | Grey dot, 3 s | Short reason, for example "No speech heard" |

- The red dot pulses slowly (once per second, well under the 3-flashes-per-second limit).
  With the OS "reduce motion" setting on, it stays solid and the level shows as a static bar.
- Esc cancels a recording; nothing is transcribed or saved.

Themes (Settings > Overlay):

| Theme | What it shows |
|---|---|
| Pill (default) | Red dot, label, horizontal level bar |
| Waveform | Red dot, label, bars scrolling the last ~2 s of mic level |
| Minimal | Red dot only, with a ring that grows with mic level |

All themes follow the OS light or dark appearance.

## Main window

Opened from the tray. Three tabs.

- **History:** time, text, copy button, newest first. Search box. Clear history button.
- **Model:** dropdown of the three models with Recommended / Downloaded / Not supported
  labels, the reason for each greyed-out row, and the detected hardware.
- **Settings:** everything below, saved to `config.json` on change.

| Setting | Options | Default |
|---|---|---|
| Hotkey | Click the field, press the combo. Modifier-only (Fn+Shift) or modifiers plus one key (Ctrl+Alt+Space). Reset button | macOS Fn+Shift, Windows Ctrl+Alt |
| Mode | Hold to talk, or press once to start and again to stop | Hold to talk |
| Output language | English (translate) or keep the spoken language | English |
| Spoken language | Auto-detect or a fixed language from Whisper's list | Auto-detect |
| Paste | Paste into the focused app, or copy only | Paste |
| Restore clipboard | Put the previous clipboard back after pasting | Off |
| Microphone | System default or a specific input device | System default |
| Overlay theme | Pill, Waveform, Minimal | Pill |
| Show overlay | On or off | On |
| Reset overlay position | Button | - |
| Sounds | Short start and stop sounds (non-visual cue) | On |
| Save history | On or off; off keeps nothing on disk | On |
| Start at login | On or off | Off |

Hotkey rules: at least one modifier, at most one non-modifier key, and the combo is
checked against a short list of system shortcuts (for example Cmd+Q, Cmd+Tab, Alt+F4,
Ctrl+Alt+Del) and refused if it matches.

## Packaging and start at login

- maturin builds a wheel per platform (`macosx_11_0_arm64`, `win_amd64`) with the binary
  as a script, so `uvx local-stt` needs no Rust and no Python packages.
- macOS: an `Info.plist` with the microphone usage text is embedded in the binary with
  `-sectcreate __TEXT __info_plist`, so the mic prompt works outside a `.app` bundle.
- `--autostart on` writes a LaunchAgent plist on macOS or a `HKCU\...\Run` value on Windows
  (a few lines of code, no plugin). It points at the binary path, so it needs `uv tool install`, not `uvx`,
  whose cache path can change.
- Where wheels are published (PyPI or GitHub releases) is decided before the first release.

## Permissions (macOS)

- Microphone: asked on first recording.
- Accessibility (and possibly Input Monitoring): for the Fn+Shift tap and the paste. The
  app opens the right Settings pane if the tap cannot be created.
- Each new version is a new binary, so macOS may ask again after upgrades (inferred).
- If the Fn key is set to open the emoji picker, the README says to set it to "Do Nothing".

## Dependencies

Crates from named organizations, last release within a year (checked 2026-10-09):

| Crate | Publisher | Latest release |
|---|---|---|
| `tauri` 2.12, `tauri-build` | Tauri org | 2026-10-09 |
| `arboard` 3.6 | 1Password | 2025-08-23, a deliberate exception to the one-year rule: it was already in the build through Tauri's clipboard plugin, and it is the only option that marks clipboard writes private |
| `cpal` 0.18 | RustAudio org | 2026-08-16 |
| `serde`, `serde_json` | serde-rs org | 2026-07-20 |
| `windows-sys` 0.61 (Windows only) | Microsoft | 2025-10-06, kept on purpose: Microsoft's generated bindings are safer than hand-written FFI |
| `cmake` 0.1, `cc` 1.6 (build only) | rust-lang org | 2026-10-08, 2026-10-03 |
| maturin 1.15 (build only) | PyO3 org | 2026-08-24 |

Not used, because the last release is older than a year or the publisher is a person:
`core-graphics` (2025-05), `whisper-rs`, `sysinfo`,
`reqwest`, `hound`. Their small parts are replaced with FFI, `curl`, or a few lines of code.

whisper.cpp is vendored as a git submodule pinned to commit
`d1be6fde11ac6e0407606b4e42fe72d34add8037` (tag `v1.9.5`, ggml-org, 2026-10-06).

## Security

The repo is public and anyone can fork it, so nothing secret lives in it and the
release path cannot be hijacked by a pull request.

App:

- No network except the model download. Hardcoded HTTPS URL per model,
  `curl --fail --proto =https --tlsv1.2`, write to a temp file, verify SHA-256, then rename.
  Mismatch deletes the file. Arguments go to `curl` as an argv list, never through a shell.
- The hotkey hook compares key events against the configured combo and keeps only a
  "combo held" flag. It never records, stores or logs any other key.
- Tauri: CSP set explicitly (`default-src 'self'`, no inline script or style); `withGlobalTauri` on because there is no bundler, which is safe only because the CSP admits no script the app did not ship; devtools off in
  release builds; no shell, fs or http plugins. Capabilities grant only events,
  window dragging and clipboard write. Frontend commands take a model id from a fixed list, never a
  path or URL. The history command takes no arguments.
- `unsafe` lives only in the FFI modules, each call wrapped in a safe function with a one-line
  `SAFETY:` comment. `#![deny(unsafe_op_in_unsafe_fn)]`.
- History and config files are created owner-only (0600 on macOS). The README says history
  is plain text on disk.
- No telemetry, no crash reporting, no update check.

Repository and CI:

- Workflows default to `permissions: contents: read`; write scopes only on the job that needs them.
- Every action pinned to a full commit SHA with the version in a comment; Dependabot keeps
  them current. Dependabot also covers `cargo` and the whisper.cpp submodule.
- No `pull_request_target`, no untrusted `${{ }}` expressions inside `run:` steps,
  `persist-credentials: false` on checkout.
- CI gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo deny`
  (advisories, licenses, sources), `zizmor` on workflows, CodeQL.
- Releases: wheels built by `PyO3/maturin-action`, uploaded by `pypa/gh-action-pypi-publish`
  with PyPI Trusted Publishing (OIDC, no stored token) and PEP 740 attestations.
  `id-token: write` only on the publish job, inside a protected `pypi` environment that
  needs a manual approval. The PyPI trusted-publisher entry is created by the owner.
- Repo settings after creation (owner approves each): branch ruleset on `main` (PR only,
  CI required), secret scanning with push protection, private vulnerability reporting.
- GitHub secret scanning with push protection blocks committed credentials; maintainers also run a
  local pre-push sweep for keys, tokens and internal names.
- App commands are scoped per window through Tauri capabilities: the recording pill can only read
  state; the main window gets exactly the commands it uses.
- Clipboard writes are marked private (skipped by clipboard managers, Windows clipboard history and
  cloud clipboard). macOS Universal Clipboard can still hand the text to the user's own nearby
  devices when Handoff is on; the README says so.
- Models are verified against the pinned SHA-256 on download and again the first time they are
  loaded in each run.
- `CLAUDE.md` and `.claude/` are gitignored.

## Error handling

- Mic unavailable, model missing, or empty text: overlay shows the error for 3 s,
  history records `ok: false`, clipboard is left unchanged.
- Download fails or checksum mismatch: file deleted, previous model stays active.
- Hotkey hook cannot be created: tray tooltip says so; macOS opens the Accessibility pane.

## Testing

- Unit: `audio` resample length, `history` append/read order, `models` fit rules for
  sample machines (8 GB Intel Windows, 16 GB Apple Silicon, 24 GB M4 Pro).
- Integration: `engine` against an English and a Hindi clip, expecting the texts measured
  above. Clips are generated at test time with the OS voice (`say` on macOS, System.Speech
  on Windows; Hindi on macOS only), so no voice audio is committed. The model is downloaded
  in CI through the app's own download code, checksum verified, and cached.
- Spike first: overlay shown while TextEdit has focus; TextEdit must keep focus.
- Manual on macOS by the user: hold, speak, release into TextEdit; paste, clipboard,
  history, overlay drag, model switch.
- Windows: same checklist, run by the user on their PC. Not verifiable from this Mac.

## Public repo files

- `README.md`: what it is in one line, feature list, install (`uvx local-stt`), first-run
  permissions, settings, models table, privacy section, FAQ, keywords people search for
  (free speech to text, offline dictation, local Whisper, private voice typing, no cloud,
  no subscription, Mac and Windows, Hindi to English). Claims stay true: free and MIT,
  runs offline after the model download, no telemetry.
- `CHANGELOG.md` (Keep a Changelog), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`
  (Contributor Covenant 2.1), `SECURITY.md` (private vulnerability reporting), `AGENTS.md`
  (build, test, layout, rules for AI coding agents), issue and PR templates.
- Search metadata: GitHub topics, PyPI keywords and classifiers in `pyproject.toml`,
  crates.io keywords and categories in `Cargo.toml`.

## Added after the first review (2026-10-10)

Requested by the owner while testing; each is implemented and tested.

| Feature | Design |
|---|---|
| Hands-free | Second shortcut (default Fn+Shift+Space / Ctrl+Alt+Space): press to start, press again to stop. The hotkey core emits `Toggle` before `Released`, so adding Space to a push-to-talk hold converts it in place. Esc cancels either. The old "mode" setting is gone. |
| Install | `local-stt install` / Settings > Install: `~/Applications/local-stt.app` (ad-hoc signed, Spotlight indexes it) and a `~/.local/bin` symlink on macOS; `%LOCALAPPDATA%\Programs\local-stt`, a Start menu shortcut and a `.cmd` shim on Windows. Uninstall removes only what it created. Start at login installs first when run from the uvx cache. |
| Smart formatting | On by default. Spoken "point one ... point two" becomes a numbered list only when the points count up from one; fillers (uh, um) and their commas are dropped; missing spaces after sentences are fixed. Pure, tested function `format::tidy`. |
| Live transcription | On by default. While recording, the last 18 s are re-transcribed about every 800 ms (one job in flight, silent tails skipped, never ahead of the final pass) and shown in a second non-focusable window above the pill (below it near the top edge). Typing live into the focused app was rejected: Whisper revises earlier words, which would mean synthetic backspacing into someone else's document. |
| Updates | Daily PyPI version check (setting, on by default; only the request is sent). Shown as a tray item and an in-app banner (Install and Restart, Release Notes, Later, Skip This Version), never a modal, following Sparkle and Rogue Amoeba guidance. Install runs `uv tool run --from local-stt==X local-stt install` with a validated version string, then restarts. |
| Demo | `local-stt demo`: scripted tour with sample history in a throwaway data folder; no hook, no network, no recording or pasting. Used for README screenshots. |
| Clean quit | The whisper context is freed on exit; ggml's Metal backend otherwise asserts at process exit (found in a real run, reproduced and fixed A/B, now a CI step). |

## Out of scope

More than three models, live streaming text, code signing and
notarization, a `.app` / `.msi` installer, Intel Macs, Windows GPU acceleration.

## Repo layout

```
local-stt/
  Cargo.toml  pyproject.toml  build.rs  shim.cpp  Info.plist
  src/        Rust modules listed above
  ui/         overlay.html, main.html (plain HTML/JS)
  vendor/whisper.cpp   git submodule at v1.9.5
  scripts/    listen.sh, transcribe.sh (earlier CLI helpers)
  docs/superpowers/specs/
  models/  samples/   gitignored
```
