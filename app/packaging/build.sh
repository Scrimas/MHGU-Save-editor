#!/usr/bin/env bash
# Build the release binaries into app/dist/:
#   MHGU-Save-Editor-<version>-x86_64.AppImage   Linux, single file
#   MHGU-Save-Editor-<version>-x86_64.exe        Windows 10/11, single file, nothing installed
#
# Needs: rustup with x86_64-pc-windows-gnu, mingw-w64-gcc, squashfs-tools.
# Optional:
#   cargo-zigbuild + zig   Linux binary linked against glibc 2.28 (runs on distros from 2019
#                          on); without it the binary needs this machine's glibc or newer
#   APPIMAGE_RUNTIME=path  AppImage type 2 runtime (github.com/AppImage/type2-runtime);
#                          or appimagetool in PATH
# The game-asset pack is rebuilt from scratch/base_romfs.bin when it is missing.
# --appimage: the AppImage only (no Windows toolchain needed).
set -euo pipefail
appimage_only=false
[ "${1:-}" = "--appimage" ] && appimage_only=true
here=$(cd "$(dirname "$0")" && pwd)
app=$(dirname "$here")
repo=$(dirname "$app")
cd "$app"
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
dist="$app/dist"
mkdir -p "$dist"

if [ ! -f assets/gen/names.json ]; then
    echo "== asset pack"
    python3 "$repo/tools/build_assets.py"
fi

if ! $appimage_only; then
    echo "== Windows"
    cargo build --release -p mhgu-editor --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/mhgu-editor.exe "$dist/MHGU-Save-Editor-$ver-x86_64.exe"
fi

echo "== Linux"
# cargo finds subcommands in ~/.cargo/bin even when it is not on PATH
if cargo zigbuild --help >/dev/null 2>&1 && command -v zig >/dev/null; then
    cargo zigbuild --release -p mhgu-editor --target x86_64-unknown-linux-gnu.2.28
    bin=target/x86_64-unknown-linux-gnu/release/mhgu-editor
else
    echo "   (cargo-zigbuild not found: linking against this machine's glibc)"
    cargo build --release -p mhgu-editor
    bin=target/release/mhgu-editor
fi
echo "   needs glibc $(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1)"

appdir=$(mktemp -d)/AppDir
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/256x256/apps" \
         "$appdir/usr/share/icons/hicolor/scalable/apps"
install -m755 "$bin" "$appdir/usr/bin/mhgu-editor"
install -m644 packaging/mhgu-save-editor.desktop "$appdir/usr/share/applications/"
install -m644 packaging/mhgu-save-editor.desktop "$appdir/"
install -m644 gui/assets/icon-256.png "$appdir/usr/share/icons/hicolor/256x256/apps/mhgu-save-editor.png"
install -m644 gui/assets/icon.svg "$appdir/usr/share/icons/hicolor/scalable/apps/mhgu-save-editor.svg"
install -m644 gui/assets/icon-256.png "$appdir/mhgu-save-editor.png"
ln -s mhgu-save-editor.png "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/mhgu-editor" "$@"
EOF
chmod 755 "$appdir/AppRun"

out="$dist/MHGU-Save-Editor-$ver-x86_64.AppImage"
if command -v appimagetool >/dev/null; then
    ARCH=x86_64 appimagetool --no-appstream "$appdir" "$out"
elif [ -n "${APPIMAGE_RUNTIME:-}" ]; then
    sq=$(mktemp)
    mksquashfs "$appdir" "$sq" -root-owned -noappend -comp zstd -quiet -no-progress
    cat "$APPIMAGE_RUNTIME" "$sq" > "$out"
    chmod 755 "$out"
    rm -f "$sq"
else
    echo "no appimagetool and no APPIMAGE_RUNTIME: AppDir left at $appdir" >&2
    exit 1
fi
rm -rf "$(dirname "$appdir")"
(cd "$dist" && sha256sum *"$ver"-x86_64.* > "SHA256SUMS-$ver")
ls -la "$dist"
