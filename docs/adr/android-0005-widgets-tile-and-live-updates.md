# ADR android-0005: Widgets read a file, buttons go to the service, Live Updates only for what the person started

Date: 11 November 2026
Status: accepted. Superseded in part by [android-0006](android-0006-album-colored-widgets.md): the three widgets' look, sizes and colors ("The three widgets") changed on 12 November 2026; everything else here stands.

## Context
Phase 9 on Android (ROADMAP; PLAN-ANDROID section 20; boards 31, 32 and 34): three Glance widgets, a Quick Settings tile, Live Updates for the sleep timer and downloads, the sleep timer itself, and a settings page. The core writes the data and owns the timer (ADR core-0018). Android's job is to draw it, route the taps, and keep the lock screen private.

The owner approved on 11 November 2026:
- **Glance 1.2.0.** It brings WorkManager, Room and DataStore with it. All are from Google, Apache-2.0, and are listed in `THIRD_PARTY_LICENSES.md`.
- **`POST_PROMOTED_NOTIFICATIONS`.** Install-time, no prompt. See PLAN-ANDROID section 19.
- **The Recap widget waits for Phase 11.**

## Decision
**Widgets never start the core.** A widget reads the profile's `snapshot.json` and its covers (`widgets/`) and nothing else. Only `WidgetSync`, which `CoreHost` starts, touches the core: it saves the open and last active profile and redraws widgets on `WidgetSnapshotChanged`. `WidgetSourceScanTest` fails the build if any other file in `widgets/` uses `CoreHost` or the core, the network or a file address. So a widget can't open the database or make a network call. Cover names and profile IDs can't leave their folder (`NowPlayingWidgetStatesTest`, `WidgetProfileTest`).

**Buttons go to `PlaybackService`.**
- Play, Pause, Next and Previous are media buttons. They work with the app closed.
- Starting a mix is a private action. It needs a per-install token (`WidgetProfileTest.theTokenMustMatchExactly`), because the service is exported for Android Auto and any app could otherwise start a mix.
- No intent carries a profile id. A widget never tells the service which profile to play.
- `WidgetStarts` gives the foreground guarantee: every start that names an action reaches the foreground in time or stops. Android kills a service that is started with a foreground service intent and doesn't call `startForeground`.

**A widget for another profile, or one with a PIN, opens the switcher.** A widget keeps its own profile (Active by default, or a fixed one). If a tap would need a profile switch or a PIN, it opens the app's switcher. A tap never switches profiles silently. Recently played also checks the album is in the widget's library before opening it.

**Words, not data, when there is no data.** The states are: Nothing yet, Server not reachable (saved mixes show), Profile is locked, Open Encore (Guest), Profile removed, and dimmed with "Open Encore to refresh" after 24 hours.

**The three widgets.**
- Now Playing: 2x1 to 6x2, with a wide tablet row. A round play button that becomes a rounded square while playing (PLAN-UX section 22). Wallpaper colors. *(Colors, sizes and the progress bar changed: see ADR android-0006.)*
- Mixes: 4x2, up to four tiles. Next up comes first, then the pinned mixes. One tap starts the mix in the background. Configurable: Next up or one pinned mix; a pinned choice that is gone falls back to Next up.
- Recently played: 4x3, up to eight covers. A tap opens the album.

**Lock screen rules.**
- Now Playing is the only widget allowed on the lock screen. Mixes and Recently played declare `not_keyguard` in their provider XML (`WidgetProviderXmlTest`).
- With "Hide listening history while locked" on (the default), Now Playing on the lock screen never shows the last played song. It shows Play and the mix name. What is playing right now may show. A locked or Guest profile is never overridden (`NowPlayingWidgetStatesTest`, the lock screen tests).
- The same toggle makes the download notification private on the lock screen, with a neutral public line ("Downloading, 15 of 24 songs"). Before this phase it was public and named the album.
- The toggle stays, against the first idea of dropping it, because dropping it would have put "last played" on the lock screen.
- Android 16 QPR1 lets a phone show lock-screen widgets; tapping a widget that opens an activity asks for an unlock first, because the main activity is not `showWhenLocked`.

