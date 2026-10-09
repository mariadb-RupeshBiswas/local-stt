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
| Repo | `mariadb-RupeshBiswas/local-stt`, work identity, standard Claude co-author trailer |
| Platforms | macOS (Apple Silicon) built and tested first; Windows x64 from the same code, tested by the user |
| Stack | Rust + Tauri 2, shipped as a Python wheel through maturin `bindings = "bin"` |
| Speech engine | whisper.cpp v1.9.5 compiled into the binary (Metal on macOS, CPU on Windows) |
| Default model | Whisper small q5_1, 181 MB, downloaded on first run |
| Other models | Medium q5_0 (514 MB) and large-v3 q5_0 (1031 MB), offered by hardware fit |
| Hotkey | macOS: hold Fn+Shift. Windows: hold Ctrl+Alt |
| Task | Translate on: Hindi becomes English, English passes through |
| Output | Clipboard, then synthetic paste (Cmd+V / Ctrl+V) |
| Indicator | Floating overlay, bottom-right by default, draggable, live mic level |

Measured on the user's M4 Pro (24 GB), file test, wall time including model load:
small q5_1 0.45 s, large-v3 q5_0 2.63 s. Both transcribed the English clip exactly
and translated the Hindi clip correctly.

## Components

One Rust module per job.

| Module | Job | Platform code |
|---|---|---|
| `hotkey` | Emit `Pressed` / `Released` for the held combo | macOS `CGEventTap` on flags-changed; Windows `WH_KEYBOARD_LL` hook. macOS hand-written FFI, Windows `windows-sys` |
| `recorder` | Capture default mic while held; RMS level every ~50 ms; return mono f32 samples | cpal |
| `audio` | Downmix and linear-resample to 16 kHz | none |
| `engine` | Load model once, keep it loaded, transcribe a buffer | whisper.cpp via a ~40-line C shim (`shim.c`) |
| `output` | Clipboard, then paste keystroke | Tauri clipboard plugin; `CGEventPost` FFI / `SendInput` via `windows-sys` |
| `history` | Append `{ts, text, duration_ms, model, ok}` to `history.jsonl`; read newest first | none |
| `hwprobe` | Read RAM, CPU model and thread count, architecture, GPU, free disk | macOS `sysctl` + `statvfs`; Windows `GlobalMemoryStatusEx`, `GetSystemInfo`, `GetDiskFreeSpaceExW` via `windows-sys`, GPU name via PowerShell CIM |
| `models` | Catalog of 3 models, fit rules, download with pinned SHA-256, switch active model | `curl` (ships with macOS and Windows 10+) |
| `config` | `config.json` in the app data dir: active model, overlay position | none |
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

- About 180 x 44 px; default bottom-right of the main screen, 24 px from the edges.
- Red dot, live level bar, state label (Recording, Transcribing, or an error for 3 s).
- Drag anywhere to move; position saved in `config.json`.
- Must never take focus, or the paste lands in the overlay. Main technical risk; tested first.

## Main window

- Opened from the tray. Model dropdown on top, history list below (time, text, copy button).
- The dropdown shows all three models with Recommended / Downloaded / Not supported labels.

## Packaging and start at login

- maturin builds a wheel per platform (`macosx_11_0_arm64`, `win_amd64`) with the binary
  as a script, so `uvx local-stt` needs no Rust and no Python packages.
- macOS: an `Info.plist` with the microphone usage text is embedded in the binary with
  `-sectcreate __TEXT __info_plist`, so the mic prompt works outside a `.app` bundle.
- `--autostart on` uses the Tauri autostart plugin (LaunchAgent on macOS, Run key on
  Windows). It points at the binary path, so it needs `uv tool install`, not `uvx`,
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
| `tauri-plugin-clipboard-manager` 2.4 | Tauri org | 2026-10-01 |
| `tauri-plugin-autostart` 2.7 | Tauri org | 2026-10-01 |
| `cpal` 0.18 | RustAudio org | 2026-08-16 |
| `serde`, `serde_json` | serde-rs org | 2026-07-20 |
| `windows-sys` 0.61 (Windows only) | Microsoft | 2025-10-06, kept by owner decision: generated bindings beat hand-written FFI |
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
- The hotkey hook reads modifier state only. It never records, stores or logs other keys.
- Tauri: CSP set explicitly to `default-src 'self'`; `withGlobalTauri` off; devtools off in
  release builds; no shell, fs or http plugins. Capabilities grant only clipboard write,
  autostart, window and tray. Frontend commands take a model id from a fixed list, never a
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
- A pre-push sweep script fails on private keys, tokens and internal names.
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

## Out of scope

Custom hotkeys, more than three models, live streaming text, code signing and
notarization, a `.app` / `.msi` installer, Intel Macs, Windows GPU acceleration.

## Repo layout

```
local-stt/
  Cargo.toml  pyproject.toml  build.rs  shim.c  Info.plist
  src/        Rust modules listed above
  ui/         overlay.html, main.html (plain HTML/JS)
  vendor/whisper.cpp   git submodule at v1.9.5
  scripts/    listen.sh, transcribe.sh (earlier CLI helpers)
  docs/superpowers/specs/
  models/  samples/   gitignored
```
