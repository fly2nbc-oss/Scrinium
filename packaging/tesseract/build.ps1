# Builds Tesseract for Windows as a single DLL (tesseract55.dll) and packs it with the licenses into a zip.
#
# Variant "B2": all dependencies (Leptonica, zlib, libpng, libjpeg-turbo, libtiff, libwebp, giflib,
# OpenJPEG) linked statically, without curl, libarchive, the legacy engine and the viewer window. Scrinium
# embeds the DLL in scrinium.exe. The recipe (ports/, triplets/, vcpkg.json) is an excerpt of vcpkg with
# these settings; vcpkg itself is pinned to a fixed commit.
#
#   pwsh packaging/tesseract/build.ps1 [-Out dist] [-Work <short path>]
#
# On Windows, -Work must not be too deep (MSBuild path limit); in the CI it is $env:RUNNER_TEMP.
param(
    [string]$Out = "dist",
    [string]$Work = $(if ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP "tess" } else { "C:\tess-build" })
)
$ErrorActionPreference = "Stop"

$VcpkgCommit = "e456309491fd875487bf02b8a0dca76c1b1b00cc"  # vcpkg of 2026-10-10: Tesseract 5.5.3, Leptonica 1.87.0
$Version = "5.5.3"
$Licenses = "tesseract", "leptonica", "zlib", "libpng", "libjpeg-turbo", "tiff", "libwebp", "giflib", "openjpeg"

$here = $PSScriptRoot
New-Item -ItemType Directory -Force $Work, $Out | Out-Null
$vcpkg = Join-Path $Work "vcpkg"
if (-not (Test-Path (Join-Path $vcpkg ".git"))) {
    git init -q $vcpkg
    git -C $vcpkg fetch -q --depth 1 https://github.com/microsoft/vcpkg $VcpkgCommit
    git -C $vcpkg checkout -q FETCH_HEAD
}
& (Join-Path $vcpkg "bootstrap-vcpkg.bat") -disableMetrics
if ($LASTEXITCODE) { throw "could not set up vcpkg" }

$installed = Join-Path $Work "installed"
& (Join-Path $vcpkg "vcpkg.exe") install `
    "--x-manifest-root=$here" "--x-install-root=$installed" `
    "--overlay-ports=$(Join-Path $here 'ports')" "--overlay-triplets=$(Join-Path $here 'triplets')" `
    --triplet=x64-windows-tessdll --host-triplet=x64-windows --disable-metrics
if ($LASTEXITCODE) { throw "vcpkg install failed" }

$root = Join-Path $installed "x64-windows-tessdll"
$pkg = Join-Path $Work "pkg"
Remove-Item -Recurse -Force $pkg -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $pkg "licenses") | Out-Null
Copy-Item (Join-Path $root "bin\tesseract55.dll") $pkg
foreach ($port in $Licenses) {
    Copy-Item (Join-Path $root "share\$port\copyright") (Join-Path $pkg "licenses\$port.txt")
}

$zip = Join-Path $Out "tesseract-$Version-windows-x64.zip"
Compress-Archive -Path (Join-Path $pkg "*") -DestinationPath $zip -Force
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
"$hash  $(Split-Path $zip -Leaf)" | Set-Content (Join-Path $Out "SHA256SUMS") -Encoding ascii
Get-ChildItem $pkg -Recurse -File | Select-Object @{ n = "File"; e = { $_.FullName.Substring($pkg.Length + 1) } }, Length
"$hash  $(Split-Path $zip -Leaf)"
