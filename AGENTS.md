# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## What this is

local-stt: free, local push-to-talk speech-to-text for macOS and Windows. Rust + Tauri 2,
whisper.cpp compiled in, shipped as a Python wheel through maturin so `uvx local-stt` runs it.

- Design spec: `docs/superpowers/specs/2026-10-09-local-stt-design.md`
- Implementation plan: `docs/superpowers/plans/2026-10-09-local-stt.md`

## Commands

| Task | Command |
|---|---|
| Build | `cargo build` |
| Unit tests | `cargo test --lib --bins` |
| Engine tests | `LOCAL_STT_TEST_MODEL=<path to ggml-small-q5_1.bin> cargo test --release --test engine` |
| Lint | `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings` |
| Wheel | `uvx maturin build --release` then `uvx --from target/wheels/<file>.whl local-stt` |
| UI checks | `cd dev/ui-tests && npm ci --ignore-scripts && npm test` (headless Chrome against `dev/preview.html`) |
| Workflow audit | `uvx zizmor .github/workflows` |
| Hardware report | `cargo run -- doctor` |

Clone with `--recurse-submodules`; whisper.cpp lives in `vendor/whisper.cpp` pinned to a release commit.
`LOCAL_STT_DATA_DIR` overrides the app data folder, which keeps tests away from real user data.
`LOCAL_STT_NO_PERMISSION_PROMPTS=1` starts without the Accessibility prompt, and
`LOCAL_STT_EXIT_AFTER_MS=<ms>` quits the app after that long, for headless start-and-quit checks.

## Layout

| Path | Responsibility |
|---|---|
| `src/main.rs` | CLI entry: app, `doctor`, `fetch-model`, `--autostart` |
| `src/app.rs` | Tauri setup, tray, windows, worker threads |
| `src/state.rs` | Pure recording state machine (hold / toggle / cancel) |
| `src/overlay.rs` | Recording pill window and multi-monitor geometry |
| `src/commands.rs` | Commands the UI may call |
| `src/engine.rs`, `shim.cpp` | whisper.cpp through a small C ABI shim (C++ only to stop exceptions) |
| `src/recorder.rs`, `src/audio.rs`, `src/sound.rs` | Microphone capture, resampling, level, cues |
| `src/hotkey/` | Combo matching (pure) and OS keyboard hooks |
| `src/output/` | Synthetic paste keystroke |
| `src/hwprobe/` | RAM, CPU, GPU, disk |
| `src/models.rs`, `src/download.rs` | Model catalog, hardware fit, verified downloads |
| `src/config.rs`, `src/history.rs`, `src/paths.rs` | Local files |
| `ui/` | Plain HTML/CSS/JS for the pill and the main window (no bundler); everything here ships |
| `dev/` | Browser preview with a mock Tauri bus, and the UI checks; never shipped |

## Rules

- Privacy first: the only network use is the pinned model downloads, the daily PyPI version
  check (user can turn it off) and a user-started update through uv. No telemetry; never store
  audio or keystrokes.
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