**Live Updates (`notifications/LiveUpdates.kt`).**
- Promoted only on Android 16 QPR1 and later (`SDK_INT` 36 and `SDK_INT_FULL` at `BAKLAVA_1`, checked in that order because the full value exists only from 36).
- Promoted only for a task the person started: a sleep timer they set, a download they began. Automatic and Smart downloads keep the plain notification.
- The settings toggles stop promotion, never the task. A user-initiated job must always show its notification, and the timer always keeps its card.
- Chip text is under 7 characters ("62%", "24m").
- Settings has "Open settings" for when the person has turned Live Updates off for the app, with a fallback to the app's notification settings.

**The sleep timer engine.**
- The fade runs on the player's own volume (`SleepFade.kt`, a pure state machine, driven by the core's `sleep_fade_plan` from the existing 1 s tick). It never uses `CorePlayback.setVolume`, which would report to other devices. The level only goes down. A pause, Add 15 min, cancel or handoff restores it.
- At the end it pauses, calls `sleep_timer_ended`, and restores the volume.
- End of song and End of album load nothing behind the stop song, and the player is paused before the song is reported Finished, so the next song is loaded once, paused.
- The card (`SleepTimerNotifier`, started from `CoreHost`) uses the system's own countdown (end time and a chronometer) and a self-timeout, so it stays right and goes away even if Android closes the app. It has Add 15 min, and a swipe only hides it. `SleepTimerReceiver` is not exported. The card names no profile and nothing played, so it is the same on the lock screen.
- The sheet (`ui/player/SleepTimerSheet.kt`) opens from Now Playing's More, Settings > Playback and quality, and the card. Without notification permission the timer still works, with no card; the first timer asks after board 25's primer.

**The Quick Settings tile (`tile/StartMixTileService.kt`).** "Start a mix" plays the first pinned mix, else Next up, in the background through the same private action. The subtitle is the mix name, "Playing", or "Unlock Encore" for a PIN profile on a locked phone. If Android refuses to start playback from a tile, it opens Now Playing. Another profile or a PIN opens the switcher. It never starts the core to draw itself.

**The settings page.** Settings > Lock screen and widgets: the "Live updates are off" banner, the two Live Update toggles, the history toggle, Widget profile (the default for new widgets) and the tile's Add button (Android 13 and later).

**CI.** The Android workflow has an API 37 phone emulator (`google_apis` x86_64) next to API 28 and the two API 36 emulators. It is the only place a Live Update can be seen; no device the owner has runs Android 16 QPR1. It needed a 6 GB data disk, a wait for the package service before installs, and the `swangle_indirect` renderer (SurfaceFlinger aborted under `swiftshader_indirect`).

## Alternatives considered
- **Widgets that open the core to read.** That starts a process that opens the database and the network stack for a redraw, and puts the "never another profile's history" rule in widget code instead of in a file's folder.
- **Widget buttons that open the app.** Slow, and unreliable with the app closed. Media buttons and a service action are allowed for a tap and work at once.
- **A profile id in the intent.** The service is exported, so any app could ask it to play another profile.
- **Dropping the history toggle.** See above.
- **Promoting every download.** Board 32 and PLAN-UX say a Live Update must start from something the person did; unwanted ones make people revoke the permission.
- **A first-sync Live Update.** PLAN-ANDROID lists only the timer and downloads, and sync doesn't run as a job, so its card could outlive the app. Not built.
- **`setVolume` for the fade.** It tells the other devices the volume changed.
- **A Recap widget now.** The owner moved it to Phase 11. The snapshot reserves `recap`.

## Consequences
- **Easier:** widgets and the tile work with the app closed; the privacy rules are enforced by tests and a source scan, not by care.
- **Harder:**
  - Glance adds WorkManager, Room and DataStore to the app.
  - Live Updates can only be seen on the API 37 emulator; lock-screen widgets can't be seen at all without an Android 16 QPR1 phone.
  - Widget sizes and colors on Samsung's launcher, and taps under Samsung's battery management, are not checked.
- **To undo:** remove the `widgets/` and `tile/` packages, the receivers and the tile service from the manifest, the Glance dependencies, and the permission from PLAN-ANDROID section 19. The core's snapshot can stay; nothing reads it.

## Not verified
- Lock-screen widgets on Android 16 QPR1: no device, a known gap.
- Samsung launcher widget sizes and colors.
- Widget and tile taps with the app closed under Samsung's battery management.
- The fade over Bluetooth.
- A timer that runs out while paused, with the process dead.
- Adding the tile on Android 13 and 14.
- Whether the sleep timer's card is promoted on API 37: `OfflineAndCarTest` checks the download card's flag; no test reads the timer card's.
