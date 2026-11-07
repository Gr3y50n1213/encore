# Android review: Phases 1 and 2

7 November 2026. The Android track's part of Phase 1 (Foundations) and Phase 2 (Jellyfin and playback), checked against their "done when" lines (docs/ROADMAP.md), PLAN-UX section 24 and the Production readiness table in docs/android/PLAN-ANDROID.md, for what these phases touched.

**Evidence:** CI run 42414284761 (`android.yml` on `android-phase2`, merged to `main` as be5ae55), every job green:
- `make check-android`: lint, 12 unit tests, debug and R8 release builds with the Rust core built inside `android/` for arm64-v8a, armeabi-v7a, x86_64 and x86, and `zipalign -c -P 16` on both APKs (7 min);
- the emulator at **API 28** (Android 9, the minimum) and at **API 36 with 16 KB pages** (`google_apis_ps16k`): 3 fresh-install tests, then the end-to-end sign-in test, then the release build's first screen read from the view tree, light and dark screenshots.

Nothing has run on a real phone yet. Every "needs a phone" row below is for the owner.

## Phase 1: "a screen whose text comes from the Rust core, and CI is green"

| Item | Result | Evidence |
| --- | --- | --- |
| The Android project | **Passes** | `android/` (ADR android-0001): AGP 9.5.1, Gradle 9.6.0, Kotlin and Compose, Material 3 1.5.0-rc01, min API 28, target 36, app ID `uk.gcartee.encore` |
| The core built for Android inside `android/` | **Passes** | `buildRustCore`: four ABIs with NDK 28.2 at API 28, 16 KB linker flags; every `.so` in the APK has 0x4000 LOAD alignment (checked with llvm-readelf) and passes `zipalign -P 16` |
| A screen whose text comes from the core | **Passes** | `CoreOnDeviceTest` on both emulators: `hello("Android")` through the bindings, the first screen shows `app_name()`, the launcher label is the core's name (read from `brand.rs` at build time). The release build's first screen shows the core's name (`device-check.sh`). |
| Design tokens wired, no copied numbers | **Passes** | the generated `EncoreTokens.kt` is a source folder; colours (Iris), type scale, button sizes, motion timings and haptic choices read from it. `ThemeTokensTest`. |
| `make check-android` real, CI green | **Passes** | run 42414284761 |

## Phase 2: "you can sign in, browse your whole library offline-fast, and listen to an album gaplessly"

| Item | Result | Evidence |
| --- | --- | --- |
| Sign in | **Passes on emulators** | `SignInAndBrowseTest` on API 28 and API 36: the real Connect screen against the owner's anonymized Jellyfin recording (replayed on the CI host, reached through `adb reverse`): the address check after 600 ms ("Found Jellyfin 12.0.0 on your home network"), Quick Connect, the profile opens, the token goes to the Keystore |
| Browse the whole library, offline-fast | **Passes on emulators** | the same test: the first sync finishes, Library lists the albums from the on-device mirror, the album page opens. Browse calls read the mirror (milliseconds); lists page 200 at a time |
| Play an album | **Partly**: queue and player wiring pass; sound **needs a phone** | the test presses Play and the core's queue holds the album with the first song current; the mini player appears. The recording holds no audio, so ExoPlayer's sound is not exercised in CI |
| Gapless | **Needs a phone** | `CorePlayback` loads the song `Preload` names as ExoPlayer's next item, so the switch is gapless for FLAC and for MP3/AAC with encoder delay metadata. Listen to a gapless album (a live record) on a phone |
| Lock screen, notification, headset and Bluetooth controls | **Needs a phone** | Media3 `MediaSessionService` (type mediaPlayback). Next and Previous from them go through the core with their origin (`controllerForCurrentRequest`: notification and lock screen, car, headset) |
| The play log | **Wired; needs a phone to listen** | `playback_started` when sound starts, `playback_progress` each second with the heard time (`ListenClockTest`), `playback_ended` on finish or error, seeks, pauses, headphones unplugged and audio-focus loss reported |

## Found and fixed in these phases

1. **The app crashed at start on a fresh install** (both emulators): `set_playback_capabilities` needs an open profile. It is now set each time a profile opens, and CORE-API says so.
2. **Lint flagged the generated bindings** for `java.lang.ref.Cleaner` (API 33). The bindings check for the class at run time and fall back to JNA's cleaner on Android 9 to 12, so `app/lint.xml` exempts only `core/bindings/kotlin` from that check.
3. **JNA's aar carries armeabi and MIPS libraries:** the APK now packages only the ABIs the core is built for.
4. **Self-signed servers:** the user trusts a fingerprint once (board 21); the core keeps it, and the app now also lets ExoPlayer and the artwork loader through for the same fingerprint (`TrustedCertificates`), or streams and covers would have failed.
5. Test and CI fixes along the way: the emulator's 300 ms Welcome guard runs on the test clock; controls below the fold are scrolled to; the API 36 emulator needed 4 GB of RAM.

## Where I changed or left out what a board draws (for the owner)

| Board | What it draws | What the app does | Why |
| --- | --- | --- | --- |
| 21 | "Step 2 of 4" | "Step 2 of 2" | "Choose what to include" needs a core call that does not exist (request android-0001); the companion step is Phase 4. A counter should count steps that exist. |
| 21, item 7 | Try a demo library | Not shown | Needs openly licensed tracks bundled in the app and a local-files source in the core (Phase 12). No button that does nothing. |
| 22 | Privacy policy and licences links | Shown, disabled | The privacy policy page does not exist yet (docs/DECISIONS.md section 4). |
| 22, item 4 | I already have a profile | Shown, disabled | Needs the companion server (Phase 4). |
| 06, item 2 | Sort: Most played, Fewest skips | Recently added, Title, Artist, Year | The core has no listening sorts yet (request android-0001). |
| 08, item 4 | Play, Shuffle, Download group | Play and Shuffle | Downloads are Phase 8. The two buttons use the token corners (outer round, facing 10 dp); the "pressed widens, neighbours squeeze" motion comes with Phase 3. |
| 08, item 1 | More menu | Not shown yet | Menus (boards 13, 06 item 4) are Phase 3. |

## Not done yet (Phase 3 and later, by the roadmap)

The moving album gradient on Now Playing, the wavy progress bar, lyrics, the queue sheet, menus and long-press actions, swipes (board 42), the tablet layouts (board 29), fold postures, screenshot tests per control state. Home's Made for you, Recently played and Rediscover, and the Mixes tab, wait for Phase 5's screens.

## What the owner needs to check on a phone

1. Sign in to the real Jellyfin server (password and Quick Connect).
2. Play a gapless album start to finish: no gap between tracks.
3. Lock the phone: the lock screen player shows the song; play, pause, next and previous work. The same from the notification and from headphones or Bluetooth.
4. Unplug headphones while playing: it pauses.
5. Haptics: Get started, the server type group, play and pause, next, like, scrubbing.
6. Rotate and resize (split screen): the song keeps playing.
