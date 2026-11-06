# Encore core API (the contract)

**Version 1** (Phase 2, 6 November 2026). Core crate version `0.1.0`.

This file is the only thing the platform tracks may rely on. Every public call, event and type of the core is listed here before a platform uses it. A change that breaks a platform gets a line under "Changes" and a request in `docs/requests/` that says what to change.

## Changes

| API version | Phase | What changed |
| --- | --- | --- |
| 0 | 1 | First version: `hello`, `core_version`, `app_name`; token constants |
| 1 | 2 | `EncoreCore`: connect and sign in to Jellyfin (password or Quick Connect), profiles, library sync, browse, search, the queue, playback reporting with the play log, streams, artwork, raw lyrics, events. Nothing from API 0 changed. Kotlin now also needs kotlinx-coroutines (async calls are suspend functions). |

## How every platform gets the core

| | Apple (Swift) | Android (Kotlin) | Windows and Linux (Dart) |
| --- | --- | --- | --- |
| Generated code | `core/bindings/swift/`: `EncoreCore.swift`, `EncoreCoreFFI.h`, `EncoreCoreFFI.modulemap` | `core/bindings/kotlin/uk/gcartee/encore/core/encore_core.kt` | The Dart package at `core/bindings/dart/` (`encore_core`) |
| Generator | UniFFI 0.32.2 | UniFFI 0.32.2 | flutter_rust_bridge 2.13.0 |
| Rust crate to build | `encore-core` (`core/encore-core`) | `encore-core` | `encore-dart` (`core/encore-dart`), which wraps `encore-core` |
| Library to build | static library `libencore_core.a` for each Apple target, packaged as an XCFramework by the Apple track | `libencore_core.so` for each Android ABI, in `jniLibs/<abi>/` | `encore_dart.dll` (Windows), `libencore_dart.so` (Linux) |
| Runtime dependency | none | JNA (`net.java.dev.jna:jna`, the `@aar` build on Android) and kotlinx-coroutines. The Android track chooses and licence-checks the versions; CI tests with JNA 5.17.0 and kotlinx-coroutines 1.11.0 | `flutter_rust_bridge` 2.13.0 (Dart package, exact version) |
| Package or module | Swift module `EncoreCore` (the FFI module is `EncoreCoreFFI`) | `uk.gcartee.encore.core` | `package:encore_core/encore_core.dart` |
| The API object | `EncoreCore` | `EncoreCore` | `Encore` (same calls, errors flattened to `ApiError`) |
| Token constants | `core/bindings/tokens/EncoreTokens.swift` | `core/bindings/tokens/EncoreTokens.kt` (package `uk.gcartee.encore.tokens`) | `package:encore_core/encore_tokens.dart` |

Rules for every platform:

- Build the library from the **same commit** as the generated code you use. UniFFI checks API checksums when the library loads and fails if they differ.
- Never edit generated files. When something is missing, write a request (`docs/requests/`).
- Each platform builds the core for its own targets inside its own folder, with `cargo build --release` from `core/` (`--target <triple>`). The Rust source stays in `core/`.
- Apple: the static library needs the system libraries rustc lists with `--print native-static-libs`. CI's `apple` job prints them for every target (today `-lSystem -lc -lm` plus the Security and CoreFoundation frameworks the certificate check uses).
- Android: link with NDK r28 or later, which aligns to 16 KB by default, or pass `-Wl,-z,max-page-size=16384 -Wl,-z,common-page-size=16384`. The `android` job in `core.yml` shows a working setup. Certificate checks use the operating system's verifier. On Android, call `EncoreTls.initialize(applicationContext)` once at start, before the first https connection; it's in `core/bindings/kotlin-android/uk/gcartee/encore/core/EncoreTls.kt`. The app build also needs the verifier's Kotlin component (request `core-0001-android-tls-init`). Until the call runs, https fails with a message saying so; nothing crashes, and plain http on the local network still works.
- Regenerate everything with `make bindings tokens` in `core/`. The Dart bindings need flutter_rust_bridge_codegen 2.13.0 and a Dart SDK; without them, run the `core` workflow by hand with "regenerate" ticked and take the files from its artifact.

