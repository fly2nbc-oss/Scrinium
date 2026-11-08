#Requires -Version 7
<#
.SYNOPSIS
Installs, starts and uninstalls the NSIS installer silently (CI only).

.DESCRIPTION
Checks that the per-user installation lands in %LOCALAPPDATA%\Scrinium with all files of the folder
dist\windows\Scrinium, gets an entry under "Apps" and that the installed exe starts. The uninstaller must
remove the program but keep the settings (%APPDATA%\scrinium\Scrinium) and the cache
(%LOCALAPPDATA%\scrinium\Scrinium: models, language data) – the cache is in the same folder as the
program.

Do not run it on a computer where Scrinium is installed or used: the script installs over that copy and
uninstalls it.
#>
param(
    [Parameter(Mandatory)][string]$Setup
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$dir = "$env:LOCALAPPDATA\Scrinium"
$stage = "$PSScriptRoot\..\..\dist\windows\Scrinium"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Scrinium"
$version = (cargo metadata --no-deps --format-version 1 --manifest-path "$PSScriptRoot\..\..\Cargo.toml" |
    ConvertFrom-Json).packages[0].version

# Stand for the users' settings and cache, which must survive the uninstall.
$markers = "$env:APPDATA\scrinium\Scrinium\config\installer-test", "$env:LOCALAPPDATA\scrinium\Scrinium\cache\installer-test"
foreach ($marker in $markers) { New-Item -ItemType File -Force $marker | Out-Null }

$p = Start-Process $Setup -ArgumentList "/S" -Wait -PassThru
if ($p.ExitCode) { throw "installer exited with $($p.ExitCode)" }

$expected = Get-ChildItem $stage -Recurse -File |
    ForEach-Object { $_.FullName.Substring((Resolve-Path $stage).Path.Length + 1) }
$absent = $expected | Where-Object { -not (Test-Path "$dir\$_") }
if ($absent) { throw "not installed: $($absent -join ', ')" }
if (-not (Test-Path "$dir\uninstall.exe")) { throw "no uninstall.exe" }
if (-not (Test-Path $uninstallKey)) { throw "no entry under Apps" }
Write-Host "installed: $($expected.Count) files in $dir"

# The installed exe starts. It is a GUI program; the output comes through the redirection.
$out = "$env:RUNNER_TEMP\scrinium-help.txt"
$run = Start-Process "$dir\scrinium.exe" -ArgumentList "--help" -Wait -PassThru `
    -RedirectStandardOutput $out -RedirectStandardError "$out.err"
Get-Content $out, "$out.err" -ErrorAction SilentlyContinue | Select-Object -First 5
if ($run.ExitCode) { throw "the installed exe exited with $($run.ExitCode)" }
if (-not (Select-String -Path $out -Pattern "^Scrinium $([regex]::Escape($version))$" -Quiet)) {
    throw "the installed exe does not report Scrinium $version"
}

# `_?=` runs the uninstaller in place and waits for it (otherwise it copies itself to %TEMP% and returns
# at once); then it cannot delete itself, so uninstall.exe stays.
$p = Start-Process "$dir\uninstall.exe" -ArgumentList "/S", "_?=$dir" -Wait -PassThru
if ($p.ExitCode) { throw "uninstall exited with $($p.ExitCode)" }
$left = $expected | Where-Object { Test-Path "$dir\$_" }
if ($left) { throw "left after uninstall: $($left -join ', ')" }
if (Test-Path $uninstallKey) { throw "entry under Apps was not removed" }
$lost = $markers | Where-Object { -not (Test-Path $_) }
if ($lost) { throw "the uninstall deleted user data: $($lost -join ', ')" }
Remove-Item -Force ($markers + "$dir\uninstall.exe")
Write-Host "uninstalled; settings and cache were kept"
