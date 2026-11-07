# Encore core and server: track plan

You are the **core and server track**. You build the shared Rust core, the companion server, the design-token pipeline and the test fixtures. The three platform tracks build on what you write, so you run one step ahead of them. Read `CLAUDE.md`, then `core/CLAUDE.md` once it exists, then `docs/ROADMAP.md`, then this plan.

**You own:** `core/`, `server/`, `tools/` (codegen, fixtures, test media), `design/tokens.json` and the code that turns it into Swift, Kotlin and Dart constants, `.github/workflows/core.yml`, `docs/core/` (including `docs/core/CORE-API.md`).

**You never edit:** `apple/`, `android/`, `flutter/` and their CI files, and `core/encore-audio/` after Phase 1 (the Windows and Linux track writes it). You may answer requests in `docs/requests/`.

Section numbers in this plan are the plan's original numbers (`docs/SECTION-MAP.md`). The shared plans are `docs/shared/PLAN-PRODUCT.md` (product, core, server, recommender, recap, privacy) and `docs/shared/PLAN-UX.md` (design system, interaction, screens, buttons, cut-outs, swipes, checklist). Headings that start with "Section N" below hold the part of section N that belongs only to this track. Read the shared section as well.

## 1. What you build

- **The Rust workspace** from section 3 (crates `encore-core`, `encore-sources`, `encore-recs`, `encore-sync`, `encore-store`, `encore-lyrics`, `encore-audio`, `encore-recap`), compiled for every target, with no UI code.
- **The companion server** `encore-server` (Rust, Axum, SQLite, one small Docker image for amd64 and arm64).
- **The bindings:** UniFFI generates Swift and Kotlin; flutter\_rust\_bridge generates Dart. You produce the bindings and describe them in `docs/core/CORE-API.md`. Each platform track compiles the core for its own targets inside its own folder.
- **The design-token pipeline:** `design/tokens.json` to Swift, Kotlin and Dart constants (Style Dictionary or a small Rust script, section 3).
- **Fixtures:** recorded HTTP responses for every adapter, test media, sample lyrics, in `tools/fixtures/`.

The repository layout from section 3:

```
encore/
  docs/                     # this plan, ADRs, API specs, design tokens
  core/                     # Rust workspace
    encore-core/            # public API exposed over FFI
    encore-sources/         # MediaSource trait + jellyfin/, subsonic/, plex/, local/
    encore-recs/            # recommender, features, bandit, model runtime
    encore-sync/            # event log, merge rules, sync client
    encore-store/           # per-profile SQLite (sqlx or rusqlite + refinery migrations)
    encore-lyrics/          # LRC, enhanced LRC, TTML parsers + providers
    encore-audio/           # desktop audio engine (Symphonia + cpal)
    encore-recap/           # yearly recap data + card selection
  server/                   # encore-server (Axum), Dockerfile, compose.yaml
  apple/                    # Xcode project: iOS, iPadOS, macOS targets, widgets, CarPlay
  android/                  # Android Studio project: Kotlin, Jetpack Compose, Media3
  flutter/                  # Flutter app: windows/, linux/ (Android is native, see android/)
  design/                   # tokens.json -> generated Swift + Dart constants
  tools/                    # codegen scripts, fixtures, test media
  .github/workflows/        # CI per platform
```

**Stack for this track (section 3)**

| Layer | Technology | Why |
| --- | --- | --- |
| Shared logic | Rust (stable), tokio, reqwest, serde | One implementation for all six platforms |
| Local storage | SQLite, one file per profile | Simple isolation between users, easy backup |
| Server | Rust, Axum, SQLite (Postgres optional), ONNX Runtime | Shares crates with clients; one small Docker image |
| Design tokens | Style Dictionary or a small Rust script | One source of truth for colors, type, spacing, motion |
| CI | GitHub Actions: macOS runner for Apple, Linux and Windows runners | Every PR builds every target |

You can work on any computer with Rust and Docker. You cannot build or sign the Apple apps; that is the Apple track's job on a Mac.

## 2. The contract: `docs/core/CORE-API.md`

