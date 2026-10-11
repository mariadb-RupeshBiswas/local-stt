# Design guide

How local-stt should look, read and be built. For people and coding agents alike.
[AGENTS.md](../AGENTS.md) has the commands, the module map and the hard rules; this guide
explains the choices behind them, so new work feels like the rest of the app.

## Product principles

1. **Private by default.** Speech is turned into text on the user's computer. Network use is
   limited to the pinned model downloads, the daily version check (which can be turned off)
   and an update the user starts. No telemetry, no accounts.
2. **Quiet.** The app lives in the menu bar or tray. It never steals focus, never opens a
   window on its own except on first run, and shows news (an update) as a banner or menu
   item, never a modal.
3. **Obvious state.** When the microphone or system audio is in use, the pill says so.
   Anything that runs in the background has a visible way to stop it.
4. **Reversible, or asked first.** Deleting asks once and says it cannot be undone. Turning
   a feature off that keeps data (history, the troubleshooting log) also deletes that data.
5. **Honest numbers.** A value that is missing shows as `-`, never `0`.

## Visual design

The UI in `ui/` follows Apple's Human Interface Guidelines on macOS and keeps the same
layout on Windows, with opaque surfaces where Windows has no native glass.

### Tokens

Use the variables in [`ui/tokens.css`](../ui/tokens.css); do not write raw colours or sizes
in component CSS (a 1 px hairline is the one exception).

- **Spacing:** a 4 px grid, `--s1` (4) to `--s8` (32).
- **Radius:** `--r-sm` 10 px (fields, small buttons), `--r-md` 14 px (cards), `--r-lg` 22 px
  (the pill and its popover).
- **Type:** the system font stack in `--font`. Body text 14 px, row titles 13 to 14 px,
  secondary text and group titles 12 px, tags 11 px.
- **Colour:** semantic names only (`--label`, `--label-2`, `--surface`, `--fill`,
  `--separator`). Coloured text uses the `-ink` variants (`--accent-ink`, `--red-ink`,
  `--green-ink`), which pass 4.5:1 on light and dark surfaces; the base colours are for dots,
  fills and icons.
- **Light and dark:** every token is defined for both. A new token gets both values in the
  same change.
- **Motion:** the easing curves in `--ease-out`, `--ease-enter` and `--ease-drawer`, and
  durations of 200 ms or less. Reduced Motion (the `.reduce-motion` class) turns animation off.

### Components

| Component | Use |
|---|---|
| Card | A group of related rows on `--surface` with hairline separators |
| Setting row | Title, one-line description, control on the right |
| Segmented control | The tabs, and two or three mutually exclusive choices |
| Switch | An on/off setting (`role="switch"`, `aria-checked`) |
| Button | `.btn` by default, `.btn-action` for the main action, `.btn-danger` for a destructive action, `.btn-danger-solid` only for the confirm button in a delete dialog |
| Icon button | 28 x 28 pt, always with an `aria-label` |
| Select bar | Sticky bar in select mode: Select All, a count, Delete |
| Dialog | The one confirmation dialog (`askConfirm`); Cancel takes focus when the action destroys data |
| Toast | A short status after an action the user started |
| Empty state | A title, one sentence, and at most one action |

Hit targets are at least 28 pt. Every control is reachable by keyboard and shows the
`--focus` ring.

### The pill

The recording pill and its live popover are separate, never-focusable windows. On macOS the
window supplies the glass (Liquid Glass, HUD material) and the page adds only a light tint;
on Windows, with Reduce Transparency or Increase Contrast, and in the demo, the page draws an
opaque capsule.
States: recording, transcribing, done, warning, error, positioning, and Notes (the recording
look with its own label and the note's timer, for as long as a note runs). A state always
pairs a colour with a label or shape, so it never relies on colour alone.

## Words

- Plain words, second person, short sentences. Say what happens, not how it works.
- **Buttons are Title Case** ("Export Report", "Clear History"); labels, descriptions and
  messages are sentence case.
- In-progress buttons end in three dots ("Saving...", "Checking...").
- No em dashes anywhere; use a comma, colon, parentheses or two sentences.
- Delete dialogs name the count and say it is permanent: "Delete 3 dictations?" then "They
  will be removed from this Mac. This can't be undone." with Cancel and Delete.
- Errors say what to do next: "Microphone access denied. Allow it in system privacy settings."
- Platform words follow the platform: "this Mac" / "this PC", Cmd / Ctrl, Fn key names on macOS.
- Never claim what the code does not guarantee. "Never uploaded" appears only where it is true.

## Code design

- **Pure core, thin edges.** Logic that decides (state machine, formatting, matching,
  geometry, segmenting) is plain functions with unit tests. Calls into the OS live in
  `macos.rs` / `windows.rs` files or behind a small trait, so tests can stand in for devices.
- **One module per feature**, exposing a few functions. A feature does not grow `app.rs`.
- **The UI is untrusted.** Every command validates its input: ids before they touch a path,
  lengths and counts before work starts. Each command is listed in `build.rs` and granted
  only to the window that needs it in `capabilities/`.
- **Files on disk:** owner-only (0600 files in 0700 folders). Whole-file rewrites go through
  a temp file, `fsync` and rename; streams append one JSON line at a time. One lock per file
  family keeps writers from interleaving.
- **Never block the UI thread.** Slow commands use `spawn_blocking`; long work runs on worker
  threads and reports back through events.
- **Child processes** go through `proc::command`, which keeps Windows from opening consoles.
- **Dependencies** come from named organisations and need the owner's go; a few lines of
  code beat a new crate.
- **Comments** are one line and say why. History belongs in commit messages.

## Privacy and security design

- [SECURITY.md](../SECURITY.md) lists everything the app can touch; a change that adds an
  entry updates that table in the same pull request.
- Audio is never written to disk. Logs record events and timings, never words, keys,
  clipboard contents or device names.
- Anything the user may share (the diagnostic report) is built from an allowlist and masked
  as a whole: home folder as `~`, user name as `<user>`.
- User text reaches the page through `textContent` only. The strict CSP allows no inline
  scripts or styles.
- Clipboard writes are marked private, so clipboard managers and cloud clipboards skip them.
- Recording other people (meeting notes) starts only on a click, shows a consent reminder
  once with a line to paste in the meeting chat, and keeps an indicator on screen throughout.
- Live transcripts use `role="log"`, add one row per new line, and dictation always runs
  before note segments, so a paste never waits behind a meeting.

## Testing

| Layer | Where | What it proves |
|---|---|---|
| Unit | `#[cfg(test)]` next to the code | Pure logic and file handling |
| Engine | `tests/engine.rs` with a real model | whisper.cpp through the shim |
| UI checks | `dev/ui-tests` against `dev/preview.html` | Layout, keyboard, copy, contrast, both themes |
| Windows end-to-end | `dev/windows-e2e.ps1` in CI | Install, shortcut, pill, hand-off, report, uninstall |
| Manual | Your own Mac or PC | The shortcut, paste, microphone and permission prompts that CI cannot drive |

A new command also gets a mock in `dev/preview.html` and at least one UI check.

## New feature checklist

1. Write down what it touches (files, network, devices) and add it to SECURITY.md.
2. Pure logic first, with tests; OS calls behind a platform file or a trait.
3. Command validated, listed in `build.rs`, granted to one window.
4. UI from existing components and tokens, both themes, keyboard and screen reader checked.
5. Words checked against the rules above.
6. README and CHANGELOG updated; AGENTS.md if the layout or commands changed.
