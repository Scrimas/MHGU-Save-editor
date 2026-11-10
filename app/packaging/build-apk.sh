#!/usr/bin/env bash
# The Android APK, with the SDK's own tools and no Gradle (as MHGU Recomp's android/apk.mk):
# the editor as libmhgu_editor.so for arm64 (cargo, the NDK's clang), MainActivity.java
# (javac + d8 -> classes.dex), the manifest and resources (aapt2), the library stored
# uncompressed and 16 KB aligned (zipalign: Android maps it straight from the APK), signed
# (apksigner).
#
#   build-apk.sh               debug build: app/target/apk/mhgu-editor-debug.apk, signed with
#                              the SDK's debug key (~/.android/debug.keystore, made if missing)
#   build-apk.sh --release     release build: app/dist/MHGU-Save-Editor-<version>-arm64.apk,
#                              signed with the release key:
#                                MHGU_APK_KEYSTORE       path of the keystore (PKCS12)
#                                MHGU_APK_KEYSTORE_PASS  its password (also the key's)
#   build-apk.sh --install     the debug build, then adb install -r
#   build-apk.sh --check       clippy for the Android target (the code under src/android
#                              builds only there), warnings as errors, no APK (CI)
#   build-apk.sh --emulator    debug build for the x86_64 emulator with Slint's MCP server
#                              on port 9315 (SLINT_MCP_PORT to change it; element ids for it
#                              with SLINT_EMIT_DEBUG_INFO=1), so the running
#                              app can be inspected and driven (packaging/emulator.sh):
#                              app/target/apk/mhgu-editor-emulator.apk
#
# Needs: rustup with aarch64-linux-android (x86_64-linux-android for --emulator), the
# Android SDK (ANDROID_HOME, default /opt/android-sdk) with build-tools, a platform
# (android.jar) and the NDK, a JDK (javac, keytool), python3. The game-asset pack as for
# the desktop builds (app/assets/gen).
set -euo pipefail
release=false
install=false
emulator=false
check=false
case "${1:-}" in
    --release) release=true ;;
    --install) install=true ;;
    --emulator) emulator=true ;;
    --check) check=true ;;
    "") ;;
    *) echo "usage: $0 [--release|--install|--emulator|--check]" >&2; exit 2 ;;
esac
triple=aarch64-linux-android
abi=arm64-v8a
if $emulator; then
    triple=x86_64-linux-android
    abi=x86_64
fi

here=$(cd "$(dirname "$0")" && pwd)
app=$(dirname "$here")
cd "$app"
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
# versionCode: Z.X.Y as ZXXYY (3.3.0 -> 30300), growing with every release
IFS=. read -r vz vx vy <<< "$ver"
code=$((vz * 10000 + vx * 100 + vy))
api=29 # minSdk: the same as MHGU Recomp

sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/opt/android-sdk}}
ndk=${ANDROID_NDK_HOME:-$(ls -d "$sdk"/ndk/* 2>/dev/null | sort -V | tail -1)}
tools=$(ls -d "$sdk"/build-tools/* | sort -V | tail -1)
jar=$(ls "$sdk"/platforms/android-*/android.jar | sort -V | tail -1)
target_api=$(basename "$(dirname "$jar")" | sed 's/android-//')
tc="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
[ -x "$tc/$triple$api-clang" ] || { echo "no NDK clang in $tc (ANDROID_HOME=$sdk)" >&2; exit 1; }

export ANDROID_HOME="$sdk" ANDROID_SDK_ROOT="$sdk" ANDROID_NDK_HOME="$ndk" ANDROID_NDK_ROOT="$ndk" ANDROID_NDK="$ndk"
t=${triple//-/_}
export "CARGO_TARGET_${t^^}_LINKER=$tc/$triple$api-clang"
export "CC_$t=$tc/$triple$api-clang"
export "CXX_$t=$tc/$triple$api-clang++"
export "AR_$t=$tc/llvm-ar"

if $check; then
    exec cargo clippy -p mhgu-editor --lib --target "$triple" -- -D warnings
fi

if [ ! -f assets/gen/names.json ]; then
    echo "   (no asset pack in app/assets/gen: the APK shows placeholders)"
fi

echo "== libmhgu_editor.so ($($release && echo release || echo debug))"
profile=debug
flags=()
if $release; then
    profile=release
    flags+=(--release)
else
    # no debug info for the editor crate itself: Slint's generated code for the phone
    # interface is large, and with it rustc needed more than 6 GB (it is stripped from
    # the APK anyway); the other crates keep theirs, so nothing else rebuilds
    flags+=(--config 'profile.dev.package.mhgu-editor.debug=0')
    # nor incremental state: for one module this size rustc's caches take gigabytes more
    # (the dependencies are never incremental, so they do not rebuild)
    export CARGO_INCREMENTAL=0
fi
if $emulator; then
    # the MCP server; the port is built in (android_main sets SLINT_MCP_PORT from it).
    # Element ids and source places for it only with SLINT_EMIT_DEBUG_INFO=1: they make
    # the generated code large enough that rustc goes past the 6 GB the local builds get;
    # the MCP tools find elements by their accessible labels without them
    flags+=(--features slint/mcp)
    export SLINT_EMIT_DEBUG_INFO="${SLINT_EMIT_DEBUG_INFO:-0}" SLINT_MCP_PORT="${SLINT_MCP_PORT:-9315}"
else
    # a port left in the environment must not reach a phone build
    unset SLINT_MCP_PORT SLINT_EMIT_DEBUG_INFO
fi
cargo rustc -p mhgu-editor --lib --crate-type cdylib --target "$triple" "${flags[@]}"
so="target/$triple/$profile/libmhgu_editor.so"

work="target/apk/$profile-$abi"
rm -rf "$work"
mkdir -p "$work/classes" "$work/dex" "$work/lib/$abi"

echo "== classes.dex"
javac -nowarn -Xlint:-options -source 8 -target 8 -encoding UTF-8 -bootclasspath "$jar" -d "$work/classes" \
    $(find packaging/android/java -name '*.java')
"$tools/d8" --release --min-api "$api" --lib "$jar" --output "$work/dex" $(find "$work/classes" -name '*.class')

echo "== resources and manifest"
"$tools/aapt2" compile --dir packaging/android/res -o "$work/res.zip"
"$tools/aapt2" link -o "$work/base.apk" --manifest packaging/android/AndroidManifest.xml -I "$jar" "$work/res.zip" \
    --min-sdk-version "$api" --target-sdk-version "$target_api" --version-code "$code" --version-name "$ver"

echo "== libraries"
"$tc/llvm-strip" --strip-debug -o "$work/lib/$abi/libmhgu_editor.so" "$so"
# Skia's C++ runtime, when the library loads it from the APK rather than having it linked in
if "$tc/llvm-readelf" -d "$so" | grep -q 'libc++_shared.so'; then
    cp "$ndk/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/$triple/libc++_shared.so" "$work/lib/$abi/"
fi

# base.apk + classes.dex + the libraries, stored (not compressed)
python3 - "$work/base.apk" "$work/unaligned.apk" "$work/dex/classes.dex" "$work/lib" <<'EOF'
import os, sys, zipfile
base, out, dex, libdir = sys.argv[1:5]
with zipfile.ZipFile(base) as src, zipfile.ZipFile(out, 'w') as dst:
    for item in src.infolist():
        dst.writestr(item, src.read(item.filename))
    dst.write(dex, 'classes.dex', compress_type=zipfile.ZIP_DEFLATED)
    for abi in sorted(os.listdir(libdir)):
        for name in sorted(os.listdir(os.path.join(libdir, abi))):
            dst.write(os.path.join(libdir, abi, name), f'lib/{abi}/{name}', compress_type=zipfile.ZIP_STORED)
EOF
"$tools/zipalign" -f -P 16 4 "$work/unaligned.apk" "$work/aligned.apk"

echo "== signing"
if $release; then
    : "${MHGU_APK_KEYSTORE:?the release key: set MHGU_APK_KEYSTORE (and MHGU_APK_KEYSTORE_PASS)}"
    : "${MHGU_APK_KEYSTORE_PASS:?set MHGU_APK_KEYSTORE_PASS}"
    mkdir -p dist
    out="dist/MHGU-Save-Editor-$ver-arm64.apk"
    # (no v4 signature: it only serves adb's incremental install)
    MHGU_APK_KEYSTORE_PASS="$MHGU_APK_KEYSTORE_PASS" "$tools/apksigner" sign --ks "$MHGU_APK_KEYSTORE" \
        --ks-pass env:MHGU_APK_KEYSTORE_PASS --v4-signing-enabled false --out "$out" "$work/aligned.apk"
else
    key="$HOME/.android/debug.keystore"
    if [ ! -f "$key" ]; then
        mkdir -p "$(dirname "$key")"
        keytool -genkeypair -keystore "$key" -storepass android -alias androiddebugkey -keypass android \
            -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
    fi
    out="target/apk/mhgu-editor-$($emulator && echo emulator || echo debug).apk"
    "$tools/apksigner" sign --ks "$key" --ks-pass pass:android --key-pass pass:android --v4-signing-enabled false \
        --out "$out" "$work/aligned.apk"
fi
echo "apk: $app/$out ($(du -h "$out" | cut -f1), version $ver, code $code)"

if $install; then
    "$sdk/platform-tools/adb" install -r "$out" 2>/dev/null || adb install -r "$out"
fi
