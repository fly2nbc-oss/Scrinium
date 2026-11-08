#Requires -Version 7
<#
.SYNOPSIS
Packages a Windows release build into dist\.

.DESCRIPTION
Run after `cargo build --release` (with the embedded Tesseract DLL, see scripts/fetch-tesseract.ps1). Result:

  dist\scrinium_<version>_x64-portable.zip  the folder Scrinium\ to unpack and start
  dist\scrinium_<version>_x64-setup.exe     NSIS installer (cargo-packager, settings in Cargo.toml)
  dist\windows\Scrinium\                    the folder that the installer installs and the ZIP contains

Besides the exe, docs and licenses, the folder holds the VC++ runtime that scrinium.exe and the embedded
Tesseract DLL import (app-local from the Redist folder of Visual Studio). So the installed and the portable
version also run without the VC++ Redistributable. Then every import is checked: it must be in the folder or
belong to Windows.
#>
param(
    # cargo output folder with scrinium.exe.
    [string]$Target = "target\release",
    # Redist folder Microsoft.VC14x.CRT; found through vswhere if not given.
    [string]$CrtDir,
    # Portable ZIP and folder only, no installer.
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location (Resolve-Path "$PSScriptRoot\..\..")

$version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages[0].version
$stage = "dist\windows\Scrinium"
if (Test-Path dist) { Remove-Item -Recurse -Force dist }
New-Item -ItemType Directory -Force $stage | Out-Null

# --- exe, docs, licenses ---------------------------------------------------------------
$exe = Join-Path $Target "scrinium.exe"
if (-not (Test-Path $exe)) { throw "$exe is missing – first run: cargo build --release" }
$tesseract = "assets\tesseract\tesseract55.dll"
if (-not (Test-Path $tesseract)) { throw "$tesseract is missing – first run: scripts\fetch-tesseract.ps1" }
Copy-Item $exe, README.md, CHANGELOG.md, LICENSE, NOTICE, THIRD_PARTY.md, config.example.toml $stage
Copy-Item assets\tesseract\licenses "$stage\licenses-tesseract" -Recurse

# --- Visual Studio tools ---------------------------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw "no Visual Studio with C++ tools found" }
$dumpbin = Get-ChildItem "$vs\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" |
    Sort-Object { [version]$_.Directory.Parent.Parent.Parent.Name } | Select-Object -Last 1
if (-not $dumpbin) { throw "dumpbin.exe not found under $vs" }
if (-not $CrtDir) {
    $crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Parent.Parent.Name -match '^\d+(\.\d+)+$' } |
        Sort-Object { [version]$_.Parent.Parent.Name }, Name | Select-Object -Last 1
    if (-not $crt) { throw "no VC++ Redist folder under $vs\VC\Redist\MSVC – pass -CrtDir" }
    $CrtDir = $crt.FullName
}
# llama.cpp computes on the processor with OpenMP: vcomp140.dll is in the sibling folder Microsoft.VC14x.OpenMP.
$openMpDir = Join-Path (Split-Path $CrtDir) ((Split-Path $CrtDir -Leaf) -replace '\.CRT$', '.OpenMP')
$runtimeDirs = @($CrtDir, $openMpDir) | Where-Object { Test-Path $_ }
Write-Host "dumpbin: $($dumpbin.FullName)"
Write-Host "VC++ runtime: $($runtimeDirs -join ', ')"

function Get-Imports([string]$File) {
    $lines = & $dumpbin.FullName /nologo /dependents $File
    if ($LASTEXITCODE) { throw "dumpbin failed on $File" }
    $lines | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^[\w.-]+\.dll$' }
}

# The embedded Tesseract DLL is unpacked into the cache at run time; Windows then looks for its imports in the
# folder of the exe. That is why it counts here.
function Get-Binaries {
    @(Get-ChildItem $stage -File | Where-Object Extension -in ".exe", ".dll") + @(Get-Item $tesseract)
}

# --- VC++ runtime app-local ------------------------------------------------------------
# Copy what is imported until nothing new is added (msvcp140 itself imports vcruntime140).
do {
    $added = $false
    foreach ($binary in Get-Binaries) {
        foreach ($dll in Get-Imports $binary.FullName) {
            if (Test-Path "$stage\$dll") { continue }
            # Under the file name in the Redist folder, not that of the import (VCOMP140.DLL): the pattern *.dll of
            # the resources in Cargo.toml is case-sensitive.
            $source = $runtimeDirs | ForEach-Object { Get-ChildItem $_ -Filter $dll -File } | Select-Object -First 1
            if ($source) {
                Copy-Item $source.FullName $stage
                $added = $true
            }
        }
    }
} while ($added)

# --- Every import is in the folder or belongs to Windows -------------------------------
# The VC++ runtime is installed on most computers (and the CI runners), but does not belong to Windows;
# finding it in System32 does not count. vulkan-1.dll comes with the graphics driver (AMD, NVIDIA, Intel).
$vcRuntime = '^(msvcp|vcruntime|concrt|vccorlib|vcomp)\d'
$missing = foreach ($binary in Get-Binaries) {
    foreach ($dll in Get-Imports $binary.FullName) {
        if (Test-Path "$stage\$dll") { continue }
        if ($dll -match '^(api|ext)-ms-win-') { continue } # API sets, resolved by Windows
        if ($dll -eq 'vulkan-1.dll') { continue }
        if ($dll -notmatch $vcRuntime -and (Test-Path "$env:SystemRoot\System32\$dll")) { continue }
        "$($binary.Name) -> $dll"
    }
}
if ($missing) {
    throw "Imports that are neither in the package nor part of Windows:`n  $($missing -join "`n  ")"
}
Get-ChildItem $stage -Recurse -File | ForEach-Object {
    "{0,12:N0}  {1}" -f $_.Length, $_.FullName.Substring((Resolve-Path $stage).Path.Length + 1)
}

# --- Portable ZIP and installer --------------------------------------------------------
# The ZIP contains the folder itself, so it unpacks to Scrinium\scrinium.exe.
Compress-Archive -Path $stage -DestinationPath "dist\scrinium_${version}_x64-portable.zip" -CompressionLevel Optimal
if (-not $NoInstaller) {
    # Folders, resources, icon and languages are in Cargo.toml.
    cargo packager --release --formats nsis
    if ($LASTEXITCODE) { throw "cargo packager failed" }
    $setup = Get-ChildItem "dist\windows\*_${version}_x64-setup.exe" | Select-Object -First 1
    if (-not $setup) { throw "cargo packager put no installer into dist\windows" }
    Move-Item $setup.FullName "dist\scrinium_${version}_x64-setup.exe"
}
Get-ChildItem dist -File | ForEach-Object {
    "{0}  {1,12:N0}  {2}" -f (Get-FileHash $_.FullName SHA256).Hash.ToLower(), $_.Length, $_.Name
}
