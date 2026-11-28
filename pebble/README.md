# Encore for Pebble

A watchapp that shows what Encore is playing on your phone: the artist, the title, and the lyric line being sung. Its buttons control the phone:

- **Up:** the previous song.
- **Select:** play or pause.
- **Down:** the next song.
- **Hold select:** like the song, or unlike it if you liked it already. The watch buzzes once, and a heart shows that the song is liked.

It runs on every Pebble: the original Pebble and Pebble Steel, Pebble Time, Time Steel, Time Round, Pebble 2, Time 2, and Core Devices' new Pebbles. The design is mock-up 50, screen 6, and the decisions are in `docs/adr/android-0009-pebble.md`.

## Install it

1. Get `encore-pebble-<version>.pbw` from a release on GitHub, or `encore-pebble` from a run of the `pebble` workflow.
2. Open the .pbw on the phone with your Pebble app (Core Devices' Pebble app, or the original one). For example, tap it in your Files app and choose the Pebble app. The Pebble app installs it on the watch.
3. The first time, the Pebble app asks whether Encore may use **Legacy PebbleKit** to exchange messages with phone apps. Allow it, or the watch can't reach Encore.
4. Open Encore on the phone, then open Encore on the watch.

## How it works

The music plays on the phone; the watch is its remote. The watchapp and Encore talk through the Pebble app with AppMessages, PebbleKit's broadcasts on the phone. Encore adds no library for it.

- When the watchapp opens, it says hello. Encore then sends Now Playing whenever it changes, at most twice a second, while the watchapp answers.
- Encore sends the title, the artist, the line being sung (only for songs with synced lyrics), whether it is playing, and whether it is liked.
- With "Hide listening history while locked" on (the default), a song shows only while it plays. A paused or stopped song shows as "Encore" and the controls, and its title never leaves the phone. Covers, mixes and the next song are never sent.
- The buttons do what the Wear OS watch's buttons do.
- The watch's buttons work while Encore is running on the phone.

## Build it

CI builds it (`.github/workflows/pebble.yml`, through `tools/ci/build_pebble.sh`) with Core Devices' Pebble SDK. To build it yourself, install the SDK (developer.repebble.com/sdk), then run `pebble build` in this folder. The .pbw lands in `build/`.

`tools/make_icons.py` draws the action bar's icons in `resources/images/`.

## Not tried yet

Nobody has run this on a watch or the emulator yet: the owner has no Pebble. What waits for a real watch is listed in the ADR.
