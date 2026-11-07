# Phase 6 review: core and server track

Prompt 6, 7 November 2026. Phase 6 is the Android, Windows and Linux apps (Phases 2 to 5 on those platforms). The core track's part is "Answers requests" (PLAN-CORE-SERVER section 3). The joint "done when" is *the same account works on all six platforms and the UI passes the same visual checklist*; the core's share is that the core gives the same answers everywhere and runs on each platform.

The owner's answer: the core may be tested on the owner's Android phones, every variation, only when no one else is using them.

**Status: the core's part is done.** CI evidence is run 42393953953, with every job green (the repository went public on 7 November 2026, which ended the Actions billing block). The phone run used run 42395366303's bundle.

## Requests

| Request | State |
| --- | --- |
| None addressed to `core` | `docs/requests/` holds only core-0001 (from the core to Android: the TLS init), which is the Android track's to answer |

## The items

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Everything the three tracks need is in the contract | **Done** | CORE-API, "Starting Android, Windows and Linux (Phase 6)": a checklist per platform covering library files, JNA, the TLS init, the data folder, the key store, the player's part and media controls. The desktop player (`core/encore-audio`, the Windows and Linux track's) needs nothing new from the core: it reports as AVPlayer and ExoPlayer do. |
| The same answers on every platform | **Passes** | Swift, Kotlin and Dart each print `parity <FNV-1a>` of the library flow's results: counts, album order, an album's songs, search results, lyrics, the palette and a playlist. Every platform printed **`c5f1d3b2`**: Swift, Kotlin and Dart on Linux; Dart on Windows; Swift on macOS and in the iOS Simulator; and all three Android phones below. CI fails on any difference. |
| The core on real Android phones | **Passes** | `tools/android-smoke/run.ps1`. The bundle comes from CI's android job: the Kotlin smoke test as one `classes.dex`, JNA 5.17.0's Android build and each ABI's `libencore_core.so`. It runs through `app_process` from `/data/local/tmp`: nothing installed, no screen input, everything removed after. It covers the whole library flow plus the https guard before `EncoreTls.initialize` (request core-0001). Results in the table below. |

| Device | Android | ABI | Result |
| --- | --- | --- | --- |
| The owner's tablet | 16 (API 36) | arm64-v8a | passed, parity c5f1d3b2, 1.6 s |
| The owner's first phone | 14 (API 34) | armeabi-v7a | passed, parity c5f1d3b2, 3 s |
| The owner's second phone | 13 (API 33) | arm64-v8a | passed, parity c5f1d3b2, 2 s |
| The owner's second phone | 13 (API 33) | armeabi-v7a | passed, parity c5f1d3b2, 1.8 s |

The phones also list plain `armeabi`, which Android's NDK no longer builds for: nothing to test. x86 and x86_64 need an emulator.

## Found by the phones, and fixed

1. **The https guard's message never reached the app.** Android without `EncoreTls.initialize` got "Couldn't reach the server: error sending request" instead of the documented reason. The HTTP client nests the TLS error two I/O errors deep, and the translator didn't look inside them. It now unwraps to any depth, on every platform, so an expired certificate also says so. `a_real_tls_failure_says_so` sends a real https request to a server that doesn't speak TLS. The first fix looked one level deep and the phones still failed; the second, tested through the real transport, passed on every device.
2. **The Kotlin smoke test died silently on a failure.** Android turns an uncaught exception into SIGKILL, so the test now prints the reason and exits 1.
3. **The runner had three bugs:**
   - adb's progress on stderr stopped Windows PowerShell 5.1;
   - the first push wasn't inside the cleanup block, which left one folder on the tablet (removed by hand);
   - `$out` clobbered `-Out`, because PowerShell names ignore case.

   A device's first port forward can also take a moment to start (the tablet failed once and passed on the rerun), so the runner retries once on "refused".

## Not claimed

- The Android TLS init itself (it needs an app's Context; the Android track's).
- x86 and x86_64 Android: no emulator, and the phones are ARM.
- Any UI, gapless playback, or media controls: the platform tracks'.