Create this file in Phase 1 and keep it current in every phase. It is the only thing the platform tracks may rely on. For each public call, event and type it gives the name, inputs, outputs, errors, which thread or async context it runs in, the phase that added it and whether it changed. A change that breaks a platform gets a version note at the top of the file and a line in the request that asked for it.

The plan already requires these things of the core. This is the first outline of the file, taken from the plan; the file you write is the contract, and the table is not.

| Area | What the core offers | Plan section |
| --- | --- | --- |
| Sources | The `MediaSource` trait, one normalized model, a library mirror in the profile's SQLite refreshed with deltas, capability flags | 4 |
| Play log | Play events written the moment a play starts and updated when it ends, with the stable song key, listened milliseconds, UTC time plus offset, how it ended, what started it, device, coarse context and lyric-line jumps, from the first playable build | 5, 8 |
| Queue | History, Up Next and Autoplay; shuffle, repeat and Smart shuffle; preload of the next track 20 seconds before the end; tells the native player what to load next; events for now playing, queue changed and sync status | 2, 8, 21 |
| Playback signals | Classifies a skip, seek, interruption, error or accident so that only real skips teach the model | 21 |
| Profiles | One folder and one SQLite database per profile; switching flushes unsent sync events; Guest and Private listening store nothing | 6, 21 |
| Sync | Local-first events with a ULID, device ID and hybrid logical clock; push, pull, WebSocket; last-writer-wins per field | 5 |
| Smart picks | Autoplay, radio, the five mixes, Smart shuffle, Why this song, Not interested, How suggestions are doing, with fallbacks | 7, 21 |
| Lyrics | LRC, enhanced LRC and TTML into one model: lines, word timings, singers, background vocals | 4 |
| Artwork palette | A colour palette per album, extracted once, for the Now Playing background and the recap | 4, 16 |
| Playlist writes | Add songs to a playlist, Up Next or Liked Songs as one call with the whole list and position, one sync event and one undo entry, duplicates found by song key | 26 |
| Downloads | Downloads, offline mode, rolling cache, storage page data | 8 |
| Widget snapshot | A small per-profile snapshot (now playing or last played, pinned mixes with thumbnails, recent albums, recap headline) written after each change | 20 |
| Handoff | The snapshot a sender packages and the confirmation steps | 11 |
| Recap | `encore-recap`: daily rollups, the recap data JSON under 100 KB, deterministic bytes | 10, 25 |
| Desktop audio | `encore-audio`: Symphonia decoding and cpal output for Windows and Linux. You create the empty crate in Phase 1; the Windows and Linux track owns and writes it from then on, because it is the only consumer | 2, 8 |

## 3. Your phases

The phase numbers and the master “done when” lines are the plan's. You build the core part of each phase one step ahead of the platform tracks. When a phase's core part is done, update `docs/core/CORE-API.md`, commit, and tell the owner so the platform sessions can `git pull`. At the start of each session read `docs/requests/` for open requests addressed to `core` and answer them first.

