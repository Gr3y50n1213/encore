# Android review: Phase 8 (offline and car)

8 November 2026. The Android track's part of Phase 8:
- downloads and their progress;
- offline mode and its sign;
- the rolling cache;
- Android Auto.

Checked against the "done when" line (*a downloaded playlist plays in airplane mode and in a car*), the approved plan, and boards 06, 07, 08, 11, 13, 17, 20, 25, 26, 29 and 31 to 34. The owner asked for:
- Material 3 Expressive throughout;
- portrait and landscape where they matter;
- a good look offline, with a "not too intrusive but notable" sign;
- every bug worked out first.

**New permission:** `RUN_USER_INITIATED_JOBS`, approved by the owner; see PLAN-ANDROID section 19 and ADR android-0002. There are no new libraries.

**Evidence:**
- **CI run 42538247424, `android.yml` on `phase8-offline` at b81081a, every job green:**
  - `make check-android`: lint, unit tests, debug and R8 release builds, four ABIs, 16 KB alignment.
  - Three emulators: API 28 phone, API 36 phone and API 36 tablet.
  - On each, the sign-in test and then the new `OfflineAndCarTest` run against the Jellyfin replay, which now serves real audio.
- **The owner's devices:**
  - Installed on the tablet, the A13 and the S20 FE, with media volume 0.
  - A live check against the owner's own Jellyfin, on the tablet.

## The "done when" line

| Item | Result | Evidence |
| --- | --- | --- |
| A downloaded collection plays with no network | **Passes on three emulators** | `OfflineAndCarTest`: Download on an album page, the job saves every song, and `stream_for` is a file. Then Wi-Fi and data go off with `svc`. The banner appears, Play starts the album from its files and the player reaches playing, and a song that isn't downloaded is refused. The tablet emulator reaches the network over Ethernet, so `svc` can't drop it; there the test switches the app offline through a test-only hook. |
| In airplane mode on a phone | **For the owner** | The same path as above with real radios off. |
| In a car | **Passes as a media browser; the car itself is for the owner** | The test connects a `MediaBrowser` to the library session the way Android Auto does. Root: Home, Library and Downloads. An album's songs are listed, and Downloads lists the downloaded album. Choosing the second song plays it through the core with the rest of the album after it. A real head unit or the desktop head unit is still to try. |

## The phase's items

| Item | Result | Evidence |
| --- | --- | --- |
| Downloads run as jobs | **Passes** | A user-initiated job on Android 14 and later, otherwise a normal job (ADR android-0002). Wi-Fi only is the job's constraint. Smart downloads run nightly on Wi-Fi while charging. On the tablet, a download on LTE with Wi-Fi only on waits, as it should. |
| The notification and its primer | **Passes** | Board 25's "Show download progress?" sheet comes before Android's prompt, on the API 36 emulators and the real tablet. The progress notification follows boards 32 and 34 ("Downloading Road trip", "15 of 24 songs", a bar, Cancel). |
| The Download button and lines | **Passes** | Album, playlist and mix pages: an arrow, then a wavy ring while songs arrive, a still ring while waiting, a check when saved, the error mark when something failed. "7 of 12 saved", or why it waits ("0 of 1 saved · Waiting for Wi-Fi"). Rows carry a saved mark, tiles a badge, and every menu has Download / Remove download. |
| Library and Search | **Passes** | Library's fifth chip, Downloaded (shot 13), and Search's Downloaded scope. |
| Settings › Downloads | **Passes, portrait and landscape** | Shots 14 and 15: the storage bar names every part (Downloads, Cache, Other, Free), then status, quality, Wi-Fi only, limit, auto-remove, Liked songs, Smart downloads, the cache, swipe to remove, and Remove all. |
| The offline sign | **Passes** | Shots 16 and 17, phone and tablet: a slim amber pill, "Offline · Showing downloaded music", floats under the status bar after 2 s and pushes no page colours down. Pages move their content by its height. Now Playing shows an Offline chip. What can't play dims, and a tap shakes with the reason. |
| Landscape and tablets | **Passes** | Board 26: past 600 dp the navigation is a rail, and the test checks it after turning the phone. On tablets the panes keep the status bar readable. |
| The rolling cache | **Passes** | Media3's `SimpleCache`, keyed by the core's `cache_key` and resizable from Settings. Songs cached whole count as playable offline. |
| Android Auto | **Passes as a media browser** | `MediaLibraryService` with Home, Library and Downloads, search, and "Play X" by voice in the car and from the phone's assistant (`MEDIA_PLAY_FROM_SEARCH`). Covers come through a provider that is not exported, granted per controller. |

## Found and fixed during this phase

1. **The owner's Jellyfin crashed** (8 November, 14:58).
   - **What happened:** at a capped quality the tablet asked for each song 3 or 4 times, about every 7 s. Each new request killed the last transcode, and one kill hit a Jellyfin bug (writing "q" to an ffmpeg that had exited).
   - **Fix:** the player now reads a song through in one connection (up to 15 minutes or 32 MB) and waits longer before retrying (10 s to connect, 20 s for data).
   - **Live check on the owner's server:** "Trauma" was requested and transcoded once, and the next song once.
   - **A second duplicate:** at a song change, Autoplay's new queue IDs made the player fetch the next song again. It now updates the item in place.
   - **On the server, with the owner's OK:** nssm restarts Jellyfin after a crash.
2. **After the server came back, open apps kept "Couldn't reach the server".** A failed sync now retries by itself in the foreground (15 s, 30 s, 1, 2, then every 5 minutes), at once when the network returns or changes, and when the app comes back after 15 minutes.
3. **UI tests could never see a playing Now Playing go idle**, because the smooth position ran on plain frames. It now runs as an infinite animation, which Compose's policy pauses in tests.
4. **The offline banner pushed the whole page down.** On an album it left a light strip with white status-bar icons on it, unreadable. It now floats (`topBarsPadding`, `LocalTopBanner`).
5. **Library's scrolling chip row was cut at the page margin.** The margin now sits inside the scroll, and the chosen chip is brought into view.
6. **Tablet panes set light status-bar icons over a light surface**, which predates this phase (`LocalInPane`).
7. **A waiting download looked like an active one.** It now shows a still ring and says why it waits.
8. **Lint:** an Auto media app must take `MEDIA_PLAY_FROM_SEARCH`, so the phone's assistant can now "Play X on Encore".

## Not claimed (for the owner)

- **Android Auto:** a real car or the desktop head unit.
- **Airplane mode on a phone:** download an album, turn airplane mode on, play it.
- **The notification:** how it feels during a long download, and Cancel from it.
- **Volume:** large libraries and a weekend of Smart downloads.
- **The tablet's Wi-Fi:** "Anderson-wifi-1.0" has no internet, so the tablet reaches the server only over LTE; its Wi-Fi is off at the owner's request. A test download there ("Crush") showed "0 of 1 saved · Waiting for Wi-Fi" and was then removed.
