#!/usr/bin/env bash
# Packages a Linux release build into dist/linux:
#
#   scrinium_<version>_amd64.deb        cargo-deb; Vulkan, OCR and SANE come from the system
#   scrinium_<version>_x86_64.AppImage  portable; Vulkan loader and graphics drivers come from the system
#
# Run after `cargo build --release` on Ubuntu 24.04: the AppImage then runs on systems with
# its glibc (2.39) or newer.
# Needs: cargo-deb, patchelf, curl.
set -euo pipefail
cd "$(dirname "$0")/../.."

bin=target/release/scrinium
if [[ ! -x $bin ]]; then
    echo "$bin is missing – first run: cargo build --release" >&2
    exit 1
fi

version=$(cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["packages"][0]["version"])')
dist=dist/linux
rm -rf "$dist"
mkdir -p "$dist"

# --- .deb -------------------------------------------------------------------
cargo deb --no-build --output "$dist/scrinium_${version}_amd64.deb"

# --- AppImage ---------------------------------------------------------------
# Fixed tool versions, verified by checksum. linuxdeploy collects the libraries, appimagetool packs
# the AppDir with the static type 2 runtime (users need no libfuse2).
tools=target/appimage-tools
mkdir -p "$tools"
fetch() { # name url sha256
    local file="$tools/$1"
    if [[ ! -f $file ]] || ! echo "$3  $file" | sha256sum --check --status; then
        curl -fsSL --retry 3 -o "$file" "$2"
        echo "$3  $file" | sha256sum --check --quiet
    fi
    chmod +x "$file"
}
fetch linuxdeploy \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
    c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
fetch appimagetool \
    https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage \
    ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
fetch runtime \
    https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64 \
    2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

appdir=target/appimage/Scrinium.AppDir
rm -rf "$appdir"
doc=$appdir/usr/share/doc/scrinium
mkdir -p "$appdir/usr/bin" "$doc"
install -m755 -s "$bin" "$appdir/usr/bin/scrinium"
cp README.md LICENSE NOTICE THIRD_PARTY.md config.example.toml "$doc/"

# The tools are AppImages themselves; CI runners have no FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
# Ubuntu's libraries are already stripped, and the strip of linuxdeploy is older than the toolchain.
export NO_STRIP=1
# The Vulkan loader must come from the system: there it finds the graphics drivers (Mesa, AMD, NVIDIA).
"$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$appdir/usr/bin/scrinium" \
    --desktop-file packaging/linux/scrinium.desktop \
    --icon-file assets/icon.png --icon-filename scrinium \
    --custom-apprun packaging/linux/AppRun \
    --exclude-library 'libvulkan.so*'
if find "$appdir/usr/lib" -name 'libvulkan.so*' 2>/dev/null | grep -q .; then
    echo "The AppImage contains the Vulkan loader – it must come from the system" >&2
    exit 1
fi

# Versions of the bundled libraries, so that their source code can be found (Ubuntu source packages).
find "$appdir/usr/lib" -name '*.so*' -type f -printf '%f\n' 2>/dev/null | sort |
    while read -r lib; do { dpkg -S "*/$lib" 2>/dev/null || true; } | cut -d: -f1; done |
    sort -u | xargs -r dpkg-query -W -f='${Package} ${Version}\n' >"$doc/bundled-libraries.txt"
cat "$doc/bundled-libraries.txt"

ARCH=x86_64 "$tools/appimagetool" --no-appstream --runtime-file "$tools/runtime" \
    "$appdir" "$dist/scrinium_${version}_x86_64.AppImage"
ls -la "$dist"