| Phase | Your part | Master "done when" |
| --- | --- | --- |
| 1 Foundations | Rust workspace from section 3; UniFFI and flutter\_rust\_bridge wired to a hello function; design-token codegen to Swift, Kotlin and Dart constants; real `make check-core`; CI for core and server; first `docs/core/CORE-API.md` | every platform shows a screen whose text comes from the Rust core, and CI is green. |
| 2 Jellyfin and playback | Jellyfin adapter on recorded fixtures, library mirror, browse and search queries, queue model and preload, play log with every section 5 field from the first playable build | you can sign in, browse your whole library offline-fast, and listen to an album gaplessly. |
| 3 Apple Music-grade UI and lyrics | Lyrics parsers (LRC, enhanced LRC, TTML), artwork palette for gradients, the add-to-playlist call (one call, one sync event, one undo entry), the skip signal for swipes | side by side with Apple Music, the screens feel equally smooth at 120 Hz, and lyrics sync word by word. |
| 4 Server, profiles and sync | Docker image, auth exchange, event log sync, profiles and the switcher state | two people share an iPad with fully separate history, and a like on the phone shows on the Mac within a second. |
| 5 Recommendations v1 | Signal logging, candidate pools, hand-tuned ranker, autoplay, radio, mixes, Why this song, Not interested, Private listening, Smart shuffle, skip-rate stats | autoplay's early-skip rate is clearly lower than shuffle on your own history, and the smart-picks items in section 24 pass. |
| 6 Android, Windows, Linux | Answers requests | the same account works on all six platforms and the UI passes the same visual checklist. |
| 7 Handoff and remote control | Presence, push, pull, remote control, the server WebSocket | phone-to-desktop transfer starts in under 1 second for cached tracks. |
| 8 Offline and car | Downloads, offline mode, rolling cache, storage page data | a downloaded playlist plays in airplane mode and in a car. |
| 9 Widgets, controls and live surfaces | Per-profile widget snapshot | a widget set to one profile never shows another profile's history, the locked screen shows nothing from the listening history, the sleep timer ends with the fade-out, and no widget or Live Activity makes a network call. |
| 10 Recommendations v2 | Audio analysis on the server, embeddings, trained ranker, model export | v2 beats v1's early-skip rate in an offline replay test. |
| 11 Year-end recap | `encore-recap` engine, golden files, parity tests, server jobs | two test profiles with different histories get visibly different recaps, a re-run for next year picks a different pack, the same log gives byte-identical recap data on the server and on every platform, and late plays synced before January 8 appear in the final recap. |
| 12 More sources | OpenSubsonic, then Plex, Emby, local files | a Navidrome user can do everything a Jellyfin user can, with no UI changes. |
| 13 Release | Server image, docs site, reproducible builds | The section 24 checklist is ticked for every platform. |

**Shared sections to read.** In PLAN-PRODUCT: everything except the platform-only parts: 1 to 8, 10 to 12, 15, 18, 19 (the shared part), 21 and 25. In PLAN-UX: 16 (signals and haptics tables, for which events the UI sends), 20 (the widget snapshot), 24 (the Smart picks, Year recap and Data, sync and security groups) and 26 (the core calls behind swipes and drag and drop). Read sections 1, 2 and 3 first, and the others when the phase you are on touches them.

## 4. Boards you read

