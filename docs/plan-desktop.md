# Plan: the Windows and Linux app

The owner said, on 27 November 2026, to start this after Play on a TV and the watch apps. The track's own plan is `docs/windows-linux/PLAN-WINDOWS-LINUX.md`. Its libraries were approved with the plan:

- Symphonia and cpal for audio;
- Riverpod and go_router for Flutter;
- Inter and Material Symbols;
- flutter_rust_bridge for the bindings.

The look is the Android app's Material 3 Expressive, drawn by the Encore theme in Flutter, with no glass. It follows the same design boards and approved mock-ups as Android.

As everywhere in this run, every build and test happens in CI: on Windows and Linux runners. The owner sees the Windows build on their PC once it is ready.

## Stages

- [ ] **D1. Foundations (Phase 1).**
  - The Flutter project in `flutter/` for Windows and Linux.
  - The core built for both by flutter_rust_bridge.
  - The first screen's text comes from the core.
  - CI (`flutter.yml`) builds, analyses and tests on Windows and Linux runners, and uploads the Windows build and a Linux bundle.
- [ ] **D2. The audio engine** (`core/encore-audio`):
  - Symphonia decodes, cpal plays;
  - gapless from one song to the next;
  - seek, volume and the ReplayGain the core gives;
  - events for position, end and errors.

  It takes what `stream_for` gives: a URL with headers, or a downloaded or local file. It is exposed to Dart through `encore-dart`, and tested in CI on both systems with generated tones.
- [ ] **D3. Sign in and play (Phase 2):**
  - Connect (all four media servers, as on Android);
  - Home, Library, album and artist pages, Search;
  - the queue, the mini player and Now Playing;
  - media keys with SMTC on Windows and MPRIS on Linux.
- [ ] **D4. Lyrics and the rest of the Android screens (Phase 3):**
  - synced lyrics, Sleep mode, playlists, likes;
  - resizable layouts in the four size classes;
  - drag and drop.
- [ ] **D5. Profiles, sync, mixes, handoff, downloads, the recap (Phases 4 to 11):** each on the core calls Android already uses.
- [ ] **D6. Release:** a Windows installer and a Linux bundle on the GitHub release, built in `release.yml`.

## Only the owner can do

- Run the Windows build on their PC and say how it feels.
- Linux: there is no Linux machine at home. The Linux build is checked in CI only, unless the owner has a Linux computer or VM to try.