## Threading, errors and privacy

- **Threads.** Every call may be made from any thread. Calls marked *async* are `async throws` in Swift, `suspend` in Kotlin and `Future` in Dart; they run on the core's own small runtime. Every other call is synchronous, reads the on-device mirror and returns in milliseconds, so it is safe on the main thread. Search stays under 50 ms on 50,000 songs (`search.budgetMs`, measured in CI).
- **Events** go to the listener on a core thread: hop to the UI thread before touching UI.
- **Errors** are one error type with one case per reason, each with a `message` sentence a person can act on (boards 18, 20 and 21): `InvalidAddress`, `InsecureAddress`, `Unreachable`, `CertificateUntrusted { sha256 }`, `NotJellyfin`, `UnsupportedVersion`, `WrongCredentials`, `SessionExpired`, `QuickConnectDisabled`, `QuickConnectExpired`, `QuickConnectCancelled`, `NotFound`, `Server { status, detail }` (show `detail` in a "Details" disclosure), `NoProfile`, `Storage` and `Internal`. Swift: `EncoreError.WrongCredentials(message:)`. Kotlin: `EncoreException.WrongCredentials`. Dart: `ApiError` with `kind` (an `ErrorKind`), `message`, and `sha256`, `status` and `detail` where they apply.
- **Tokens never live in the core.** Signing in returns `SignedIn.token`. Store it in the keychain or keystore under `SignedIn.credential_key`. When the app starts, read it back and pass it to `open_profile`. The core keeps it in memory only, never writes it to disk or a log, and leaves it out of every `Debug` print. Stream and artwork URLs carry it, so never log those either.
- **Nothing leaves the device** except calls to the user's own media server. No telemetry.

## Starting up

```
EncoreCore(config)                  CoreConfig { data_dir, device_id, device_name, platform }
  set_listener(listener)            events: see below
  profiles()                        the profiles on this device, with their credential keys
  open_profile(id, tokens)          tokens read from the keychain; a play left open by a crash is closed
  sync_library()                    async; first a full sync, then deltas
```

`device_id` is stable per installation; the app stores it. `data_dir` is a folder the app owns; each profile is `profiles/<profile_id>/library.sqlite` inside it.

## Calls

Inputs and outputs use the types under "Types". "async" marks calls that wait on the network.

### Connect and sign in (boards 18 and 21)

| Call | Returns | Notes |
| --- | --- | --- |
| `check_address(address)` async | `ServerCheck` | What the user typed: a bare host, a host and port, or a full URL. `https` is added when missing. Without a port it tries 443, then 8920, then plain http on 8096 for local addresses only. Plain http to a public address fails with `InsecureAddress`. `plain_http_on_local_network` asks for the quiet warning. A self-signed certificate fails with `CertificateUntrusted { sha256 }`. |
| `trust_certificate(address, sha256)` | – | After the user saw the fingerprint and chose to trust it. Then call `check_address` again. The pin is saved with the server on sign-in. |
| `sign_in(base_url, username, password)` async | `SignedIn` | The password is sent once and never kept. `WrongCredentials` on a wrong password. Opens the account's profile. |
| `quick_connect_start(base_url)` async | `QuickConnectStart` | The 6-character code and its expiry (`library.quickConnectExpirySeconds`, 300). `QuickConnectDisabled` when the server has it off. |
| `quick_connect_wait()` async | `SignedIn` | Returns once the code is approved on another device. Fails with `QuickConnectExpired` or `QuickConnectCancelled`. |
| `quick_connect_cancel()` | – | Cancel on board 21. |

### Profiles