You draw nothing. Read the boards that define data, signals and rules, because the core must feed them. Platform specs mention screens; take the data and the rules.

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 01 | Start here | 17 | [png](../../design/wireframes/png/01-start-here.png) | [spec](../../design/wireframes/specs/01-start-here.md) |
| 04 | System: album gradients | 9 | [png](../../design/wireframes/png/04-system-album-gradients.png) | [spec](../../design/wireframes/specs/04-system-album-gradients.md) |
| 07 | Search | 17 | [png](../../design/wireframes/png/07-search.png) | [spec](../../design/wireframes/specs/07-search.md) |
| 10 | Lyrics | 9 | [png](../../design/wireframes/png/10-lyrics.png) | [spec](../../design/wireframes/specs/10-lyrics.md) |
| 12 | Output and handoff | 11 | [png](../../design/wireframes/png/12-output-and-handoff.png) | [spec](../../design/wireframes/specs/12-output-and-handoff.md) |
| 14 | Profile switcher | 6 | [png](../../design/wireframes/png/14-profile-switcher.png) | [spec](../../design/wireframes/specs/14-profile-switcher.md) |
| 15 | Made for you playlist | 7.1 | [png](../../design/wireframes/png/15-made-for-you-playlist.png) | [spec](../../design/wireframes/specs/15-made-for-you-playlist.md) |
| 16 | Year recap (entry card, story player, share sheet; card count, timing and where numbers are computed are superseded by 39 to 41) | 10, 25 | [png](../../design/wireframes/png/16-year-recap.png) | [spec](../../design/wireframes/specs/16-year-recap.md) |
| 18 | Servers and sources | 4 | [png](../../design/wireframes/png/18-servers-and-sources.png) | [spec](../../design/wireframes/specs/18-servers-and-sources.md) |
| 19 | Privacy and data | 12, 19 | [png](../../design/wireframes/png/19-privacy-and-data.png) | [spec](../../design/wireframes/specs/19-privacy-and-data.md) |
| 21 | Connect a server | 4, 17 | [png](../../design/wireframes/png/21-connect-a-server.png) | [spec](../../design/wireframes/specs/21-connect-a-server.md) |
| 23 | Mixes tab | 7.1, 21 | [png](../../design/wireframes/png/23-mixes-tab.png) | [spec](../../design/wireframes/specs/23-mixes-tab.md) |
| 31 | Live surfaces and rules | 20 | [png](../../design/wireframes/png/31-live-surfaces-and-rules.png) | [spec](../../design/wireframes/specs/31-live-surfaces-and-rules.md) |
| 37 | Smart picks: signals and rules | 21 | [png](../../design/wireframes/png/37-smart-picks-signals-and-rules.png) | [spec](../../design/wireframes/specs/37-smart-picks-signals-and-rules.md) |
| 38 | Smart picks: screens and states | 21 | [png](../../design/wireframes/png/38-smart-picks-screens-and-states.png) | [spec](../../design/wireframes/specs/38-smart-picks-screens-and-states.md) |
| 39 | Year recap: the story | 10, 25 | [png](../../design/wireframes/png/39-year-recap-the-story.png) | [spec](../../design/wireframes/specs/39-year-recap-the-story.md) |
| 40 | Year recap: packs and years | 10, 25 | [png](../../design/wireframes/png/40-year-recap-packs-and-years.png) | [spec](../../design/wireframes/specs/40-year-recap-packs-and-years.md) |
| 41 | Year recap: devices and states | 10, 25 | [png](../../design/wireframes/png/41-year-recap-devices-and-states.png) | [spec](../../design/wireframes/specs/41-year-recap-devices-and-states.md) |
| 42 | Swipe gestures: mini player, Now Playing and rows | 16, 26 | [png](../../design/wireframes/png/42-swipe-gestures-mini-player-now-playing-and-rows.png) | [spec](../../design/wireframes/specs/42-swipe-gestures-mini-player-now-playing-and-rows.md) |
| 43 | Drag and drop on desktop and tablets | 16, 26 | [png](../../design/wireframes/png/43-drag-and-drop-on-desktop-and-tablets.png) | [spec](../../design/wireframes/specs/43-drag-and-drop-on-desktop-and-tablets.md) |

## Section 18: Libraries (shared)

| Library | Used for | License |
| --- | --- | --- |
| UniFFI | Rust to Swift and Kotlin bindings | MPL-2.0 |
| flutter\_rust\_bridge | Rust to Dart bindings | MIT |
| Axum | Companion server | MIT |
| SQLite | Local and server storage | Public domain |
| ONNX Runtime | On-device ranking models | MIT |
| ttml\_processor | Parsing word-timed lyrics | MIT |

The audio embedding model and the nearest-neighbour library each need a licence check before release; some audio analysis tools use copyleft licences that fit the server (AGPL) but not the apps (MPL) (section 21, rule 6). Check each licence again when you add a library, and keep `THIRD_PARTY_LICENSES` current.

## Section 24: Your acceptance checklist

These groups of the section 24 checklist are yours. The platform tracks run the rest on their devices.

**Smart picks**

- [ ] Offline replay on at least three test histories (light, heavy, one with many skips) beats plain shuffle on skip rate of app-chosen songs, with the numbers saved in CI.
- [ ] Unit tests prove that interruptions, errors, seeks and plays under 2 s write no negative signal.
- [ ] Private listening, Guest and Learn from my listening off write zero events, checked at the database level.
- [ ] Reset suggestions and Clear listening history behave differently as designed, and both sync as tombstones that another device cannot undo.
- [ ] A cold profile, a profile with 49 plays and one with 50 plays each show the right mixes, labels and fallbacks.
- [ ] Every smart surface has a working fallback with no model, with the server off and with no network.
- [ ] Budgets hold on a mid-range phone: next-song decision under 50 ms, mix build under 20 s and under 150 MB.
- [ ] Licences of the audio embedding model and the nearest-neighbour library are checked and recorded in `THIRD_PARTY`.

**Year recap**

