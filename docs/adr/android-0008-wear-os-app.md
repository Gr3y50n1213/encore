# ADR android-0008: The Wear OS app

- **Status:** accepted
- **Date:** 27 November 2026
- **Owner's decision:** mock-up 50 approved in full on 27 November 2026 (screens 4 and 5), including Google Play services' Wearable Data Layer for the link between the phone and the watch.
- **Plan:** `docs/plan-tv-and-watches.md`, stage W1

## Context

The owner wants Encore on a Wear OS watch (the Galaxy Watch 4 and later, the Pixel Watch): Now Playing with the line being sung, the controls, the crown for volume, Like, Up Next, and a tile and a complication to start a mix and see the song (mock-up 50). The music plays on the phone; the watch is its remote. The phone's privacy rules apply: nothing personal on a locked screen, no listening history under Guest or Private listening (PLAN-PRODUCT section 12), and no new connection to anything but the user's own devices.

## Decision

**A `wear` module** in `android/`, a second app with the phone app's application ID (`uk.gcartee.encore`, `.debug` for debug builds), signed with the same key (the owner's test key, else the debug key). The Data Layer only connects apps with the same ID and the same signature, so no other app can read what the phone sends or send it commands. It is Wear OS 3 and later (minSdk 30), not standalone: without the phone it says "Open Encore on your phone". It is built with Wear Compose Material 3 (the expressive Material for Wear), Tiles with ProtoLayout Material 3, and the complication data source library.

**The link: Google's Wearable Data Layer** (`play-services-wearable`), in both apps. The protocol is one small shared Kotlin folder, `android/wearlink/src`, that both modules compile, with JVM tests in the wear module.

- **Phone to watch, two DataItems.** `/encore/now`: title, artist, album, the lyric line being sung (or empty), playing or not, the position and the wall-clock time it was taken, the duration, liked, the next song's title, a cover JPEG of at most 200 px as an Asset, and `private` (the phone's "Hide listening history while locked"). `/encore/mixes`: up to four pinned mixes (ID, title, colours) and whether to offer Liked songs. Each has a version number; a watch that finds another version shows the empty state.
- **Only what changes, at most twice a second.** The phone sends `/encore/now` when the song, the play state, the like, the next song, the lyric line or the cover changes, or the position jumps by over 2 seconds (a seek). The position moving on is never sent: the watch works it out, as the phone's widgets do. A burst of changes waits out the rest of the 500 ms interval, and then the last state goes.
- **Only while a watch is there.** The watch app declares the capability `encore_watch`; the phone sends nothing unless a node with it is reachable (`CapabilityClient`, `FILTER_REACHABLE`), which is narrower than any connected node: a watch without Encore costs nothing. On a phone without Play services the link does nothing at all.
- **No new core calls.** Now Playing comes from the player (`PlaybackHub`), the queue and `are_liked`; the mixes and the cover come from the widget snapshot the core already writes (CORE-API "Widgets"). The lyric line comes from `lyrics()`, as the lyrics view reads it, and only for a song the library marks as having lyrics, so a watch never starts a lookup the phone wouldn't. Unsynced lyrics send no line.
- **Watch to phone, messages** on `/encore/cmd`, one UTF-8 command each: `play`, `pause`, `next`, `previous`, `like`, `unlike`, `volume:+1`, `volume:-1`, `mix:<id>` and `liked`. Anything else is ignored, and a mix ID must be a plain ASCII name. The phone's `WearCommandService` (a `WearableListenerService`, which checks that the caller is Play services) maps each to what the phone's own controls do. While the player runs, it goes through `PlaybackHub` and the player, and through `RemoteControl` while the phone controls another device. Skips count as a headset's. Volume moves the phone's media volume one step, without a slider on the phone. A like is only ever for the song playing, and never starts the core. With the player stopped, Play, a mix and Liked songs start the playback service the way the Quick Settings tile does: `startForegroundService` with a media button or the private Start a mix action (`WidgetIntents`), and `WidgetStarts` keeps the foreground promise.

**Privacy on the watch.**

- A locked watch (`KeyguardManager.isDeviceLocked`) shows no title, artist, line, cover, next song or mix name anywhere: Now Playing shows "Encore" and the controls, the tile says "Unlock your watch to see your mixes", and the complication says "Encore".
- On the watch face, with the phone's "Hide listening history while locked" on (the default), the complication shows a song only while it plays, never the last one played, as on the phone's lock screen.
- Guest, a profile with a PIN, and Private listening send no mixes. Guest and a PIN also hide Liked songs. The controls still work.
- What the watch received is kept in the watch app's private storage only (no backup), so the tile and complication draw without waiting.

## What isn't done

- **Play on** from the watch is left out. Opening the phone's Play on sheet needs an activity started on the phone from the background, which Android refuses, or another library (`wear-remote-interactions`), which wasn't approved.
- **Songs on the watch** for runs without the phone (downloads to the watch, its own playback and Bluetooth headphones) is later work, as mock-up 50 says.
- **Up Next** shows the next song's title only, not the queue.
- **Starting music with Encore closed.** Android 12 and later may refuse a foreground-service start from the background. Then a mix or Play from a watch with Encore closed on the phone does nothing, and the reason is logged. The tile on the phone opens the app instead, which a watch can't do.
- **The complication and the lock.** Wear OS doesn't tell a data source when the watch locks. The complication is redrawn whenever the song or its play state changes, and at least every 5 minutes, so a watch face may keep the last text for up to that long after the watch locks.

## Consequences

- The phone app now links Google Play services' wearable, base, basement and tasks libraries (Android Software Development Kit License, closed, not copyleft). The phone already relies on Play services for Android Auto. An F-Droid build, if one ever comes, would leave the watch link out.
- CI builds, lints and unit tests the wear module in `make check-android`, and uploads its release APK with the phone's.
- Only the real watch can show the look on a round screen, how the Galaxy Watch 5's bezel and crown feel as volume, the tile and complication on a watch face, the lock rules, and whether a start from the watch is allowed with Encore closed. The owner turns on developer options and Wi-Fi debugging on the watch to install builds.
