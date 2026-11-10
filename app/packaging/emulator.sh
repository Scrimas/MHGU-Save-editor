#!/usr/bin/env bash
# The Android emulator, to test the app on this computer instead of a phone: an x86_64
# Android 36 AVD (the SDK's google_apis image, KVM), capped so the desktop stays usable,
# its window on an empty workspace (Hyprland: workspace 10, or EMULATOR_WORKSPACE); and the
# emulator build of the app (build-apk.sh --emulator), whose Slint MCP server is forwarded
# to 127.0.0.1:9315 (endpoint /mcp) to inspect and drive it.
#
#   emulator.sh start            create the AVD if missing, boot it, wait for Android; the
#                                window opens on workspace 10, not the one in use
#   emulator.sh headless         the same without a window
#   emulator.sh stop             shut it down (the next start resumes from a snapshot)
#   emulator.sh build            build-apk.sh --emulator, capped like the other local builds;
#                                a running emulator is stopped first (the two don't fit in
#                                memory together)
#   emulator.sh install          install the emulator build, forward the MCP port, start the app
#   emulator.sh run              build, start the emulator again, install
#   emulator.sh restart          stop the app and start it again: an activity Android
#                                recreated in the same process keeps the first MCP server's
#                                port with nothing answering on it
#   emulator.sh save DIR         copy a save folder (a save_0, or one holding it) to
#                                /sdcard/Download/<its name> for the folder picker
#   emulator.sh size phone       1080x2340 at 450 dpi: 384 dp wide, a Galaxy S24 Ultra (default)
#   emulator.sh size fold        1856x2160 at 420 dpi: 707 dp, a Galaxy Z Fold's big screen
#   emulator.sh size tablet      2560x1600 at 320 dpi: 1280 dp, a landscape tablet
#   emulator.sh dark|light       the system theme
#   emulator.sh shot FILE.png    the whole screen, system bars included
#   emulator.sh log              the app's log (this run)
#   emulator.sh mcp TOOL [JSON]  call a tool of the app's MCP server, print the answer
#   emulator.sh mcpshot FILE.png the app's window as Slint renders it (MCP take_screenshot)
#
# Needs: the Android SDK with the emulator and system-images;android-36;google_apis;x86_64
# (ANDROID_HOME, default /opt/android-sdk), /dev/kvm, python3; build also needs what
# build-apk.sh needs, with rustup's x86_64-linux-android.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
app=$(dirname "$here")
sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/opt/android-sdk}}
avd=mhgu-phone
avd_home=${ANDROID_AVD_HOME:-$HOME/.android/avd}
image="system-images/android-36/google_apis/x86_64"
port=${SLINT_MCP_PORT:-9315}
serial=emulator-5554
package=io.github.scrimas.mhgusaveeditor
unit=mhgu-emulator
# the emulator and the builds share one memory ceiling, so together they can't take the
# desktop's memory whatever runs them (each has its own cap inside it as well)
slice=mhgu-test.slice
slice_max=7G
adb() { command adb -s "$serial" "$@"; }
command -v adb >/dev/null || PATH="$sdk/platform-tools:$PATH"

make_avd() {
    [ -d "$sdk/$image" ] || { echo "no $image in $sdk: the SDK lacks the emulator image" >&2; exit 1; }
    [ -f "$avd_home/$avd.avd/config.ini" ] && return
    mkdir -p "$avd_home/$avd.avd"
    printf 'avd.ini.encoding=UTF-8\npath=%s\npath.rel=avd/%s.avd\ntarget=android-36\n' \
        "$avd_home/$avd.avd" "$avd" > "$avd_home/$avd.ini"
    cat > "$avd_home/$avd.avd/config.ini" <<EOF
AvdId=$avd
avd.ini.displayname=MHGU editor test phone
abi.type=x86_64
hw.cpu.arch=x86_64
image.sysdir.1=$image/
tag.id=google_apis
tag.display=Google APIs
target=android-36
hw.cpu.ncore=2
hw.ramSize=2048
vm.heapSize=256
disk.dataPartition.size=4G
hw.lcd.width=1080
hw.lcd.height=2340
hw.lcd.density=450
hw.initialOrientation=portrait
hw.keyboard=yes
hw.mainKeys=no
hw.sdCard=no
hw.audioInput=no
hw.audioOutput=no
hw.camera.back=none
hw.camera.front=none
hw.gps=no
hw.sensors.proximity=no
showDeviceFrame=no
EOF
    echo "AVD $avd created in $avd_home"
}

