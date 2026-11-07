# Phase 6 review: core and server track

Prompt 6, 7 November 2026. Phase 6 is the Android, Windows and Linux apps (Phases 2 to 5 on those platforms). The core track's part is "Answers requests" (PLAN-CORE-SERVER section 3). The joint "done when" is *the same account works on all six platforms and the UI passes the same visual checklist*; the core's share is that the core gives the same answers everywhere and runs on each platform.

The owner's answer: the core may be tested on the owner's Android phones, every variation, only when no one else is using them.

**Status: partly done. Two items wait for CI**, which GitHub stopped running on 7 November 2026: "recent account payments have failed or your spending limit needs to be increased", because the account's Actions minutes ran out. Nothing is wrong with the code; the owner was given the steps to fix it.

## Requests

| Request | State |
| --- | --- |
| None addressed to `core` | `docs/requests/` holds only core-0001 (from the core to Android: the TLS init), which is the Android track's to answer |

## The items

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Everything the three tracks need is in the contract | **Done** | CORE-API, "Starting Android, Windows and Linux (Phase 6)": a checklist per platform covering library files, JNA, the TLS init, the data folder, the key store, the player's part and media controls. The desktop player (`core/encore-audio`, the Windows and Linux track's) needs nothing new from the core: it reports as AVPlayer and ExoPlayer do. |
| The same answers on every platform | **Built; first CI comparison pending** | Swift, Kotlin and Dart each print `parity <FNV-1a>` of the library flow's results: counts, album order, an album's songs, search results, lyrics, the palette and a playlist. The Linux job fails unless the three agree, and macOS, the iOS Simulator, Windows and the phones must print the same value. CI run 42376334693 failed before reaching the comparison: Kotlin's https check didn't compile. That's fixed in 780bfb0, and the run after it was refused by billing. |
| The core on real Android phones | **Built; not yet run** | `tools/android-smoke/run.ps1`. The android CI job builds the bundle: one `classes.dex` from the Kotlin smoke test, JNA 5.17.0's Android build (pinned by SHA-256) and coroutines; each ABI's libraries; a throwaway certificate. The runner first checks no one else is using the phones. It then runs the library flow and the https guard on every ABI each phone lists, through `app_process` from `/data/local/tmp` (nothing installed, no screen input), and removes everything. Attached: Tab S11+ (SM-X828U), Galaxy A13 (SM-A135U) and Galaxy S20 FE (SM-G781V). The results table goes here once CI produces the bundle. |

## Not claimed

- The Android TLS init itself (it needs an app's Context; the Android track's).
- x86 and x86_64 Android: no emulator, and the phones are ARM.
- Any UI, gapless playback, or media controls: the platform tracks'.