| Call | Returns | Notes |
| --- | --- | --- |
| `profiles()` | `[ProfileSummary]` | Each with its `credentials`: where each source's token is kept. |
| `open_profile(profile_id, tokens)` | – | `tokens`: `[SourceToken]` read from the keychain. A source without a token is browsable offline and reports `signed_in = false`. |
| `close_profile()` | – | Ends the current play as "ended by the system". |
| `current_profile()` | `profile_id?` | |

Phase 4 adds the profile switcher, Guest and sync to the companion server.

### Servers and library sync

| Call | Returns | Notes |
| --- | --- | --- |
| `servers()` | `[ServerRow]` | Board 18 rows: name, type, version, user, song count, last sync, signed in. |
| `sync_library()` async | `[SyncResult]` | Every signed-in source. The first time it is a full sync, then deltas from the server's own clock (only changed items plus a removal check). Progress arrives as `SyncProgress` events ("Indexing 3,200 of 12,480"). `SessionExpired` means sign in again. |
| `sign_out(source_id)` async | – | Revokes the token on the server and drops it here. The mirror stays. Delete the keychain entry. |
| `remove_server(source_id)` | – | Deletes the local index of that server; nothing on the server is touched; history stays. |

### Browse (offline, synchronous)

| Call | Returns |
| --- | --- |
| `albums(sort, offset, limit)` | `[AlbumSummary]`; `AlbumSort`: Title, Artist, Year, RecentlyAdded |
| `album(album_id)` | `AlbumDetail` (songs in disc and track order) |
| `songs(sort, offset, limit)` | `[Song]`; `SongSort`: Title, Artist, Album, RecentlyAdded |
| `artists(offset, limit)` | `[ArtistSummary]` |
| `artist(artist_id)` | `ArtistDetail` (albums newest first) |
| `artist_songs(artist_id, offset, limit)` | `[Song]` |
| `playlists(offset, limit)` | `[PlaylistSummary]` |
| `playlist(playlist_id)` | `PlaylistDetail`; an entry whose song left the library has `song = null` and stays listed (board 20) |
| `genres()` | `[GenreSummary]` |
| `genre_songs(genre, offset, limit)` | `[Song]` |
| `song(song_id)` | `Song` |

### Search (board 07, synchronous)

| Call | Returns | Notes |
| --- | --- | --- |
| `search(query)` | `SearchResults` | As the user types. Matches any part of a name, ignores accents and case, and tolerates typos. Sections: `top` (exactly one of its fields is set, as `kind` says), then songs, albums, artists and playlists (limits in `search.limits`). Lyrics search comes in Phase 3. |
| `add_recent_search(query)` | – | The return key. Saved per profile, newest first, at most `search.recentSearchesMax`. |
| `recent_searches()` / `remove_recent_search(query)` / `clear_recent_searches()` | | |

### Queue (boards 11 and 42, synchronous)

Every change returns the new `QueueState` and also sends `QueueChanged` (and `NowPlaying` when the current song changed).

| Call | Notes |
| --- | --- |
| `queue()` | The current state. |
| `play(song_ids, start_index, started_from, context_id)` | Play a list (an album, a playlist, search results) from `start_index`. `context_id` is the album's or playlist's ID, kept in the play log. |
| `play_next(song_ids)` / `add_to_up_next(song_ids)` | Top or end of Up Next. |
| `next()` | Button, swipe, headset, lock screen. At the end of the queue the current song stays. |
| `previous(position_ms, from_swipe)` → `PreviousResult` | `Restarted`: seek to 0:00 (the button after `previousButtonRestartAfterSeconds`, 3 s). `Moved`: play the new current song. The swipe (`from_swipe`) always moves back. |
| `jump_to(uid)` | The songs passed over move to History. |
| `replay_from_history(uid)` | |
| `remove_from_queue(uid)` / `move_in_queue(uid, to_index)` | Moving an Autoplay song puts it in Up Next. |
| `clear_up_next()` | Autoplay stays. |
| `undo_queue()` | Undo the last change (the Undo toasts); `QueueState.can_undo` says whether there is one. |
| `set_shuffle(on)` | Shuffles Up Next only; off restores the order the songs were added in. |
| `set_repeat(mode)` | Off, All (the session starts again), One (replays on finish; Next moves on). |

