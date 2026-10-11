# First-run onboarding: design

Status: draft, 2026-10-11. Branch `feature/onboarding`, stacked on `feature/insights`.

## Goal

Get a new user from install to a first successful dictation in about a minute, show honestly what
the app is doing while it sets up, and leave every choice changeable later in Settings.

Principles (sources in the research notes, section numbers below refer to them):

- Brief, skippable, one decision per screen; Skip is always visible (NN/g, Fluent 2).
- Ask for information only when it does something: the name is optional and used for greetings.
- Ask for a permission right before the step that needs it, with our own screen first (priming).
- Progress is real: rows tick when the work is done; only the model download shows a percentage,
  from real bytes. No artificial delays beyond a 300 ms minimum so rows do not flicker.
- The recommended path must be good on its own; Customize stays reachable later in Settings.

## When it shows

- A fresh install (no config file) has `setup_done: false`, so the main window opens on the
  onboarding view.
- A config written before onboarding existed has no `setup_done` key and loads as `true`: people
  already using the app never see it unasked.
- The demo never shows it.
- Settings > General > **Run Setup Again** shows it at any time. Re-running never resets settings.
- Quitting midway is safe: nothing is final until a choice is made, and setup starts again
  next time (it is short).
- There is always a way out: **Skip Setup** on every step (a quiet link in the header) and Skip
  on the last step both save `setup_done: true`, so a closed window never loops.
- A clean Reinstall deletes the config, so setup runs again afterwards; the Reinstall dialog says so.

## Steps

The view replaces the tabs inside the main window. A header shows "Step N of 5" and the step
title; focus moves to the title on each step. Back works on every step after the first.

1. **Welcome.** Title "Dictate anywhere. Your voice stays on this Mac." ("this PC" on Windows;
   the version check and model download use the network, so the title never says nothing leaves).
   Optional field "What should we call you?" (placeholder "Your name", hint "Optional. Stays on
   this Mac."). Buttons: Continue (primary), Skip. Any text up to 60 characters; trimmed; empty
   means no name. No validation beyond that.
2. **System check.** Title "Setting things up for you, {name}" (or "Setting things up for you").
   A checklist; each row is Waiting, Running (spinner plus a "now doing" line), Done (tick plus a
   result line), Needs You, or Failed (with Try Again). Rows:
   - Checking your computer: chip or CPU, memory, free disk (from `get_state.hardware`).
   - Choosing a speech model: "Whisper Small fits your Mac" (the recommended model from
     `get_state.models`; if the active model is already downloaded, that one).
   - Finding a microphone: the number of inputs, or "No microphone found" (Needs You).
   - Checking Accessibility (macOS only): Allowed or "You'll allow this in a moment".
   - Downloading the speech model: real bytes ("120 of 181 MB"), or "Already downloaded".
     A fresh install already started it at launch, so the row reads `get_state.downloading` and
     follows `model-progress` / `model-done`; it calls `choose_model` only when nothing is
     downloading and nothing is downloaded. The download keeps running in the background.
   Continue is enabled once every row except the download is Done or Needs You.
3. **Setup choice.** Title "How do you want to set up?" Two cards:
   - **Use Recommended Settings** (primary): lists what that means: the model, the shortcut
     (Fn + Shift or Ctrl + Alt), English output, smart formatting on, paste on, history on,
     start at login off. These are the defaults; choosing it changes nothing.
   - **Customize**: one page with the existing Settings controls for push-to-talk shortcut,
     language, paste, smart formatting, save history, and start at login. Theme and the rest
     stay in Settings.
4. **Permissions.** One card each, skippable:
   - **Microphone**: "Needed to hear you. Audio stays on this Mac and is never saved." Button
     **Test Microphone** runs `check_microphone`: macOS asks for the microphone the first time;
     the card shows a live level bar for 2 seconds while the user says something, then "Heard
     you" or "Didn't hear anything" with "Open Microphone Settings" and Try Again. Testing here
     means the permission prompt never lands in the middle of the first dictation, and it may
     wake a Bluetooth headset early (unconfirmed: one silent AirPods recording, retest pending).
   - **Accessibility** (macOS only): "Needed for the shortcut and to paste into the app you are
     using." Button **Open Settings** (`open_accessibility_settings`, which also lists this build
     in the pane). The card shows "Waiting for you to turn on local-stt" and flips to Allowed
     on the `permissions-changed` event, without a restart.
5. **Try it.** On macOS without Accessibility the shortcut cannot work, so the step shows "Allow
   Accessibility first" with Open Settings instead of the box, and switches to the box on
   `permissions-changed`. Otherwise: "Hold {shortcut} and say something." A focused text box receives the pasted
   text (the app is the frontmost window, so the normal paste lands there). When text arrives:
   "That's it. Hold {shortcut} in any app." If the model is still downloading, the box is
   disabled with the download progress. Buttons: Finish (primary), Skip.
   Finish and Skip both save `setup_done: true` and the name, show the History tab and a toast
   "local-stt lives in the menu bar" ("system tray" on Windows).

## Backend

| Change | Detail |
|---|---|
| `config.setup_done: bool` | Default `false`; a config file without the key loads as `true` |
| `config.user_name: Option<String>` | Trimmed, at most 60 characters, no control characters; empty means `None` (validated in `merge_patch`) |
| First run | `first_run_or_load` shows the main window when `!setup_done` (and not in the demo), as well as when no model is downloaded |
| `check_microphone` command | Opens the chosen (or default) input for 2 s with the same recorder dictation uses, emits `mic-check-level` `{ level }` about 30 times a second, returns `{ heard: bool, levelDb: number or null }`. Refuses while a dictation or note is recording, and a shared flag makes the shortcut ignore presses during the check, so two streams never fight over one headset. Granted to the main window only |
| Diagnostic report | `user_name` shows as "set" or "not set", never the name (unit test) |
| Settings | "Your name" text row and Run Setup Again (both UI over `set_config`) |

Nothing is logged about the name. The level check logs only "microphone check: heard" or
"microphone check: silent".

## Accessibility and motion

- Each step has one `h1`; progress text "Step 2 of 5"; checklist rows are a list with text
  states, never colour or icon alone; row changes announced through the existing polite status
  region, not every percent (download announced every 25 percent).
- Every control reachable by keyboard; Enter triggers the primary button; Esc does nothing
  destructive.
- Spinners and the level bar stop under Reduced Motion (static "Running" text instead).

## Testing

- Unit: `setup_done` default and missing-key load, `user_name` validation (length, control
  characters, trimming, empty), and a report from a config with a name does not contain it.
- UI checks (preview `?onboarding=1`): each step's title and Step N of 5, Skip and Back, name
  saved via `set_config`, system check rows reach Done with mock data, download row shows bytes,
  Needs You for no microphone (`?inputs=none`), Recommended changes nothing, Customize shows the
  listed controls, microphone test shows a level and Heard you, Accessibility flips on
  `permissions-changed`, Try It shows success on text input, Finish sends `setup_done: true`,
  Run Setup Again shows step 1, Windows has no Accessibility row or card, both themes, contrast.
- Real app: a fresh data folder (`LOCAL_STT_DATA_DIR`) shows onboarding; an existing config does not.
- CI: the Windows end-to-end run starts with a fresh data folder, so it lands on onboarding; its
  checks are global (process, pill, report) and do not read the main window.

## Decisions for the owner

1. Ask for a name at all (built: optional, greeting only)?
2. Start at login on the Recommended path (built: off)?

## Not in this version

Speaker or vocabulary hints from the name, a meeting-notes step (System Audio is asked when Notes
is first used), and resuming midway.
