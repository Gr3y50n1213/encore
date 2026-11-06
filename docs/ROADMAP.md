# Roadmap: thirteen phases, four tracks

The plan's thirteen phases and their "done when" lines are unchanged. What changed is who builds each phase. The core and server track builds the shared engine; the Apple, Android and Windows and Linux tracks build their apps on it, at the same time, on different computers. Phase numbers still mean what they meant in every other document.

## How the tracks fit together

- **The core runs one step ahead.** A platform track can only build a feature once the core part of it exists and is described in `docs/core/CORE-API.md`. Run the core session for a phase first, `git pull` on the other computers, then run the platform sessions side by side.
- **Phase 1 is four small jobs.** The core track makes the workspace, the bindings and the token codegen. Each platform track makes its own project, compiles the core for its targets and shows a screen whose text comes from the core.
- **Android and Windows and Linux do not wait for Apple.** The master plan built Apple first and then brought the other platforms in at Phase 6. With separate tracks, Android and Windows and Linux follow Phases 2 to 5 in the same order as Apple, building the same things for their platform, as soon as the core part exists. Phase 6's list (Media3 on Android, the Rust audio engine, SMTC and MPRIS on desktop, cut-outs and fold postures, swipes, drag and drop) is added to those phases in the table below. Phase 6's "done when" is the joint gate, and the owner checks it.
- **A phase is finished for the whole project** when every track's part has passed and the owner has checked its "done when" line on real devices.

## The master roadmap (word for word from the plan)

The plan's first sentence below, "Build one great experience on Apple devices first", described one builder working alone. With separate tracks, the rules in "How the tracks fit together" above decide the order; everything else below is unchanged.

Build one great experience on Apple devices first, then bring the server, the brains and the other platforms. Each phase ends with a usable app and a clear "done when" check, so you can stop at any phase and still have something good.

1. **Foundations.** Monorepo, Rust workspace, UniFFI and flutter\_rust\_bridge wired to a hello-world on every platform, design tokens pipeline, CI building all six targets.
   - Done when: every platform shows a screen whose text comes from the Rust core, and CI is green.
2. **Jellyfin and playback on iPhone and Mac.** Jellyfin adapter, library mirror, browse and search, queue, gapless playback, lock screen and media keys. The local play log is written from the first playable build with every field in section 5 (stable song key, listened milliseconds, UTC offset, lyric-line jumps), because history cannot be rebuilt later.
   - Done when: you can sign in, browse your whole library offline-fast, and listen to an album gaplessly.
3. **Apple Music-grade UI and lyrics.** Home, Library, Album, Artist, Now Playing, mini player, the full lyrics view with word sync, iPad and Mac layouts, Liquid Glass, the mini player and artwork swipes and row swipes, and Mac drag and drop (section 26).
   - Done when: side by side with Apple Music, the screens feel equally smooth at 120 Hz, and lyrics sync word by word.
4. **Companion server, profiles and sync.** Docker image, auth exchange, event log sync, multiple profiles on one device with the switcher.
   - Done when: two people share an iPad with fully separate history, and a like on the phone shows on the Mac within a second.
5. **Recommendations v1.** Signal logging, candidate pools, hand-tuned ranker, autoplay, radio, home mixes, "Why this song?", skip-rate stats page, Not interested, Private listening, Smart shuffle and the Mixes and suggestions settings (section 21).
   - Done when: autoplay's early-skip rate is clearly lower than shuffle on your own history, and the smart-picks items in section 24 pass.
6. **Android, Windows, Linux.** Compose app on Android and Flutter app on Windows and Linux reaching feature parity with phases 2 to 5, including cut-outs and fold postures (section 23), swipes and drag and drop on Android tablets, Windows and Linux (section 26), Media3 on Android, Rust audio on desktop, SMTC and MPRIS.
   - Done when: the same account works on all six platforms and the UI passes the same visual checklist.
7. **Handoff and remote control.** Presence, push, pull and remote control; Apple `NSUserActivity`.
   - Done when: phone-to-desktop transfer starts in under 1 second for cached tracks.
8. **Offline and car.** Downloads, offline mode, CarPlay, Android Auto, and progress for user-started downloads (section 20).
   - Done when: a downloaded playlist plays in airplane mode and in a car.
9. **Widgets, controls and live surfaces.** Per-profile widget snapshot in the core, Home Screen, Lock Screen and Mac widgets, Control Center controls, the sleep timer with its Live Activity and Live Update, download progress, then the Android Glance widgets and Quick Settings tile, and the Windows Widgets board widget (section 20).
   - Done when: a widget set to one profile never shows another profile's history, the locked screen shows nothing from the listening history, the sleep timer ends with the fade-out, and no widget or Live Activity makes a network call.
10. **Recommendations v2.** Audio analysis on the server, embeddings, trained ranker, on-device Core ML and ONNX models, NPU paths.
    - Done when: v2 beats v1's early-skip rate in an offline replay test.
