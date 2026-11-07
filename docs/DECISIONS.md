# Decisions log

Status of every decision in this project. Claude Code: build on "Decided" and "Chosen for the owner" rows. Treat "Assumed" rows as defaults you may change when the owner says so, and ask before building anything that is hard to undo on top of them. Never treat "Unverified" rows as fact.

Last updated: 6 November 2026, Phase 2 (core): the owner's choices on fixtures and libraries, the name kept, and new unverified items (section 5). Earlier the same day: the colours and the type scale (section 1). Before that: 5 November 2026, after the plan was split into four tracks (core and server, Apple, Android, Windows and Linux; `docs/ROADMAP.md`), on top of the year recap engine, swipe gestures and drag and drop (plan sections 25 and 26, boards 39 to 43).

## 1. Decided by the owner

| Decision | Choice |
| --- | --- |
| What it is | A free, open-source, privacy-first music streaming app for the owner's self-hosted library |
| Platforms | macOS, iOS, iPadOS, Windows, Android, Linux |
| Architecture | Shared Rust core with native UIs; small companion server beside the media server |
| First media server | Jellyfin. The source layer is generic (`MediaSource`), so Navidrome/OpenSubsonic, Plex, Emby and local files can be added later without UI changes |
| Audience | Free, open-source public release |
| Look on Apple devices | iPhone, iPad and Mac all get Liquid Glass |
| Look on Android | Material 3 Expressive, built natively with Jetpack Compose (Flutter is not used on Android) |
| Look on Windows and Linux | Flutter with an Encore theme closer to the Android look |
| Album visuals | Each album gets its own background gradient built from its cover art, like Apple Music |
| Layout | Every window on every platform is resizable; one layout engine with four size classes |
| Features | Word-by-word synced lyrics, YouTube Music-style recommendations that learn from skips, local and private personalization, a Wrapped-style year recap that differs each year, multiple profiles that swap on one device with data synced to the server, account-based handoff, five mood mixes |
| Live surfaces | Widgets, lock screen and Live Activities (or the platform equivalent) planned for every supported platform (plan section 20) |
| Colours (6 November 2026) | Iris (`#5B3FC4`) is the default accent on Android and on Windows and Linux. Eight profile accents (Iris, Cobalt, Lagoon, Fern, Amber, Coral, Rose, Graphite), each with light and dark Material 3 schemes that pass 4.5:1 for text. Album colours still come first. Apple devices use Apple's own system colours and design system; the core does not generate them. `design/tokens.json` `color` |
| Name and identifiers (6 November 2026) | Keep "Encore". The reverse-domain prefix is `uk.gcartee` (the owner's domain, gcartee.uk), for example `uk.gcartee.encore.core`. A check that the store name is free remains before the first upload. `docs/adr/core-0001-names-and-identifiers.md` |
| Jellyfin test fixtures (6 November 2026) | Recorded from the owner's own Jellyfin 12.0 server through Quick Connect, with every name, title, ID and path replaced by consistent stand-ins before committing (the repository will be public). `tools/fixtures/jellyfin/RECORDING.md` |
| Phase 2 core libraries (6 November 2026) | Approved: tokio, reqwest with rustls, rustls-platform-verifier, rusqlite with bundled SQLite, ulid, thiserror, url, unicode-normalization, sha2. Anything else is asked first. `core/THIRD_PARTY_LICENSES.md` |
| Type (6 November 2026) | Android, Windows and Linux follow the type drawn on the boards: a compact scale under 600 wide and a wide scale from 600, by window width. Every Android page title is 28 sp, weight 500, as on the Home board. Apple devices use the system text styles. `design/tokens.json` `typography.scale` |
| Phase 3 core libraries (6 November 2026) | Approved: quick-xml 0.42.0 (TTML lyrics) and png 0.18.1 (album covers, lossless so every device makes the same palette). `core/THIRD_PARTY_LICENSES.md` |
| Phase 3 recording (6 November 2026) | One Quick Connect session that may write to the owner's server and undo it: a temporary playlist ("Encore test - safe to delete") made, changed and deleted, and one favourite set and cleared. Done on 7 November 2026; the server was left as it was. `tools/fixtures/jellyfin/RECORDING.md` |
| Phase 4 answers (7 November 2026) | Live updates by long-poll now, the WebSocket with handoff in Phase 7; only crates already in the build, used directly (rusqlite, sha2, getrandom, ulid, url, encore-sources), no mDNS yet (the address is typed); one read-only Quick Connect recording (done 7 November: `tools/fixtures/jellyfin/RECORDING-PHASE4.md`); the companion server is built and run in CI only, never on the owner's PC. |
| Likes (6 November 2026) | A like is also a Jellyfin favourite, so other Jellyfin apps and devices see it before Phase 4 sync; favourites set elsewhere arrive as likes. `docs/adr/core-0008-edits-outbox-undo.md` |

## 2. Chosen for the owner (the owner delegated these)

| Decision | Choice | Where |
| --- | --- | --- |
| Database encryption | No SQLCipher in version 1. Use OS file protection and the OS keychain or keystore | Plan section 12 |
| Licences | MPL-2.0 for the apps, AGPL-3.0 for the server (suggestion, see open items) | Plan section 3 |
| Server language | Rust with Axum and SQLite; Postgres optional | Plan section 5 |
| Minimum OS versions | iOS and iPadOS 17, macOS 14, Android 9, Windows 10, current Ubuntu and Fedora LTS; newest look on the 26 releases | Plan section 15 |
| Telemetry | None. No analytics, no crash upload without the user pressing Send | Plan section 12 |
| Android button shape rule | Play and pause is round when paused and a flatter rounded square when playing; pressed is squarer still. Previous and Next are round on the side their arrow points to and 10 dp on the side beside Play. These radii are Encore tokens | Plan section 22, board 35 |
| Smart picks engine | One engine behind Autoplay, Mixes, Smart shuffle, Radio, Rediscover, widgets, CarPlay and Android Auto, and Smart downloads | Plan section 21 |
| Year recap engine | One deterministic `encore-recap` engine in the Rust core, run on the server and, when the server is off, on any device; same log gives the same bytes. NPU or ONNX only builds per-track audio fingerprints; templates (no language model) write the words | Plan section 25, boards 39 to 41 |
| Year recap timing | Counts through 31 December (no early cutoff), opens 1 December as "so far", final run 1 to 8 January for late plays, locked after 8 January | Plan section 25 |
| Year recap length and look | 10 to 12 cards, about 75 seconds; four launch packs (Editorial, Tide, Bloom, Glass); pack, palette and layout change every year from a seed; packs ship with app updates | Plan sections 10 and 25, boards 39 to 41 |
| Play event fields | Stable song key, listened milliseconds, UTC time plus UTC offset, and lyric-line jump events are in the first event log, because history cannot be rebuilt | Plan section 5 |
| Swipe gestures | Mini player and Now Playing artwork: swipe to skip or go back, with direction lock, 60 pt arm, coalescing, rubber band at the queue ends, and signals the same as the Next button. Row swipes arm at 25% or 60% | Plan section 26, board 42 |
| Drag and drop | Songs drag from any list to playlists, Up Next, the player bar and Liked Songs on Mac, Windows, Linux, iPad and Android tablets; copy by default; one core call, one sync event and one undo entry; a menu twin for every drop | Plan section 26, board 43 |
| Work split | One monorepo built by four tracks: core and server, Apple, Android, Windows and Linux. Each track has its own plan, its own folders and its own Claude Code session, so a Mac session and a PC session can run at the same time. The core runs one step ahead; Android and Windows and Linux follow Phases 2 to 5 as soon as the core part exists, instead of waiting for Apple | `docs/ROADMAP.md`, `CLAUDE.md` |
| TTML parser (Phase 3) | quick-xml directly with Encore's own mapping, instead of the plan's ttml_processor (a 6-day-old release with four more crates and a model of its own) | `docs/adr/core-0006-lyrics.md` |
| Artwork disk cache | Not in the core: each platform's image loader caches display images; offline artwork comes with downloads (Phase 8). The core keeps only palettes | `docs/core/CORE-API.md` |
| Lyrics search (Phase 3 tidy-up) | A word index of kept lyric lines (FTS5 with word prefixes), not the trigram index the names use: on 400,000 lines a trigram phrase took up to 174 ms, words about 10 ms. Lyrics match whole words in order, the last one unfinished; no typo tolerance and no mid-word matches. Songs are found only once their lyrics are kept, so `fetch_lyrics_ahead` fetches them after a sync | `core/encore-store/src/search.rs`, `docs/core/CORE-API.md` |
| Export format (Phase 3 tidy-up) | One JSON object, documented field by field, written row by row and renamed into place when whole. JSON over CSV because playlists and plays nest; the plan says "JSON of history, likes, playlists, recaps" | `docs/core/EXPORT-FORMAT.md` |
| Trusted media servers (Phase 4) | The companion is configured with the Jellyfin servers it trusts (`ENCORE_JELLYFIN_URLS`) and never takes a Jellyfin address from a device, which could otherwise claim any user. | `docs/adr/core-0009-companion-auth.md` |
| Refresh-token reuse (Phase 4) | Reuse of a rotated refresh token revokes the device, except the same token within 2 minutes while its replacement is unused (a lost reply), so a crash between rotating and saving doesn't lock the owner out. | `docs/adr/core-0009-companion-auth.md` |
| Like conflicts and Jellyfin (Phase 4) | Last writer wins by (hybrid clock, device ID); the device whose own decision lost sends the winner to Jellyfin too, so Jellyfin ends where the clock says. Playlists are re-read from Jellyfin, which stays their truth. | `docs/adr/core-0010-sync.md` |
| Guest (Phase 4) | A mode of the open profile (its media server still serves the music), not a second sign-in: nothing recorded or saved, its own empty queue, the profile's queue back at the end; it ends after 4 hours idle or when a profile opens. | `docs/adr/core-0011-profiles-on-a-device.md` |
| PIN storage (Phase 4) | Salted, 100,000-round SHA-256 in the device registry; the lock-out (5 tries, 30 s doubling to an hour) is the real guard, since 10^6 PINs can't be made safe by a hash. A screen lock, not encryption (plan section 6). | `docs/adr/core-0011-profiles-on-a-device.md` |
| Server image (Phase 4) | Debian trixie slim, non-root (uid 10001), one `/data` volume; arm64 cross-compiled on an amd64 builder rather than emulated. Not published to a registry yet: built from source with `server/compose.yaml`. | `server/Dockerfile` |
| Browse speed target | No browse budget exists in tokens, so every browse page is held to the search budget (50 ms on 50,000 songs) | `core/encore-store/tests/search_budget.rs` |
| Where the build glue lives | Each platform track compiles the Rust core for its own targets inside its own folder (an XCFramework for Apple, the Android ABIs, the Windows and Linux libraries). The Rust source stays in `core/`. Each track has its own CI workflow file and its own `check` Makefile target | `CLAUDE.md` |

## 3. Assumed defaults (not confirmed by the owner)

The owner has not answered these. They are the plan's defaults (plan section 15). Confirm the ones marked **ask** before they are hard to change.

| Topic | Default | Ask? |
| --- | --- | --- |
| Owner has a Mac with Xcode | Yes | **ask first** (Apple builds need it) |
| Jellyfin version and hardware | 10.10 or later, modest x86 or ARM box, no GPU | ask |
| Away-from-home access | Tailscale or an existing reverse proxy | |
| People and kids | 2 to 5 profiles; kid profiles with an explicit-content filter | |
| Publish where | All stores after phase 11 | |
| Extra devices (Watch, TV, Wear OS, web) | Not in the first 12 phases | |
| Casting | AirPlay and Bluetooth only at first | |
| Lossless and hi-res | Play as-is; no Atmos | |
| Equalizer | 10-band, phase 8 | |
| Scrobbling | Optional, off by default | |
| Online lyrics provider | LRCLIB only, off by default, through the companion server | |
| Import past history | Optional Last.fm or ListenBrainz import | |
| Word-by-word timing for songs without it | Not in v1. The lyrics model leaves room (optional word timings, source and confidence per line); a later server-side forced-alignment job on the user's own library is the likely route. No commercial or scraped lyric source | Decide after the Phase 2 or 3 lyrics spike |
| Household learning across profiles | Opt-in | |
| Default exploration | About 10 percent new-to-you picks ("Balanced") | |
| Recap release | Opens 1 December as "so far", counts through 31 December, final run 1 to 8 January, locked after; four starter design packs | |
| Dragging songs "to an album" | The owner asked for dragging songs into an album. Built as adding to a **playlist**, because albums come from the files and tags on the server and the app never rewrites them. Album tiles refuse the drop and say why | **ask** (if the owner meant something else, the plan changes) |
| Inspiration | Apple Music only; light and dark both, following the system | |

## 4. Open: only the owner can answer these

| Open item | Why it matters | When |
| --- | --- | --- |
| **Store name check** | The owner chose to keep "Encore" and `uk.gcartee` (section 1). Whether "Encore" is free in each store is not checked | Before any store upload |
| **Build hardware** | Apple targets need a Mac with Xcode; testing needs a Windows and a Linux machine or VM and real phones | Before phase 2 |
| **Developer accounts** | Apple Developer Program (paid, annual), Google Play Console (one-time fee), GitHub for CI. Signing and TestFlight need them | Before phase 2 (Apple) and phase 6 (Google) |
| **Licence confirmation** | MPL-2.0 apps and AGPL-3.0 server is a suggestion; the licence file should not be added until confirmed | Before the first public push |
| **Recap design packs** | Editorial, Tide, Bloom and Glass are defaults; Contour, Aura, Neon Grid and Paper Collage are held back for later releases | Before phase 11 |

## 5. Unverified: check before relying on or releasing

| Item | Why it is open | Check against |
| --- | --- | --- |
| Companion on real devices and networks | The like-within-a-second was measured with two cores and the server on one machine (106 to 135 ms, the Docker images included); never over Wi-Fi or with phones | Phase 6 devices on the owner's network |
| arm64 server speed | The arm64 image ran end to end under QEMU in CI (118.5 ms like), which proves it works, not its speed | A Raspberry Pi 5 or an arm64 NAS |
| Avatar size | For a 256 px request the owner's Jellyfin returned a 2.7 MB original (not PNG); the address now also asks `maxWidth`/`maxHeight` | A device loading avatars |
| Queued delete while the companion is offline | Board 19's "Waiting to finish on server" isn't built: `delete_profile_everywhere` refuses while the companion is unreachable | A later phase, if wanted |
| Word-by-word lyrics source | Spike done in Phase 3 (7 Nov 2026): all 40 recorded songs with lyrics came back line-synced, none with word cues. The core reads word timing from TTML, enhanced LRC and Jellyfin's cues, tested only on written fixtures | Real word-timed files (enhanced LRC sidecars or TTML) when the owner has some; a later provider or the companion server's alignment |
| `Metadata.Offset` in Jellyfin's lyrics JSON | Applied once, the LRC way (positive shows lyrics earlier); Jellyfin's own LRC parser never sets it, and nothing in the owner's library does | A server or provider that reports it |
| Palette numbers (`albumGradient`: ranking weight, duplicate distance, light-art threshold, base and light-field lightness) | Starting values; within 0.038 OKLab of board 04 on its four covers, readable on the owner's 38 real covers | The look on devices, next to Apple Music, in Phase 3's platform work |
| Skip-signal thresholds (`smartPicks`: 30 s early skip, 80% late, 2 s accident, 3 s interruption window, 2 h too soon; 600 ms swipe coalescing) | Starting values from the plan; classification tested, weights unused until Phase 5 | Offline replay on real histories in Phase 5 |
| Stream transcode settings | AAC in ADTS and Opus in Ogg through `/Audio/{id}/universal`, chosen from Jellyfin's API document; not played on any device | Each platform's player in its Phase 2 |
| Certificate checks through each operating system's verifier | Built for every target in CI; not run against a real https server with a private CA or a self-signed certificate | Each platform track's first https sign-in; Android also needs request `core-0001-android-tls-init` |
| Search speed on devices | Under 50 ms on 50,000 songs on a CI runner, not on a phone | The platform tracks, Phase 6 |
| Exact Material 3 Expressive corner radii | The library documents shape slots, not numbers; 24 dp, 14 dp and 50% are Encore tokens | The pinned Compose Material 3 version, on a device |
| New MacBook Pro Dynamic Island and touch screen | Known from press coverage of Apple's announcement (ships Nov 6); how apps use it is not documented | Apple's developer documentation and a real Mac |
| Mac apps starting their own Live Activity | Not documented; only the iPhone mirror is planned | Apple's ActivityKit and macOS documentation |
| iPhone Duo fold region and arrangement API names | Known only from a developer write-up of Apple's Tech Talks | Apple's Xcode and SDK documentation |
| Samsung Flex mode details | Flex mode is sourced from Samsung's pages; Samsung gives no Flip cover-screen app rules | A real Fold and a real Flip, and Samsung's developer pages |
| Every smart-picks weight, half-life, threshold and budget | Starting values, not measurements | Offline replay on real histories |
| Audio embedding model and nearest-neighbour library licences | Not yet chosen or checked | The licence texts, before release |
| Recap numbers: card timing, thresholds, 60° hue gap, 90-day device window | Starting values, not measurements (`design/tokens.json`, `recap`) | Testing with real histories and real people |
| Recap build time for 100,000 plays | Not measured | A benchmark on a mid-range phone and on a Raspberry Pi 5 |
| Swipe numbers: 12 pt lock, 60 pt arm, 500 pt/s flick, 600 ms coalesce, 40 pt rubber band | Starting values, not measurements (`design/tokens.json`, `gestures`) | Trying them on real phones, and against Apple Music |
| SwiftUI, Compose and Flutter drag and drop API names and touch behaviour | Written from memory of the frameworks, not checked against the pinned versions | Each framework's official documentation and a real device |
| Store dates in plan section 19 | Checked 5 Nov 2026; they change | The linked Apple and Google pages, the week before each submission |
| Windows and Linux store and packaging rules (Microsoft Store and MSIX, winget, Flathub) | Plan section 19 covers Apple and Google only; nothing was researched for these stores | Each store's current documentation, at the start of the Windows and Linux Phase 13 |
| Flutter desktop builds need the system they target (Windows apps on Windows, Linux apps on Linux) | Stated from general knowledge of Flutter, not checked against the pinned version | The Flutter documentation for the pinned version |

## 6. Spikes to run early (cheap tests that could change the design)

Run each as its own small, throwaway experiment before building the real feature on top of it.

1. **Rust core over FFI on all six platforms**: UniFFI to Swift and Kotlin, flutter_rust_bridge to Dart. Phase 1 proves this; if one path fails, stop and report.
2. **Jellyfin against real files**: library sync speed on a large library, gapless albums, lyrics endpoints and what word timing exists, transcoding versus direct play.
3. **Liquid Glass fidelity in SwiftUI**: confirm the Now Playing bar and sidebar look right with the real system APIs before building every screen on them.
4. **Material 3 Expressive in Compose**: check the toggleable shape and connected button group APIs in the pinned version, and update the Encore tokens if the numbers differ.
5. **Flutter Encore theme**: one screen (Now Playing) with the shape rule and wavy progress bar, before porting the rest.
6. **Desktop audio**: gapless playback with Symphonia and cpal on Windows and Linux, plus SMTC and MPRIS.
7. **Smart picks offline replay harness**: replay a real history against plain shuffle, as the plan's phase 5 requires.
8. **Recap parity and speed**: run a stub of the `encore-recap` engine over a synthetic log of 100,000 plays on the server build, an iPhone, an Android phone and a Raspberry Pi 5. The bytes must match and the time must be recorded (plan section 25).
9. **Swipe and drag and drop APIs**: one throwaway screen each for the mini player swipe and for dragging songs onto a playlist, on SwiftUI (iPad and Mac), Compose (Android tablet) and Flutter (Windows or Linux). Confirm the API names and the touch and pointer behaviour in plan section 26.

## 7. Lead-time items (start early; these take calendar time)

- Apple Developer Program enrolment, and the CarPlay audio entitlement request (Apple grants it on request).
- Google Play: a new personal developer account must run a closed test with at least 12 testers for 14 consecutive days before production (plan section 19).
- Android developer verification for apps outside Play (plan section 19).
- Privacy policy page and an account-deletion web page (plan section 19), hosted somewhere public.
- Domain name for the policy pages and the docs site.
