# ADR android-0009: The Pebble watchapp

- **Status:** accepted
- **Date:** 27 November 2026
- **Owner's decision:** mock-up 50 approved in full on 27 November 2026 (screen 6), including Core Devices' Pebble SDK.
- **Plan:** `docs/plan-tv-and-watches.md`, stage W2

## Context

The Pebble app's own music controls already work with Encore through its media session. The owner wants an Encore watchapp on top: the artist, the title, the line being sung, and Like with a long press of the middle button, on the black-and-white and the colour Pebbles, the old ones and Core Devices' new ones (mock-up 50, screen 6). The music plays on the phone. The privacy rules of the Wear OS app apply (ADR android-0008). The owner has no Pebble, so all of this is built and tested in CI only.

## Decision

**A watchapp in C**, `pebble/`, built with Core Devices' Pebble SDK (4.44, through `pebble-tool` 5.2.0; `tools/ci/build_pebble.sh`) for aplite, basalt, chalk, diorite, emery, flint and gabbro, every platform the SDK names. One window shows:

- the artist, then the title in bold;
- a rule, and the lyric line in italics. The Pebble's system fonts have no italic, so the line is drawn upright and each pixel row is then shifted right a little more the higher it sits in its line, in the frame buffer;
- an action bar: up for previous, select for play or pause, down for next. A long press of select likes the song, or unlikes it if it is liked already, and a heart shows that it is liked.

With nothing from the phone it says "Open Encore on your phone".

**The link: PebbleKit Android's broadcasts, written by hand** (`android/app/.../pebble/`), with no new library. The names and the format come from PebbleKit's source (github.com/pebble/pebble-android-sdk: `Constants.java`, `PebbleKit.java`, `util/PebbleDictionary.java`):

- **Phone to watch:** `com.getpebble.action.app.SEND` with `uuid` (the watchapp's UUID as a `java.util.UUID`), `transaction_id` (an int) and `msg_data`. `msg_data` is a JSON array of `{key, type, length, value}`: `type` is `string`, `int`, `uint` or `bytes`, and `length` is an integer's width in bytes (1, 2 or 4), 0 for a string. The send names the Pebble app's package, so no other app receives the titles. The Pebble app answers `com.getpebble.action.app.RECEIVE_ACK` or `RECEIVE_NACK` with the transaction ID.
- **Watch to phone:** `com.getpebble.action.app.RECEIVE` with the same extras. The phone checks the UUID and answers every message with `com.getpebble.action.app.ACK` (or `NACK` for one it can't read) and the transaction ID. PebbleKit says every message must be answered, or the watch times out.
- **The keys** (`pebble/package.json`, `messageKeys`): title 1, artist 2, lyric 3, playing 4, liked 5 and private 6 to the watch, all of them in every message; cmd 7 from the watch: 1 hello, 2 previous, 3 play or pause, 4 next, 5 like. Texts are cut to 100, 100 and 160 bytes of UTF-8, so a message fits the watch's 512-byte inbox.

**When it runs.**

- Only with a Pebble app installed: Core Devices' (`coredevices.coreapp`) or the original (`com.getpebble.android.basalt`), checked with `PackageManager`. The manifest's `<queries>` names both.
- Only while the watchapp is open. It says hello when it opens, and again every 15 seconds until the phone answers. The phone stops sending when 10 seconds pass without an ACK, or after a refusal and two retries, until the watch says something again.
- Now Playing is built once by `WearLink` for both watches: on the same changes and within the same limit of two sends a second (`Throttle`), from the same `NowPlaying`. The Pebble gets only what it shows, and only when that changes: the position moving on is never sent, but the lyric line is.
- **The receiver is registered at run time, not in the manifest.** The Pebble app sends `RECEIVE` to every app, with no package named (Core's `PebbleKitClassic.kt` sends it as an ordered broadcast). Android 8 and later don't deliver such a broadcast to a receiver declared in an app's manifest. So the watch's buttons work while Encore's process runs, like the Wear link, which runs only where the core runs. The receiver is exported, as the Pebble app is another app.
- **The buttons** do what the Wear OS watch's commands do (`WearCommands`): through `PlaybackHub` and `RemoteControl`, skips as a headset's, a like only for the song playing, and with the player stopped, a start of the playback service the way the Quick Settings tile does. Play or pause follows the player's own state. Like or unlike follows what the watch was last shown.

**Privacy.**

- A Pebble has no lock, and the watchapp is on the wrist for anyone to see. So it follows the phone's lock-screen rule, as the Wear OS complication does (`WatchPrivacy`, `onWatchFace`). With "Hide listening history while locked" on, the default, only a song playing now is shown. Once it stops, the phone sends `private` and no title, artist or line, and the watch shows "Encore" and the controls.
- The phone sends no cover, no mixes and no next song to a Pebble.
- What the watch sends (button presses) goes through the Pebble app's broadcast to every app. It says nothing about the music.
- Anything the Pebble app sends with Encore's UUID can press these buttons: play, pause, skip, like and unlike the song playing. A media button does the same through Android already, so this adds only Like. Nothing can be read back that way.

## What Core Devices' app supports

Read from github.com/coredevices/mobileapp (libpebble3, 27 November 2026; `PebbleKitClassic.kt`, `CompanionAppLifecycleManager.android.kt`, `AppPermissions.kt`, `LibPebbleConfig.kt`):

- It still takes PebbleKit's `SEND`, `ACK` and `NACK` broadcasts, reads `uuid` as a `UUID` (or a string), and sends `RECEIVE`, `RECEIVE_ACK` and `RECEIVE_NACK`. It listens only while the watchapp runs on the watch.
- This is its "Legacy PebbleKit" setting, on by default. A watchapp with an Android companion that declares PebbleKit 2 uses that instead. PebbleKit 2 is Core's newer link, addressed to the companion's own package, but it needs Core's library, which wasn't approved.
- Each app must be allowed "Legacy PebbleKit" ("Broadcast messages to every app on your phone"). The watchapp declares it in `usesPermissions`, and Core's app asks the person. Until they allow it, Core's app refuses the phone's messages and passes none of the watch's.
- Its transaction IDs are one byte, so the phone counts 0 to 255.

## What isn't verified

No Pebble and no Pebble emulator ran. CI builds the .pbw for every platform and checks that each is in the bundle. The JVM tests cover the JSON, the privacy rule, the commands and the keys shared with the C code. These are untried:

- the layout on each screen, and the slant on the black-and-white and the round ones;
- the icons;
- the long press;
- the AppMessage round trip through Core's app and the original Pebble app, and its permission prompt;
- whether a start with the player stopped is allowed from the background.

## Consequences

- No library in the phone app. The Pebble SDK, `pebble-tool`, `uv` and the toolchain are build tools in CI (`pebble.yml`, and the `pebble` job in `release.yml`, which adds `encore-pebble-<version>.pbw` to a release), never shipped with Encore.
- A Pebble app installed later is picked up the next time Encore starts.
- Moving to PebbleKit 2 later would take Core's library, or its protocol rewritten by hand, and would let the phone receive the watch's messages with Encore closed.
