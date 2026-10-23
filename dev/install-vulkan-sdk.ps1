# Installs the pinned LunarG Vulkan SDK on a CI runner so ggml builds its Vulkan backend; build tool only, never shipped.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$version = '1.4.357.0'
# From https://sdk.lunarg.com/sdk/sha/1.4.357.0/windows/vulkan_sdk.exe.txt, kept here so a changed download fails.
$sha256 = '81f474711e9042f4cd22b31b2f7a8870db2e428b21586fb43dd80150be97310d'
$root = "C:\VulkanSDK\$version"

$before = Test-Path "$env:SystemRoot\System32\vulkan-1.dll"
Write-Host "vulkan-1.dll in System32 before the SDK: $before"

$exe = Join-Path $env:RUNNER_TEMP "vulkansdk-windows-X64-$version.exe"
# LunarG sometimes drops the connection mid-download; the checksum below still guards every attempt.
Invoke-WebRequest -Uri "https://sdk.lunarg.com/sdk/download/$version/windows/vulkansdk-windows-X64-$version.exe" -OutFile $exe -MaximumRetryCount 4 -RetryIntervalSec 15
$got = (Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash.ToLowerInvariant()
if ($got -ne $sha256) { throw "Vulkan SDK checksum mismatch: expected $sha256, got $got" }

$p = Start-Process -FilePath $exe -ArgumentList @('--root', $root, '--accept-licenses', '--default-answer', '--confirm-command', 'install') -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "Vulkan SDK install failed with exit code $($p.ExitCode)" }
Remove-Item -LiteralPath $exe

$after = Test-Path "$env:SystemRoot\System32\vulkan-1.dll"
Write-Host "vulkan-1.dll in System32 after the SDK: $after"

Add-Content -LiteralPath $env:GITHUB_ENV -Value "VULKAN_SDK=$root"
# ggml's nested Vulkan shader build passes the 260-character path limit under the default target folder.
Add-Content -LiteralPath $env:GITHUB_ENV -Value "CARGO_TARGET_DIR=C:\t"
Add-Content -LiteralPath $env:GITHUB_PATH -Value "$root\Bin"
