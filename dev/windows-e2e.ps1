# End-to-end check on a real Windows desktop (GitHub runner): install, launch, hotkey, pill, uninstall.
param(
    [Parameter(Mandatory = $true)][string]$Wheel,
    [Parameter(Mandatory = $true)][string]$Shots
)
$ErrorActionPreference = 'Stop'
$failed = 0
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if ($ok) { Write-Host "PASS $name $detail" } else { Write-Host "FAIL $name $detail"; $script:failed++ }
}

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Native {
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    public static string[] VisibleTitles(uint pid) {
        var list = new List<string>();
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid && IsWindowVisible(h)) { var sb = new StringBuilder(256); GetWindowText(h, sb, 256); list.Add(sb.ToString()); }
            return true;
        }, IntPtr.Zero);
        return list.ToArray();
    }
}
"@

function Shot([string]$name) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save((Join-Path $Shots "$name.png"))
    $g.Dispose(); $bmp.Dispose()
}

New-Item -ItemType Directory -Force -Path $Shots | Out-Null
$data = Join-Path $env:RUNNER_TEMP 'lstt-e2e-data'
$env:LOCAL_STT_DATA_DIR = $data
$env:LOCAL_STT_NO_PERMISSION_PROMPTS = '1'

# Install from the wheel exactly the way a user would.
uvx --from $Wheel local-stt install
Check 'install exits 0' ($LASTEXITCODE -eq 0)
$exe = Join-Path $env:LOCALAPPDATA 'Programs\local-stt\local-stt.exe'
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\local-stt.lnk'
$shim = Join-Path $env:USERPROFILE '.local\bin\local-stt.cmd'
Check 'installed exe exists' (Test-Path $exe) $exe
Check 'Start menu shortcut exists' (Test-Path $lnk) $lnk
if (Test-Path $lnk) {
    $target = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk).TargetPath
    Check 'shortcut points at installed exe' ($target -eq $exe) $target
}
Check 'terminal command shim exists' (Test-Path $shim) $shim
$version = & $shim --version
Check 'terminal command runs' ($version -match '^local-stt \d') "$version"

# Real download through the app's verified path, then a hardware report.
& $exe fetch-model small
Check 'model download verified' ($LASTEXITCODE -eq 0)
$doctor = (& $exe doctor) -join "`n"
Check 'doctor reports models' ($doctor -match 'Whisper Small') ''

# Launch the app and check it stays up without showing the pill.
$proc = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 15
Check 'app is running' (-not $proc.HasExited)
$before = [Native]::VisibleTitles([uint32]$proc.Id)
Check 'pill hidden at rest' (-not ($before -contains 'local-stt recording')) ($before -join ', ')
Shot '1-idle'

# Hold Ctrl+Alt: the hook must fire and the pill must appear (no microphone on runners, so it shows an error state).
[Native]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
[Native]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 1200
$during = [Native]::VisibleTitles([uint32]$proc.Id)
Shot '2-holding-ctrl-alt'
[Native]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[Native]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
Check 'pill appears while the shortcut is held' ($during -contains 'local-stt recording') ($during -join ', ')
Start-Sleep -Seconds 4
Shot '3-after-release'
Check 'app still running after a dictation attempt' (-not $proc.HasExited)

# An update installs over the copy that is running right now; Windows must allow that.
uvx --from $Wheel local-stt install
Check 'reinstall over the running app (update path) exits 0' ($LASTEXITCODE -eq 0)
Check 'installed exe still present after reinstall' (Test-Path $exe)

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

# The troubleshooting report: events from the run above, nothing that names this user.
& $exe diagnostics
Check 'diagnostics exits 0' ($LASTEXITCODE -eq 0)
$report = Get-ChildItem (Join-Path $data 'diagnostics') -Filter '*.txt' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
Check 'diagnostic report saved' ($null -ne $report)
if ($report) {
    $text = Get-Content -Raw $report.FullName
    Check 'report holds the app log' ($text -match 'keyboard hook started') ''
    Check 'report leaves out the user name and home folder' (-not ($text.Contains($env:USERNAME) -or $text.Contains($env:USERPROFILE))) ''
}
Check 'error output is captured for a Start menu style launch' (Test-Path (Join-Path $data 'logs\stderr.log'))
Get-Content -ErrorAction SilentlyContinue (Join-Path $data 'logs\local-stt.log')

# Typed in a terminal, local-stt hands over to the installed copy (no console of its own) and returns at once.
$savedData = $env:LOCAL_STT_DATA_DIR
Remove-Item Env:LOCAL_STT_DATA_DIR, Env:LOCAL_STT_NO_PERMISSION_PROMPTS -ErrorAction SilentlyContinue
$clock = [Diagnostics.Stopwatch]::StartNew()
$said = (& $shim) -join ' '
Check 'a terminal launch returns at once' ($clock.Elapsed.TotalSeconds -lt 60) "$($clock.Elapsed.TotalSeconds) s"
Check 'a terminal launch says where the app runs' ($said -match 'system tray') $said
Start-Sleep -Seconds 8
$handed = @(Get-Process -Name 'local-stt' -ErrorAction SilentlyContinue)
Check 'the handed-off app keeps running on its own' ($handed.Count -ge 1)
$handed | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
$env:LOCAL_STT_DATA_DIR = $savedData
$env:LOCAL_STT_NO_PERMISSION_PROMPTS = '1'

# Uninstall from outside the installed folder, as a running exe cannot delete itself on Windows.
uvx --from $Wheel local-stt uninstall
Check 'uninstall exits 0' ($LASTEXITCODE -eq 0)
Check 'shortcut removed' (-not (Test-Path $lnk))
Check 'terminal command removed' (-not (Test-Path $shim))
Check 'program folder removed' (-not (Test-Path (Split-Path $exe)))

if ($failed -gt 0) { Write-Host "$failed FAILED"; exit 1 }
Write-Host 'ALL PASSED'
