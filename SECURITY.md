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
| System audio (what the computer plays) | Transcribe the other side of a call as Others | Only between New Note and Stop; never written to disk |
| Microphone | Record your speech | Only while the push-to-talk shortcut is held, or between the two presses of the hands-free shortcut; and for 2 seconds when you click Test Microphone in setup (nothing kept but whether it heard you) |
| Keyboard events | Detect the shortcut | Always while running; only the keys currently held are kept in memory to match the shortcut, never recorded, stored or sent. One key is held back from the focused app: the hands-free key (Space by default), and only while its exact modifiers are down |
| Synthetic paste keystroke | Type the text for you | After each dictation, when Paste is on |
| Clipboard (write, marked private) | Hand over the text; clipboard managers, Windows clipboard history and cloud clipboard skip it. macOS Universal Clipboard may still share it with your own nearby devices when Handoff is on | After each dictation |
| Network | Download a model from `huggingface.co/ggerganov/whisper.cpp` (the whisper.cpp author's repository) over HTTPS | First run and when you choose another model |
| Network | Download the voice model for telling speakers apart from `github.com/k2-fsa/sherpa-onnx` releases over HTTPS, checked against a pinned SHA-256 (Mac with Apple silicon) | Once, the first time you start a note |
| Your web browser | Open a model's Hugging Face page or the release notes; the app builds the address, the page never passes one | Only when you click View on Hugging Face or Release Notes |
| Network | Ask `pypi.org` for the latest version number (nothing else is sent) | 30 s after start, then once a day; Settings > Check automatically turns it off. Also when you click Check for Updates or Reinstall |
| Running uv | Install a newer release you chose to install | Only when you click Install and Restart |
| Files in the app data folder | Settings (including the optional name you give in setup, never logged or put in a report), history, models | Always; created readable only by your user |
| Install locations | `~/Applications/local-stt.app` and `~/.local/bin/local-stt` (macOS); `%LOCALAPPDATA%\Programs\local-stt`, a Start menu shortcut and `~\.local\bin\local-stt.cmd` (Windows) | When you install, or the first time you start it from a terminal; uninstall removes exactly these and never a file it did not create |
| Troubleshooting log | App events such as starts, errors, timings and paste results in `logs/` inside the app data folder, never dictated text, audio, keys, clipboard contents or device names; home folder and user name masked; two files of 512 KB at most | While running, when Troubleshooting log is on (default); off deletes it |
| Error output | This run's stderr in `logs/stderr.log`, so a native crash's reason survives a launch from Finder, the Start menu or login | Same as the log |
| macOS crash reports | Reads local-stt's own `.ips` files in `~/Library/Logs/DiagnosticReports` and keeps only the cause and crashed thread | Only when you export a report |
| Diagnostic report | A text file in `diagnostics/` inside the app data folder, the five newest kept; never sent anywhere | Only when you click Export Report or run `local-stt diagnostics` |
| Insights | `insights.json` in the app data folder: words, dictations and seconds per day, words by hour, the streak, and (with Word insights on) counts for the 200 words you use most; never sentences. Readable only by your user | After each dictation; Reset Insights deletes it, Word insights off deletes the word counts |
| Notes | One file per note in `notes/` inside the app data folder (lines appended while recording, tidied on Stop), readable only by your user; delete removes the file | From New Note until you delete it |
| Voice fingerprints | A list of numbers per line from the call, in memory only, to tell voices apart at Stop; never written to disk, never kept between notes | While a note records (Mac with Apple silicon) |
| Your voice fingerprint | `voice.json` in the app data folder: a running average of your own voice as 192 numbers and a count, never audio, never anyone else's voice; readable only by your user; discarded if the voice model changes | After each dictation once the voice model exists, when Learn my voice is on (default, Mac with Apple silicon); off deletes it |
| Permission reset (macOS) | `tccutil reset All io.github.localstt` clears local-stt's own privacy entries, never another app's, so macOS asks again | When you click Reinstall or run `local-stt reinstall`, or when an install replaces a copy macOS sees as a different app |
| Login item | `~/Library/LaunchAgents/io.github.localstt.plist` (macOS) or a `HKCU\...\Run` value (Windows) | Only when Start at login is on |

The app sends no telemetry and no crash reports, and has no account.

Started from a terminal, local-stt opens its installed copy only if that copy was made from the
same build and, on macOS, its code signature still verifies; otherwise it reinstalls first.

macOS releases are signed with local-stt's own self-signed certificate (its SHA-1 is in
`dev/macos-signing-cert.sha1`), so macOS keeps your permissions across updates. Your Mac is never
asked to trust that certificate, and Apple does not vouch for it: it proves that later releases come
from the same key, not who holds it. The private key exists only as a secret of the GitHub release
environment, and a release fails rather than ship an app without it; the publish step also stops
unless the macOS wheel it uploads is the signed one. Builds you make yourself are
signed ad-hoc on your Mac.

## Supply chain

- Every GitHub Action is pinned to a full commit SHA; Dependabot keeps pins current.
- Workflows run with read-only tokens; write scopes exist only on the jobs that need them.
- `cargo deny` checks advisories, licenses and crate sources on every pull request.
- CodeQL and zizmor scan the code and the workflows.
- The Model provenance workflow rebuilds each catalog model from OpenAI's own checkpoint (whose
  SHA-256 is in OpenAI's download URL) with whisper.cpp's converter and quantizer, and requires the
  SHA-256 pinned in `src/models.rs`; only the quantizer version number in the header may differ.
  Models come only from that list, never from a URL a user types.
- Release wheels are published with PyPI Trusted Publishing (no stored tokens) and carry
  build provenance attestations.
- whisper.cpp is vendored as a git submodule pinned to a release commit.
- sherpa-onnx's prebuilt static library (with ONNX Runtime) is fetched by `build.rs` from its GitHub release and checked against a pinned SHA-256 before it is linked.
- Models are pinned by SHA-256, verified on download and again the first time they load in each run.
- Each app window can call only the commands it needs (Tauri capabilities); the recording pill can only read state.