Autoplay is empty until Phase 5 fills it.

### Playback reporting (plan sections 5 and 8)

The platform's own player plays the audio; the core owns the queue and the play log. The sequence for every song:

1. `stream_for(song_id, network)` → `StreamInfo`: load `url` with `headers` (ExoPlayer, the desktop engine) or `url_with_api_key` (players that cannot send headers, such as AVPlayer).
2. `playback_started(uid, utc_offset_minutes)` when sound starts. This writes the play event at once. `utc_offset_minutes` is the device's current offset (east positive).
3. `playback_progress(position_ms, listened_ms)` about once a second. `listened_ms` counts time actually played, not seeks. The core checkpoints every `playLog.checkpointSeconds` (10 s) and sends `Preload` with the next song 20 s before the end (`playback.preloadBeforeEndSeconds`): call `stream_for` for it and load it for gapless playback.
4. `playback_ended(reason, listened_ms)` → the new current `QueueEntry?`. `Finished` and `Error` move the queue on (Repeat One replays a finished song); `PausedAbandoned`, `Interrupted` and `EndedBySystem` leave it. A song interrupted by Next or Previous needs no call: the next `playback_started` closes it as skipped with the last reported `listened_ms`.

| Call | Notes |
| --- | --- |
| `lyric_line_tapped(line_index, line_start_ms)` | Logged with the current play (a section 5 field). |
| `set_listening_mode(mode)` | `Private`, `Guest` and `LearningOff` record nothing at all (checked at the database level); `Normal` records. Switching away from Normal closes the current play. |

Each play event stores the song key, the source and its item ID, the title, artist and album (so history survives the song leaving the library), the duration, the UTC start and offset, the local hour and weekday, listened milliseconds, the end reason, what started it, the context ID, who chose it (user or app), the device ID, a ULID and a hybrid logical clock value. No location is ever stored.

### Streams, artwork, lyrics

