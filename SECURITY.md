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
| Microphone | Record your speech | Only while the shortcut is held (or between presses in toggle mode) |
| Keyboard events | Detect the shortcut | Always while running; only the keys currently held are kept in memory to match the shortcut, never recorded, stored or sent |
| Synthetic paste keystroke | Type the text for you | After each dictation, when Paste is on |
| Clipboard (write, marked private) | Hand over the text; clipboard managers, Windows clipboard history and cloud clipboard skip it. macOS Universal Clipboard may still share it with your own nearby devices when Handoff is on | After each dictation |
| Network | Download a model from `huggingface.co` over HTTPS | First run and when you choose another model |
| Files in the app data folder | Settings, history, models | Always; created readable only by your user |

The app sends nothing anywhere: no telemetry, no crash reports, no update checks.

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
