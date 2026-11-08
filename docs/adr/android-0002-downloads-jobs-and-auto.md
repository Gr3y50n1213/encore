# ADR android-0002: Downloads run as jobs; Android Auto browses the library session

Date: 8 November 2026
Status: accepted

## Context
Phase 8 on Android: downloads, offline, Android Auto, and progress for downloads the person starts (ROADMAP; PLAN-ANDROID sections 8 and 20). The core fetches songs itself (ADR core-0017), so Android has three jobs: give the core the time and network to do it, report the network, and give the car its lists. The plan names user-initiated data transfer jobs for user-started downloads, and Live Updates with a plain ongoing notification as the fallback. The owner approved the new install-time permission `RUN_USER_INITIATED_JOBS` on 8 November 2026.

## Decision
**Downloads (`downloads/`).** A `JobService` calls `run_downloads` and, in `onStopJob`, `stop_downloads`.
- **Which job:** a download the person starts is a user-initiated job on Android 14 and later, with its required notification. Otherwise it is a normal job, which reschedules itself when Android stops it.
- **Wi-Fi only** is the job's network constraint (unmetered), so a job never wakes up on cellular to find it can't run.
- **Smart downloads** are a daily job on an unmetered network while charging, with no progress notification.
- **Rescheduling:** jobs aren't persisted, so no `RECEIVE_BOOT_COMPLETED` is needed; whatever waits is rescheduled when a profile opens.
- **The notification** follows boards 32 and 34: one notification in a low-importance channel, "Downloading Road trip", "15 of 24 songs", a bar, Cancel. Cancel pauses what waits.
- **Notification permission:** asked after board 25's primer, the first time a download starts.
- **Live Updates** come with Phase 9.

**Connectivity (`core/Connectivity.kt`).** One watcher feeds both the core (`set_connectivity`, with the rolling cache's whole songs) and the offline banner. It settles 2 s each way (board 20). Offline means no network at all; a network without the internet still reaches a server at home.

**Rolling cache (`playback/RollingCache.kt`).** Media3's `SimpleCache` behind a `CacheDataSource`, keyed by the core's `cache_key`. A small LRU evictor lets the size change while playing (Settings applies at once). Downloaded songs are `file://` and bypass it.

**Android Auto (`playback/AutoLibrary.kt`).** `PlaybackService` is a `MediaLibraryService`.
- **The tree:** Home (the day's mixes, recently played), Library (Playlists, Albums, Artists, Liked songs) and Downloads, with search. Offline, the lists hold only what can play.
- **Playing:** a car's choice goes to the core as `play` with its album, playlist or mix around it. The forwarding player then loads the core's current song and never the controller's list. Voice "play X" goes through the core's search.
- **Covers** come from a provider that isn't exported, granted per address to the asking controller, because a URL with the token must never leave the app.

## Alternatives considered
- **Media3's DownloadService.** A dataSync foreground service, which Android 15 caps at 6 hours a day, and its own cache format the core can't read.
- **WorkManager.** A new library; JobScheduler does what's needed.
- **Persisted jobs.** They need the boot permission, which isn't on the section 19 list.
- **Exported artwork URLs with the API key.** That would leak the token to another app.

## Consequences
- **Easier:** downloads survive the app closing; the system shows and manages user-initiated jobs; Auto needs no layouts (version 1 draws none, board 31).
- **Harder:**
  - Android may still defer normal jobs on older versions.
  - Auto can only be checked in a car or on the desktop head unit; the emulator test drives the same session with a `MediaBrowser`.
- **To undo:** remove the job service, the receiver, the provider and the metadata from the manifest, and the permission from section 19.