| Call | Returns | Notes |
| --- | --- | --- |
| `stream_for(song_id, network)` | `StreamInfo` | Quality from the setting for that network: Wi-Fi defaults to original (gapless info intact), cellular to high (320 kbps). `SessionExpired` when the server has no token. |
| `stream_quality(network)` / `set_stream_quality(network, quality)` | `StreamQuality` | Original, High (320), Normal (160), DataSaver (96) kbps (`playback.quality`). |
| `set_playback_capabilities(direct_play_containers, transcode_codec)` | – | Once per platform: the containers the player plays as they are (default flac, mp3, m4a, aac, alac, ogg, opus and wav), and the codec for transcodes (`Aac` or `Opus`; default AAC). |
| `artwork(of, id, size_px)` | `ArtworkRequest` | The image at exactly `size_px` square. `ArtworkOf`: Album, Song (its album's art when there is one), Artist, Playlist. Load it with `headers`. Phase 3 adds the disk cache and the colour palette. |
| `lyrics_raw(song_id)` async | `string?` | Jellyfin's lyrics JSON as sent (lines with start ticks and word cues), or null. Phase 3 adds the parsed lyrics model; do not parse this in the UI. |

## Events

`set_listener(listener)` in Swift and Kotlin (implement `CoreListener.on_event`), `events()` in Dart (a `Stream<CoreEvent>`). One `CoreEvent` record; only the fields its `kind` names are set.

| `kind` | Set fields | When |
| --- | --- | --- |
| `QueueChanged` | `queue` | After every queue change |
| `NowPlaying` | `now_playing` (null when the queue ended) | The current song changed |
| `Preload` | `preload` | 20 s before the end: load this song now |
| `SyncProgress` | `sync_source_id`, `sync_done`, `sync_total` | During `sync_library` |

## Types

All are plain records and enums. IDs and times are 64-bit signed integers (milliseconds, UTC) because Dart has no unsigned 64-bit integer; counts are 32-bit.

| Type | Fields |
| --- | --- |
| `CoreConfig` | `data_dir`, `device_id`, `device_name`, `platform` |
| `ServerCheck` | `base_url`, `name`, `version`, `server_id`, `plain_http_on_local_network` |
| `QuickConnectStart` | `code`, `expires_in_seconds` |
| `SignedIn` | `profile_id`, `source_id`, `credential_key`, `token`, `user_name` |
| `SourceToken` | `source_id`, `token` |
| `ProfileSummary` | `profile_id`, `display_name`, `credentials: [CredentialKey { source_id, key }]` |
| `ServerRow` | `source_id`, `kind`, `name`, `base_url`, `version?`, `user_name`, `song_count`, `last_sync_utc_ms?`, `signed_in` |
| `SyncResult` | `source_id`, `artists`, `albums`, `songs`, `playlists`, `removed` |
| `Song` | `id`, `source_id`, `title`, `artist`, `album_artist`, `album_id?`, `album_title?`, `disc_number?`, `track_number?`, `duration_ms`, `year?`, `container?`, `has_lyrics`, `track_gain_db?`, `album_gain_db?` (ReplayGain dB) |
| `AlbumSummary` | `id`, `title`, `artist`, `year?`, `song_count`, `duration_ms`, `date_added_utc_ms?`, `has_artwork` |
| `AlbumDetail` | `album`, `songs` |
| `ArtistSummary` / `ArtistDetail` | `id`, `name`, `album_count` / `artist`, `albums` |
| `PlaylistSummary` / `PlaylistDetail` / `PlaylistEntry` | `id`, `name`, `song_count`, `duration_ms` / `playlist`, `entries` / `position`, `song?` |
| `GenreSummary` | `name`, `song_count` |
| `TopResult` / `SearchResults` | `kind`, `song?`, `album?`, `artist?`, `playlist?` / `top?`, `songs`, `albums`, `artists`, `playlists` |
| `QueueEntry` | `uid`, `song`, `chosen_by` (User, App) |
| `QueueState` | `history`, `current?`, `up_next`, `autoplay`, `shuffle`, `repeat`, `autoplay_enabled`, `can_undo` |
| `StreamInfo` | `song_id`, `url`, `url_with_api_key`, `headers: [HttpHeader { name, value }]`, `may_transcode` |
| `ArtworkRequest` | `url`, `headers` |
| `CoreEvent` | `kind`, `queue?`, `now_playing?`, `preload?`, `sync_source_id?`, `sync_done`, `sync_total` |
| Enums | `AlbumSort`, `SongSort`, `ResultKind`, `StartedFrom` (Album, Playlist, Artist, Songs, Genre, Search, Radio, Mix, Queue, History, Other), `ChosenBy`, `RepeatMode`, `ListeningMode` (Normal, Private, Guest, LearningOff), `EndReason` (Finished, PausedAbandoned, Interrupted, Error, EndedBySystem), `PreviousResult`, `Network` (Wifi, Cellular), `StreamQuality`, `TranscodeCodec`, `ArtworkOf`, `EventKind`, `ErrorKind` |

Names follow each language: `base_url` is `baseUrl` in Swift, Kotlin and Dart.

## Calling them

**Swift**

```swift
let core = try EncoreCore(config: CoreConfig(dataDir: dir, deviceId: id, deviceName: "Sam's iPhone", platform: "ios"))
core.setListener(listener: myListener)          // a class conforming to CoreListener
let server = try await core.checkAddress(address: "music.example.org")
let signed = try await core.signIn(baseUrl: server.baseUrl, username: user, password: pass)
keychain.save(signed.token, for: signed.credentialKey)
_ = try await core.syncLibrary()
let albums = try core.albums(sort: .recentlyAdded, offset: 0, limit: 50)
let detail = try core.album(albumId: albums[0].id)
let state = try core.play(songIds: detail.songs.map(\.id), startIndex: 0, startedFrom: .album, contextId: nil)
```

**Kotlin**

```kotlin
val core = EncoreCore(CoreConfig(dataDir, deviceId, "Pixel 9", "android"))
core.setListener(object : CoreListener { override fun onEvent(event: CoreEvent) { /* post to UI */ } })
val server = core.checkAddress("music.example.org")             // suspend
val signed = core.signIn(server.baseUrl, user, pass)            // suspend
keystore.save(signed.credentialKey, signed.token)
core.syncLibrary()
val albums = core.albums(AlbumSort.RECENTLY_ADDED, 0u, 50u)
```

**Dart**

```dart
await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
final core = Encore.create(config: CoreConfig(dataDir: dir, deviceId: id, deviceName: 'Desktop', platform: 'linux'));
core.events().listen((e) { /* ... */ });
final server = await core.checkAddress(address: 'music.example.org');
final signed = await core.signIn(baseUrl: server.baseUrl, username: user, password: pass);
await secretStore.write(signed.credentialKey, signed.token);
await core.syncLibrary();
final albums = core.albums(sort: AlbumSort.recentlyAdded, offset: 0, limit: 50);
```

The Phase 1 calls are unchanged: `hello(name)`, `core_version()` and `app_name()` are synchronous and never fail.

## Token constants

Generated from `design/tokens.json` by `tools/encore-tokens` (`make tokens` in `core/`). Never type a colour, size or duration; read it from here. Keys keep their tokens.json names. Groups become nested types, and `$source` and `$status` notes become doc comments. The core reads the same file, so the numbers it uses (preload, checkpoints, ports, search limits, stream bitrates) are the ones the platforms see.

| Token | Swift | Kotlin | Dart |
| --- | --- | --- | --- |
| `breakpoints.railWidth` | `EncoreTokens.Breakpoints.railWidth` | `EncoreTokens.Breakpoints.railWidth` | `EncoreTokens.breakpoints.railWidth` |
| `motion.timing.reduceMotionCrossfadeMs` | `EncoreTokens.Motion.Timing.reduceMotionCrossfadeMs` | `EncoreTokens.Motion.Timing.reduceMotionCrossfadeMs` | `EncoreTokens.motion.timing.reduceMotionCrossfadeMs` |
| `smartPicks.explorationPercent.default` | ``EncoreTokens.SmartPicks.ExplorationPercent.`default` `` | `EncoreTokens.SmartPicks.ExplorationPercent.default` | `EncoreTokens.smartPicks.explorationPercent.defaultValue` |

Types: whole numbers are `Int` (Swift and Kotlin; `Long` in Kotlin above 32 bits) or `int`; decimals are `Double` or `double`; text is `String`; lists are arrays or `List`. A list of objects (for example `albumGradient.exampleAlbums`) becomes a list of small records named `<Key>Item`. A Dart name that is a reserved word gets `Value` at the end. Swift and Kotlin escape reserved member names with backticks. In Swift, a group named `Type`, `Self`, `Protocol` or `Any` gets `Group` added, because `Foo.Type` always means the metatype, so `recap.type` is `EncoreTokens.Recap.TypeGroup` in Swift and `EncoreTokens.Recap.Type` in Kotlin. A group may not share a name with a built-in type the constants use (`String`, `Int`, `Double`, `Bool`, `Long`, `Boolean`, `List`, `Sendable`, `Equatable`); the generator refuses it.

**Colours** (`color`, approved 6 November 2026). `color.accents.<name>.light` and `.dark` hold the 33 Material 3 colour roles (`primary`, `onPrimary`, `primaryContainer` and the rest, through `scrim`) as `#RRGGBB` strings, for the eight profile accents. `color.defaultAccent` is `iris` on Android and desktop. Android and the Windows and Linux app use them when no album colour applies. Apple devices use Apple's system colours and do not read these. `tools/palette/palette.py` generates the schemes, and CI checks that `tokens.json` matches it.

| | Swift | Kotlin | Dart |
| --- | --- | --- | --- |
| Iris, light, primary | `EncoreTokens.Color.Accents.Iris.Light.primary` | `EncoreTokens.Color.Accents.Iris.Light.primary` | `EncoreTokens.color.accents.iris.light.primary` |

**Type** (`typography.scale`, approved 6 November 2026, measured from the renders). Pick the scale by window width, never by device: `compact` under 600 and `wide` from 600. Each role has `size`, `weight` and `lineHeight` (sp on Android, logical px in Flutter); `navLabel` and `buttonLabel` add `selectedWeight`. The family is Inter. Apple devices use Dynamic Type and do not read this scale.

| | Kotlin | Dart |
| --- | --- | --- |
| Compact page title (28, weight 500) | `EncoreTokens.Typography.Scale.Compact.PageTitle.size` | `EncoreTokens.typography.scale.compact.pageTitle.size` |

## Proven so far

- Rust tests for every layer (`cargo test` in `core/`). The Jellyfin adapter, the library sync and the whole `EncoreCore` API run end to end on a recording from a real Jellyfin 12.0.0 server, anonymized (`tools/fixtures/jellyfin/`). That covers Quick Connect, sync, browse, search, playing an album, the preload event, the play log and private listening. Error paths a recording cannot hold use stand-in replies written from the server's OpenAPI document.
- CI (`.github/workflows/core.yml`) builds every target on every pull request and every push to `main`. These jobs prove the bindings and the builds, not the platforms. The Apple, Android, and Windows and Linux tracks prove them on their own devices.

| Job | Runner | What it proves |
| --- | --- | --- |
| `linux` | Ubuntu | `make check-core`: fmt, clippy and tests for `core/`, `tools/` and `server/`, plus proof that the committed bindings and token constants match what the generators produce. The search budget on 50,000 songs, with the timings in the log. Then it calls the core from Swift (Swift 6 mode), from Kotlin on a JVM with JNA and coroutines, and from Dart: constructing `EncoreCore`, an async call returning a typed error, and a call needing a profile. |
| `apple` | macOS | Builds `libencore_core.a` for iOS (arm64), the iOS Simulator (arm64, x86_64) and macOS (arm64, x86_64) with the plan's minimums (iOS 17, macOS 14). Links the Swift bindings for all five in Swift 6 mode and runs them on macOS and in an iOS Simulator. The iOS device build is linked but not run, because CI has no iPhone. Packages an XCFramework of the three platforms. |
| `android` | Ubuntu | Builds `libencore_core.so` for arm64-v8a, armeabi-v7a, x86_64 and x86 at API 28 (Android 9). Checks that every one is 16 KB page aligned, as Google Play will require for app updates from 1 February 2027 ([Android docs](https://developer.android.com/guide/practices/page-sizes)), and that it exports every symbol the Kotlin bindings call. Nothing runs, because that needs an emulator or a phone. |
| `windows` | Windows | Core and server tests, then calls the core from Dart through `encore_dart.dll`. |

A run started by hand (Actions, `core`, "Run workflow") also uploads the XCFramework (`encore-core-apple`) and the Android `jniLibs` folder (`encore-core-android`) for 7 days. A platform track can try them before its own build is set up. `apple` and `android` are skipped when a change touches only `server/`.

Not proven by the core track: signing in on a device, gapless playback, the lock screen and media keys. The platform tracks prove those with their players.