- [ ] The same log, year and seed produce byte-identical recap data from the server build and from each platform's core, checked against golden files in CI.
- [ ] Snapshot tests cover every pack and every beat on the extreme histories in section 25 (one artist, 5,000 artists, very light and very dark art, very long names, non-Latin and right-to-left scripts, 100 plays and 100,000 plays).
- [ ] Private listening, Guest, interruptions and plays under 2 s are never counted; the same song on two albums counts once; plays land on the day of their own time zone across a daylight saving change.
- [ ] Plays synced between 31 December and 8 January appear in the final recap, and after 8 January the recap never changes. Clear listening history restarts the recap as designed.
- [ ] A recap build over 100,000 plays runs in the background on a mid-range phone with the server off without blocking playback, and its time is recorded.
- [ ] A new year picks a different pack and a lead hue at least 60° from last year's; an older app draws the fallback look and says so; a profile with under 10 plays shows no recap and one with 10 plays shows the short version.
- [ ] Every card has a text version for screen readers, Reduce Motion shows the final state, and nothing flashes more than three times a second.

**Data, sync and security**

- [ ] Local and server databases have versioned migrations tested forward and, where promised, backward, and a backup and restore test passes.
- [ ] Sync conflict tests cover two devices editing a like, a playlist and Not interested at the same time while offline.
- [ ] No telemetry or crash report leaves the device without the user pressing Send; the network log of a full session shows only the media server, the companion server and the providers the user enabled.
- [ ] Tokens live in the OS keychain or keystore; nothing sensitive appears in logs, backups or screenshots.
- [ ] Dependency and licence scans run in CI and fail on a new unreviewed licence.

**Unverified items (core and server)**

| Item | Why it is open | Check against |
| --- | --- | --- |
| Every smart-picks weight, half-life, threshold and budget | Starting values, not measurements | Offline replay on real histories |
| Audio embedding model and nearest-neighbour library licences | Not yet chosen or checked | The licence texts, before release |
| Recap numbers: card timing, thresholds, 60° hue gap, 90-day device window | Starting values, not measurements | Testing with real histories and real people |
| Recap build time for 100,000 plays | Not measured | A benchmark on a mid-range phone and on a Raspberry Pi 5 |
| Swipe numbers: 12 pt lock, 60 pt arm, 500 pt/s flick, 600 ms coalesce, 40 pt rubber band | Starting values, not measurements | Trying them on real phones, and against Apple Music |
| SwiftUI, Compose and Flutter drag and drop API names and touch behaviour | Written from memory of the frameworks, not checked against the pinned versions | Each framework's official documentation and a real device |

## Tests, fixtures and CI

- Every Rust change has unit tests. Adapters use recorded HTTP fixtures in `tools/fixtures/`, so tests run without a live server (section 4).
- Golden files for the recap: fixed logs give exact expected recap data, and the same logs run through the server build and the core give identical bytes (section 25, Tests).
- Offline replay on at least three test histories (light, heavy, one with many skips) against plain shuffle, with the numbers saved in CI (section 24, Smart picks).
- `make check-core` is real after Phase 1: fmt, clippy and tests for `core/` and `server/`. CI for the core and server is `.github/workflows/core.yml`. The platform CI files are the platform tracks'.

## Spikes to run early (core and server)

- Spike 1: **Rust core over FFI on all six platforms**: UniFFI to Swift and Kotlin, flutter_rust_bridge to Dart. Phase 1 proves this; if one path fails, stop and report.
- Spike 2: **Jellyfin against real files**: library sync speed on a large library, gapless albums, lyrics endpoints and what word timing exists, transcoding versus direct play.
- Spike 7: **Smart picks offline replay harness**: replay a real history against plain shuffle, as the plan's phase 5 requires.
- Spike 8: **Recap parity and speed**: run a stub of the `encore-recap` engine over a synthetic log of 100,000 plays on the server build, an iPhone, an Android phone and a Raspberry Pi 5. The bytes must match and the time must be recorded (plan section 25).

## Hand-offs

- **To the platform tracks you give:** the bindings, `docs/core/CORE-API.md`, the token constants, and answers in `docs/requests/`.
- **From the platform tracks you take:** requests, and facts about platform behaviour that change a core rule.
- **To the owner you give:** at the end of each phase, what is done in the core, what the platform tracks can now build, and what you could not verify without a device.
