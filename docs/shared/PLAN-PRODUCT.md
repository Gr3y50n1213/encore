# Encore: product and engine plan

Exported 5 November 2026 from the planning document and split by track the same day. This file is shared by every track. It holds what the app does and how the shared Rust core and the companion server work: the product, the media sources, the server, profiles, the recommender and smart picks, the playback engine, the year recap, handoff, privacy, the open questions and the shared part of the store rules.

Section numbers are the plan's original numbers, so references such as "section 21" still work. `docs/SECTION-MAP.md` says which file holds each section. Sections 9, 16, 17, 20, 22 to 24 and 26 (design, interaction and live surfaces) are in `docs/shared/PLAN-UX.md`. Each platform plan holds the parts that belong to one platform: the operating-system integration rows of section 8, the libraries of section 18, the store requirements of section 19, the live surfaces of section 20, the button, cut-out and fold rules of sections 22 and 23. The roadmap (section 13) is `docs/ROADMAP.md`.

Wherever the plan says "the wireframe canvas" or "the canvas", read `design/wireframes/` (start with its `INDEX.md`).

## 1. Vision and locked decisions

Build a free, open-source music player for self-hosted libraries. It should look and feel like Apple Music, learn your taste like YouTube Music, and keep every listening signal on hardware you own. Working codename: **Encore** (placeholder; rename any time).

**Goals**

- Apple Music-grade UI and motion on all six platforms: macOS, iOS, iPadOS, Windows, Android, Linux.
- Synced, word-by-word lyrics presented the way Apple Music does them.
- Recommendations and autoplay that learn from skips, replays and context, computed only on your devices or your server.
- Multiple people on one device, each with their own profile, history, likes and recommendations.
- Seamless handoff: start on the phone, continue on the Mac.
- A year-end recap that looks different every year.
- Jellyfin first, with a source layer that can add Navidrome/Subsonic, Plex, Emby and local files without touching the UI.

**Non-goals for v1**

- No cloud service run by the project, no telemetry, no ads, no accounts on third-party servers.
- No music catalog or discovery beyond what is on the user's own server (see section 7 for what "discovery" means here).
- No video, podcasts or audiobooks (can come later).

**Decisions already made**

| Decision | Choice |
| --- | --- |
| Platform approach | Shared Rust core; SwiftUI on Apple devices; Jetpack Compose on Android; Flutter on Windows and Linux |
| Backend | Small self-hosted companion server (Docker) beside the media server |
| Media sources v1 | Jellyfin only, behind a generic source interface |
| Audience | Free, open-source public release |

**Defaults I chose (change any of them in section 15)**: SQLite everywhere except an optional Postgres for big servers; Rust (Axum) for the companion server so it shares code with the clients; iOS 17 / macOS 14 minimum, with the newest Liquid Glass look on iOS 26, iPadOS 26 and macOS 26 and later.

## 2. System architecture

Every app is a thin native UI on top of one shared Rust library, so behavior (queue, recommendations, sync) is identical everywhere and only the look adapts per platform.

```text
 CLIENTS (thin native UI, one shared Rust core inside each)
 ----------------------------------------------------------------------
  iOS / iPadOS / macOS      Android                 Windows / Linux
  SwiftUI, Liquid Glass     Compose, Material 3     Flutter, Encore M3E theme
  AVFoundation audio        Expressive, Media3      Rust audio (Symphonia+cpal)
  widgets, Live Activities  Glance widgets, Live    SMTC (Windows), MPRIS (Linux)
  CarPlay                   Update, Android Auto
        |  UniFFI (Swift)       |  UniFFI (Kotlin)        |  flutter_rust_bridge (Dart)
        v                       v                         v
 ----------------------------------------------------------------------
  encore-core (Rust): sources, queue, downloads, recommender, sync,
  per-profile SQLite, lyrics, artwork, widget snapshot. No UI code.
 ----------------------------------------------------------------------
        |                                         |
        | audio, library, lyrics, art             | profiles, synced history and likes,
        | (direct, lowest latency)                | handoff presence, heavy jobs
        v                                         v
  MEDIA SERVER                              COMPANION SERVER (optional)
  Jellyfin now; OpenSubsonic, Plex,         encore-server: Rust + Axum + SQLite
  Emby, local files later                   audio analysis, model training, recap
                                            lookups to LRCLIB / MusicBrainz go
                                            through it, never from devices
 No companion server: the app still plays, shows lyrics and recommends from
 on-device history; it only loses sync and handoff.
```

Devices stream audio straight from the media server for the lowest latency, and talk to the companion server for everything personal.

**What lives where**

