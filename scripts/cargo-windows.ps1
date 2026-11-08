<#
.SYNOPSIS
    Runs cargo for Scrinium on Windows with short paths and a fixed environment.

.DESCRIPTION
    The Vulkan build of llama.cpp creates paths of up to about 235 characters under target\
    (CMake subproject vulkan-shaders-gen). If the project sits deep, e.g. under
    C:\Users\<name>\Projects\Scrinium, they exceed the limit of 260 characters, which MSBuild
    keeps even with long paths enabled. The build then fails.

    This script
      1. maps a drive as a short path to the project folder if the project path is long
         (subst, default S:, change it with SCRINIUM_BUILD_DRIVE) and runs cargo from there,
      2. puts CMake from the Visual Studio Build Tools and LLVM for bindgen on the PATH.

    Important: CMake remembers the path from which llama.cpp was first built. So always build
    through this script. Running cargo directly from the long path fails as soon as llama.cpp
    is configured again ("CMakeCache.txt directory ... is different").

    The results are under target\ in the project folder as usual. The subst mapping disappears
    when you sign out; the script creates it again on the next run.

.EXAMPLE
    pwsh scripts/cargo-windows.ps1 build --release
    pwsh scripts/cargo-windows.ps1 test --release
    pwsh scripts/cargo-windows.ps1 clippy --release --all-targets -- -D warnings
#>
# No param block: this way all arguments reach cargo unchanged, including "--" and "-D".
$cargoArgs = @($args)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path.TrimEnd('\')

# Up to this length of the project path, the deepest llama.cpp paths stay below 260 characters.
$maxRepoPath = 20
$workDir = $repo
if ($repo.Length -gt $maxRepoPath) {
    $drive = if ($env:SCRINIUM_BUILD_DRIVE) { $env:SCRINIUM_BUILD_DRIVE.TrimEnd('\') } else { 'S:' }
    $mapping = subst | Where-Object { $_ -like "$drive\*" } | Select-Object -First 1
    if ($mapping) {
        $target = ($mapping -split '=>', 2)[1].Trim().TrimEnd('\')
        if ($target -ne $repo) {
            throw "$drive already points to $target. Choose a free drive with SCRINIUM_BUILD_DRIVE, e.g. `$env:SCRINIUM_BUILD_DRIVE = 'T:'."
        }
    } elseif (Test-Path "$drive\") {
        throw "$drive is already in use. Choose a free drive with SCRINIUM_BUILD_DRIVE, e.g. `$env:SCRINIUM_BUILD_DRIVE = 'T:'."
    } else {
        subst $drive $repo
        if ($LASTEXITCODE -ne 0) { throw "subst $drive $repo failed." }
        Write-Host "Short path created: $drive -> $repo"
    }
    $workDir = "$drive\"
}

# CMake from the Visual Studio Build Tools (if not on the PATH yet), LLVM for bindgen.
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $vs = & $vswhere -latest -products * -property installationPath
        $cmake = Join-Path $vs 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
        if (Test-Path $cmake) { $env:PATH = "$cmake;$env:PATH" }
    }
}
$llvm = Join-Path $env:ProgramFiles 'LLVM\bin'
if (Test-Path $llvm) {
    if (-not ($env:PATH -split ';' | Where-Object { $_ -eq $llvm })) { $env:PATH = "$llvm;$env:PATH" }
    if (-not $env:LIBCLANG_PATH) { $env:LIBCLANG_PATH = $llvm }
}

Push-Location $workDir
try {
    & cargo @cargoArgs
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
