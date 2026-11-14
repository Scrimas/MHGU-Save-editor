#!/usr/bin/env bash
# Build the release binaries into app/dist/:
#   MHGU-Save-Editor-<version>-x86_64.AppImage   Linux, single file
#   MHGU-Save-Editor-<version>-x86_64.exe        Windows 10/11, single file, nothing installed
# MHGU_ARCH=arm64: MHGU-Save-Editor-<version>-arm64.AppImage only, built on an ARM64 Linux (the
# binary links the build machine's libudev, so no cross build; the ARM64 .exe is built on
# Windows, see .github/workflows/release.yml).
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
arch=${MHGU_ARCH:-x86_64}
case "$arch" in
    x86_64) triple=x86_64-unknown-linux-gnu; appimage_arch=x86_64 ;;
    arm64) triple=aarch64-unknown-linux-gnu; appimage_arch=aarch64; appimage_only=true ;;
    *) echo "MHGU_ARCH must be x86_64 or arm64, not $arch" >&2; exit 1 ;;
esac
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
    cargo zigbuild --release -p mhgu-editor --target "$triple.2.28"
    bin=target/$triple/release/mhgu-editor
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

out="$dist/MHGU-Save-Editor-$ver-$arch.AppImage"
# zstd at level 19 with 1 MiB blocks: a smaller file, unpacked as fast as the default
if command -v appimagetool >/dev/null; then
    ARCH=$appimage_arch appimagetool --no-appstream --comp zstd \
        --mksquashfs-opt -Xcompression-level --mksquashfs-opt 19 --mksquashfs-opt -b --mksquashfs-opt 1M "$appdir" "$out"
elif [ -n "${APPIMAGE_RUNTIME:-}" ]; then
    sq=$(mktemp)
    mksquashfs "$appdir" "$sq" -root-owned -noappend -comp zstd -Xcompression-level 19 -b 1M -quiet -no-progress
    cat "$APPIMAGE_RUNTIME" "$sq" > "$out"
    chmod 755 "$out"
    rm -f "$sq"
else
    echo "no appimagetool and no APPIMAGE_RUNTIME: AppDir left at $appdir" >&2
    exit 1
fi
rm -rf "$(dirname "$appdir")"
# SHA256SUMS-<version> for the x86_64 files, SHA256SUMS-arm64-<version> for the other
sums="SHA256SUMS-$ver"
[ "$arch" = x86_64 ] || sums="SHA256SUMS-$arch-$ver"
(cd "$dist" && sha256sum *"$ver-$arch".* > "$sums")
ls -la "$dist"
