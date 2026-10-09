# Contributing to local-stt

Thanks for helping make free, private speech-to-text better.

## Ground rules

- Be kind. This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).
- Privacy is the product. Changes must not add network calls, telemetry or storage of
  audio or keystrokes.
- Security issues go through [private reporting](SECURITY.md), not public issues.

## Getting started

Requirements: Rust (the version in `rust-toolchain.toml` installs automatically through
rustup), CMake, and on macOS the Xcode Command Line Tools.

```bash
git clone --recurse-submodules https://github.com/mariadb-RupeshBiswas/local-stt
cd local-stt
cargo build
cargo test
```

Run the engine integration tests with a model on disk:

```bash
cargo run --release -- fetch-model small
LOCAL_STT_TEST_MODEL="$HOME/Library/Application Support/local-stt/models/ggml-small-q5_1.bin" \
  cargo test --release --test engine
```

Build the Python wheel the way users get it:

```bash
uvx maturin build --release
uvx --from target/wheels/*.whl local-stt
```

## Before you open a pull request

- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- New behaviour comes with a test. UI changes include before and after screenshots.
- Keep `unsafe` inside the platform modules, each block with a one-line `SAFETY:` comment.
- Add a line to `CHANGELOG.md` under Unreleased.

## Project layout

See [AGENTS.md](AGENTS.md) for the module map and conventions.
