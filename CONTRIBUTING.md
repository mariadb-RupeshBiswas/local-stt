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
cargo run      # a debug build runs right here in the terminal
```

A release build started from a terminal installs itself in Applications (or the Start menu)
and hands over to that copy, as users get it. Set `LOCAL_STT_IN_PROCESS=1` to keep a release
build in the terminal, for example to watch its error output.

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

- `cd dev/ui-tests && npm ci --ignore-scripts && npm test` when you touch `ui/`
- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- New behaviour comes with a test. UI changes include before and after screenshots.
- Keep `unsafe` inside the platform modules, each block with a one-line `SAFETY:` comment.
- Add a line to `CHANGELOG.md` under Unreleased.

## Releasing

1. In a pull request, set the same new plain `X.Y.Z` version in `Cargo.toml` and
   `pyproject.toml`, refresh `Cargo.lock` with `cargo check`, and move the Unreleased notes
   under that version. CI's "Version files agree" check (`dev/check-version.sh`) catches a mismatch.
2. Merge it. `release.yml` publishes only a version that is neither tagged `v<version>` nor on
   PyPI: it runs the full CI on that commit while it builds the macOS and Windows wheels,
   publishes to PyPI (environment `pypi`) only after both pass, then tags the commit and creates
   the GitHub release with the wheels attached.

macOS wheels are signed in the `sign-macos` job by `dev/sign-macos-wheel.sh`, with the certificate
in the `pypi` environment secrets `MACOS_SIGNING_P12` (base64) and `MACOS_SIGNING_P12_PASSWORD`;
`dev/macos-signing-cert.sha1` names it, and the job fails rather than publish an ad-hoc app. A new
key means a new SHA-1 in that file, and users allow permissions once more. Pull requests run the
same script with a throwaway certificate. Builds you make locally stay ad-hoc: started from a
terminal, a release build replaces an installed release copy, so macOS asks for permissions again.

If the GitHub release step fails after PyPI has the files, re-run only that failed job. A later
merge stops with an error rather than build different wheels for a version PyPI already has.
A tag rule that restricts creating `v*` tags must let GitHub Actions through, or every release
fails at its last step.

## Project layout and design

See [AGENTS.md](AGENTS.md) for the module map and conventions, and the
[design guide](docs/design-guide.md) for how the app should look, read and be built.
