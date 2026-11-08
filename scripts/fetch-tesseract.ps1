# Downloads the Tesseract DLL from the release named in packaging/tesseract/RELEASE, checks the zip against
# packaging/tesseract/SHA256SUMS and puts DLL and licenses into assets/tesseract/. build.rs then embeds the DLL in
# scrinium.exe. Needs the GitHub CLI (gh) with access to the repository; in the CI, GH_TOKEN is enough.
#
#   pwsh scripts/fetch-tesseract.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$tag = (Get-Content (Join-Path $root "packaging/tesseract/RELEASE") -Raw).Trim()
$line = (Get-Content (Join-Path $root "packaging/tesseract/SHA256SUMS") | Where-Object { $_.Trim() } | Select-Object -First 1).Trim()
$expected, $name = $line -split "\s+", 2
$name = $name.TrimStart("*")

$work = Join-Path ([System.IO.Path]::GetTempPath()) "scrinium-tesseract-$PID"
New-Item -ItemType Directory -Force $work | Out-Null
try {
    gh release download $tag --repo fly2nbc-oss/Scrinium --pattern $name --dir $work --clobber
    if ($LASTEXITCODE) { throw "Could not download release $tag" }
    $zip = Join-Path $work $name
    $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) { throw "$name has the checksum $actual instead of $expected" }
    $dest = Join-Path $root "assets/tesseract"
    Remove-Item -Recurse -Force $dest -ErrorAction SilentlyContinue
    Expand-Archive $zip -DestinationPath $dest
    # New modification time, so that cargo runs the build script again and embeds the DLL again.
    (Get-Item (Join-Path $dest "tesseract55.dll")).LastWriteTime = Get-Date
    "Tesseract from $tag in $dest"
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
