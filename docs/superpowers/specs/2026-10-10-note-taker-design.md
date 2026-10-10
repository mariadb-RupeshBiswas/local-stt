# Note Taker: design

Status: draft for review, 2026-10-10. Branch `feature/note-taker`, stacked on `feature/dictation-app`.

## Goal

A Notes tab that turns a meeting (Zoom, Google Meet, Teams, Slack huddle, any call) into a
transcript split by speaker, tidied after the call, with an optional summary, all on the
user's own computer. Same promise as dictation: free, local, nothing uploaded.

Non-goals: a bot that joins meetings, cloud processing, recording video or the screen,
keeping audio, sharing notes with other people from inside the app.

## What others do (public sources)

| Product | Detects calls | Other side's audio | Speaker names | Processing |
|---|---|---|---|---|
| Wispr Flow Notetaker ([page](https://wisprflow.ai/notetaker)) | Yes, asks before taking notes | System audio | "You / Them" live, full names after the call | Audio stored on the device and in some cases in cloud storage; a second AI pass reviews the recording |
| Granola ([docs](https://docs.granola.ai/help-center/taking-notes/speaker-attribution)) | Calendar | System audio | Me / Them, names read from the meeting app through Accessibility | Cloud |
| MacWhisper ([help](https://macwhisper.helpscoutdocs.com/article/30-record-meetings)) | "Meeting Detected" notice | Screen Recording permission | Not documented | Local |

local-stt takes the Wispr flow (detect, ask, record, tidy, summarise) and keeps every step local.

## Experience

### Notes tab

Tabs become History, Notes, Model, Settings; Cmd/Ctrl + 1..4 move with them.

- List of notes, newest first, grouped by day like History. Each row: title, start time,
  length, and the first line of the transcript.
- **New Note** button at the top. Search box. **Select** mode with checkboxes, a box per day,
  Shift-click ranges, Select All and Delete, the same pattern and code as History.
- Empty state: "No notes yet. Click New Note when a call starts. local-stt can also ask
  when it sees a call (Settings > Notes)."

### Before the first note

A one-time sheet: "Taking notes records the other people on the call. In many places you
need their consent. Tell them you are taking notes." Buttons: **Copy Consent Line** and
**Continue**. Continue sets `notes_consent_seen` in the config, so the sheet shows once.

The consent line, copied as plain text for the meeting chat: "Heads up: I'm taking notes of
this call with a transcription app that runs only on my computer. Tell me if you'd rather I
didn't." It is always available again from the note header.

### Taking notes

- Clicking New Note (or Start Notes on the meeting prompt, or the tray item) opens a new note
  and starts at once. macOS asks for System Audio Recording the first time.
- The note shows a header (title, red dot, timer, **Stop**, **Copy Consent Line**) and a
  live transcript of turns labelled **Me** and **Others**, each with a time. New turns
  appear a few seconds after each pause.
- The recording pill switches to a Notes state (red dot, timer, "Notes") and stays on screen
  for the whole note, even with the main window closed. Dictation still works during a note;
  the pill shows dictation, then returns to Notes. The tray menu shows **Stop Notes (12:03)**.
- Stop (button, tray item, or 4 hours, the cap) ends capture. The note shows "Tidying..."
  while the last segments finish, then the final transcript: fillers removed, spacing fixed,
  spoken lists numbered (the dictation smart formatting), and back-to-back turns from the
  same speaker merged.

### A saved note

- Title (editable, default "Zoom call, 10:30" when the app is known, else "Note, 10:30").
- **Transcript** view (default) with Me / Others (later Speaker 1..N or names) and times.
- **Summary** view: a **Summarize** button that runs a local model (phase 3). Until a
  summary model is installed it says what it needs and offers the download.
- Copy (whole note as plain text, marked private on the clipboard) and Delete.

### Meeting prompt (phase 2)

When a known call app (or a browser) has held the microphone for 10 seconds, a small glass
card appears above the pill: "Zoom call detected. Take notes?" with **Start Notes** and
**Not Now**. It fades after 15 seconds. Not Now stays quiet until that app releases the
microphone. Nothing records until the click. Setting: Settings > Notes > Suggest notes for
calls (on).

## Architecture

### Capture (phase 1)

- Two cpal input streams: the microphone (as dictation) is **Me**; the default output device
  opened with `build_input_stream` is **Others**. cpal 0.18.2 turns that into a system-audio
  tap on macOS (Core Audio process tap, cpal declares macOS 14.6+) and WASAPI loopback on
  Windows. No new dependency.
- Each channel is mixed to mono and resampled to 16 kHz (existing `audio.rs`).
- A pure segmenter cuts each channel at a pause of 600 ms or at 25 s of speech, using the
  existing RMS level. Silent stretches are dropped.
- Audio is never written to disk. Each segment lives in memory until it is transcribed, then
  it is dropped. Memory stays at a few seconds of audio per channel.
- macOS: `NSAudioCaptureUsageDescription` is added to `Info.plist` (embedded in the binary and
  in the installed app bundle). Whether macOS shows the System Audio Recording prompt for the
  bare binary that `uvx` runs, and not only for `~/Applications/local-stt.app`, is untested;
  it is a phase 1 acceptance check. If the bare binary never prompts, New Note offers to
  install the app first, as Start at login does.

### Transcription

- Segments go to the existing worker thread as a new job type, so the one loaded Whisper
  model serves dictation and notes, one job at a time.
- Dictation always goes first: the worker moves note jobs from its channel into its own
  queue, runs one only when the channel is empty, and checks the channel again after each.
  A backlog of note segments never delays a paste by more than one segment (a few seconds of
  audio, well under a second to transcribe on Apple Silicon).
- Language and translate follow the dictation settings.
- Echo: with speakers instead of headphones, the microphone hears the others, so the same
  words come back on Me. Loudness cannot tell echo from people talking over each other, so
  the rule is on text: a Me segment that overlaps an Others segment in time and shares at
  least 70% of its words is echo and is dropped; different words are crosstalk and both stay.
  The note header suggests headphones. Acoustic echo cancellation is a later item.
- Throughput risk: Whisper Small transcribes much faster than real time on Apple Silicon. On
  a slow Windows CPU two channels may lag; segments queue and the note catches up after Stop.
  The log records queue depth so reports show it.

### Notes store

- While recording, each finished segment is appended as one line to `<data>/notes/<id>.jsonl`
  (cheap, survives a crash). On Stop the note is compacted into `<data>/notes/<id>.json` with
  the same temp-file, fsync and rename helper as history, and the `.jsonl` is removed. Files
  0600, folder 0700.
- `id` is 16 random hex characters. Shape:

```json
{
  "id": "3f9c0a6b1d2e4f58",
  "title": "Zoom call, 10:30",
  "started_ms": 1791624255000,
  "ended_ms": 1791627855000,
  "source": "Zoom",
  "status": "done",
  "speakers": { "me": "Me", "others": "Others" },
  "segments": [
    { "start_ms": 0, "end_ms": 4200, "speaker": "me", "text": "Let's start with the numbers." }
  ],
  "summary": null
}
```

- `status` is `recording`, `tidying`, `done` or `failed`. At start, a `.jsonl` with no
  matching `.json` (the app crashed mid-call) is compacted into a `done` note with what it has.
- Notes are separate from dictation history and are not trimmed. A setting to delete notes
  older than N days can come later.

### Commands and events

Main window only, each input validated like the existing commands:

| Command | Does |
|---|---|
| `list_notes` | id, title, start, length, first line, status, newest first |
| `get_note(id)` | the whole note |
| `start_note` | starts capture, returns the new id; refuses a second note |
| `stop_note` | stops capture |
| `rename_note(id, title)` | 1 to 200 characters |
| `delete_notes(ids)` | 1 to 10,000 ids; refuses the note being recorded |
| `summarize_note(id)` | phase 3 |
| `rename_speaker(id, speaker, name)` | phase 4 |

Events: `note-segment {id, segment}`, `note-state {id, status}`, `meeting-detected {app}`.

### Meeting detection (phase 2)

- macOS: the Core Audio process list (`kAudioHardwarePropertyProcessObjectList`, then per
  process `kAudioProcessPropertyIsRunningInput`, `kAudioProcessPropertyPID` and
  `kAudioProcessPropertyBundleID`) through a few `extern "C"` declarations in a `macos.rs`
  file, polled every 2 seconds. Reading it needs no permission (checked with a small probe
  on macOS 27). Call apps by bundle id: Zoom, Teams (old and new), Webex, Slack, Discord,
  FaceTime. Browsers (Chrome, Safari, Firefox, Edge, Arc, Brave) count as "a browser call".
  local-stt's own process and other dictation apps are ignored.
- Windows: the microphone consent store under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone`
  (`LastUsedTimeStop` of 0 means in use), read with windows-sys registry calls, polled every
  2 seconds. The audio-session API (`IAudioSessionManager2`) is the fallback if the registry
  proves unreliable.

### Summary (phase 3)

- Model: Qwen3-4B-Instruct-2507, Q4_K_M GGUF, 2.5 GB, Apache-2.0, pinned by SHA-256 and
  downloaded through the existing verified path, offered only where it fits (8 GB RAM or
  more); a smaller Qwen3-1.7B tier for 8 GB machines. Gemma (custom terms) and Qwen2.5-3B
  (research licence) are out.
- Engine: llama.cpp, vendored like whisper.cpp. Two copies of ggml cannot be linked into one
  binary. Options:
  1. In process, both projects built against one ggml (whisper.cpp has
     `WHISPER_USE_SYSTEM_GGML`, llama.cpp has `LLAMA_USE_SYSTEM_GGML`; an API mismatch fails the
     build instead of misbehaving). One binary, but a llama.cpp abort would also end dictation,
     as the Metal exit crash once did.
  2. A separate helper program with its own ggml: crash isolation, but a second native binary
     in the wheel (maturin include rule, its own signing on macOS).
  3. Recommended: option 1's single binary, run as a child of itself
     (`local-stt summarize --note <id>`, transcript in, summary out on stdin and stdout). One
     binary and one ggml, and a crash in the summary only ends the child; the model's memory
     is freed when it exits.
- Prompt: map-reduce for long meetings (summarise 15-minute chunks, then the chunk
  summaries). Output sections: Summary, Decisions, Action items (with the person when it was
  said), Open questions.
- macOS 26+ could also use Apple's on-device Foundation Models (no download) through a small
  Swift shim; Private Cloud Compute is a cloud model and is never used.

### Speakers (phase 4 and 5)

- Phase 4: split the Others channel into Speaker 1..N with sherpa-onnx (Apache-2.0)
  diarization: pyannote segmentation 3.0 (7 MB, MIT) plus a CAM++ speaker embedding (about
  29 MB), run after Stop. Speaker embeddings are used in memory and never stored. Click a
  label to rename that speaker across the note.
- Phase 5 (optional): real names by reading the active speaker from the meeting app's
  window through Accessibility (macOS) or UI Automation (Windows), as Granola documents. Each
  meeting app needs its own reader and breaks when that app changes its layout, so this is
  last and behind a setting.

## Privacy and safety

- Recording starts only on a click. Detection only asks.
- A visible indicator for the whole note: the pill's Notes state, the tray item, and the
  operating system's own microphone and system-audio indicators.
- Consent: the first-use sheet, Copy Consent Line in every note, and a README section that
  says recording laws differ (several US states and many countries need everyone's consent)
  and that this is not legal advice.
- No audio on disk, ever. No network beyond pinned model downloads.
- Notes are owner-only files; Delete removes the file. The diagnostic report shows only the
  number of notes. `diag::log` records note events by length and timing, never titles, text,
  speaker names or app window titles.
- Note text is rendered with `textContent` only; titles and speaker names are length-capped.
- New permissions are asked when first needed: System Audio Recording (macOS, first note).
  Accessibility is already granted for paste and is only needed again for phase 5.

## Gating

- macOS older than 14.6: the Notes tab explains "Needs macOS 14.6 or later" and New Note is
  off. Dictation is unchanged (its floor stays macOS 11).
- No output device (some CI runners, some servers): New Note reports "No system audio device
  found" and offers Me-only notes.

## Phases

| Phase | Ships | Done when |
|---|---|---|
| 1 | Manual notes: Me / Others capture, live turns, tidy pass, notes list with select and delete, note view, copy, consent sheet, Notes pill state, gating | Unit, UI and engine tests pass; a real call on macOS gives a split transcript |
| 2 | Meeting prompt: macOS process list, Windows consent store | Prompt appears for a live Zoom or Meet call and never for dictation |
| 3 | Local summary | A one-hour transcript summarises on a 16 GB Mac |
| 4 | Speaker 1..N on Others, rename | Two remote voices split correctly in a test call |
| 5 | Optional names from the meeting app | Behind a setting, Zoom first |

## Decisions for the owner

1. Summary engine: the single binary run as a child of itself (recommended), in process, or
   a separate helper program.
2. Diarization (phase 4): sherpa-onnx built from source in `build.rs` like whisper.cpp
   (its Rust crate is published by a person, not an organisation, so it fails the dependency
   rule). Its build downloads ONNX Runtime; that needs a pinned checksum or a vendored copy.
3. Whether phase 5 (scraping names from meeting apps) is wanted at all.

## Testing

- Pure units: segmenter (pause, max length, silence), turn merging, echo drop rule, note store
  (permissions, delete, corrupt file, crashed-while-recording recovery), consent-line text.
- Engine: two synthetic channels through the real model, as the existing engine tests do.
- UI checks: Notes list, select and delete, note view, consent sheet, recording header,
  gating copy, with the preview's mock bus.
- Windows E2E: start a note from the CLI test hook, check the "no system audio" or capture path
  on the runner, stop, list, delete.
- Manual, on a real call before phase 1 is called done (owner checklist):
  1. Start local-stt from `uvx` and from the installed app; New Note asks for System Audio
     Recording once in each case.
  2. Join a Zoom or Meet call with headphones, click New Note, talk with one other person for
     two minutes: turns show as Me and Others within a few seconds of each pause.
  3. Repeat without headphones: your own words appear once, not twice.
  4. Dictate with the push-to-talk shortcut during the call: the paste is as quick as usual and
     the pill returns to Notes.
  5. Close the main window: the pill still shows Notes and the timer; Stop from the tray.
  6. The saved note reads cleanly; rename it, copy it, delete it.
  7. Quit local-stt during a note (force quit): on the next start the note is there.
