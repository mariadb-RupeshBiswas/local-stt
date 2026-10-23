<div align="center">

# local-stt

**Free, private, offline speech-to-text for macOS and Windows.**
Hold a shortcut, talk, let go: your words are typed into any app.
Take meeting notes too: Zoom, Meet, Teams and Slack calls become a transcript with speaker names and a summary.
A small Whisper model runs on your own laptop. No cloud, no account, no subscription. Free forever.
A free, local, open-source alternative to Wispr Flow and Superwhisper.

**[Website](https://mariadb-rupeshbiswas.github.io/local-stt/)** | **[PyPI](https://pypi.org/project/local-stt/)**

[![CI](https://github.com/mariadb-RupeshBiswas/local-stt/actions/workflows/ci.yml/badge.svg)](https://github.com/mariadb-RupeshBiswas/local-stt/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/local-stt)](https://pypi.org/project/local-stt/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/LICENSE)
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
- **Works offline.** After a one-time model download, dictation never needs the internet.
- **Push-to-talk voice typing everywhere.** Mail, Slack, docs, your code editor, the browser: if it has a text cursor, local-stt can type into it.
- **Speaks your language.** Dictate in English or Hindi (and the other languages Whisper knows). By default everything comes out as English text.
- **Small and fast.** The default model is 181 MB. On an Apple M4 Pro a short sentence comes back in about half a second.
- **Private meeting notes.** No bot joins the call. Each voice is told apart and named from the call app, and an optional local model writes the summary and action items. See [Meeting notes](#meeting-notes).
- **Hands-free when you want it.** Hold to talk, or press a second shortcut to keep listening while you think out loud.
- **Live preview, tidy result.** See your words as you speak; Smart formatting turns "point one, point two" (or "number one", "first, second") into a numbered list.
- **No telemetry.** No analytics, no crash reporting, no account. The only optional ping is a once-a-day version check you can switch off.

## How it works

![Recording pill: recording with the live level, transcribing, pasted](https://raw.githubusercontent.com/mariadb-RupeshBiswas/local-stt/main/docs/site/pill-light.png)

1. Hold **Fn + Shift** on a Mac, or **Ctrl + Alt** on Windows. Change it any time in Settings.
2. Speak. A small glass recording pill shows a red dot and your live microphone level, with a live preview of your words above it.
3. Let go. Your words are transcribed on your laptop, tidied, copied to the clipboard and pasted where your cursor is.

**Hands-free:** press **Fn + Shift + Space** (Windows: **Ctrl + Alt + Space**) to start listening, and press it again to stop. Already holding push-to-talk? Add Space to switch to hands-free without stopping. **Esc** cancels either way. A single recording stops and pastes after 5 minutes.

Every dictation is kept in a local history you can search and copy from. Click Select to tick the ones you no longer need (one, a whole day, or all of them) and delete them, or switch history off.

**Under the hood:** a Rust app built with Tauri. A keyboard hook spots your shortcut, the microphone is recorded in memory, Whisper (whisper.cpp) turns it into text on your computer, simple rules tidy it, and the app pastes it. The full tour, written for newcomers, is in [docs/architecture.md](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/docs/architecture.md).

## Get started

### Requirements

| | macOS | Windows |
|---|---|---|
| Computer | Apple Silicon (M1 or later), macOS 11 or later | 64-bit PC, Windows 10 or 11 |
| Memory | 4 GB for Whisper Small (more for larger models) | 4 GB for Whisper Small (more for larger models) |
| Disk | About 200 MB for the app and Whisper Small | About 200 MB for the app and Whisper Small |
| Graphics | Built-in GPU, always used | Any GPU with a Vulkan driver (Intel, AMD, NVIDIA) is used when present; otherwise the CPU |
| Internet | Once, to download the speech model | Once, to download the speech model |
| Installer | [uv](https://docs.astral.sh/uv/getting-started/installation/) | [uv](https://docs.astral.sh/uv/getting-started/installation/) |

> [!IMPORTANT]
> **uv is the only thing you install yourself.** You do not need Python: if your computer has none, uv downloads its own copy the first time. Everything else, the speech model and the microphone and Accessibility permissions, is set up inside the app on first run.

### macOS

Open **Terminal** and run:

```bash
brew install uv        # once; without Homebrew, see the uv link above
uvx local-stt          # installs local-stt in Applications and opens it
```

### Windows

Open **PowerShell** and run:

```powershell
winget install --id=astral-sh.uv -e   # once
# close PowerShell and open a new one, so the uv command is found
uvx local-stt                          # installs local-stt in the Start menu and opens it
```

If it does not start on Windows:

- **"VCRUNTIME140.dll" or "MSVCP140.dll" was not found:** install Microsoft's Visual C++ runtime with `winget install Microsoft.VCRedist.2015+.x64`, then run `uvx local-stt` again.
- **Behind a company proxy:** set `HTTPS_PROXY` before running `uvx`. If your company inspects HTTPS traffic, also set `UV_SYSTEM_CERTS=true` so uv trusts the company certificate.
- **"This processor lacks AVX2":** the speech engine needs a CPU from about 2013 or later (Intel Haswell, AMD Excavator or newer).
- **Windows on Arm:** uv runs the x64 build under emulation, which needs Windows 11 24H2 with updates from December 2025 or later (for AVX2).

### Linux

> [!WARNING]
> **Linux is not supported yet.** The shortcut and the paste need code for each desktop system, and that code exists only for macOS and Windows today.

### More commands

```bash
uvx local-stt install        # install without opening
local-stt --autostart on     # start it when you log in
uvx local-stt demo           # a 30-second tour with sample data; nothing is recorded or pasted
local-stt --help             # every command
```

Started from a terminal, local-stt runs as its own app, not as part of the terminal: you can close the terminal, and macOS asks for permissions in local-stt's name. On first start it downloads the default speech model (181 MB, checked against a pinned SHA-256 checksum) and puts a microphone icon in your menu bar or system tray. **Settings > General > Install** does the same as `install`, and turning on **Start at login** installs it first if needed.

### First run

A short setup opens the first time, about a minute:

1. **Welcome**: what should local-stt call you (optional, stays on your computer).
2. **System check**: your computer, the speech model that fits it, microphones, permissions, and the model download, each ticked off as it really finishes.
3. **Recommended or Customize**: the recommended settings work for most people; Customize shows the shortcut, language, paste, smart formatting, history and start at login.
4. **Permissions**: a microphone test and, on macOS, Accessibility.
5. **Try it**: hold the shortcut and watch your words appear.

Every step can be skipped, and Settings > General > **Run Setup Again** brings it back.

### Permissions on macOS

macOS asks for two permissions, in local-stt's name. Both are needed and both stay on your Mac:

- **Microphone**: to hear you while you hold the shortcut.
- **Accessibility**: to notice the shortcut and to paste the text for you.

If the emoji picker opens when you press Fn, set **System Settings > Keyboard > Press fn key to** "Do Nothing".

## Meeting notes

Open the **Notes** tab (or **New Note** in the menu bar or tray) when a call starts. local-stt records your microphone as **Me** and what your computer plays as **Others**, and turns both into a transcript on your computer while the call runs. Click **Stop** and the note is tidied: "uh" and "um" go, spacing is fixed, spoken lists are numbered, and back-to-back lines from one side are joined.

- Works with Zoom, Google Meet, Teams, Slack, or any app that plays sound. No bot joins the call.
- Audio is never saved. Each few seconds of speech is transcribed and then dropped.
- The call's voices are told apart: when you click Stop, Others becomes **Speaker 1**, **Speaker 2** and so on. Each line from the call leaves a voice fingerprint (numbers, not audio) in memory until Stop; it is never saved. The 28 MB voice model downloads once, the first time you take a note.
- **Learn my voice** (Settings, on by default): your own voice fingerprint is built from your dictations and kept on your computer in `voice.json`, readable only by you. Notes use it to tell you from a person sitting in the room, who then gets a Speaker label instead of Me. It is numbers, never audio, and other people's voices are never stored. Turn it off and the file is deleted.
- Notes stay on your computer. Rename, copy or delete them; Select deletes several at once.
- Names from Zoom: while a note records, local-stt reads the names on Zoom's video tiles, and who Zoom shows talking (a tile marked active speaker, Speaker view, or the mini window when the meeting is minimized). Turn on **Read Zoom's speaking border** (Settings > Notes, off by default, needs Screen Recording on a Mac) and gallery view names the talker too, from the green border Zoom draws around them. At Stop each voice gets the name Zoom showed while it talked, when one name clearly wins, or the name people used when they spoke to it ("What do you think, Gaurav?"); a one-to-one call names the other person on its own, and any voice left unnamed gets the names as one-click choices. Keep the meeting in a window or minimized: a full-screen meeting behind another app cannot be read. The same works for Google Meet (Chrome, Edge, Brave, Arc), Microsoft Teams and Slack huddles, read from the page the app shows (on Windows through UI Automation, not yet tried in a real call); these use the class names the apps ship today, so tell us in an issue when one stops working.
- Name the speakers: type "Priya" next to **Speaker 2** (or **Others**) above the transcript and every one of their lines, and the copied note, says Priya. Clear it to go back.
- **Summarize** a finished note on your computer: an overview, what each person said, and the action items with who owns them and when they were said. Pick a summary model in the **Model** tab (Summary model section), which recommends one for your computer: Granite 4.0 Micro (IBM, 2.1 GB, fastest) or Gemma 4 E4B (Google, 5.2 GB, best summaries). Both are optional downloads and Summaries are off until you turn them on (the Summaries switch, or downloading a model); the note never leaves your computer.
- Laptop speakers work: echo cancellation takes the call's sound out of your microphone before it is transcribed (Settings > Notes > Echo cancellation, on by default). Headphones still give the cleanest split, and any echo words left are cut from your lines. Bluetooth headphones that switch to call mode mid-note are picked up again within a second.
- Your lines show the name you gave in setup.
- macOS 14.6 or later (macOS asks once for System Audio Recording), or Windows 10 and 11.

**Consent.** Recording other people is regulated: several US states and many countries need everyone's consent. local-stt shows a reminder the first time and gives you a line to paste in the meeting chat. This is not legal advice.

- **Detect calls** (Settings > Notes, on by default): when Zoom, Microsoft Teams, Slack, Webex or FaceTime starts using your microphone, or a browser window shows Google Meet or Microsoft Teams, a small prompt appears at the bottom centre of the screen you are using: "<App> call detected", "Take notes on this call?", with **Start note** and a close button. It hides after 20 seconds unless the pointer rests on it, and shows once per call (a call that drops for about 15 seconds is the same call). It does not appear while a note is recording. Nothing is recorded until you click Start note. Turn the switch off to stop the check.

## Insights

The **Insights** tab shows how dictation is going, kindly and privately:

- **Workday streak**: days in a row with at least one dictation, Monday to Friday only. Weekends never count and never break it.
- **Streak freezes**: you start with 5. A missed workday spends one by itself, and every 10 active workdays earns one more (10 at most). When none are left the streak starts again; your longest streak is kept.
- **Words and speed**: words today, this week and in all, words per minute, and an estimate of the time saved against typing at 40 words a minute.
- **When you dictate**: words by hour of the day.
- **Your words** (English): the nouns and verbs you use most and the mix of word types. It is approximate. Turn **Word insights** off and those counts are deleted.
- **Milestones** at 1,000 words and up.

Insights keep totals only (counts per day, the 200 words you use most), never sentences. They stay on your computer, keep counting when history is off, and **Reset Insights** deletes them.

## Settings

Open the app from the menu bar or tray icon.

![Settings, light and dark](https://raw.githubusercontent.com/mariadb-RupeshBiswas/local-stt/main/docs/screenshots/main-settings.png)

| Setting | What it does | Default |
|---|---|---|
| Push to talk | Hold to talk, release to paste. Any combo with at least one modifier | Fn + Shift (Mac), Ctrl + Alt (Windows) |
| Hands-free | Press once to start listening, again to stop; can be turned off | Fn + Shift + Space (Mac), Ctrl + Alt + Space (Windows) |
| Output language | English (translate) or keep the language you spoke; see [Languages](#languages) | English |
| Spoken language | Auto-detect or a fixed language | Auto-detect |
| Paste | Paste into the focused app, or only copy to the clipboard | Paste |
| Smart formatting | Numbered lists from spoken markers, mixed freely ("first point ... point two ... number three"); removes "uh", "um" and "er"; fixes spacing. English output only | On |
| Restore clipboard | Put your previous clipboard back after pasting | Off |
| Microphone | System default or a specific input | System default |
| Recording style | Pill, Waveform or Minimal | Pill |
| Live transcription | Shows your words above the pill while you speak (the pasted text is the final, more accurate pass) | On |
| Sounds | Short start and stop sounds | On |
| History | Keep a local history of dictations | On |
| Start at login | Launch quietly when you log in | Off |
| Check for updates | Looks for a new version once a day; an update shows as a menu item and a banner, never a pop-up | On |
| Troubleshooting log | Keeps app events (starts, errors, timings) on this computer, never your words, audio or keys | On |

The recording pill shows on the screen you are typing on. Drag it anywhere: it snaps to corners and edges and keeps that spot, in proportion, on every screen.

## Models

local-stt checks your computer (memory, processor, graphics, free disk) and only offers models that will run well on it. The best fit is marked **Recommended**; models that will not run well are greyed out with the reason.

| Model | Download | Good for | Needs |
|---|---|---|---|
| Whisper Small (default) | 181 MB | Everyday dictation on any modern laptop | 4 GB RAM |
| Whisper Medium | 514 MB | Better accuracy, accents, noisy rooms | 8 GB RAM, Apple Silicon or 8+ CPU threads |
| Whisper Large v3 | 1.0 GB | Best accuracy | 16 GB RAM and Apple Silicon |

Run `local-stt doctor` to see your hardware and which models fit, right in the terminal.

Each model shows its SHA-256 checksum and a **View on Hugging Face** button that opens the file's page there, which lists the same checksum, so you can see exactly what the app downloads. **Delete** removes a downloaded model you no longer use and frees its disk space; the model in use stays until you switch to another.

The files come from the Hugging Face repository of whisper.cpp's author. A CI check rebuilds each one from OpenAI's own Whisper checkpoint and requires the same SHA-256, so they are OpenAI's models converted for whisper.cpp; only a version number in the file header may differ. You cannot add a model by URL: the app's C++ code reads the model file, and a crafted file could attack it.

![Model picker with hardware check](https://raw.githubusercontent.com/mariadb-RupeshBiswas/local-stt/main/docs/screenshots/main-model.png)

## Languages

**Speak in any of 99 languages and get English text**, ready to paste into Claude, ChatGPT or any other AI assistant. Or keep the language you spoke.

| Output language | You speak | You get |
|---|---|---|
| **English** (default) | Any of the 99 languages | English, translated on your computer |
| **Spoken language** | Any of the 99 languages | The same language, every word as spoken |

- **Indian and South Asian languages:** Hindi, Bengali, Tamil, Telugu, Marathi, Gujarati, Kannada, Malayalam, Punjabi, Urdu, Assamese, Nepali, Sindhi, Sanskrit and Sinhala.
- **Also:** English, Chinese, Spanish, French, German, Arabic, Japanese, Korean, Portuguese, Russian, Indonesian, Swahili and more. The full list is under Settings > **Spoken language**, which detects the language by itself unless you pick one.
- **Every model speaks every language.** Whisper Small, Medium and Large v3 all handle all 99; the larger models are more accurate, especially for languages with less training data.
- **Translation goes into English only.** That is how Whisper is built.

> [!NOTE]
> **Translating to English can shorten what you said.** English output uses Whisper's built-in translation, which writes its own English version rather than every word, and can leave parts out, most of all in long dictations and with Whisper Small.
> - For every word, set **Output language** to **Spoken language**.
> - For better translations, pick Whisper Medium or Large v3 on the Model tab.
> - If a long translated dictation comes back short, try shorter pieces.


## Privacy and security

- Speech recognition runs locally with [whisper.cpp](https://github.com/ggml-org/whisper.cpp). Audio stays in memory and is discarded after each dictation.
- The app talks to the network for three things only: downloading a model from Hugging Face over HTTPS (verified against a pinned checksum on download and again when it loads); a once-a-day version check against pypi.org that sends nothing but the request itself (you can turn it off); and, only when you click Install and Restart, uv fetching the new release from PyPI.
- Text is put on the clipboard marked as private, so clipboard managers, Windows clipboard history and cloud clipboard skip it. On a Mac with Handoff on, Universal Clipboard can still pass it to your own nearby devices.
- The shortcut listener only checks whether your chosen combo is held. It never records or stores any other keys.
- The troubleshooting log records events such as "recording stopped: 4.2 s" or "paste blocked", never what you said, audio, key presses, clipboard contents or microphone names. It stays on your computer; turning it off deletes it.
- Meeting notes record what your computer plays only between New Note and Stop. Audio is never written to disk; notes are local files readable only by your user account.
- Insights keep daily counts and your most used words, never sentences; Reset Insights deletes them.
- History and settings are plain local files readable only by your user account. History can be switched off, cleared, or trimmed to the dictations you pick, at any time.
- Releases are built in GitHub Actions, published with PyPI Trusted Publishing and carry build provenance attestations.

See [SECURITY.md](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/SECURITY.md) to report a vulnerability privately.

## FAQ

**Is it really free?**
Yes. MIT licensed, no paid features, no limits, no account.

**Does it work without internet?**
Yes, after the first model download.

**Which languages can I speak?**
Any language Whisper supports, including English and Hindi. Text comes out in English by default; switch "Output language" to keep the spoken language.

**Does smart formatting use an AI model?**
No. Whisper writes the words and the punctuation. Smart formatting is a set of rules in the app that runs on that text: it removes fillers, fixes spacing and turns spoken list markers into a numbered list. Nothing is sent anywhere, and the same words always format the same way. Small local AI models we tested (350M to 3B parameters) dropped or changed words, so the app keeps rules.

**Does it work on Windows?**
Yes, Windows 10 and 11 on x64. The default shortcut is Ctrl + Alt.

**How is this different from built-in dictation?**
It runs a model you choose, entirely on your device, translates to English if you want, keeps a searchable history, and works the same way on Mac and Windows.

## Troubleshooting

Something not working? Open Settings > Troubleshooting and click **Export Report**, or run `local-stt diagnostics` (it works even when the app will not start). You get one text file, opened in Finder or Explorer, and its full path is shown (Settings shows a **Show in Finder** or **Show in Explorer** button to open it again). Reports are saved in `~/Library/Application Support/local-stt/diagnostics` on a Mac and `%APPDATA%\local-stt\diagnostics` on Windows. Each report holds:

- app version, how it was started, and your settings (the microphone shows only as "system default" or "a chosen device");
- your computer's OS, processor, memory, graphics and free disk;
- which models are downloaded, and how many dictations history holds (never their text);
- on macOS, a short summary of any local-stt crash in the last 30 days (the cause and the crashed thread, without device ids or paths);
- recent error output and the troubleshooting log.

Your home folder shows as `~` and your user name as `<user>`. Nothing is sent anywhere: read the file, then attach it to an issue or send it to whoever is helping you. Plain text also works well pasted into an AI assistant.

**The shortcut does nothing on macOS, though local-stt is switched on.** macOS can keep the switch of an older copy. In System Settings > Privacy & Security > Accessibility (Device Control and Data Access on newer macOS), select local-stt, click the minus button and add it again, or use Reinstall below. Settings > General shows **Allowed** once it works.

**Pressing Fn switches your keyboard language instead of recording.** With two or more keyboard layouts, macOS makes the Fn (🌐) key change the input source, which swallows an Fn shortcut. In System Settings > Keyboard, set **Press 🌐 key to** to **Do Nothing**. local-stt notices this, during setup or any time later, and shows a banner with a button to Keyboard settings.

**A shortcut you picked does nothing.** The system may use it too: on macOS, Cmd + Space opens Spotlight and Ctrl + Space switches input source; on Windows, Win + Space and Ctrl + Shift switch keyboard layout. local-stt warns under the shortcut field and in a banner when your shortcut matches one of these, including shortcuts you set in System Settings > Keyboard > Keyboard Shortcuts on a Mac. Pick another shortcut, or turn the system one off. Shortcuts that other apps grab cannot be seen; if one still does nothing, try a different combination.

**Reinstall.** Settings > Troubleshooting > **Reinstall** (or `local-stt reinstall`) quits local-stt, installs it again (the newest version when one is out) and opens it. On macOS it also resets local-stt's permissions and opens setup, so you can allow them again; your name, history and insights stay. An update that macOS sees as a new app does the same on its own. Tick the box, or add `--clean`, to also delete your settings, history, notes, insights, voice fingerprint and logs; downloaded models stay.

## Updating

local-stt checks for a new version once a day (Settings > General > Check automatically). When one is out you see an "Update to ..." item in the menu bar or tray and a banner in the app with **Install and Restart**, **Release Notes**, **Later** and **Skip This Version**. From a terminal: `local-stt check-update`. When the new version starts, its window opens with a short burst of confetti and "Updated to local-stt ..." (no confetti when Reduce Motion is on).

On macOS, releases are signed with local-stt's own certificate, so an update from 0.3.0 or later keeps Microphone and Accessibility allowed, with no setup. Skip 0.4.0: its macOS build was not signed, so installing it asks for permissions again; 0.4.1 replaced it. Updating from a version before 0.3.0, or from 0.4.0, asks once more and opens setup; your history and insights stay.

## Uninstall

```bash
local-stt uninstall          # removes the app, the terminal command and the login item
```

Your settings, history and models live in `~/Library/Application Support/local-stt` (macOS) or `%APPDATA%\local-stt` (Windows). Delete that folder to remove everything.

## Contributing

Bug reports, ideas and pull requests are welcome. See [CONTRIBUTING.md](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/CONTRIBUTING.md).

### Report a bug

1. Save a diagnostic report: **Settings > Troubleshooting > Export Report**, or `local-stt diagnostics` in a terminal. Read it; it never holds audio or dictated text.
2. [Open a bug report](https://github.com/mariadb-RupeshBiswas/local-stt/issues/new/choose), describe what happened and attach the report.
3. Security problems go to [private reporting](https://github.com/mariadb-RupeshBiswas/local-stt/security/advisories/new), never a public issue.

## License

[MIT](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/LICENSE). Whisper models are released by OpenAI under the MIT license; whisper.cpp is MIT licensed by ggml-org. Telling voices apart uses sherpa-onnx (Apache-2.0, k2-fsa) with ONNX Runtime (MIT, Microsoft) and the CAM++ voice model from 3D-Speaker (Apache-2.0, ModelScope). Echo cancellation uses sonora (BSD-3-Clause), a Rust port of WebRTC's AEC3. Summaries use llama.cpp (MIT, ggml-org) with Granite 4.0 Micro (Apache-2.0, IBM) or Gemma 4 E4B (Apache-2.0, Google).

Full third-party licence notices (Rust crates, bundled native code, downloadable models) are in [THIRD_PARTY_NOTICES.md](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/THIRD_PARTY_NOTICES.md).

Wispr Flow, Superwhisper and other product names mentioned here are trademarks of their owners. local-stt is an independent open-source project and is not affiliated with, endorsed by or sponsored by them; the names are used only to say what local-stt can be used instead of.

---

<sub>Keywords: free speech to text, offline dictation, local Whisper, private voice typing, voice to text for Mac, voice to text for Windows, push to talk dictation, Hindi to English speech translation, open source Wispr Flow alternative, open source Superwhisper alternative, no cloud transcription.</sub>