start() {
    [ -r /dev/kvm ] && [ -w /dev/kvm ] || { echo "/dev/kvm is not usable: the emulator would be far too slow" >&2; exit 1; }
    make_avd
    if systemctl --user -q is-active "$unit"; then
        echo "the emulator is already running"
    else
        # its windows (the screen and the toolbar beside it) straight onto an empty
        # workspace, without showing on the current one; once per Hyprland session
        local mark="${XDG_RUNTIME_DIR:-/tmp}/mhgu-emulator-rule"
        if [ "${1:-}" != -no-window ] && command -v hyprctl >/dev/null \
            && [ "$(cat "$mark" 2>/dev/null)" != "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
            hyprctl eval "hl.window_rule({ match = { class = \"^Emulator\$\" }, workspace = \"${EMULATOR_WORKSPACE:-10} silent\" })" >/dev/null \
                && echo "${HYPRLAND_INSTANCE_SIGNATURE:-}" > "$mark"
        fi
        # a user service, capped: 2 CPUs' worth, 4 GB (2 for Android, the rest the emulator's)
        limit_slice
        systemd-run --user --unit "$unit" --slice "$slice" --collect -q -p CPUQuota=200% -p MemoryHigh=4G -p MemoryMax=5G -p Nice=10 \
            --setenv=ANDROID_HOME="$sdk" --setenv=ANDROID_SDK_ROOT="$sdk" --setenv=ANDROID_AVD_HOME="$avd_home" \
            --setenv=PATH="$PATH" \
            "$sdk/emulator/emulator" -avd "$avd" "$@" -no-audio -no-boot-anim -gpu "${EMULATOR_GPU:-host}" \
            -port 5554 -no-metrics
        echo "emulator starting (journalctl --user -u $unit for its output)"
    fi
    local t=0
    until [ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; do
        systemctl --user -q is-active "$unit" || { echo "the emulator stopped:" >&2; journalctl --user -u "$unit" -n 30 --no-pager >&2; exit 1; }
        t=$((t + 2)); [ $t -gt 300 ] && { echo "Android did not finish booting in 5 minutes" >&2; exit 1; }
        sleep 2
    done
    # the MCP port (forwards don't survive a restart)
    adb forward "tcp:$port" "tcp:$port" >/dev/null
    # and any emulator window the rule missed
    if [ "${1:-}" != -no-window ] && command -v hyprctl >/dev/null; then
        hyprctl eval "for _, w in ipairs(hl.get_windows({ class = \"Emulator\" })) do if w.workspace.id ~= ${EMULATOR_WORKSPACE:-10} then hl.dispatch(hl.dsp.window.move({ workspace = ${EMULATOR_WORKSPACE:-10}, window = w, follow = false, silent = true })) end end" >/dev/null || true
    fi
    # predictive back's animations, as on a phone with them turned on
    adb shell settings put global enable_back_animation 1 >/dev/null 2>&1 || true
    echo "Android is up ($serial)"
}

relaunch() {
    adb forward "tcp:$port" "tcp:$port" >/dev/null
    adb shell am force-stop "$package"
    adb logcat -c
    adb shell am start -W -n "$package/.MainActivity" >/dev/null
    echo "app started; MCP server: 127.0.0.1:$port/mcp"
}

install() {
    local apk="$app/target/apk/mhgu-editor-emulator.apk"
    [ -f "$apk" ] || { echo "no $apk: run $0 build first" >&2; exit 1; }
    adb install -r "$apk"
    relaunch
}

limit_slice() {
    systemctl --user set-property --runtime "$slice" MemoryMax="$slice_max" MemorySwapMax=0
}

stop() {
    systemctl --user -q is-active "$unit" || return 0
    # through the console, so the emulator saves its state and the next start resumes from it
    adb emu kill >/dev/null 2>&1 || systemctl --user stop "$unit"
    local t=0
    while systemctl --user -q is-active "$unit"; do
        t=$((t + 1)); [ $t -gt 60 ] && { systemctl --user stop "$unit"; break; }
        sleep 1
    done
}

build() {
    if systemctl --user -q is-active "$unit"; then
        echo "stopping the emulator for the build (about 3 GB with the build's 4-6 GB is too much)"
        stop
    fi
    cd "$app"
    limit_slice
    systemd-run --user --scope --quiet --slice "$slice" -p CPUQuota=200% -p MemoryMax=6G -p MemorySwapMax=0 \
        nice -n 19 env CARGO_BUILD_JOBS=1 bash packaging/build-apk.sh --emulator
}

mcp() {
    python3 - "$port" "$@" <<'EOF'
import base64, json, sys, urllib.request
port, tool = sys.argv[1], sys.argv[2]

def call(method, params):
    req = urllib.request.Request(f"http://127.0.0.1:{port}/mcp", method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json, text/event-stream"},
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode())
    body = urllib.request.urlopen(req, timeout=60).read().decode()
    # a JSON answer, or server-sent events whose data lines hold it
    msg = json.loads(body) if body.lstrip().startswith("{") else \
        json.loads("".join(l[5:] for l in body.splitlines() if l.startswith("data:")))
    if "error" in msg:
        sys.exit(json.dumps(msg["error"]))
    return msg["result"]

if tool == "list":
    for t in call("tools/list", {})["tools"]:
        print(f"{t['name']}: {(t.get('description') or '').splitlines()[0]}")
elif tool == "shot":
    windows = json.loads(call("tools/call", {"name": "list_windows", "arguments": {}})["content"][0]["text"])
    handles = windows.get("windowHandles") or sys.exit("the app has no window")
    res = call("tools/call", {"name": "take_screenshot", "arguments": {"windowHandle": handles[0]}})
    image = next(c for c in res["content"] if c.get("type") == "image")
    open(sys.argv[3], "wb").write(base64.b64decode(image["data"]))
    print(sys.argv[3])
else:
    args = json.loads(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else {}
    for c in call("tools/call", {"name": tool, "arguments": args}).get("content", []):
        print("(an image)" if c.get("type") == "image" else c.get("text", c))
EOF
}

case "${1:-}" in
    start) start ;;
    headless) start -no-window ;;
    stop) stop ;;
    build) build ;;
    install) install ;;
    run) build && start && install ;;
    restart) relaunch ;;
    save)
        src=${2:?the save folder}
        name=$(basename "$src")
        adb shell rm -rf "/sdcard/Download/$name"
        adb push "$src" "/sdcard/Download/" >/dev/null
        # the media scanner, so the folder shows in the picker at once
        adb shell am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d "file:///sdcard/Download/$name" >/dev/null
        echo "/sdcard/Download/$name"
        ;;
    size)
        case "${2:-phone}" in
            phone) adb shell wm size reset; adb shell wm density reset ;;
            fold) adb shell wm size 1856x2160; adb shell wm density 420 ;;
            tablet) adb shell wm size 2560x1600; adb shell wm density 320 ;;
            *) echo "sizes: phone, fold, tablet" >&2; exit 2 ;;
        esac
        ;;
    dark) adb shell cmd uimode night yes ;;
    light) adb shell cmd uimode night no ;;
    shot) adb exec-out screencap -p > "${2:?the file}"; echo "$2" ;;
    log)
        pid=$(adb shell pidof -s "$package" | tr -d '\r')
        if [ -n "$pid" ]; then adb logcat -d --pid="$pid"; else adb logcat -d -s mhgu-editor AndroidRuntime; fi
        ;;
    mcp) shift; mcp "${1:?the tool, or list}" "${2:-}" ;;
    mcpshot) mcp shot "${2:?the file}" ;;
    *) sed -n '2,/^set -e/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 2 ;;
esac
