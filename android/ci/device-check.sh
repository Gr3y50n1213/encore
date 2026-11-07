#!/usr/bin/env bash
# Runs on a booted emulator or an attached device (CI's emulator job). Takes the APK
# folder from the `check` job:
#   1. the instrumented tests (HelloScreenTest) on the debug build;
#   2. the release build (R8-shrunk, debug-signed): it starts, stays up, and its first
#      screen shows the core's sentence, read from the view tree;
#   3. screenshots of both, light and dark, into device-check/ for the review.
# Nothing here needs a network.
set -euo pipefail

apks="${1:?usage: device-check.sh <apk folder>}"
out=device-check
mkdir -p "$out"
debug="$(find "$apks" -name app-debug.apk | head -n 1)"
tests="$(find "$apks" -name app-debug-androidTest.apk | head -n 1)"
release="$(find "$apks" -name app-release.apk | head -n 1)"
test -n "$debug" && test -n "$tests" && test -n "$release"

# On any failure, keep the device log and print the crash, so the job says why.
on_exit() {
  status=$?
  if [ "$status" -ne 0 ]; then
    adb logcat -d > "$out/logcat.txt" 2>/dev/null || true
    echo "---- crash, from logcat ----" >&2
    grep -E -A 40 'FATAL EXCEPTION|Fatal signal|E AndroidRuntime' "$out/logcat.txt" | head -n 120 >&2 || true
  fi
  exit "$status"
}
trap on_exit EXIT

adb wait-for-device
sdk="$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
abi="$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
page="$(adb shell getconf PAGE_SIZE | tr -d '\r' || echo unknown)"
echo "device: API $sdk, $abi, page size $page" | tee "$out/device.txt"
adb logcat -c || true

shot() { # name
  sleep 2
  adb exec-out screencap -p > "$out/$1.png"
}

echo "== 1. Instrumented tests (debug)"
adb install -r -t "$debug"
adb install -r -t "$tests"
runner=uk.gcartee.encore.debug.test/androidx.test.runner.AndroidJUnitRunner
signin=uk.gcartee.encore.app.SignInAndBrowseTest
# First on a fresh install (no profile yet), everything but the sign-in test.
adb shell am instrument -w -r -e notClass "$signin" "$runner" | tee "$out/instrument.txt"
grep -q '^OK (' "$out/instrument.txt" || { echo "instrumented tests failed" >&2; exit 1; }

echo "== 1b. Sign in to the Jellyfin replay, browse, play"
# The owner's anonymized recording, served on this host and reached from the device
# through adb reverse (tools/ci/replay_jellyfin.py).
rm -f "$out/replay-url.txt"
python3 -I -B tools/ci/replay_jellyfin.py tools/fixtures/jellyfin/recorded.json "$out/replay-url.txt" &
replay_pid=$!
for _ in $(seq 1 100); do [ -s "$out/replay-url.txt" ] && break; sleep 0.1; done
replay="$(cat "$out/replay-url.txt")"
port="${replay##*:}"
adb reverse "tcp:$port" "tcp:$port"
adb shell am instrument -w -r -e class "$signin" -e replayUrl "$replay" "$runner" | tee "$out/instrument-signin.txt"
adb reverse --remove "tcp:$port" || true
kill "$replay_pid" || true
grep -q '^OK (1 test)' "$out/instrument-signin.txt" || { echo "the sign-in test failed" >&2; exit 1; }
# The release build below must start fresh again, on Welcome.
adb uninstall uk.gcartee.encore.debug > /dev/null || true

echo "== 2. Release build"
adb install -r "$release"
adb shell am start -W -n uk.gcartee.encore/uk.gcartee.encore.app.MainActivity | tee "$out/start.txt"
sleep 4
pid="$(adb shell pidof uk.gcartee.encore | tr -d '\r' || true)"
if [ -z "$pid" ]; then
  adb logcat -d > "$out/logcat.txt"
  echo "the release build is not running after start" >&2
  grep -A 20 'FATAL EXCEPTION' "$out/logcat.txt" >&2 || true
  exit 1
fi
adb shell uiautomator dump /sdcard/ui.xml > /dev/null
adb shell cat /sdcard/ui.xml > "$out/ui-release.xml"
# A fresh install opens on Welcome (board 22), whose name comes from the core's
# app_name(), the one brand constant (core/encore-core/src/brand.rs).
name="$(sed -nE 's/^pub const APP_NAME: &str = "([^"]+)";/\1/p' core/encore-core/src/brand.rs)"
test -n "$name"
grep -Fq "text=\"$name\"" "$out/ui-release.xml" \
  || { echo "the core's name ($name) is not on the release build's first screen" >&2; exit 1; }
grep -Fq 'text="Get started"' "$out/ui-release.xml" \
  || { echo "Welcome's Get started is not on screen" >&2; exit 1; }
shot "release-light-api$sdk"

echo "== 3. Dark theme"
adb shell cmd uimode night yes || true
shot "release-dark-api$sdk"
adb shell cmd uimode night no || true

adb logcat -d > "$out/logcat.txt"
if grep -q 'FATAL EXCEPTION' "$out/logcat.txt"; then
  grep -A 20 'FATAL EXCEPTION' "$out/logcat.txt" >&2
  exit 1
fi
echo "device-check: passed on API $sdk ($abi)"
