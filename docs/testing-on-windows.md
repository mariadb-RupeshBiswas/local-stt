# Testing on Windows

Two free ways, from fully automatic to hands-on.

## 1. Every pull request (automatic)

The `Windows end-to-end` job in `.github/workflows/ci.yml` runs on GitHub's Windows runners,
which are free for public repositories. It runs `dev/windows-e2e.ps1`, which:

- installs the wheel with `uvx ... local-stt install`
- checks the Start menu shortcut and the `local-stt` terminal command
- downloads the small model through the app's verified download
- launches the app, holds Ctrl + Alt with synthetic key presses, and checks the recording pill
  window appears (runners have no microphone, so the pill shows its error state)
- uninstalls and checks everything was removed

Screenshots of each step are attached to the run as the `windows-e2e-screenshots` artifact.

What it cannot check: real speech (no microphone), paste into another app, and how the
pill looks to a person.

## 2. A Windows VM on your Mac (hands-on)

Free for testing; Windows runs unactivated with a small "Activate Windows" watermark.

1. Install [UTM](https://mac.getutm.app/) (free from its website).
2. Download the Windows 11 Arm ISO from Microsoft:
   <https://www.microsoft.com/en-us/software-download/windows11arm64>
3. In UTM: Create a New Virtual Machine > Virtualize > Windows. Choose the ISO, tick
   "Install drivers and SPICE tools", give it 8 GB RAM, 4 cores and 64 GB disk.
4. Install Windows. When asked for a product key, choose "I don't have a product key".
5. In the VM, open PowerShell and install uv:
   `powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"`
6. Copy the Windows wheel into the VM (download it from the CI run's artifacts, or from a
   release), then run it:
   `uvx --from .\local_stt-0.1.0-py3-none-win_amd64.whl local-stt`
7. Allow the microphone when Windows asks. Whether your Mac's microphone reaches the VM depends
   on the UTM sound settings; if it does not, everything except real speech can still be tested.
8. Hold Ctrl + Alt in Notepad, speak, release.

Notes:

- The wheel is built for x64. Windows 11 on Arm runs it through its built-in x64 emulation,
  so transcription is slower than on a native x64 PC; use the Small model.
- On many non-US keyboard layouts Ctrl + Alt is the same as AltGr. If that gets in your
  way, change the shortcut in Settings.