- **Rust core** (`encore-core`): media-source clients, the play queue, download cache, recommender, sync engine, per-profile database, lyrics and artwork fetching, and the small per-profile snapshot that widgets and Live Activities read. No UI code. Compiled for every target.
- **Bridges**: [UniFFI](https://github.com/mozilla/uniffi-rs) generates Swift bindings; [flutter\_rust\_bridge](https://github.com/fzyzcjy/flutter_rust_bridge) generates Dart bindings. The core exposes async functions plus an event stream (now playing, queue changed, sync status).
- **Audio output is platform-native**, because that is where lock screen, CarPlay, Android Auto and AirPlay live:
  - Apple: AVFoundation (`AVQueuePlayer` for gapless, `AVAudioEngine` for crossfade), `MPNowPlayingInfoCenter`, `MPRemoteCommandCenter`.
  - Android: Media3 ExoPlayer with a `MediaSessionService`, called straight from the Kotlin Compose app.
  - Windows and Linux: a Rust audio engine (Symphonia to decode, cpal to output) with System Media Transport Controls on Windows and MPRIS on Linux.
  - In every case the core owns the queue and tells the native player what to load next.
- **Companion server** (`encore-server`): Rust + Axum, SQLite by default. Holds profiles, synced history and likes, handoff presence, and the heavy jobs (audio analysis, model training, the yearly recap).
- **Graceful degradation**: with no companion server, the app still plays, shows lyrics and recommends from on-device history. It just cannot sync or hand off.

## 3. Repository layout and tech stack

One monorepo, so Claude Code can see and change the core, both UIs and the server in a single session.

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

| Layer | Technology | Why |
| --- | --- | --- |
| Shared logic | Rust (stable), tokio, reqwest, serde | One implementation for all six platforms |
| Local storage | SQLite, one file per profile | Simple isolation between users, easy backup |
| Apple UI | SwiftUI, Observation, Swift Concurrency | Native Liquid Glass, CarPlay, widgets, Handoff |
| Apple audio | AVFoundation, MediaPlayer framework | Gapless, AirPlay, lock screen |
| Other UI | Windows and Linux: Flutter (stable), Riverpod, go\_router, with a custom Material 3 Expressive theme. Android: Kotlin, Jetpack Compose, Material 3 Expressive | One Flutter codebase for Windows and Linux; Android is native so it gets the expressive components. Flutter has none, so the Encore theme draws the shapes, connected button groups and wavy progress itself |
| Android audio | Media3 ExoPlayer + MediaSessionService | Background play, Android Auto |
| Desktop audio | Symphonia + cpal (Rust) | No GStreamer or libmpv dependency |
| On-device ML | Core ML on Apple, ONNX Runtime elsewhere | Uses the Neural Engine or NPU when present |
| Server | Rust, Axum, SQLite (Postgres optional), ONNX Runtime | Shares crates with clients; one small Docker image |
| Design tokens | Style Dictionary or a small Rust script | One source of truth for colors, type, spacing, motion |
| CI | GitHub Actions: macOS runner for Apple, Linux and Windows runners | Every PR builds every target |

**License suggestion:** MPL-2.0 for the apps and AGPL-3.0 for the server. MPL keeps changes to the app's files open while staying compatible with App Store distribution, where GPL is legally murky. AGPL makes anyone who hosts a modified server share their changes. This is an open question in section 15.

**Fonts:** SF Pro may only be used on Apple platforms. Use the system font on Apple devices and bundle Inter (SIL Open Font License) on Windows, Linux and Android to get the same clean look.

## 4. Media sources, lyrics and artwork

The UI never talks to Jellyfin directly. It talks to a `MediaSource` trait in the core, and Jellyfin is just the first adapter, so Navidrome, Plex, Emby and local files slot in later with no UI changes.

```rust
#[async_trait]
pub trait MediaSource: Send + Sync {
    fn kind(&self) -> SourceKind;              // Jellyfin, Subsonic, Plex, Emby, Local
    fn capabilities(&self) -> Capabilities;     // lyrics? transcoding? playlists writable? ratings?
    async fn authenticate(&self, creds: Credentials) -> Result<Session>;
    async fn sync_library(&self, since: Option<Cursor>) -> Result<LibraryDelta>;
    async fn stream_url(&self, track: &TrackId, q: StreamQuality) -> Result<StreamInfo>;
    async fn artwork(&self, item: &ItemRef, size: u32) -> Result<ArtworkSource>;
    async fn lyrics(&self, track: &TrackId) -> Result<Option<RawLyrics>>;
    async fn playlists(&self) -> Result<Vec<Playlist>>;
    async fn update_playlist(&self, edit: PlaylistEdit) -> Result<()>;
    async fn report_playback(&self, event: PlaybackReport) -> Result<()>; // optional, user can turn off
}
```

**Rules for every adapter**

- Map everything into one normalized model: `Artist`, `Album`, `Track`, `Playlist`, `Genre`, with stable internal IDs plus the source's own ID.
- Mirror the library into the profile's local SQLite, so browsing and search are instant and work offline. Refresh with deltas, not full rescans.
- A capability flag hides any feature the server cannot do; the UI never shows a dead button.
- One profile can attach several sources at once later (for example Jellyfin at home plus local files). Duplicate tracks merge by MusicBrainz ID, then by artist + title + duration.
- Each adapter ships with recorded HTTP fixtures, so tests run without a live server.

**Jellyfin adapter (v1)**

- Sign in with username and password or Quick Connect; store the access token in the OS keychain.
- Library from the `/Items` endpoints filtered to music types; artwork from the image endpoints at the exact pixel size the UI needs.
- Stream original files when the network allows, and request transcoding (AAC or Opus) on cellular or when the user picks a quality cap.
- Lyrics from Jellyfin's lyrics endpoint (Jellyfin 10.9 and later reads `.lrc` sidecars and embedded tags).
- Playback reporting back to Jellyfin is optional and off by default, since the companion server keeps the real history.

**Later adapters, in suggested order**: Navidrome and other servers through the [OpenSubsonic](https://opensubsonic.netlify.app/) API (one adapter covers many servers), then Plex, then Emby (close to Jellyfin's API), then local files.

**Lyrics pipeline**

1. Look for lyrics in this order: the media server, embedded tags, sidecar files, then online providers only if the user turned them on.
2. Parse LRC, enhanced LRC (word timings), and TTML (word timings, duet singers, background vocals) into one internal model: lines with start and end times, optional per-word timings, a singer tag per line, and background-vocal spans.
3. Online lookups (for example [LRCLIB](https://lrclib.net), which is free and open) go through the companion server, so providers see the server, not each device, and results are cached for every profile.
4. Unsynced plain lyrics still display, as a clean scrolling page.
5. **Room for word timing from other sources later (not built in v1).** The internal lyrics model keeps word timings optional, and each lyric set records where it came from (server file, embedded tag, sidecar, provider, or generated) and a confidence value per line. A later, optional companion-server job may generate word timings by forced alignment from line-synced lyrics plus the track's audio on the user's own server. Results are stored per track and shared by every profile, only used when confidence is high enough, and replaced by any word-timed file the user supplies. Where there is no word timing the lyrics view highlights whole lines. Nothing in the UI may assume the source of word timings, and no third-party lyric service is added without the owner's decision (section 19).

**Artwork**: request sized images from the server, cache on disk per source, and extract a color palette (dominant and accent colors) once per album for the Now Playing background and the recap themes.

## 5. Companion server

The companion server is one small Docker container that sits next to Jellyfin and stores everything personal: history, likes, handoff state, models and recaps. It never stores music.

**Identity: no new passwords**

- A profile is identified by *(media server, media-server user ID)*. The user signs in to Jellyfin from the app as usual.
- The app sends its Jellyfin token to the companion once. The companion checks it against Jellyfin, then issues its own long-lived device token. Revoking a device on the companion signs it out everywhere.
- When more sources arrive, a profile can link several source accounts to one companion identity.

**What it stores, per profile**

- Play events (append-only): a stable song key (the MusicBrainz ID when tagged, otherwise artist + title + duration), the start time in UTC plus the UTC offset at that moment, how many milliseconds were actually listened (not the track length), how it ended (finished, skipped, paused and abandoned), what started it (album, playlist, radio, search), device, and coarse context (time of day, day of week). Lyric-line jump events (which line was tapped in the lyrics view) are logged too. No location. These fields must exist from the first build, because the year recap (sections 10 and 25) cannot be rebuilt without them.
- Mutable state: likes and dislikes, ratings, pinned items, app-side playlists, settings.
- The device registry and live handoff presence.
- Recommender artifacts: track embeddings (shared by all profiles on that library) and each profile's own trained ranking weights.
- Recap data by year (a small JSON document per profile and year, section 25) and the profile's recap progress.

**Sync protocol**

- Every change on a device is written locally first as an event with a ULID, the device ID and a hybrid logical clock timestamp. The app works fully offline.
- Devices push unsent events and pull everything after their last cursor. Play events simply append. Mutable fields merge last-writer-wins per field, using the clock.
- Transport is HTTPS for push and pull plus one WebSocket per device for live updates (handoff, a like on another device, new recommendations).

**API surface (documented in `docs/api/openapi.yaml`)**

| Area | Endpoints |
| --- | --- |
| Auth | `POST /auth/exchange`, `POST /auth/refresh`, `DELETE /devices/{id}` |
| Sync | `POST /sync/push`, `GET /sync/pull?cursor=` |
| Live | `GET /ws` (presence, handoff, remote control, notifications) |
| Recs | `GET /recs/home`, `POST /recs/radio`, `GET /recs/model` (weights for on-device use) |
| Lyrics | `GET /lyrics/{trackId}` (cached proxy to optional providers) |
| Recap | `GET /recap/{year}` (the recap data), `POST /recap/{year}/regenerate`; recap progress syncs as an ordinary mutable field |
| Admin | `GET /admin/status`, a small web page for jobs, devices and storage |

**Background jobs**

- Audio analysis: read each new track once (streamed through the Jellyfin API, or from a read-only mount of the music folder if the user provides one) and compute an audio embedding, tempo, key, loudness and energy.
- Nightly: retrain each profile's ranking model, refresh the co-listen graph, precompute home-screen mixes.
- Recap: build the "so far" recap on December 1, run the final build on January 1, rebuild quietly when late plays arrive until January 8, then lock it (section 25). Devices run the same engine when the server is off.
- Jobs are throttled and can be limited to night hours, so they never compete with Jellyfin transcoding.

**Deployment**

- One `compose.yaml` example that runs next to an existing Jellyfin container; image targets amd64 and arm64 (so it runs on a Raspberry Pi 5 or a NAS).
- Apps find the server on the home network through mDNS, or by a URL the user types. Remote access is left to the user's own reverse proxy, Tailscale or WireGuard; the app never relies on a public relay.
- Data lives in one volume; backups are a copy of that folder.

## 6. Multiple users on one device

Each person who signs in on a device gets a fully separate profile, and switching takes two taps, like switching accounts in Netflix or Instagram.

**How it works**

- A profile is a folder: its own SQLite database, download cache index, artwork cache, recommender state and settings. Nothing personal is shared between profiles, and deleting a profile deletes its folder.
- Downloaded audio files can be shared on disk between profiles when two people download the same track (saves space), but each profile keeps its own list of what it downloaded.
- Tokens live in the OS keychain (Apple Keychain, Android Keystore, Windows Credential Manager, Linux Secret Service), keyed by profile ID.
- Only one profile is active at a time. Switching stops playback (or asks to keep it), flushes unsent sync events, then loads the next profile.

**Profile switcher UI**

- Tap the avatar at the top of Home or Library to open a sheet with every signed-in profile, plus "Add account".
- Long-press the app icon (iOS, Android) or use the menu bar and tray menu (Mac, Windows, Linux) to switch straight to a profile.
- Avatars come from the media server's user image, with initials as a fallback.
- Optional per-profile PIN or Face ID / Touch ID / Windows Hello, so kids cannot open a parent's profile. The PIN is a screen lock, not encryption.
- Optional "guest" listening: a temporary profile that records nothing.

**Edge cases Claude Code must handle**

- Two profiles from different servers on one device.
- The same person signed in on many devices (handled by sync and handoff, sections 5 and 11).
- A profile whose token was revoked: show a clean "sign in again" state and keep local data until the user chooses.

## 7. Recommendation engine

The success measure is simple: songs the app picks should rarely be skipped in the first 30 seconds. Everything below is designed to drive that early-skip rate down, using only data on the user's own devices and server.

Section 21 is the full logic for how this engine is used: which signals count and which never do, what each surface asks for, cold start, fallbacks and the controls the user gets. It marks anything it adds to this section as New.

**An honest constraint.** YouTube Music learns from billions of listeners and a global catalog. Here the app only knows your library and your own history. It makes up for that with audio understanding (what tracks actually sound like), metadata, and fast learning from your skips. "Discovery" means surfacing music in your library you forgot or never played, plus anything new you add.

**Signals and starting weights** (tune with real data)

| Event | Label weight | Notes |
| --- | --- | --- |
| Liked | +3.0 | Also a permanent positive |
| Replayed, queued by hand, or searched then played | +1.5 | Strong intent |
| Finished | +1.0 |  |
| Played past 80%, then skipped | +0.5 | Mostly enjoyed |
| Skipped between 30 seconds and 80% | -0.5 | Mild negative |
| Skipped in the first 30 seconds, app-chosen track | -1.5 | The key signal |
| Skipped, track the user picked | -0.2 | Often just browsing; counts only after 3 skips of the same song in 14 days (section 21) |
| Not interested | Exclude | Track, or artist if chosen; never decays (section 21) |
| Paused and never resumed | 0 | Ambiguous, ignore |

Every event also carries context: time of day, day of week, device type, and what started playback.

**Pipeline for every autoplay, radio or mix**

1. **Candidates (about 500 tracks)** from several pools: tracks you played in the same sessions (co-listen graph); tracks that sound similar to the current session (audio embedding neighbors); same and related artists, genres and eras (metadata, MusicBrainz relations); a rediscovery pool of loved tracks unplayed for 60+ days; and new additions to the library.
2. **Features per candidate**: your affinity for the track, artist and genre; similarity to the current session; days since last played; your skip rate for similar tracks in this context; time-of-day fit; tempo, key and energy fit with the previous track; novelty.
3. **Ranking**: a small per-profile model predicts "will be played past 30 seconds and finished". Start with hand-tuned scoring for new users, then switch to a gradient-boosted tree model trained nightly on the server and exported to ONNX or Core ML.
4. **Exploration**: about 10% of slots go to promising but uncertain tracks (Thompson sampling), so the app keeps learning instead of looping favorites. A "Familiar to Adventurous" control lets the user change that share.
5. **Sequencing for flow**: no artist twice within 3 tracks, smooth tempo and energy changes, no repeat within 24 hours unless the track is loved.
6. **Live adaptation**: an early skip immediately pushes the rest of the session away from that sound; a like or replay pulls toward it. This happens on the device, before the next track starts.

**Features this powers**

- Autoplay (infinity) when an album, playlist or queue ends, as in Apple Music.
- Radio from any song, album, artist or playlist.
- Home mixes: your liked music clustered by sound into up to six named mixes (for example "Mellow Indie Mix"), refreshed daily.
- "Deep Cuts": tracks in your library you have never played that match your taste.
- Mood and activity mixes from audio features (focus, workout, wind down).
- Time capsules: "Your 2023", "Summer favorites".
- "Why this song?": a sheet that explains each pick in one line ("Similar to Holocene, which you finish 95% of the time"). It builds trust and is great for debugging.

**Where the work runs**

| Task | Runs on | Hardware |
| --- | --- | --- |
| Audio analysis of the library (embeddings, tempo, key, energy) | Companion server | CPU, or GPU if present |
| Nightly model training, co-listen graph, precomputed mixes | Companion server | CPU |
| Live ranking and in-session adaptation | Each device | CPU is enough; the model is tiny |
| Analysis of local files and offline-only tracks | Each device | Apple Neural Engine via Core ML; NPU via ONNX Runtime (NNAPI or QNN on Android, DirectML on Windows) |
| Fallback when no companion server exists | Each device | Training runs while charging and idle |

The NPU matters most for audio analysis, not ranking. Ranking a few hundred candidates with a small tree model takes milliseconds on any phone.

**Audio model choice**: use an open audio embedding model (for example LAION-CLAP, or an Essentia-TensorFlow model) and check its license before shipping, since some are non-commercial only. Store embeddings per track so analysis runs once.

**Optional, opt-in extras**

- Household learning: profiles on the same server can share anonymous co-listen counts (never who listened), which helps small libraries.
- Import past history from a Last.fm or ListenBrainz export file to skip the cold start.

**How we know it works**: replay each user's history offline to test changes, and show a private stats page with the early-skip rate of app-chosen tracks per week. The v1 target is to beat a shuffle baseline by a wide margin; set the exact goal once there is real data.

### 7.1 Made-for-you mood playlists

Every night the app builds five playlists for each profile, so they are waiting when the user opens the app. They are organized by mood and moment, not genre: one playlist can mix folk, electronic and jazz if they feel the same and the user plays them in the same situations.

**What the user sees**

- A "Made for you" row at the top of Home with five artwork tiles. Each tile is a collage of the playlist's top album covers, tinted with their shared palette.
- A short title of 2 to 4 words and a one-line subtitle, for example *Slow Sunday Mornings* / "Mostly Bon Iver, Iron & Wine and 12 more".
- Each playlist is 25 to 40 tracks, about 90 to 120 minutes, ordered for flow rather than sorted.
- Actions: Play, Shuffle, Pin (keeps it past tonight), Save to Library (turns it into a normal playlist, written back to Jellyfin when the server allows), Regenerate, and "Why these songs?".

**How a "mood" is found (not from genre tags)**

1. **Sessions, not tracks.** Split the listening history into sessions (a gap of 30+ minutes starts a new one). Each session gets a fingerprint: average audio features, time of day, day of week, how many tracks were skipped, and which tracks the user chose by hand.
2. **Place every track on a mood map.** Each track becomes a point built from the audio embedding plus a few readable features: energy, tempo, valence (bright vs. dark), acousticness, danceability, instrumentalness, and optionally lyric sentiment computed locally.
3. **Find the user's own moods.** Cluster the tracks the user actually enjoys (finished, liked, replayed) on that map, weighted by the sessions they appear in. Tracks that keep showing up together in the same kind of session end up in the same cluster, even if their genres differ.
4. **Choose five that are clearly different.** Pick the five clusters that are most played or most loved, then drop any that are too close to one already chosen, so the row never shows five versions of the same playlist. Reserve slots like this:
   - Two for the user's strongest recurring moods.
   - One tied to the current time of day or day of week (for example the user's weekday-evening sound).
   - One for a mood the user has been drifting toward in the last 30 days.
   - One stretch playlist: a familiar mood with about 30% songs the user has never played, or has not heard in 60+ days.
5. **Fill and order each playlist.** Take the nearest tracks to the cluster center, remove anything with a high early-skip rate in similar contexts, cap at 2 tracks per artist and 4 per album, keep the user's most-played songs to about 40% of the list, and order by smooth tempo and energy changes. Playlists may overlap by at most 20%.

**Using skips**

- Tracks the user skipped within 30 seconds in this mood or a similar one are excluded and nudge the cluster away from that sound.
- Skips inside a generated playlist are the strongest feedback: after three early skips in a row, the app quietly swaps the rest of the list in the background.
- A playlist the user plays through more than 70% of the time becomes more likely tomorrow; one they skip through repeatedly is retired.

**Titles that fit but do not shout**

Title rules, enforced in code and tested:

- 2 to 4 words, plain language, Title Case, no emoji, no hype words ("ultimate", "epic", "vibes", "bangers"), no puns.
- Say the mood or moment first, a texture second: *Late Night Focus*, *Warm and Mellow*, *Upbeat Commute*, *Quiet Piano Evenings*, *Slow Sunday Mornings*, *Heavy Rotation Comfort*.
- No artist names in the title (they go in the subtitle), and no genre unless it truly dominates the playlist.
- No two titles in the same row share a main word, and a title is not reused for the same profile within 14 days unless the playlist is pinned.
- Never claim facts the app does not know (no weather, no location).

How titles are made, in order of preference:

1. A fixed vocabulary of about 40 mood words and 30 moment words, each tied to a region of the mood map or a time context (for example low energy plus high acousticness plus evening gives "Quiet ... Evenings"). This always works, on every device, offline.
2. An optional small language model picks or refines the wording from the vocabulary and the playlist's measurable traits. Apple's on-device Foundation Models on supported Apple devices, a small local model through ONNX Runtime on devices with an NPU, or the companion server otherwise. Output must pass the rules above or the template title is used.
3. The user can rename any playlist, and a rename is kept when the playlist is pinned or saved.

**When and where it is generated**

| Step | Runs on | When |
| --- | --- | --- |
| Mood map and clustering | Companion server (or the device if there is no server) | Nightly, plus after big listening changes |
| Track selection and ordering | Same | Right after clustering |
| Title wording | Server, or an NPU/Apple device when available | After selection |
| Delivery | Synced to every device on the profile through the normal sync, so all devices show the same five | Ready before the user wakes up |

- Unpinned playlists are replaced once a day, never while the user is playing one. Pinned ones stay until unpinned.
- With no server, the device builds them while charging and idle, with the same code in the Rust core.
- Cold start: with fewer than about 50 plays, show up to three playlists built from audio features of the library (calm, upbeat, focus-friendly) and grow to five as history builds.

**Testing**

- Offline replay: for each test profile, check that tracks in generated playlists have a lower early-skip rate than random picks from the user's liked songs.
- Diversity checks: five distinct clusters, overlap under 20%, at most two tracks per artist.
- Title tests: a few hundred generated titles checked against the rules, plus a banned-word list.

**Roadmap placement:** a first version ships in phase 5 using listening sessions and time of day only (no audio features yet, so moods come from how the user listens, with template titles). The full audio-based mood map and model-written titles arrive in phase 9 with the audio analysis.

## 8. Playback engine

Playback must feel instant and never stutter: the next track is always buffered, and nothing on screen waits for the network.

**Core behavior (in Rust, shared)**

- One queue model with three parts, shown in the UI like Apple Music: History, Up Next (what the user added), and Autoplay (what the app picked). Shuffle and repeat apply to the right part. Smart shuffle (section 21) adds one suggestion for every four songs being shuffled.
- Preload the next track 20 seconds before the current one ends; keep a rolling disk cache of recently played tracks (size set by the user).
- Gapless playback for albums that need it (live albums, classical, DJ mixes), using encoder delay and padding info.
- Optional crossfade (1 to 12 seconds), automatically turned off inside gapless albums.
- Volume normalization from ReplayGain tags, or loudness measured by the companion server.
- Stream quality settings for Wi-Fi and cellular separately: original, high (320 kbps AAC/Opus), normal (160 kbps), data saver (96 kbps).
- Every play writes an event the moment it starts and updates it when it ends, so history survives crashes.

**Offline downloads**

- Download any song, album or playlist; "Download liked songs" and "Download my mixes" as automatic options.
- Downloads live per device and are listed per profile; a storage page shows usage and lets the user clear by size or age.
- Offline mode hides anything not downloaded instead of showing errors.

**OS integration checklist**

What each platform must support (lock screen, media keys, Now Playing, car, widgets) is one table row per platform, and each row is in the plan of the track that builds it: iOS, iPadOS and macOS in `docs/apple/PLAN-APPLE.md`, Android in `docs/android/PLAN-ANDROID.md`, Windows and Linux in `docs/windows-linux/PLAN-WINDOWS-LINUX.md`, all under section 8.

Chromecast, Sonos and casting to Jellyfin clients are later features; list them in section 15 if you want them sooner.

## 10. Year-end recap

Each profile gets a full-screen, tap-through story every December. One shared engine builds it from the profile's own play log, so it comes out the same on the companion server and on any signed-in device. It stays fresh because both the content and the visual style are chosen anew each year. Section 25 is the build spec for the engine, the timing and the device rules, and boards 39 to 41 draw it.

**Three layers of variation**

1. **Story beats chosen by what is interesting.** The engine computes about 40 possible facts, scores each by how far it sits from this profile's own baseline, how personal it is and how different it is from the beats already chosen, and picks 10 to 12. Eight always appear (intro, total minutes, top song, top artist, top album, your sound, personality, closer); 2 to 4 rotate.
2. **A design pack per year.** Each pack is a complete visual language: type, shape language, motion style and background technique. Four ship first (Editorial, Tide, Bloom and Glass) and one or two more come with each release, because packs ship inside app updates (downloaded code is not allowed, section 19). Contour, Aura, Neon Grid and Paper Collage are held back for later releases. A profile never gets the same pack two years in a row.
3. **Generative details.** Palettes come from that year's top three albums. Layouts and decoration come from a seed of profile ID + year + pack version, so every person's recap looks different, and re-opening it, or opening it on another device, looks the same.

**Beat catalog (starting set)**

- Minutes listened, with a comparison to last year.
- Top 5 songs, artists, albums and genres, each with its own animated card.
- Your listening personality, from a set of about 12 archetypes (for example "The Deep Diver": few artists, many plays each; "The Explorer": many new artists).
- Biggest discovery: the artist you played most who was new to you this year.
- Comeback of the year: an old favorite you returned to.
- Song of each season.
- Your listening clock: what hour and day you listen most.
- Longest streak of days listening.
- Most replayed song in a single day.
- Mood arc of the year, from the audio features of what you played.
- A Deep Cut you loved that almost nobody on your server plays (only with household learning turned on).
- Most-sung lyric line (the line you jumped to most in the lyrics view).

**Experience**

- Opens with a short animated intro and a year line built from your top traits ("Late nights, loud choruses"); tap or swipe through the beats; each song card plays 12 seconds of the song. About 75 seconds in all, never more than 90.
- Ends with a generated playlist "Your Top Songs 2026", saved with one tap, and a detail page with the top 50 lists and Show the math for every number.
- Share any card as an image made on the device (9:16 or 1:1, nothing uploaded, no watermark), with an option to hide minutes or specific artists. A short video can come later.
- Recording never stops early. Plays are counted through 31 December in each play's own time zone. The story opens on December 1 marked "so far", with the counted-through date on every card; it is rebuilt quietly as late plays sync until January 8, then locked and kept in the archive. Past years stay available.
- A smaller "So far this year" and a monthly recap use the same engine.

**Engineering notes**

- The engine is the `encore-recap` crate in the Rust core. It is deterministic: the same log, year and seed give the same bytes anywhere. It outputs a small recap data document (`beats[]`, `pack`, `seed`, `palette`, the counted-through date and a hash of the log it used); each app renders it natively, so it looks right on every platform.
- The NPU or ONNX runtime is used only to build audio fingerprints for tracks (for the mood arc and sound clusters). Counting plays needs no accelerator.
- Every beat is a small, self-contained view type and every pack draws every beat type, so adding new beats and packs is cheap and well-suited to Claude Code.

## 11. Handoff between devices

Any device signed in to the same profile can take over playback in one tap, at the exact second, with the same queue. The companion server's WebSocket carries it.

**Three ways to move music**

- **Push**: in the output picker, other signed-in devices appear next to AirPlay and Bluetooth ("Living Room Mac", "Pixel 9"). Pick one and playback moves there.
- **Pull**: open the app on another device while something is playing elsewhere, and a glass banner offers "Continue from iPhone". One tap pulls it over.
- **Remote control**: control a device that is playing without moving the music, for example the desktop speakers from your phone.

**What happens on a transfer**

1. The sender packages a snapshot: queue (history, up next, autoplay), current track, position, shuffle and repeat, autoplay context, volume.
2. The server relays it to the target and records the handoff in history, so the play is counted once.
3. The target preloads the track, starts at the position plus measured network delay, and confirms.
4. Only after confirmation does the sender fade out and pause. If the target fails, the sender keeps playing and shows why.

Goal: under 1 second from tap to sound when the track is cached on the target, under 2 seconds when it must stream.

**Extras**

- Apple devices also publish an `NSUserActivity` for the system Handoff icon: see section 11 in `docs/apple/PLAN-APPLE.md`.
- Presence shows what each device is doing, so the pull banner only appears when it makes sense.
- Devices of different profiles never see each other, unless a household explicitly marks a device as shared (for example a living-room speaker computer) that anyone can control.
- Without a companion server, handoff is unavailable and the UI hides it.

## 12. Privacy and security

Listening data goes to exactly two places the user owns: their devices and their companion server. The project runs no servers and collects nothing.

**Rules (hard requirements for every change)**

- No analytics, crash reporting or ad SDKs. Crash logs stay on the device until the user chooses to share one.
- No network call to any third party unless the user turned that feature on (lyrics providers, MusicBrainz lookups, history import). Each one is listed by name in Settings > Privacy, off by default except MusicBrainz metadata, which asks on first launch.
- All third-party lookups go through the companion server, so outside services see one server, not every device. Widgets and Live Activities add no connection at all: they read a local snapshot and use no push service (section 20).
- TLS for every connection outside the home network; warn clearly on plain HTTP to a home-network address, and refuse plain HTTP to every other address. iOS blocks it anyway, so the rule is the same on every platform (section 19).
- Tokens only in the OS keychain; nothing secret in plain files or logs.
- Per-profile data is protected at rest by the operating system, with no app-level database encryption in version 1 (no SQLCipher). Apple: Data Protection on iPhone and iPad, FileVault advised on Mac, tokens in the Keychain. Android: file-based encryption and the Keystore. Windows: Credential Manager for tokens, with BitLocker or Device Encryption advised. Linux: Secret Service for tokens, with disk encryption advised. This keeps the store export answers exempt, and an app-level key would not protect an unlocked device anyway. The profile PIN is a screen lock, not encryption. The companion server can optionally encrypt a profile's synced data with a key derived from the server's secret. SQLCipher may return later as an optional, off-by-default setting for Windows and Linux direct builds only.
- Export everything (JSON of history, likes, playlists, recaps) and delete everything, per profile, from Settings. Deleting a profile on the server removes its events, models and recaps. This is the in-app account deletion that both stores require (section 19). Private listening, Guest sessions and Learn from my listening switched off store no listening events; Clear listening history deletes the events on every device, and Reset suggestions only forgets what was learned (section 21).
- Reproducible builds and signed releases so users can verify binaries match the source.

**Threats considered**

| Threat | Mitigation |
| --- | --- |
| Someone else on a shared device | Separate profiles, optional PIN or biometrics per profile |
| Lost phone | Revoke its token from any other device or the server admin page |
| Server exposed to the internet | Rate-limited auth, short-lived access tokens with refresh, admin page bound to the LAN by default |
| Lyrics or metadata services learning your taste | Off by default, proxied, results cached so each track is looked up once |

## 13. Roadmap

The roadmap is `docs/ROADMAP.md`. It keeps the plan's thirteen phases and their "done when" lines word for word and says which track does which part.

## 14. How to build it with Claude Code

The project is built with Claude Code, in four tracks (docs/ROADMAP.md). The sessions' working notes and prompts are kept with each working copy, outside the repository; everything they build from (the plans, the boards, the decisions and the API contract) is here.

## 15. Open questions for you

**Status for Claude Code:** the owner answered only the decisions in section 1 and the Android, Flutter and Liquid Glass design decisions below. Every other row is an assumption, not a confirmed answer. Build on the defaults, but do not treat them as settled; `docs/DECISIONS.md` lists the ones to confirm with the owner before they are hard to change (name and identifiers, build hardware, developer accounts).

Each default is what the plan assumes until the owner says otherwise.

**About you and your setup**

| Question | Default in this plan |
| --- | --- |
| Do you have a Mac to build on? (Apple builds need Xcode) | Yes |
| How much coding do you do yourself vs. leave to Claude Code? | Claude Code writes nearly all code; you review and test |
| Which Jellyfin version runs your server, and on what hardware (NAS, Pi, PC, GPU)? | Jellyfin 10.10 or later on a modest x86 or ARM box, no GPU |
| How do you reach your server away from home? | Tailscale or a reverse proxy you already run |
| How many people will use it, and any kids? | 2 to 5 profiles, kid profiles wanted |

**Product scope**

| Question | Default in this plan |
| --- | --- |
| App name? | Codename Encore until you choose |
| License: GPL/AGPL (forks stay open) or MPL/MIT (easier App Store)? | MPL-2.0 for apps, AGPL-3.0 for the server |
| Publish on the App Store and Play Store, or sideload/F-Droid only? | All stores, after phase 11 |
| Minimum OS versions | iOS/iPadOS 17, macOS 14, Android 9, Windows 10, current Ubuntu/Fedora LTS |
| Extra devices: Apple Watch, Apple TV, Android TV, Wear OS, a web player? | Later; not planned in the first 12 phases |
| Casting: Chromecast, Sonos, other Jellyfin clients? | AirPlay and Bluetooth only at first |
| Lossless, hi-res and Dolby Atmos files? | Play lossless and hi-res as-is; no Atmos |
| Equalizer and audio effects? | Simple 10-band EQ in phase 8 |
| Scrobbling to Last.fm or ListenBrainz? | Optional, off by default |
| Explicit-content filter per profile? | Yes, for kid profiles |
| Podcasts or audiobooks? | No |

**Recommendations and recap**

| Question | Default in this plan |
| --- | --- |
| Allow opt-in household learning across profiles on one server? | Yes, opt-in |
| Allow online lyrics providers? Which ones? | LRCLIB only, off by default |
| Import past listening history from Last.fm or ListenBrainz files? | Yes, as an optional step |
| Default exploration level for autoplay | About 10% new-to-you picks |
| Recap release date | December 1, finalized January 1 |
| Any visual styles you want in the first four recap design packs? | Editorial (a heavy serif on flat colour), Tide (layered waves), Bloom (big soft colour) and Glass (lenses over glows). A set that was all fluid curves was tried and dropped, because every card looked alike and the type was too light; Contour, Aura, Neon Grid and Paper Collage are held back for later releases |

**Design**

| Question | Default in this plan |
| --- | --- |
| On Android, Windows and Linux: copy the Apple look exactly, or adapt it to feel at home there? | Adapt. iPhone, iPad and Mac use Liquid Glass, Android uses Material 3 Expressive, and Windows and Linux use the same Expressive look as a Flutter theme. Structure, flows and behavior are the same everywhere. |
| Any screens from other apps you love that should influence the design? | Apple Music only |
| Light mode, dark mode, or both? | Both, following the system |
| Use native Jetpack Compose on Android (needed for Material 3 Expressive), or keep Flutter there? | Decided: native Compose, because Flutter has said it is not building Material 3 Expressive. Flutter stays for Windows and Linux. |

## 18. Free and legal libraries

Check each license again when you add a library, keep a `THIRD_PARTY_LICENSES` file, and avoid copyleft licenses stronger than MPL-2.0 in the shipped apps unless you choose to release the app under that license too. This list is a starting point, not legal advice.

| Library | Used for | License |
| --- | --- | --- |
| Symphonia | Decoding on Windows and Linux | MPL-2.0 |
| cpal | Audio output on Windows and Linux | Apache-2.0 |
| UniFFI | Rust to Swift and Kotlin bindings | MPL-2.0 |
| flutter\_rust\_bridge | Rust to Dart bindings | MIT |
| Axum | Companion server | MIT |
| SQLite | Local and server storage | Public domain |
| ONNX Runtime | On-device ranking models | MIT |
| Media3 ExoPlayer | Android playback | Apache-2.0 |
| Jetpack Compose and Material 3 | Android UI, Material 3 Expressive | Apache-2.0 |
| Riverpod | Flutter state | MIT |
| go\_router | Flutter navigation | BSD-3-Clause |
| flutter\_liquid\_glasses | Glass look on Flutter | Apache-2.0 |
| ttml\_processor | Parsing word-timed lyrics | MIT |
| Inter | Type on Android, Windows and Linux | SIL OFL 1.1 |
| SF Symbols | Icons on Apple platforms only; Apple's license does not allow them in Android, Windows or Linux apps | Apple license |
| Material Symbols | Icons on Android, Windows and Linux | Apache-2.0 |

Community lyric-display projects such as AMLL have their own licenses; read theirs before reusing any code. SwiftUI, `MeshGradient` and the Liquid Glass APIs come with the Apple SDKs and need no extra license.

| Library | Used for | License |
| --- | --- | --- |
| UniFFI | Rust to Swift and Kotlin bindings | MPL-2.0 |
| flutter\_rust\_bridge | Rust to Dart bindings | MIT |
| Axum | Companion server | MIT |
| SQLite | Local and server storage | Public domain |
| ONNX Runtime | On-device ranking models | MIT |
| ttml\_processor | Parsing word-timed lyrics | MIT |

Community lyric-display projects such as AMLL have their own licenses; read theirs before reusing any code. SwiftUI, `MeshGradient` and the Liquid Glass APIs come with the Apple SDKs and need no extra license.

Libraries that belong to one platform are listed in that platform's plan under section 18.

## 19. Store requirements and release checklist

Checked on 5 November 2026 against Apple's and Google's current pages. The plan already clears most rules by design: no tracking, no ads, no payments, and nothing collected by the project. This section lists what is still to be written, declared, set or shipped. Dates change, so re-check each linked page the week before every submission. This is a checklist, not legal advice.

The build and platform deadlines, the Android 16 rules, networking and encryption rules, the permission lists and each store's sources are in the plan of the track that ships to that store (section 19 in `docs/apple/PLAN-APPLE.md`, `docs/android/PLAN-ANDROID.md`, `docs/windows-linux/PLAN-WINDOWS-LINUX.md`). What follows applies to every store.

**Privacy, accounts and review access**

- **Privacy policy.** One public page, linked in App Store Connect, in Play Console and in the app (Welcome, Privacy and data, About). Apple guideline 5.1.1(i) says it must state what is collected and how, how long it is kept, and how a user revokes consent or asks for deletion. For Encore it says the project collects nothing and that listening and profile data go only to servers the user added.
- **Account deletion.** Profiles are created in the app, so both stores require deletion inside the app (Apple 5.1.1(v)). It is Privacy and data, Delete profile and its data, and it also removes the profile on the companion server and on every signed-in device. The user's Jellyfin account and music are never touched. Google Play also needs a public web page for deletion requests, entered in the Data safety form. The project runs no server, so that page explains the in-app path and the companion server's admin page.
- **Privacy labels.** Apple's label and Google's Data safety form must match what the app does and the "What this app connects to" list. Proposed answers: Apple, Data Not Collected, since the project and third parties receive nothing. Google, whose form asks about data transmitted off the device: declare listening history, profile information and device identifiers as sent to the user's own servers, encrypted in transit and deletable. Confirm the wording against Play's current guidance at submission.
- **Review access.** Apple needs working access to every feature (guideline 2.1). Put a live demo Jellyfin and companion server, with credentials, in the review notes. Also ship the in-app Demo library (Connect board), which needs no server; Apple wants advance approval for a built-in demo mode, so ask in the notes and keep the live server as the main path. In Play Console fill in App access with the same credentials. Demo tracks use open licenses and are credited in About.
- **Sign in with Apple** is not required (guideline 4.8). Encore uses only its own sign-in and the user's own servers, with no third-party or social login.
- **Permissions.** Only the ones on the Permissions board, each asked in context after a primer, none at launch, none blocking a feature. iOS purpose strings: NSLocalNetworkUsageDescription (plus NSBonjourServices if the companion server advertises itself) and NSFaceIDUsageDescription. Capabilities: background audio. Android: FOREGROUND\_SERVICE\_MEDIA\_PLAYBACK, USE\_BIOMETRIC, optional POST\_NOTIFICATIONS, and ACCESS\_LOCAL\_NETWORK once the app targets API 37. Nothing is tracked, so there is no App Tracking Transparency prompt, and sharing goes through the system share sheet, so there is no photo library permission.

**Content, names and code**

- **Name and marks.** "Encore" is a placeholder: check trademarks and store availability, and keep the name under 30 characters (Apple 2.3.7). Keep "Jellyfin" out of the app name and keyword field. Jellyfin's policy reserves the name for the project and its official clients but allows describing an app as "a Jellyfin client", so say "works with Jellyfin" once in the description. Never use the Jellyfin logo. Keep "Apple Music", Apple logos and look-alike icons out of the listing, the app text and the screenshots (Apple 5.2.5); using Liquid Glass through the system APIs is fine.
- **Media rights.** Apple 5.2.3 bars apps that save or download media from third-party services without authorization. Encore plays only the user's own files from the user's own server. Say so in the listing and the review notes.
- **No downloaded code.** Apple 2.5.2 forbids downloaded code that changes an app's features, and Google Play limits code loaded from outside the store. Recap design packs and ranking models are data only (colors, shapes, text, model weights with a fixed architecture). Metal and AGSL shaders ship inside the app.
- **No payments or donation links in the apps.** Apple 3.1.1 limits external payment and donation links outside the US storefront. Keep sponsorship links on the project website only.
- **Minimum functionality and spam** (Apple 4.2 and 4.3). Other Jellyfin clients exist, so the review notes should name what Encore adds: mixes, recaps, profiles and handoff.
- **Icons.** SF Symbols on Apple platforms only; Material Symbols elsewhere (section 18).
- **Accessibility.** Touch targets of at least 44 pt on Apple and 48 dp on Android even when the drawn shape is smaller, Dynamic Type and font scale, screen-reader labels, Reduce Motion.

**Before each submission**

- [ ] Privacy policy page live and linked in App Store Connect, Play Console and the app
- [ ] Deletion web page live and its URL entered in the Play Data safety form
- [ ] Apple privacy label and Play Data safety answers match the "What this app connects to" list
- [ ] PrivacyInfo.xcprivacy present in the app and every bundled framework
- [ ] Info.plist: only used purpose strings, ATS settings, ITSAppUsesNonExemptEncryption, background audio
- [ ] Android manifest reviewed: permissions, media playback service type, target API 36; Play foreground service declaration filled in
- [ ] Apple age rating and Google IARC questionnaires answered
- [ ] Review notes: demo server address and credentials, how to open the Demo library, why there is no account with the app maker; Play App access filled in
- [ ] Screenshots from real builds on each device class, with no Apple Music marks
- [ ] Name, subtitle and keywords checked for Jellyfin, Apple marks and trademark conflicts
- [ ] Play closed test finished: 12 testers, 14 consecutive days
- [ ] EU trader status declared; Android developer verification registered
- [ ] THIRD\_PARTY\_LICENSES matches the About screen, including demo track credits

**Not checked in this pass.** Apple's accessibility labels for the App Store page, Play's exact Data safety wording for self-hosted servers, how App Review treats a justification for `NSAllowsArbitraryLoads`, 16 KB page-size alignment for the Rust and ONNX native libraries on Android, and trademark clearance for "Encore". Check them before the first submission.

**Sources**

- [Jellyfin branding policy](https://jellyfin.org/docs/general/contributing/branding)

## 21. Smart picks: logic for everything that chooses a song

Autoplay, the five mixes, Smart shuffle, radio, Rediscover, widgets and Smart downloads all go through one engine with one set of rules, so a skip teaches every surface at once. This section extends sections 7 and 7.1 without changing them: the signal weights, the 30 second early-skip boundary, the 10% default exploration and the five mixes stay as written, and what is new is marked New. The Smart picks boards (signals and rules, screens and states) draw all of it. Section 7 says "track" for the engine and the screens say "song"; they mean the same thing.

### What this section adds to section 7

| Topic | Section 7 | Now |
| --- | --- | --- |
| Skip of a song the user picked | -0.2, often browsing | Same weight, but ignored until the same song is skipped 3 times in 14 days (New) |
| Skip between 30 seconds and 80% | -0.5 | Fades with a 30 day half-life (New) |
| Skip in the first 30 seconds, app-chosen | -1.5, the key signal | Same weight, fades with a 30 day half-life, and steers the rest of the session |
| Added to a playlist or downloaded | Not listed | +1.5 strong intent; removing it later does not undo the signal (New) |
| Not interested | Track, or artist if chosen | A set of adds and removes with scope, optional reason, Undo for 5 s and a list in Settings (New) |
| Interruptions and errors | Not listed | Never count as skips (below) |
| Private listening and Guest | Not listed | Store nothing, learn nothing (New) |
| Smart shuffle, Smart downloads | Not listed | Two new surfaces (New) |

### Events that do not count

A wrong lesson is worse than no lesson, so these never become a negative signal:

- **Interruptions**: a skip within 3 seconds of a headphone unplug, a Bluetooth or AirPlay switch, a call or an assistant request.
- **Errors**: unavailable songs, failed loads and automatic skips made by the app.
- **Seeking**: dragging the progress bar is never a skip.
- **Accidents**: anything stopped under 2 seconds after it started.
- **Too soon**: a negative for a song heard in the last 2 hours is stored as "too soon", not as dislike.
- **Ends by the system**: a sleep timer, a closed app or a lost connection.
- **Private listening, Guest, or Learn from my listening switched off**: nothing is stored at all.

### In-session adjustment (starting values)

- Three early skips in a row of app-chosen songs: exploration drops to zero for the next 10 picks, the session's sound moves 30% away from the skipped songs, and the unplayed autoplay songs are swapped in the background. Songs the user queued are never touched.
- A like or a replay moves the session 30% toward that song, and the next two picks come from its nearest neighbours.
- Both run on the device before the next song starts. Tune the numbers with replay tests on real histories.

### Where the engine is used

| Surface | Asks the engine for | Refresh | When there is no data, offline or learning is off |
| --- | --- | --- | --- |
| Home: Made for you | The five mixes (section 7.1) | Nightly and after 25 or more new plays; never while a mix plays | Yesterday's mixes with an age label. Under 50 plays: up to three mixes from library audio. Learning off: Recently added and Most played |
| Home: Rediscover | Loved songs not played for 60+ days, 12 of them | Daily | Random liked songs |
| Home: Recently played | Not smart: plain history order | Live | Always available |
| Mixes tab | The same mixes with a reason line, plus Deep Cuts | With the mixes | Same as Home |
| Queue: Autoplay | 10 songs seeded by the current song and the last 5 finished | Refills when fewer than 5 are upcoming; touches only the Autoplay section | Related artists, genre and era; offline, downloaded songs only |
| Smart shuffle (New) | One suggestion per 4 songs of the list being shuffled | Live | Plain shuffle; suggestions carry a spark mark and swipe away |
| Radio | Autoplay with the seed held for the whole session | Live | The seed's related artists |
| Search | Order of equally good text matches only | Live | Text relevance; the personal boost is capped at 15% and never hides an exact match |
| Widgets, CarPlay, Android Auto | The same mix snapshot, written per profile | With the mixes | Last snapshot with an age label |
| Smart downloads (New) | Top songs of pinned mixes inside a size budget | Nightly, on Wi-Fi and charging | Off by default; with no data it downloads nothing; 2 GB default budget |
| Year-end recap | Reads the event log only, never the model | Year end | Needs 100 plays, otherwise a short card |

### Ranking and shaping rules

1. **Hard filters first**: Not interested, hidden explicit songs when set, unavailable or not downloaded songs when offline, duplicates of the same recording, and anything played in the last 24 hours unless it is loved.
2. **Score** is the model's chance that the song is played past 30 seconds and finished. Until a model exists, use 0.35 affinity, 0.20 session fit, 0.15 time-of-day fit, 0.10 freshness, minus 0.20 for the recent skip rate in similar contexts (starting values).
3. **Exploration**: Familiar 5%, Balanced 10% (default) or Adventurous 20% of slots, chosen by Thompson sampling. Never the first slot, never two in a row, and always right after a song the user is likely to finish. A change applies from the next refill and never reshuffles a list being heard.
4. **Diversity**: re-rank with maximal marginal relevance (0.7 weight on score) over sound and artist. No artist twice within 3 songs, at most 2 per artist and 4 per album in a mix.
5. **Flow**: tempo and energy move one step at a time, except at a planned mood shift.
6. **Stability**: a shelf changes by at most 30% on refresh unless the user pinned it or taps Regenerate.
7. **Explainable**: every pick keeps its top two reasons for "Why this song".

**Budgets (starting values)**: the next-song decision takes under 50 ms because it reads a precomputed list; a full mix build takes under 20 s for 20,000 songs on a mid-range phone, uses under 150 MB of memory and never runs in the first seconds of playback; background jobs stop on a thermal warning.

### Cold start

| Plays | What the user sees |
| --- | --- |
| 0 | Recently added, Rediscover and a Starter mix from the server's favourites and most played. Autoplay follows artist, genre and era. An optional import of Last.fm or ListenBrainz history skips this step |
| 1 to 49 | Up to three mixes from library audio (calm, upbeat, focus), a one-time "Encore is still learning" note, exploration fixed at Balanced |
| 50 or more | Five mixes, session adjustment and specific "Why this song" reasons |
| 30 days of history | Time-of-day mix, a mix for where taste has drifted, and the stretch mix with about 30% new-to-you songs |

### When something is missing

| Situation | What happens |
| --- | --- |
| No companion server | The device trains while charging and idle with the same Rust code; nothing else changes |
| Server unreachable | The last model keeps working; events queue on the device and sync later |
| Model file missing, corrupt or too new | Rebuilt from the event log; hand-tuned scoring runs in the meantime |
| Audio analysis not finished | Metadata and co-listen only; analysis continues and upgrades the picks later |
| Songs removed from the library | Dropped silently and refilled; a tile never points at a missing song |
| Offline | Candidates limited to downloaded songs; mixes say "Only downloaded songs" |
| Battery saver or high temperature | Background builds wait; live ranking still runs, since it takes milliseconds |
| Library over 100,000 songs | Candidates come from an approximate nearest-neighbour index, capped at 200 per pool |
| Profile switch or Guest | Model, events and mixes belong to the profile and swap in under a second; Guest learns nothing and uses the plain fallbacks |

### Controls the user gets

- **Why this song**: opened from More on Now Playing, a queue row or a mix tile. One or two reasons, never more, built from facts the user can check (what they played, when, what sounds similar), and up to two "sounds like" songs that open on tap. It never guesses mood, health or location. More like this counts +1.5 for the song and its neighbours in this context without adding a like; Fewer like this counts -1.0. Both give a light haptic and a one-line confirmation.
- **Not interested**: scope is this song or this artist, with an optional reason. Not my taste lowers the song and its neighbours; Heard it too much only rests the song for 30 days; Wrong moment lowers it in that time of day only. The song leaves every smart surface at once but still plays when the user picks it. A warning haptic confirms, Undo shows for 5 s, and later undo lives in Settings. The same sheet opens from an artist page.
- **How adventurous**: Familiar, Balanced or Adventurous, with a plain sentence under it ("About 1 in 10 picks will be new to you").
- **Learn from my listening**: turning it off asks once, keeps saved history and switches every smart surface to the plain fallbacks. Turning it on again does not back-fill the time it was off.
- **Private listening**: a per-session switch in the Now Playing menu that ends after 2 hours of silence and shows a small badge while on. Guest sessions behave the same way.
- **Reset suggestions** forgets the learned model, co-listen graph and mixes and starts a new learning epoch; likes, playlists, history and the Not interested list stay. **Clear listening history** deletes the events on every device and the server, and the year-end recap then starts from that day. They are different buttons with different confirm text, both with a red button separated from Cancel.
- **Smart downloads** is a Settings switch, off by default.
- **How suggestions are doing**: a private, local page with skip rate of app-chosen songs, completion, distinct artists per 20 songs and repeats, per shelf, over 4 weeks. Nothing leaves the device.

### Mix card states

Fresh, Building (up to 20 s, the rest of Home stays usable and yesterday's mix keeps playing), Out of date (after 48 hours without a good build, with a Refresh button, never an empty tile), Not enough plays yet (with a count such as 12 of 50), Learning is off, Private listening, Offline, and the Not interested undo toast. Every state has a text label, so colour is never the only signal.

### Rules that apply everywhere

1. **The event log is the truth.** Models, mixes and caches can always be rebuilt from it.
2. **Who chose the song matters.** A skip of a song the user picked is browsing; a skip of a song the app picked is the strongest negative there is.
3. **Sync.** Events are append-only and merged by id. Models carry a version and a training epoch, and the newest epoch wins. Not interested is a set of adds and removes, last write wins per item. Clear and Reset sync as tombstones, so another device cannot bring the data back.
4. **Privacy.** Listening data stays on the user's devices and their own server. Looking up related artists on MusicBrainz, importing from or scrobbling to ListenBrainz or Last.fm are off by default, ask per profile and send song identifiers only.
5. **Measuring.** Offline replay on test histories runs in CI, and the v1 target is to beat plain shuffle by a wide margin (section 7). No telemetry leaves the device.
6. **Licences.** The audio embedding model and the nearest-neighbour library each need a licence check before release. Some audio analysis tools use copyleft licences that fit the server (AGPL) but not the apps (MPL).

**Not verified.** Every number in this section (weights, half-lives, the 0.7 diversity weight, budgets, thresholds) is a starting value chosen for this plan, not a measured result. Tune them with offline replay on real listening histories before release.

## 25. Year recap: engine, devices and look

This section is the build spec behind section 10. Boards 39, 40 and 41 draw it. It replaces what the older recap board said about card count, timing and where the numbers are computed. Every number here is a starting value to tune, not a measurement; keep them in `design/tokens.json` (`recap`) or config.

**What it has to do.** One recap per profile and year that people want to finish and share, that never looks the same two years running, that comes out identical on every device the profile is signed in on, and that keeps counting until the year really ends. Spotify stops counting weeks before the year closes; Encore counts to 23:59:59 on 31 December and folds in late plays until 8 January.

### How it is made

- **Events.** Every device writes play events as you listen, even offline. The fields the recap needs are listed in section 5 and must exist from the first build, because history cannot be rebuilt later: a stable song key, listened milliseconds, the time in UTC plus the UTC offset at that moment, and lyric-line jump events.
- **Rollups.** The core keeps small daily rollups per profile (plays, listened time, per song, per artist, per hour of the week). They are rebuildable from the log at any time.
- **The engine.** `encore-recap` is a crate in the Rust core. It is a pure function: the same log through a date, the same year, the same profile settings and the same pack list always give the same bytes. It computes about 40 candidate facts, scores them, picks the story, then chooses pack, palette and seed.
- **Where it runs.** On the companion server on 1 December, on 1 January and whenever new plays arrive until 8 January. On any device when the server is off or has not caught up. Because the engine is the same code, the two never disagree; a parity test in CI proves it.
- **Output.** Recap data: a small JSON document under 100 KB with `beats[]`, the pack and its version, the seed, the palette, the date it is counted through and a hash of the log it used. The server archives every year; devices cache it.
- **Where the NPU or ONNX runtime helps.** Only in building a sound fingerprint (an audio embedding) for each track, once, when it joins the library: Core ML on Apple devices, ONNX Runtime elsewhere and on the server. The recap reads those fingerprints for the mood arc and the sound clusters. Counting plays needs no accelerator. If a track has no fingerprint, beats that need it are left out, never faked.
- **Words.** Sentences come from a bank of templates chosen by the seed. There is no language model in version 1. If an on-device model is added later it may reword sentences and may never write or change a number.
- **What stays out.** No cloud service, no other people's data and no claims such as "top 1% of listeners", because the app cannot know. Household beats use only profiles on the same server that have both opted in.

### Counting rules

- The same song on two albums counts once. A song is matched by MusicBrainz ID when tagged, otherwise by artist, title and duration (within 2 s).
- Listened time is counted, not track length.
- Private listening, Guest, interruptions and plays under 2 s are excluded. Section 21 already stops them being stored.
- Each play lands on the calendar day of its own time zone, using the stored offset, so travel and daylight saving time do not move plays between days.
- Clear listening history deletes the plays and restarts the recap from the day of the clear ("Since 3 Aug"). Archived past years stay unless the user ticks Delete past recaps in the same confirm sheet.
- Kid profiles filter explicit titles and skip late-night beats.
- A beat needs enough data: a streak needs 7 days, a discovery needs 50 plays.
- Under 10 plays: no recap card on Home. From 10 plays up to 20 hours and 100 plays: a short 3-card recap (minutes, top song, top artist). From 20 hours and 100 plays: the full story.

### When it is ready

| Window | What happens |
| --- | --- |
| 1 January to 30 November | Every play is counted as it happens. "So far this year" is available any time and uses the same engine. |
| 1 to 31 December | The full story opens, marked "so far", and updates daily. Every card says what date it is counted through. |
| 1 to 8 January | Final run. It is rebuilt quietly as devices that were offline sync late plays. A small note says how many plays were added since you last looked. |
| From 8 January | Locked. It never changes again and stays in the archive, which survives a reinstall. |

### Story rules (board 39)

1. **Length.** 10 to 12 cards, about 75 seconds, never more than 90. Eight are fixed (intro, minutes, top song, top artist, top album, your sound, personality, closer) and 2 to 4 rotate. A card runs 6 to 8 s and a list card 9 s.
2. **Arc.** Opener, a build of the big numbers, a turn (the sound drifted), surprises (the rotating beats), identity (the personality), then the close. The order follows these roles so it feels built, not like a list of stats.
3. **Rotating beats.** A pool of about 12: listening clock, biggest discovery, comeback, obsession week, longest streak, song of each season, most replayed day, mood arc, most-sung lyric line and a household deep cut (opt in). Each is scored by distance from this profile's own baseline, how personal it is and how different it is from beats already chosen.
4. **Top 5.** Each fixed card shows number one big and reveals the rest of its top 5 after 2 s. Full lists live on the detail page.
5. **Sound.** A song card plays 12 s of the track, starting where the profile most often jumped to, otherwise 30% in, with a 400 ms crossfade. The Sound button is remembered per device. Offline and not downloaded: silent, with a small note. Hold pauses the sound too.
6. **Taps.** Tap the right 65% to go forward and the left 35% to go back. Hold pauses and hides the controls. Swipe down closes, and it resumes where it left off for 30 days. The first tap is ignored for 300 ms after a card appears.
7. **Copy.** Friendly, never judgmental, no mood or health claims. A year line built from the top traits ("Late nights, loud choruses") opens the story and titles every share card.
8. **Share.** Every card exports on the device as 9:16 and 1:1. Nothing is uploaded and there is no watermark. An option hides song and artist names first.
9. **Detail page.** After the story: top 50 songs, artists and albums, month by month, and every number behind the personality. Show the math on any card lists the counts and the rule, such as "Deep Diver: your top six artists were 34% of plays".
10. **Accessibility.** Every card has a text version read in order, every chart has its values as text, Reduce Motion shows the final state, nothing flashes more than three times a second, and auto-advance is off while a screen reader runs.
11. **Motion.** Smooth and unhurried in every pack. Shapes ease in over 400 to 600 ms and numbers count up over 600 ms. Ambient motion (Tide's waves, Bloom's colour fields, Glass's glow) drifts at 8 to 14 s a cycle, is capped at 30 fps and stops in the background and in Low Power Mode. Editorial uses slow wipes and rolling numerals. Reduce Motion shows the still final frame. Backgrounds are vector shapes and gradients, not video, so they stay sharp and small.

### Looking different every year (board 40)

- **Pack.** Four ship first: Editorial (a heavy serif on flat paper, ink or one spot colour, with ruled lines and ledger charts), Tide (solid layered waves, with a sunset, night or sea mood on each card), Bloom (big soft colour fields, petals and blobs) and Glass (lenses and bubbles over blurred glows). Each is a complete visual language: type, shapes, motion, background and how it draws the data. Editorial uses a bundled open-licence serif, Tide and Bloom use Inter in heavy weights, and Glass uses the system font on Apple devices and Inter elsewhere. The pack is picked by a seed made from the profile, the year and the pack version, and is never the same as last year (never either of the last two once there are six or more).
- **Palette.** Built from the year's top three albums with the sampling in section 16, then fitted to the pack: Editorial paper, ink and one spot colour, Tide a sky gradient, four waves and deep water, Bloom a cream ground and four saturated colour fields, Glass four glows on near-black. The lead hue must sit at least 60° from last year's, or the next best colour cluster is used.
- **Layout.** The same seed picks one of three or four layouts for every beat and places the shapes, so the same pack looks different from one person to the next and from one year to the next. Reopening it, or opening it on another device, gives the identical result.
- **The data is the artwork.** Each pack draws the person's own numbers in its own way: Editorial a ruled bar chart and an hour grid; Tide a swell for the months, a flowing stream for the genre mix and a tide ring for the hours; Bloom petals and a blob; Glass bubbles sized by plays and a lens ring.
- **Repeat packs still change.** When a pack comes round again it gets a new palette and a different layout.
- **New packs** arrive with app updates, because downloaded code is not allowed (section 19). Plan on one or two per release. Held back for later releases: Contour (flowing map lines), Aura (mesh gradient with glass orbs), Neon Grid and Paper Collage.
- **Pack contract.** Every pack draws every beat type, passes a 4.5:1 text contrast check, and passes snapshot tests on extreme histories: one artist, 5,000 artists, very light and very dark art, very long names, non-Latin and right-to-left scripts, and 100 plays against 100,000.

### Devices and sync (board 41)

- **Which pack a profile sees.** The recap data pins the pack. It is chosen from the packs that every device the profile used in the last 90 days has. An older app draws the pack's named fallback and says "Shown in a simpler look on this device".
- **Recap data** is cached on each device. The newest counted-through date wins. Two devices that built it from the same log produce identical bytes.
- **Progress** (card number and finished flag) syncs as one small field, last writer wins. Open it on the Mac and it resumes where the phone stopped. The Sound toggle does not sync. The "new" dot on Home clears on every device once the story is finished.
- **Late plays** synced after 31 December are counted by the time they happened, and the recap is rebuilt quietly until 8 January.
- **Window sizes.** Compact (under 600): full screen, the background runs under the system bars. Medium (600 to 839): the 9:16 card sits in the centre of a soft glow made from the palette. Expanded and Large (840 and up): the card keeps its shape and a Details panel opens beside it with the full top 5, Show the math and Share. Arrow keys step, Space pauses, Escape closes.
- **States.** Not much data (short version), very new profile (nothing yet), preview in December ("So far" tag), final (tag gone, archived), updated by a late sync, server off (device builds it), offline with art not cached (shapes in place of covers, sound off), older app (fallback look), kid profile, Guest or Private (no recap), Recap off in Settings (nothing shown and no rollups kept).

### Tests

- Golden files: fixed logs give exact expected recap data. The same logs run through the server build and each platform's core give identical bytes.
- Snapshot tests for every pack and every beat on the extreme histories above.
- A property test that the same input always gives the same output, and that a different year gives a different pack.
- A timing test: a build over 100,000 plays on a mid-range phone with the server off. The budget is to be measured; the build must run in the background and never block playback.

### Unverified

The card timing, the 60° hue gap, the 20 hour and 100 play thresholds and the 90-day device window are starting values. The build time for 100,000 plays has not been measured. The audio embedding model and its licence are not chosen. The frame rate cap and the battery cost of the drifting backgrounds on a low-end phone have not been measured.
