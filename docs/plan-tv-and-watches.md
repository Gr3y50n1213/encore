# Plan: Play on a TV, and the watch apps (mock-up 50)

The owner approved mock-up 50 in full on 27 November 2026, including Google's Cast library, Google Play services for the Wear OS link, and Core Devices' Pebble SDK. The Windows and Linux app comes after this. Decisions: ADR android-0007 (TV). The watch ADR is written at stage W1.

As before, every build and test runs in CI, and the phones and the tablet are used when they are free. Tick each step as it lands.

## Play on a TV

- [ ] **T1. The relay and the TV player.**
  - The phone's relay: a per-session pass, Range support, and the queue's songs only (ADR android-0007).
  - A Media3 `SimpleBasePlayer` that drives a remote renderer.
  - Encore's session switches to it and back without a restart, keeping the position.
  - The "TVs" group in Play on (mock-up 50, screen 1).
  - The "Playing on <TV>" band and TV volume on Now Playing (screen 2).
- [ ] **T2. DLNA renderers** ("Music only"): SSDP discovery and AVTransport and RenderingControl SOAP. Tests against a recorded renderer in CI.
- [ ] **T3. Google Cast** with the Default Media Receiver: `media3-cast` and Google's Cast framework, from the relay's addresses. The covers and titles come from the relay too.
- [ ] **T4. The Encore TV app as a target.** It is already a "Your devices" entry through the companion. It gets the TV icon and "Encore TV app" in the TVs group, and handoff moves the music.
- [ ] **T5. Roku** (channel in `roku/`, PR 4; the phone side, `playback/tv/Roku.kt`, on `play-on-tv`; to try on the owner's Roku in developer mode): SSDP plus ECP, and an Encore channel (BrightScript, SceneGraph) built as a zip in CI. The owner installs it in developer mode.
- [ ] **T6. Encore's own Cast receiver page:** the mock-up 45 look with synced lyrics, served by the companion over https. It needs the owner's Cast registration (US$5).

## Watches

- [x] **W1. Wear OS app** (the `wear` module; ADR android-0008, 27 November 2026):
  - Now Playing as screen 4: artist, title, the lyric line in italics, previous, play or pause and next, and a progress ring;
  - the crown or bezel sets the phone's volume;
  - swipe up for Like and Up Next (the next song's title). Play on is left out: the phone can't open its sheet from the background (ADR android-0008);
  - a tile with up to four chips (the pinned mixes, then Liked songs) and "Tap a mix to play", and a complication with the playing song that opens Now Playing.

  The Wearable Data Layer carries it from the phone: two DataItems, sent only while a watch with Encore is reachable and at most twice a second, and commands back as messages. A locked watch shows no titles, only "Encore" and the controls. Guest, a PIN and Private listening send no mixes. Material 3 Expressive for Wear. CI builds, lints and tests it; the look, the bezel, the tile, the complication and the lock rules wait for the owner's Galaxy Watch 5.
- [x] **W2. Pebble** (`pebble/`; ADR android-0009, 27 November 2026):
  - a watchapp in C as screen 6: the artist, the title in bold, a rule and the lyric line in italics (slanted by the app, as the Pebble's fonts have none), and an action bar with previous, play or pause and next; a long press of the middle button likes or unlikes the song. It builds with Core Devices' Pebble SDK 4.44 in CI for all seven platforms (aplite, basalt, chalk, diorite, emery, flint, gabbro), and releases carry the .pbw;
  - on the phone, PebbleKit Android's AppMessage broadcasts written by hand (no library; `app/.../pebble/`), through Core Devices' Pebble app or the original one. They are sent only with a Pebble app installed and while the watchapp is open, from the same Now Playing as the Wear link, on the same changes and at most twice a second. The buttons map to the Wear watch's commands.

  A Pebble has no lock, so with "Hide listening history while locked" on, it shows a song only while it plays, otherwise "Encore" and the controls. No cover, mixes or next song go to it. Core's app asks the person to allow "Legacy PebbleKit" for Encore. CI builds the .pbw and tests the phone's side on the JVM. Nothing has run on a watch or the emulator: the owner has no Pebble.

## Only the owner can do

- At home to test with (owner, 27 November 2026): a Roku TV, the projector with Encore TV, and a Galaxy Watch 5. No Cast device and no Pebble, so Cast and Pebble are tested in CI only.
- Roku: turn on developer mode (Home three times, Up twice, Right, Left, Right, Left, Right on the remote) and choose a password, for sideloading.
- Cast: register a Cast receiver app with Google (one-time US$5) when T6 is reached.
- Galaxy Watch: turn on developer options and Wi-Fi debugging, so builds can be installed.
