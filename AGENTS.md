# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## What this is

local-stt: free, local push-to-talk speech-to-text for macOS and Windows. Rust + Tauri 2,
whisper.cpp compiled in, shipped as a Python wheel through maturin so `uvx local-stt` runs it.

- Design guide (look, words, code and security design): `docs/design-guide.md`
- Architecture tour for newcomers (threads, data flow, files): `docs/architecture.md`
- Design spec: `docs/superpowers/specs/2026-10-09-local-stt-design.md`
- Implementation plan: `docs/superpowers/plans/2026-10-09-local-stt.md`

## Commands

| Task | Command |
|---|---|
| Build | `cargo build` |
| Unit tests | `cargo test --lib --bins` |
| Engine tests | `LOCAL_STT_TEST_MODEL=<path to ggml-small-q5_1.bin> cargo test --release --test engine` |
| Summary tests | `LOCAL_STT_TEST_SUMMARY_MODEL=<path to a summary model .gguf> cargo test --release --test summary` (add `LOCAL_STT_TEST_SUMMARY_NOTE=<saved note .json>` for a real call) |
| Lint | `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings` |
| Wheel | `uvx maturin build --release` then `uvx --from target/wheels/<file>.whl local-stt` |
| UI checks | `cd dev/ui-tests && npm ci --ignore-scripts && npm test` (headless Chrome against `dev/preview.html`) |
| Workflow audit | `uvx zizmor .github/workflows` |
| Hardware report | `cargo run -- doctor` |
| Note pipeline on real audio (macOS, needs mic and System Audio permission) | `cargo run --release --example note_probe -- 20` while something plays |

Clone with `--recurse-submodules`; whisper.cpp lives in `vendor/whisper.cpp` pinned to a release commit, and
llama.cpp in `vendor/llama.cpp` at the commit whose ggml matches it (libllama builds on whisper's ggml).
`LOCAL_STT_DATA_DIR` overrides the app data folder, which keeps tests away from real user data.
`LOCAL_STT_NO_PERMISSION_PROMPTS=1` starts without the Accessibility prompt, and
`LOCAL_STT_EXIT_AFTER_MS=<ms>` quits the app after that long, for headless start-and-quit checks.
Any of these also keeps a terminal run in the terminal: a release build of `local-stt` with no
subcommand otherwise installs or refreshes the app and hands over to it (`install::hand_off`), so
macOS asks for permissions as local-stt. Debug builds (`cargo run`) never hand over. `LOCAL_STT_IN_PROCESS=1` keeps it in place on its own (debugging with
stderr in the terminal). `LOCAL_STT_REPLACES` is set by an update on the new copy; do not set it.

## Layout

| Path | Responsibility |
|---|---|
| `src/main.rs` | CLI entry: the app and its subcommands (`local-stt --help`) |
| `src/app.rs` | Tauri setup, tray, windows, worker threads |
| `src/state.rs` | Pure recording state machine (hold / toggle / cancel) |
| `src/overlay.rs` | Recording pill window and multi-monitor geometry |
| `src/commands.rs` | Commands the UI may call |
| `src/engine.rs`, `shim.cpp`, `voice_shim.c` | whisper.cpp through a small C ABI shim (C++ only to stop exceptions), plus the stderr redirect; sherpa-onnx voice fingerprints (Mac with Apple silicon) |
| `src/recorder.rs`, `src/audio.rs`, `src/sound.rs` | Microphone capture, resampling, level, cues |
| `src/hotkey/` | Combo matching (pure) and OS keyboard hooks |
| `src/output/` | Synthetic paste keystroke; Windows console detach |
| `src/summary/`, `llm_shim.cpp` | Note summaries: model catalog and hardware fit, the part-by-part writer with its grammar, background download and run |
| `src/notes/` | Meeting notes: store (`mod.rs`), segmenter, tidy pass, recording session, voices (telling speakers apart), call names (`callnames/`: Zoom's participant list per OS), echo cancellation (`echo.rs`) |
| `src/insights/`, `src/localtime/` | Insights: workday streak and freezes (pure), word counts and part-of-speech tagger, `insights.json`; local date and hour from the OS |
| `src/focus/` | The focused window's centre, so the pill shows on that screen |
| `src/diag.rs` | Troubleshooting log, panic hook, stderr capture, diagnostic report |
| `src/hwprobe/` | RAM, CPU, GPU, disk |
| `src/models.rs`, `src/download.rs` | Model catalog, hardware fit, verified downloads |
| `src/config.rs`, `src/history.rs`, `src/paths.rs` | Local files |
| `src/format.rs`, `src/clipboard.rs` | Smart formatting of transcripts; private clipboard writes |
| `src/install.rs`, `src/autostart.rs`, `src/update.rs` | Install, hand-off and reinstall; start at login; update check and install |
| `src/instance.rs`, `src/proc.rs`, `src/demo.rs` | One running copy, console-free child processes, the scripted demo |
| `ui/` | Plain HTML/CSS/JS for the pill and the main window (no bundler); everything here ships |
| `dev/` | Browser preview with a mock Tauri bus, and the UI checks; never shipped |

## Design principles

- **Standard Cargo layout** ([Cargo guide](https://doc.rust-lang.org/cargo/guide/project-layout.html)):
  the app is a library (`src/lib.rs`) with a thin binary (`src/main.rs`, CLI only), so tests reach
  every module; integration tests live in `tests/`, examples in `examples/`, native builds in `build.rs`.
- **One Cargo project at the root**, not Tauri's usual `src-tauri/`
  ([Tauri layout](https://v2.tauri.app/start/project-structure/)): there is no JavaScript build
  (`ui/` is static files) and maturin builds the wheel from the root crate.
- **Pure core, thin OS edges.** Decisions are plain functions with unit tests (`state.rs`,
  `format.rs`, hotkey matching, overlay geometry, the notes segmenter and tidy pass). OS calls sit
  in `macos.rs` / `windows.rs` files or behind a small trait (`notes::session::Feed`).
- **Feature folders own their files** (`hotkey/`, `hwprobe/`, `output/`, `notes/`) and expose a
  few functions; a new feature gets its own module, not more lines in `app.rs`.
- **Least privilege by construction:** every command is listed in `build.rs` and granted per
  window in `capabilities/`; child processes go through `proc::command`.
- **Known debt:** `app.rs` is the integration hub (tray, windows, controller, worker). Split it
  into `app/{tray,windows,worker}.rs` the next time it grows, in a change of its own.

## Rules

- Privacy first: the only network use is the pinned model downloads, the daily PyPI version
  check (user can turn it off) and a user-started update through uv. No telemetry; never store
  audio or keystrokes.
- `diag::log` takes events, never content: no dictated text, audio, keys, clipboard or device
  names. Report sections are allowlists, and the whole report is masked before it is saved.
- `unsafe` only in `src/engine.rs` and the `macos.rs` / `windows.rs` platform files, each
  block with a one-line `// SAFETY:` comment.
- Dependencies: crates from named organizations with a release in the last year. Ask before
  adding one; prefer a few lines of code.
- UI: no inline scripts or `style` attributes (strict CSP); render user text with
  `textContent`, never `innerHTML`.
- Missing values show as `-`, never `0`.
- Comments are one line and explain why.
- New behaviour comes with a test. Run lint and tests before every commit.
- Workflows: actions pinned to full SHAs, least-privilege permissions, no untrusted
  `${{ }}` inside `run:`; keep `uvx zizmor .github/workflows` clean.
- Update `CHANGELOG.md` under Unreleased.