11. **Year-end recap.** The `encore-recap` engine in the core (section 25), four design packs, sharing, the detail page, monthly and year-so-far recaps, and the same recap on the server and on every device.
    - Done when: two test profiles with different histories get visibly different recaps, a re-run for next year picks a different pack, the same log gives byte-identical recap data on the server and on every platform, and late plays synced before January 8 appear in the final recap.
12. **More sources.** OpenSubsonic (Navidrome and others), then Plex, Emby and local files.
    - Done when: a Navidrome user can do everything a Jellyfin user can, with no UI changes.
13. **Release.** Docs site, install guides, App Store and TestFlight, Play Store and F-Droid, Flathub, Windows installer (winget), reproducible builds. The section 24 checklist is ticked for every platform.

## Who builds what

| Phase | Core and server | Apple | Android | Windows and Linux |
| --- | --- | --- | --- | --- |
| 1 Foundations | Rust workspace from section 3; UniFFI and flutter\_rust\_bridge wired to a hello function; design-token codegen to Swift, Kotlin and Dart constants; real `make check-core`; CI for core and server; first `docs/core/CORE-API.md` | Xcode project for iOS, iPadOS and macOS; builds the core for Apple targets; hello-world screen from the core; CI on a macOS runner | Android Studio project; builds the core for Android; hello-world screen from the core; CI | Flutter project for Windows and Linux; flutter\_rust\_bridge hello-world; CI on Windows and Linux runners |
| 2 Jellyfin and playback | Jellyfin adapter on recorded fixtures, library mirror, browse and search queries, queue model and preload, play log with every section 5 field from the first playable build | Sign in, browse, search, queue; gapless playback with AVFoundation; lock screen and media keys on iPhone and Mac | The same on Android with Media3 and a MediaSession (master Phase 6) | The same with the Rust audio engine (Symphonia and cpal), SMTC and MPRIS (master Phase 6) |
| 3 Apple Music-grade UI and lyrics | Lyrics parsers (LRC, enhanced LRC, TTML), artwork palette for gradients, the add-to-playlist call (one call, one sync event, one undo entry), the skip signal for swipes | Home, Library, Album, Artist, Now Playing, mini player, word-synced lyrics, iPad and Mac layouts, Liquid Glass, swipes, Mac and iPad drag and drop | The same screens in Compose with Material 3 Expressive, cut-outs and fold postures, swipes, tablet drag and drop (master Phase 6) | The same screens in Flutter with the Encore theme, resizable windows, drag and drop (master Phase 6) |
| 4 Server, profiles and sync | Docker image, auth exchange, event log sync, profiles and the switcher state | Profile switcher, sync status; two people on one iPad | The same | The same |
| 5 Recommendations v1 | Signal logging, candidate pools, hand-tuned ranker, autoplay, radio, mixes, Why this song, Not interested, Private listening, Smart shuffle, skip-rate stats | Mixes tab, Made for you, Why this song, Not interested, Mixes and suggestions settings | The same | The same |
| 6 Android, Windows, Linux | Answers requests | Not involved | Phases 2 to 5 for Android, with Media3, cut-outs and folds | Phases 2 to 5 for Windows and Linux, with the Rust audio engine, SMTC, MPRIS |
| 7 Handoff and remote control | Presence, push, pull, remote control, the server WebSocket | Output picker, pull banner, `NSUserActivity` | Output picker, pull banner | Output picker, pull banner |
| 8 Offline and car | Downloads, offline mode, rolling cache, storage page data | Downloads screens, CarPlay, progress for user-started downloads | Downloads screens, Android Auto, progress for user-started downloads | Downloads screens, taskbar and notification progress |
| 9 Widgets, controls and live surfaces | Per-profile widget snapshot | Widgets, controls, sleep timer Live Activity, Mac widgets | Glance widgets, Quick Settings tile, Live Updates | Windows Widgets board provider (Store build), Jump List, taskbar progress; Linux progress through notifications and launchers |
| 10 Recommendations v2 | Audio analysis on the server, embeddings, trained ranker, model export | Core ML and Neural Engine paths | ONNX Runtime with NNAPI or QNN | ONNX Runtime with DirectML on Windows |
| 11 Year-end recap | `encore-recap` engine, golden files, parity tests, server jobs | Draws the four packs in SwiftUI, share cards, detail page, Home entry | The same in Compose | The same in Flutter |
| 12 More sources | OpenSubsonic, then Plex, Emby, local files | No UI change | No UI change | No UI change |
| 13 Release | Server image, docs site, reproducible builds | App Store and TestFlight | Play Store and F-Droid | Windows installer (winget), Flathub |

## Joint gates the owner checks

- Phase 1: every platform shows a screen whose text comes from the Rust core, and CI is green for all four tracks.
- Phase 6: the same account works on all six platforms and the UI passes the same visual checklist (section 24).
- Phase 11: the same log gives byte-identical recap data on the server and on every platform.
- Phase 13: the section 24 checklist is ticked for every platform.
