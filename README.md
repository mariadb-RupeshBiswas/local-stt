<div align="center">

# local-stt

**Free, private, offline speech-to-text for macOS and Windows.**
Hold a shortcut, talk, let go: your words are typed into any app.
A small Whisper model runs on your own laptop. No cloud, no account, no subscription. Free forever.

[![CI](https://github.com/mariadb-RupeshBiswas/local-stt/actions/workflows/ci.yml/badge.svg)](https://github.com/mariadb-RupeshBiswas/local-stt/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows-lightgrey)
![Offline](https://img.shields.io/badge/runs-100%25%20offline-brightgreen)

</div>

```bash
uvx local-stt
```

---

## Why local-stt

- **Free forever.** MIT licensed open source. No trial, no paid tier, no usage limits.
- **Private by design.** Your voice is turned into text on your own computer. Audio never leaves your device and is never written to disk.
- **Works offline.** After a one-time model download, no internet connection is needed.
- **Push-to-talk voice typing everywhere.** Mail, Slack, docs, your code editor, the browser: if it has a text cursor, local-stt can type into it.
- **Speaks your language.** Dictate in English or Hindi (and the other languages Whisper knows). By default everything comes out as English text.
- **Small and fast.** The default model is 181 MB. On an Apple M4 Pro a short sentence comes back in about half a second.
- **No telemetry.** No analytics, no crash reporting, no update pings, no account.

## How it works

1. Hold **Fn + Shift** on a Mac, or **Ctrl + Alt** on Windows. Change it any time in Settings.
2. Speak. A small glass recording pill shows a red dot and your live microphone level.
3. Let go. Your words are transcribed on your laptop, copied to the clipboard and pasted where your cursor is.

Every dictation is kept in a local history you can search and copy from, or you can switch history off.

## Install

You need [uv](https://docs.astral.sh/uv/getting-started/installation/), the fast Python package manager. You do not need Python, Rust or anything else.

```bash
uvx local-stt                # try it right now
uv tool install local-stt    # keep it installed
local-stt --autostart on     # start it when you log in (after uv tool install)
```

On first start local-stt downloads the default speech model (181 MB, checked against a pinned SHA-256 checksum) and puts a microphone icon in your menu bar or system tray.

### First run on macOS

macOS asks for two permissions. Both are needed and both stay on your Mac:

- **Microphone**: to hear you while you hold the shortcut.
- **Accessibility**: to notice the shortcut and to paste the text for you.

If the emoji picker opens when you press Fn, set **System Settings > Keyboard > Press fn key to** "Do Nothing".

## Settings

Open the app from the menu bar or tray icon.

| Setting | What it does | Default |
|---|---|---|
| Shortcut | Any combo with at least one modifier, for example Fn + Shift or Ctrl + Alt + Space | Fn + Shift (Mac), Ctrl + Alt (Windows) |
| Mode | Hold to talk, or press once to start and again to stop | Hold to talk |
| Output language | English (translate) or keep the language you spoke | English |
| Spoken language | Auto-detect or a fixed language | Auto-detect |
| Paste | Paste into the focused app, or only copy to the clipboard | Paste |
| Restore clipboard | Put your previous clipboard back after pasting | Off |
| Microphone | System default or a specific input | System default |
| Recording style | Pill, Waveform or Minimal | Pill |
| Sounds | Short start and stop sounds | On |
| History | Keep a local history of dictations | On |
| Start at login | Launch quietly when you log in | Off |

The recording pill can be dragged anywhere, on any monitor. It snaps to corners and edges and remembers where you put it.

## Models

local-stt checks your computer (memory, processor, graphics, free disk) and only offers models that will run well on it. The best fit is marked **Recommended**; models that will not run well are greyed out with the reason.

| Model | Download | Good for | Needs |
|---|---|---|---|
| Whisper Small (default) | 181 MB | Everyday dictation on any modern laptop | 4 GB RAM |
| Whisper Medium | 514 MB | Better accuracy, accents, noisy rooms | 8 GB RAM, Apple Silicon or 8+ CPU threads |
| Whisper Large v3 | 1.0 GB | Best accuracy | 16 GB RAM and Apple Silicon |

Run `local-stt doctor` to see your hardware and which models fit, right in the terminal.

## Privacy and security

- Speech recognition runs locally with [whisper.cpp](https://github.com/ggml-org/whisper.cpp). Audio stays in memory and is discarded after each dictation.
- The only network request the app ever makes is downloading a model from Hugging Face over HTTPS, verified against a pinned checksum before use.
- The shortcut listener only checks whether your chosen combo is held. It never records or stores any other keys.
- History and settings are plain local files readable only by your user account. History can be switched off or cleared at any time.
- Releases are built in GitHub Actions, published with PyPI Trusted Publishing and carry build provenance attestations.

See [SECURITY.md](SECURITY.md) to report a vulnerability privately.

## FAQ

**Is it really free?**
Yes. MIT licensed, no paid features, no limits, no account.

**Does it work without internet?**
Yes, after the first model download.

**Which languages can I speak?**
Any language Whisper supports, including English and Hindi. Text comes out in English by default; switch "Output language" to keep the spoken language.

**Does it work on Windows?**
Yes, Windows 10 and 11 on x64. The default shortcut is Ctrl + Alt.

**How is this different from built-in dictation?**
It runs a model you choose, entirely on your device, translates to English if you want, keeps a searchable history, and works the same way on Mac and Windows.

## Uninstall

```bash
uv tool uninstall local-stt
```

Your settings, history and models live in `~/Library/Application Support/local-stt` (macOS) or `%APPDATA%\local-stt` (Windows). Delete that folder to remove everything.

## Contributing

Bug reports, ideas and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE). Whisper models are released by OpenAI under the MIT license; whisper.cpp is MIT licensed by ggml-org.

---

<sub>Keywords: free speech to text, offline dictation, local Whisper, private voice typing, voice to text for Mac, voice to text for Windows, push to talk dictation, Hindi to English speech translation, open source Wispr Flow alternative, open source Superwhisper alternative, no cloud transcription.</sub>
