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
# Room for the player's event log (debug builds log every player event) in the review.
adb logcat -G 16M || true
adb logcat -c || true

shot() { # name
  sleep 2
  adb exec-out screencap -p > "$out/$1.png"
}

# The API 37 image reports "booted" while its package service is still settling: an
# install right away failed with "Failure calling service package: Broken pipe" (run
# 43058504996). Wait for the package service to answer, then retry an install a few times.
wait_for_packages() {
  for _ in $(seq 1 60); do
    if [ "$(adb shell getprop sys.boot_completed | tr -d '\r')" = "1" ] && adb shell pm path android >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "the package service never answered" >&2
  return 1
}
install() { # apk
  for attempt in 1 2 3; do
    wait_for_packages
    if adb install -r -t "$1"; then return 0; fi
    echo "install of $1 failed (attempt $attempt), retrying" >&2
    sleep 10
  done
  return 1
}

echo "== 1. Instrumented tests (debug)"
install "$debug"
install "$tests"
runner=uk.gcartee.encore.debug.test/androidx.test.runner.AndroidJUnitRunner
signin=uk.gcartee.encore.app.SignInAndBrowseTest
offline=uk.gcartee.encore.app.OfflineAndCarTest
playback=uk.gcartee.encore.app.PlaybackFlowTest
sleep_sheet=uk.gcartee.encore.app.SleepTimerSheetTest
# First on a fresh install (no profile yet), everything but the replay tests.
adb shell am instrument -w -r -e notClass "$signin,$offline,$playback,$sleep_sheet" "$runner" | tee "$out/instrument.txt"
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
echo "== 1c. Phase 8: download an album, play it with no network, browse it as a car"
# Signed in by 1b; the replay serves made-up audio. The test turns the network back on.
adb shell am instrument -w -r -e class "$offline" -e replayUrl "$replay" "$runner" | tee "$out/instrument-offline.txt"
adb shell svc wifi enable || true
adb shell svc data enable || true
echo "== 1d. A song that ends plays the next one, which was already loaded"
adb shell am instrument -w -r -e class "$playback" -e replayUrl "$replay" "$runner" | tee "$out/instrument-playback.txt"
echo "== 1e. Phase 9: the sleep timer sheet (set, cancel, Undo)"
adb shell am instrument -w -r -e class "$sleep_sheet" -e replayUrl "$replay" "$runner" | tee "$out/instrument-sleep-sheet.txt"
# The player's story for the review: the app's decisions and Media3's events.
adb logcat -d -s CorePlayback:V EncorePlayer:V > "$out/logcat-playback.txt" 2>/dev/null || true
adb reverse --remove "tcp:$port" || true
kill "$replay_pid" || true
# The screens the test photographed, for the review.
mkdir -p "$out/screens"
for f in $(adb shell 'ls /data/local/tmp/encore-shot-*.png 2>/dev/null' | tr -d '\r'); do
  adb pull "$f" "$out/screens/api$sdk-$(basename "$f" | sed 's/^encore-shot-//')" > /dev/null || true
done
adb shell 'rm -f /data/local/tmp/encore-shot-*.png' || true
grep -q '^OK (1 test)' "$out/instrument-signin.txt" || { echo "the sign-in test failed" >&2; exit 1; }
grep -q '^OK (1 test)' "$out/instrument-offline.txt" || { echo "the offline and car test failed" >&2; exit 1; }
grep -q '^OK (' "$out/instrument-playback.txt" || { echo "the playback flow test failed" >&2; exit 1; }
grep -q '^OK (' "$out/instrument-sleep-sheet.txt" || { echo "the sleep timer sheet test failed" >&2; exit 1; }
# The release build below must start fresh again, on Welcome.
adb uninstall uk.gcartee.encore.debug > /dev/null || true

echo "== 2. Release build"
install "$release"
adb shell am start -W -n uk.gcartee.encore/uk.gcartee.encore.app.MainActivity | tee "$out/start.txt"
sleep 4
pid="$(adb shell pidof uk.gcartee.encore | tr -d '\r' || true)"
if [ -z "$pid" ]; then
  adb logcat -d > "$out/logcat.txt"
  echo "the release build is not running after start" >&2
  grep -A 20 'FATAL EXCEPTION' "$out/logcat.txt" >&2 || true
  exit 1
fi
# uiautomator can answer "null root node" while the first frame is still drawing (seen
# once on API 28, 9 November 2026): try again for a few seconds.
for try in 1 2 3 4 5; do
  adb shell rm -f /sdcard/ui.xml
  adb shell uiautomator dump /sdcard/ui.xml > /dev/null || true
  if adb shell cat /sdcard/ui.xml > "$out/ui-release.xml" 2> /dev/null && grep -q '<hierarchy' "$out/ui-release.xml"; then break; fi
  echo "uiautomator gave no view tree (try $try); trying again"
  sleep 2
done
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
