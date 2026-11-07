# Encore core API (the contract)

**Version 4** (Phase 5, 7 November 2026). Core crate version `0.1.0`.

This file is the only thing the platform tracks may rely on. Every public call, event and type of the core is listed here before a platform uses it. A change that breaks a platform gets a line under "Changes" and a request in `docs/requests/` that says what to change.

## Changes

| API version | Phase | What changed |
| --- | --- | --- |
| 0 | 1 | First version: `hello`, `core_version`, `app_name`; token constants |
| 1 | 2 | `EncoreCore`: connect and sign in to Jellyfin (password or Quick Connect), profiles, library sync, browse, search, the queue, playback reporting with the play log, streams, artwork, raw lyrics, events. Nothing from API 0 changed. Kotlin now also needs kotlinx-coroutines (async calls are suspend functions). |
| 2 | 3 | Added, nothing removed. Lyrics (`lyrics`, the word-timing-ready model); album and playlist palettes; the skip signal (`skip_next` / `skip_previous` with an origin, `next_target` / `previous_target`, `playback_interrupted`, `playback_seeked`); playlist writes and likes (`drop_preview`, `add_to`, `like`, `unlike`, `are_liked`, `liked_songs`, `remove_from_playlist`, `move_in_playlist`, `rename_playlist`, `undo`, `pending_edits`, `send_edits`); `backup_profile` / `restore_profile`; the error kind `NotAllowed`. `next()` and `previous()` stay, as the button. A platform that switches over every `ErrorKind` adds `NotAllowed`. |
| 4 | 5 | Added, nothing removed. Smart picks: autoplay fills itself (`refresh_autoplay`, `set_autoplay`), `start_radio`, `smart_shuffle`, `why_this_song`, `more_like_this` / `fewer_like_this`, Not interested (`not_interested_song`, `not_interested_artist`, `remove_not_interested`, `not_interested_list`, `clear_not_interested`), `set_adventurousness` / `adventurousness`, `reset_suggestions`, `clear_listening_history`, `home`, `regenerate_mixes`, `mix`, `pin_mix`, `hide_mix`, `not_in_this_mix`, `save_mix_as_playlist`, `suggestion_stats`; their types. Private listening ends after 2 hours of silence. Queue entries the app chose (Autoplay, Smart shuffle) have `chosen_by` App. A platform that builds `QueueState` itself is unaffected. |
| 3 | 4 | Added, nothing removed. The companion server (`connect_companion`, `disconnect_companion`, `sync_status`, `sync_now`, `companion_devices`, `revoke_device`, `delete_profile_everywhere`); profiles on one device (`switch_profile`, `unlock_profile`, `set_profile_pin`, `remove_profile_pin`, `biometrics_changed`, `rename_profile`, `reorder_profiles`, `remove_profile_from_device`, `last_active_profile`); Guest (`start_guest`, `end_guest`, `guest_session`). `ProfileSummary` gains `initials`, `avatar_url`, `locked`, `active`, `needs_sign_in`; `CoreEvent` gains `sync_status`, `remote_change`, `credential` and three kinds. `open_profile` refuses a locked profile until `unlock_profile`; a `SourceToken` with `source_id` 0 is the companion's. New `profiles` tokens. A platform that switches over every `EventKind` adds the three new kinds; one that builds `ProfileSummary` or `CoreEvent` itself adds the fields. |
| 2.1 | 3 | Added, nothing removed. Lyrics search: `SearchResults.lyrics` (`[LyricMatch]`) and `fetch_lyrics_ahead(max_songs)`; `search.limits.lyrics` in tokens. Export: `export_profile(profile_id, dest_path)` returns `ExportSummary`. A platform that builds `SearchResults` itself (tests, previews) adds the `lyrics` field. |
| 1, revised | 2 | Same day, after CI ran the bindings. Each error case carries its sentence as `reason`, no longer `message`, because the Kotlin exceptions clashed with `Throwable.message`. New calls: `clear_listener` (Swift, Kotlin) and `stopEvents` (Dart), which end the events. A platform that copied the first version renames `message` to `reason` in its error handling; nothing else changes. |

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
- **Errors** are one error type with one case per reason, each with a `message` sentence a person can act on (boards 18, 20 and 21): `InvalidAddress`, `InsecureAddress`, `Unreachable`, `CertificateUntrusted { sha256 }`, `NotJellyfin`, `UnsupportedVersion`, `WrongCredentials`, `SessionExpired`, `QuickConnectDisabled`, `QuickConnectExpired`, `QuickConnectCancelled`, `NotFound`, `Server { status, detail }` (show `detail` in a "Details" disclosure), `NoProfile`, `Storage`, `Internal` and `NotAllowed` (Guest saving, a drop on an album, a read-only playlist, an Undo too late: the `reason` says which). Each case carries the sentence as `reason`, because Kotlin exceptions already have a `message`. Swift: `EncoreError.WrongCredentials(reason:)`. Kotlin: `EncoreException.WrongCredentials`; show `e.reason` to the user, because `e.message` reads `reason=<text>`. Dart: `ApiError` with `kind` (an `ErrorKind`), `message`, and `sha256`, `status` and `detail` where they apply.
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
| `profiles()` | `[ProfileSummary]` | The switcher's list (board 14): the open profile first, then the user's order. Each has `credentials` (where each token is kept; `source_id` 0 is the companion server's), `display_name` (the name set here, or the media-server user name), `initials` (the avatar fallback), `avatar_url` (the media server's user image, served without a token; load it like artwork; null shows initials), `locked` (a PIN is set), `active`, and `needs_sign_in` (the media server ended the sign-in: show "Sign in again"; the data stays). |
| `open_profile(profile_id, tokens)` | – | `tokens`: `[SourceToken]` read from the keychain, the companion's included (`source_id` 0: sync starts by itself). A source without a token is browsable offline and reports `signed_in = false`. The profile's last queue loads, paused. A locked profile is `NotAllowed` until `unlock_profile`. Open `last_active_profile()` at launch. |
| `switch_profile(profile_id, tokens, keep_playing)` | `QueueState` | Async. Board 14, item 2: this profile's unsent changes go first (3 s at most), its queue is kept, and the other opens with its own queue, paused. `keep_playing` true ("Keep playing"): the current queue moves over instead; keep the audio going and call `playback_started` again, so the rest of the song counts for the new profile. Then cross-fade, reset to Home. |
| `unlock_profile(profile_id, pin?)` | – | Before switching to or opening a locked profile; each unlock is used by the next opening. `pin` null: your biometric check (Face ID, Touch ID, Windows Hello) passed. Wrong PINs: `NotAllowed` with "Wrong PIN"; after 5, "Too many wrong tries. Try again in 30 s." (then longer); even the right PIN waits. After `biometrics_changed()`, the PIN is needed once. |
| `set_profile_pin(profile_id, pin)` / `remove_profile_pin(profile_id)` | – | The open profile only (Manage profiles). 4 to 6 digits (`profiles` tokens). A screen lock, not encryption. |
| `biometrics_changed()` | – | Call when the device's enrolled biometrics change: every PIN is needed once. |
| `rename_profile(profile_id, name?)` / `reorder_profiles(profile_ids)` | – | Manage profiles. Null name: back to the media-server name; at most 40 characters. |
| `remove_profile_from_device(profile_id)` | – | Board 14, item 3, after its confirm sheet: deletes this device's copy only (its companion data stays). Then delete its keychain entries. |
| `last_active_profile()` | `profile_id?` | The profile to open at launch. |
| `start_guest()` / `end_guest()` | `QueueState` | Board 14, item 5, on the open profile: nothing is recorded, recommended or synced; likes and playlists are refused (`NotAllowed`); the queue starts empty and the profile's own queue comes back at the end. Show "Guest, nothing is saved". |
| `guest_session()` | `GuestSession?` | `started_utc_ms`, `ends_if_idle_utc_ms`. Guest ends by itself after `profiles.guestIdleHours` without playing (null then, and the queue is back), and when a profile opens. |
| `close_profile()` | – | Ends the current play as "ended by the system". |
| `current_profile()` | `profile_id?` | |
| `backup_profile(profile_id, dest_path)` | – | A consistent copy of the profile's database, safe while playing: library, history, settings; never tokens. Blocking: call off the main thread. |
| `export_profile(profile_id, dest_path)` | `ExportSummary` | Board 19, item 5: one JSON file with the play log, likes, playlists (songs in order), settings and recent searches, in the open format of [EXPORT-FORMAT.md](EXPORT-FORMAT.md); never tokens. Reads through its own connection, so playback carries on. `ExportSummary`: `plays`, `likes`, `playlists`, `playlist_songs`, `bytes` for the finished row. `NotAllowed` in Guest mode ("Nothing is saved in Guest mode"). Blocking: call off the main thread and show the progress row until it returns, then hand the file to the share sheet. |
| `restore_profile(profile_id, from_path)` | – | Checks the file first (intact, an Encore profile, not from a newer app); if it fails nothing changes. The replaced database is kept as `library.sqlite.before-restore`. Closes the profile if it was open; open it again with its tokens. The core also keeps a copy before every schema upgrade (`library.sqlite.before-v<N>`, the newest two). |

### Companion server and sync (plan section 5; boards 18 and 19)

The companion server is optional: without it everything works on this device and nothing syncs. Design: docs/adr/core-0009 and core-0010; the server's own API is docs/api/openapi.yaml.

| Call | Returns | Notes |
| --- | --- | --- |
| `connect_companion(url)` | `SyncStatus` | Async. The address the user typed (`encore:8097`, `https://music.example.com`): https anywhere, plain http only on the home network (`InvalidAddress` otherwise). Needs the open profile signed in to its media server (`SessionExpired` otherwise); `NotAllowed` in Guest. Sends that token once so the companion can check it. A `CredentialChanged` event follows: store the token in the keychain under the profile's `source_id` 0 key. Sync then runs by itself. |
| `disconnect_companion()` | – | Async. Stops syncing; this device is signed out there when reachable; everything stays here. `CredentialChanged` with an empty token: delete the keychain entry. |
| `sync_status()` | `SyncStatus` | The sync row: `state` (NotConnected, Idle, Syncing, Offline, SignedOut), `pending` ("Waiting to sync"), `last_synced_utc_ms`, `server_url`, `reason` (for Offline and SignedOut). Changes arrive as `SyncStatusChanged`. SignedOut: the server no longer accepts this device (signed out elsewhere, or the profile deleted there): offer "Connect again"; nothing local was lost. |
| `sync_now()` | `SyncStatus` | Async. Send and receive at once (pull to refresh). |
| `companion_devices()` | `[CompanionDevice]` | Async. The Devices list: `device_id`, `name`, `platform`, `added_utc_ms`, `last_seen_utc_ms`, `signed_out`, `this_device`. |
| `revoke_device(device_id)` | – | Async. Sign a device out everywhere (a lost phone). |
| `delete_profile_everywhere(profile_id, typed_name)` | – | Async. Board 19's delete, for the open profile, after its confirm sheet: `typed_name` must be the profile's `display_name` (`NotAllowed` otherwise). Deletes it on the companion server (every device stops syncing), then this device's copy; the media-server account and music are never touched. If the companion can't be reached nothing is deleted (`Unreachable`): say so and offer to try again. Then delete every keychain entry of the profile. |

**What syncs.** Plays (from start, and again once ended), lyric-line taps, likes and playlist edits. Likes merge by the latest change, Jellyfin included; playlists are re-read from the media server. Another device's like arrives in about a tenth of a second while both are online (`RemoteChange`): refresh hearts, Liked Songs, the playlists named, and the history. When `library_sync_suggested` is set, call `sync_library` when convenient.

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
| `search(query)` | `SearchResults` | As the user types. Matches any part of a name, ignores accents and case, and tolerates typos. Sections: `top` (exactly one of its fields is set, as `kind` says), then songs, albums, artists and playlists (limits in `search.limits`). From three characters, `lyrics` adds songs found by a line of their lyrics (not already in `songs`): each `LyricMatch` has the `song`, the `line` that matched, its `line_index` and `start_ms`. Show them in Songs after the title matches with the line under the title; a tap opens the lyrics view at that line. Lyrics match by whole words in order, the last one possibly unfinished ("paper lant" finds "paper lanterns"), ignoring accents and case; no typo tolerance and no mid-word matches. Only kept lyrics are searched: see `fetch_lyrics_ahead`. |
| `fetch_lyrics_ahead(max_songs)` | `u32` | Async. Fetches and keeps lyrics for up to `max_songs` songs that have them and aren't kept yet, so search finds songs never opened. Call after a library sync (on Wi-Fi, in the background) with a batch such as 25, again while it returns more than 0. Returns how many songs it tried; a song that fails is skipped for the session. Throws `Unreachable` only if the first song couldn't be fetched. |
| `add_recent_search(query)` | – | The return key. Saved per profile, newest first, at most `search.recentSearchesMax`. |
| `recent_searches()` / `remove_recent_search(query)` / `clear_recent_searches()` | | |

### Queue (boards 11 and 42, synchronous)

Every change returns the new `QueueState` and also sends `QueueChanged` (and `NowPlaying` when the current song changed).

| Call | Notes |
| --- | --- |
| `queue()` | The current state. |
| `play(song_ids, start_index, started_from, context_id)` | Play a list (an album, a playlist, search results) from `start_index`. `context_id` is the album's or playlist's ID, kept in the play log. |
| `play_next(song_ids)` / `add_to_up_next(song_ids)` | Top or end of Up Next. |
| `next()` | The button: the same as `skip_next(Button)`. |
| `skip_next(origin)` | From anywhere: `SkipOrigin` Button, Swipe, Headset, Car, LockScreen, Keyboard. Every origin runs the same code and teaches the same lesson (board 42, rule 15); the origin is stored on the play it ends. At the end of the queue the current song stays. |
| `skip_previous(origin, position_ms)` → `PreviousResult` | A swipe always goes back (`Moved`); every other origin restarts the song after 3 s, as the button does. With Repeat All, back from the first song wraps to the last. |
| `next_target()` / `previous_target()` → `SkipTarget` | Where a swipe would land (draw the neighbour), or `end_of_queue`: the rubber band and "End of queue" (board 42, rule 8). |
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
| `playback_interrupted(kind)` | `InterruptionKind`: HeadphonesUnplugged, OutputChanged, Call, Assistant. A skip in the next 3 s (`smartPicks.interruptionWindowSeconds`) reads as an interruption, never a skip. |
| `playback_seeked(from_ms, to_ms)` | Dragging the progress bar: counted, never a skip. Seeking back before the preload point lets `Preload` come again. |
| `set_listening_mode(mode)` | `Private`, `Guest` and `LearningOff` record nothing at all (checked at the database level); `Normal` records. Switching away from Normal closes the current play. |

**The skip signal (plan section 21).** When a play closes, the core decides how it reads and stores that with the play: finished; skipped late (past 80%), mid, early (an app-chosen song under 30 s) or a user pick; or, never a negative, interruption, error, accident (under 2 s), too soon (a negative within 2 hours of hearing it), system, paused and abandoned. A song reached by a skip and skipped again within 600 ms (`gestures.playerSwipe.coalesceMs`) was only passed over: it leaves no play at all, so load audio only after the finger settles. Phase 5 weighs these signals.

Each play event stores the song key, the source and its item ID, the title, artist and album (so history survives the song leaving the library), the duration, the UTC start and offset, the local hour and weekday, listened milliseconds, the end reason, what started it, the context ID, who chose it (user or app), the device ID, a ULID and a hybrid logical clock value. No location is ever stored.

### Streams, artwork, lyrics

| Call | Returns | Notes |
| --- | --- | --- |
| `stream_for(song_id, network)` | `StreamInfo` | Quality from the setting for that network: Wi-Fi defaults to original (gapless info intact), cellular to high (320 kbps). `SessionExpired` when the server has no token. |
| `stream_quality(network)` / `set_stream_quality(network, quality)` | `StreamQuality` | Original, High (320), Normal (160), DataSaver (96) kbps (`playback.quality`). |
| `set_playback_capabilities(direct_play_containers, transcode_codec)` | – | Once per platform: the containers the player plays as they are (default flac, mp3, m4a, aac, alac, ogg, opus and wav), and the codec for transcodes (`Aac` or `Opus`; default AAC). |
| `artwork(of, id, size_px)` | `ArtworkRequest` | The image at exactly `size_px` square. `ArtworkOf`: Album, Song (its album's art when there is one), Artist, Playlist. Load it with `headers`. The platform's image loader caches it; the core keeps no image cache (offline artwork comes with downloads in Phase 8). |
| `album_palette(album_id)` / `playlist_palette(playlist_id)` async | `ArtworkPalette?` | Board 04's colours for the moving field: `scheme` (Dark: white text; Light: dark text), `base`, four `stops` (leading first: the mini player and album page tint), `text`, `lowest_contrast` (4.5:1 or better on dark, 7:1 on light), `sampled`. Made once per cover, the same bytes on every device; offline once made. None: no cover, so use the profile accent. |
| `lyrics(song_id)` async | `SongLyrics?` | Ready to draw (board 10): see "Lyrics" below. None: "No lyrics for this song". Kept per song, so offline once fetched. Call it when the `Preload` event names the next song, so its lyrics are ready before the view opens (a first read of a long word-timed song took 1.8 ms in CI and up to 9 ms on a busy desktop; a kept copy under 2 ms). |
| `lyrics_raw(song_id)` async | `string?` | Jellyfin's lyrics JSON as sent, for diagnostics. Draw `lyrics` instead. |

### Lyrics (board 10)

`SongLyrics`: `sync_level` (Word, Line, Unsynced), `origin` (MediaServer, EmbeddedTag, Sidecar, Provider, Generated), `format`, `lines`, `singers`, `gaps`, `instrumental`, `language?`.

- **Lines** come in the order they are sung. Synced lines always have `start_ms` and `end_ms`: the core fills the ends (the next line, the last word, never past the song).
- **Words.** A line with `words` fills word by word across each word's `start_ms` to `end_ms`; a line without words lights up whole. A line's text is its words' texts with a space after each word whose `space_after` is set, and syllables of one word have none. Render each word as its own run; never index into the string.
- **Singers.** `side` Opposite is a second singer, aligned to the other side. `singer` indexes `singers` (each with a `name` and a `kind`: Person, Group, Other).
- **Background vocals.** `background`: smaller and dimmer, shown under the line while sung, with their own words.
- **Gaps.** `gaps` are silences of `lyrics.instrumentalGapSeconds` (5 s) or more, for the breathing dots; `after_line` null is before the first line.
- **Instrumental.** Show the dots and the title only.
- `translation` and `pronunciation` when the source has them (TTML). `confidence` is 1 for anything from a file or a server.
- Today Jellyfin's lyrics are line-synced (the owner's library has no word timing). Word-timed TTML and enhanced LRC already come through with words, and nothing in the UI may assume where word timing comes from.

### Playlists and likes (section 26, board 43, synchronous)

Every edit is **one call** with the whole list in the order shown, one event and one undo entry, however many songs. It works offline: the library shows the change at once, and the core sends it when the server can be reached, in order, even after the app was closed.

| Call | Returns | Notes |
| --- | --- | --- |
| `drop_preview(target, song_ids)` | `DropPreview` | Before the drop: `count`, `already_there` (by song, so a song on two albums counts once), `needs_confirm` (over `gestures.dragAndDrop.confirmOverSongs`, 100), `suggested_name` (for a new playlist: the album's name, or the first artist's), or a `refusal` with `refusal_reason` to show (GuestCannotSave, AlbumsTakeNoSongs, PlaylistReadOnly, NothingToAdd). |
| `add_to(target, song_ids, position, allow_duplicates)` | `EditResult` | `DropTarget { kind, id }`: Playlist (`id`; at `position`, or the end), NewPlaylist, LikedSongs, UpNext (at `position`, the line), PlayNext. Duplicates are skipped unless `allow_duplicates` ("Add anyway"); the toast reads "Added {added} · {already_there} already there". `edit_id` null: nothing changed. Refusals throw `NotAllowed`. |
| `like(song_ids)` / `unlike(song_ids)` | `EditResult` | Liked Songs, by song. A like is also a favourite on the song's server; unlike clears every copy. Favourites set elsewhere arrive as likes with the next sync. |
| `are_liked(song_ids)` | `[bool]` | The heart, in order. |
| `liked_songs(offset, limit)` | `[Song]` | Newest like first; each song once. |
| `remove_from_playlist(playlist_id, positions)` | `EditResult` | A row swipe's Remove. |
| `move_in_playlist(playlist_id, from_position, to_position)` | `EditResult` | Reordering is always a move. |
| `rename_playlist(playlist_id, name)` | `EditResult` | The name field a new playlist opens with; an empty name keeps the old one. |
| `undo(edit_id)` | – | Until `undo_until_utc_ms` (`gestures.dragAndDrop.shortcutUndoMs`, 10 s; the toast may close sooner). Too late, or twice: `NotAllowed`. |
| `pending_edits()` | `u32` | Edits not yet accepted by their server. |
| `send_edits()` async | `SentEdits` | Send now, when the network returns. Also done after every edit and before every sync. `refused`: the server said no, and the next sync brings back its version. |

Albums refuse drops: their songs come from the server's tags and folders. Guest can queue but not save. Copy versus move is the platform's choice: a move is `add_to`, then `remove_from_playlist`.

### Smart picks (plan section 21; boards 15, 23, 37, 38)

One engine chooses every song the app chooses, so a skip teaches every surface. Version 1 learns from how the user listens: what they finish and skip, what they play together, when they play it, likes and playlist adds. Audio features come in Phase 9. Design: docs/adr/core-0012 to 0014.

| Call | Returns | Notes |
| --- | --- | --- |
| (automatic) | | When a song starts and fewer than 5 are upcoming, the core fills the queue's Autoplay section with 10 songs in the background and sends `QueueChanged`. Those entries have `chosen_by` App: show them under "Autoplay". Songs the user queued are never touched. |
| `refresh_autoplay()` | `QueueState` | Fill Autoplay now (after Not interested, say). |
| `set_autoplay(enabled)` | `QueueState` | The Queue's Autoplay switch (board 11). Off: the queue ends with its last song. |
| `start_radio(seed, id)` | `QueueState` | `RadioSeed` Song, Album, Artist or Playlist with its ID. It plays the seed, then Autoplay keeps that seed for the session; playing anything by hand ends the radio. |
| `smart_shuffle(song_ids, started_from, context_id)` | `QueueState` | The list shuffled with one suggestion after every 4 songs. Suggestions have `chosen_by` App: show the spark mark, and a swipe removes one (`remove_from_queue`). |
| `why_this_song(song_id)` | `WhyThisSong` | Board 38: `reasons` (one or two `PickReason`s) and `sounds_like` (up to two songs that open on tap). Word each reason from its `kind` and fields; examples are in the types. |
| `more_like_this(song_id)` / `fewer_like_this(song_id)` | – | A light haptic and a one-line confirmation. `NotAllowed` in Guest and with learning off. |
| `not_interested_song(song_id, reason)` / `not_interested_artist(artist, reason)` | `NotInterestedItem` | The song or artist leaves every smart surface at once; a song still plays when picked. Show Undo for 5 s: `remove_not_interested(item)`. `NotInterestedReason`: NotMyTaste, HeardTooMuch (rests it 30 days), WrongMoment (this time of day only). |
| `not_interested_list()` / `clear_not_interested()` | `[NotInterestedItem]` / – | Settings, Not interested. |
| `set_adventurousness(level)` / `adventurousness()` | – / `Adventurousness` | Familiar 5%, Balanced 10%, Adventurous 20% of picks new to the user ("About 1 in 10 picks will be new to you"); fixed at Balanced under 50 plays. Applies from the next refill. |
| `reset_suggestions()` | – | Board 38, note 5: forgets what was learned. Likes, playlists, history and Not interested stay. Synced; no device can undo it. |
| `clear_listening_history()` | `u32` | Board 38, note 6: deletes every play and lyric tap so far, here, on the companion server and on every device; returns how many plays went. |
| `home()` / `regenerate_mixes()` | `Home` | Async. `made_for_you` (a `MixShelf`: `state`, `mixes` (pinned first), `plays_so_far` of `plays_needed`, `made_utc_ms`, `from_companion`), `rediscover`, `recently_played`, `deep_cuts`, `recently_added`, `most_played`. The set is the companion server's when the profile has one that answers, else built on this device. A new set comes at the first request after 06:00, after 25 new plays or on Regenerate, never while one of its mixes plays. A build can take up to 20 s on a big library: show yesterday's tiles meanwhile. |
| `mix(mix_id)` | `MixPage` | Board 15: the summary and its songs, each with one reason (`MixSong.reason`) and `new_to_you`. Play it with `play(ids, i, StartedFrom::Mix, Some(mix_id))`. |
| `pin_mix(mix_id, pinned)` / `hide_mix(mix_id)` | – | Up to 10 pinned (`NotAllowed` past that). Hide takes it out of today's set. |
| `not_in_this_mix(mix_id, song_id)` | – | Board 15, item 5: removes it, and the mix's mood learns it doesn't fit. |
| `save_mix_as_playlist(mix_id)` | `EditResult` | A normal playlist named after the mix, written to the media server. |
| `suggestion_stats()` | `SuggestionStats` | How suggestions are doing (board 38, note 7), the last 4 weeks per shelf. Numbers are per 1,000 (show 123 as 12.3%). Local only. |

**Mix states** (`MixState`): Fresh; OutOfDate (48 h without a good build: show the age and Refresh, never an empty tile); NotEnoughPlays ("Encore is still learning", with 12 of 50); LearningOff (Recently added and Most played instead); Private (Private listening or Guest). Under 50 plays the shelf may hold a Starter mix or up to three family mixes. Every state has a text label.

**What doesn't count** (unchanged from Phase 3): interruptions, errors, seeks, accidents under 2 s, too soon, ended by the system. Private listening, Guest and Learn-from-my-listening-off store nothing.

## Events

`set_listener(listener)` in Swift and Kotlin (implement `CoreListener.on_event`), `events()` in Dart (a `Stream<CoreEvent>`). One listener at a time: setting a new one replaces the old. `clear_listener()` (Swift, Kotlin) and `stopEvents()` (Dart) stop events. In Dart, `stopEvents()` is what ends the stream: cancelling a subscription alone does not, because the core still holds the stream. One `CoreEvent` record; only the fields its `kind` names are set.

| `kind` | Set fields | When |
| --- | --- | --- |
| `QueueChanged` | `queue` | After every queue change |
| `NowPlaying` | `now_playing` (null when the queue ended) | The current song changed |
| `Preload` | `preload` | 20 s before the end: load this song now |
| `SyncProgress` | `sync_source_id`, `sync_done`, `sync_total` | During `sync_library` |
| `SyncStatusChanged` | `sync_status` | The companion sync's state or pending count changed |
| `RemoteChange` | `remote_change`: `liked_song_ids`, `unliked_song_ids`, `playlist_ids`, `history`, `library_sync_suggested` | Another device's changes arrived |
| `CredentialChanged` | `credential` (`source_id` 0, `token`; empty means delete) | The companion token rotated: store it before anything else, the old one no longer works |

## Types

All are plain records and enums. IDs and times are 64-bit signed integers (milliseconds, UTC) because Dart has no unsigned 64-bit integer; counts are 32-bit.

| Type | Fields |
| --- | --- |
| `CoreConfig` | `data_dir`, `device_id`, `device_name`, `platform` |
| `ServerCheck` | `base_url`, `name`, `version`, `server_id`, `plain_http_on_local_network` |
| `QuickConnectStart` | `code`, `expires_in_seconds` |
| `SignedIn` | `profile_id`, `source_id`, `credential_key`, `token`, `user_name` |
| `SourceToken` | `source_id`, `token` |
| `PickReason` / `ReasonKind` | `kind`, `song?`, `count?`, `of?`, `days?`, `time_of_day?`, `artist?`, `genre?` / FinishedOften, PlayedTogether, TimeOfDay, YouLikedIt, Rediscover, SameArtist, SameGenre, PlayedElsewhere, NewToYou, RecentlyAdded |
| `WhyThisSong` | `reasons`, `sounds_like` |
| `NotInterestedItem` | `song_id?`, `artist?`, `label`, `reason`, `added_utc_ms` |
| `Home` / `MixShelf` / `MixSummary` | see Smart picks / `state`, `mixes`, `plays_so_far`, `plays_needed`, `made_utc_ms?`, `from_companion` / `mix_id`, `kind`, `title`, `subtitle`, `cover_song_ids` (the 2×2 mosaic), `song_count`, `pinned`, `made_utc_ms` |
| `MixPage` / `MixSong` | `summary`, `songs` / `song`, `reason?`, `new_to_you` |
| `SuggestionStats` / `ShelfStat` | `since_utc_ms`, `shelves` / `shelf`, `plays`, `early_skips_per_1000`, `finished_per_1000`, `artists_per_20_x10`, `repeats_per_1000` |
| `ProfileSummary` | `profile_id`, `display_name`, `credentials: [CredentialKey { source_id, key }]`, `initials`, `avatar_url?`, `locked`, `active`, `needs_sign_in` |
| `GuestSession` | `started_utc_ms`, `ends_if_idle_utc_ms` |
| `SyncStatus` / `SyncState` | `state`, `pending`, `last_synced_utc_ms?`, `server_url?`, `reason?` / NotConnected, Idle, Syncing, Offline, SignedOut |
| `RemoteChange` | `liked_song_ids`, `unliked_song_ids`, `playlist_ids`, `history`, `library_sync_suggested` |
| `CompanionDevice` | `device_id`, `name`, `platform`, `added_utc_ms`, `last_seen_utc_ms`, `signed_out`, `this_device` |
| `ServerRow` | `source_id`, `kind`, `name`, `base_url`, `version?`, `user_name`, `song_count`, `last_sync_utc_ms?`, `signed_in` |
| `SyncResult` | `source_id`, `artists`, `albums`, `songs`, `playlists`, `removed` |
| `Song` | `id`, `source_id`, `title`, `artist`, `album_artist`, `album_id?`, `album_title?`, `disc_number?`, `track_number?`, `duration_ms`, `year?`, `container?`, `has_lyrics`, `track_gain_db?`, `album_gain_db?` (ReplayGain dB) |
| `AlbumSummary` | `id`, `title`, `artist`, `year?`, `song_count`, `duration_ms`, `date_added_utc_ms?`, `has_artwork` |
| `AlbumDetail` | `album`, `songs` |
| `ArtistSummary` / `ArtistDetail` | `id`, `name`, `album_count` / `artist`, `albums` |
| `PlaylistSummary` / `PlaylistDetail` / `PlaylistEntry` | `id`, `name`, `song_count`, `duration_ms` / `playlist`, `entries` / `position`, `song?` |
| `GenreSummary` | `name`, `song_count` |
| `TopResult` / `SearchResults` | `kind`, `song?`, `album?`, `artist?`, `playlist?` / `top?`, `songs`, `albums`, `artists`, `playlists`, `lyrics` |
| `LyricMatch` | `song`, `line`, `line_index`, `start_ms?` |
| `ExportSummary` | `plays`, `likes`, `playlists`, `playlist_songs`, `bytes` |
| `QueueEntry` | `uid`, `song`, `chosen_by` (User, App) |
| `QueueState` | `history`, `current?`, `up_next`, `autoplay`, `shuffle`, `repeat`, `autoplay_enabled`, `can_undo` |
| `StreamInfo` | `song_id`, `url`, `url_with_api_key`, `headers: [HttpHeader { name, value }]`, `may_transcode` |
| `ArtworkRequest` | `url`, `headers` |
| `CoreEvent` | `kind`, `queue?`, `now_playing?`, `preload?`, `sync_source_id?`, `sync_done`, `sync_total`, `sync_status?`, `remote_change?`, `credential?` |
| `SkipTarget` | `song?`, `end_of_queue` |
| `SongLyrics` / `LyricLine` / `LyricWord` / `LyricBackground` / `LyricGap` / `Singer` | see "Lyrics" |
| `ArtworkPalette` / `PaletteSwatch` | `scheme`, `base`, `stops`, `text`, `lowest_contrast`, `sampled` / `colour`, `share` |
| `DropTarget` / `DropPreview` / `EditResult` / `SentEdits` | see "Playlists and likes" |
| Enums | `AlbumSort`, `SongSort`, `ResultKind`, `StartedFrom` (Album, Playlist, Artist, Songs, Genre, Search, Radio, Mix, Queue, History, Other), `ChosenBy`, `RepeatMode`, `ListeningMode` (Normal, Private, Guest, LearningOff), `EndReason` (Finished, PausedAbandoned, Interrupted, Error, EndedBySystem), `PreviousResult`, `Network` (Wifi, Cellular), `StreamQuality`, `TranscodeCodec`, `ArtworkOf`, `EventKind`, `ErrorKind`; from Phase 3: `SkipOrigin`, `InterruptionKind`, `LyricsSync`, `LyricsOrigin`, `LyricsFormat`, `LyricSide`, `SingerKind`, `PaletteScheme`, `DropTargetKind`, `DropRefusal` |

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

- Rust tests for every layer (`cargo test` in `core/`). The Jellyfin adapter, the library sync and the whole `EncoreCore` API run end to end on a recording from a real Jellyfin 12.0.0 server, anonymized (`tools/fixtures/jellyfin/`). That covers Quick Connect, sync, browse, search, playing an album, the preload event, the play log and private listening; and from Phase 3 the recording also holds 40 songs' lyrics, 38 covers as PNG (replaced in the fixture by board 04's own art), favourites, and real playlist and favourite writes that were undone in the same session. Error paths a recording cannot hold use stand-in replies written from the server's OpenAPI document.
- CI (`.github/workflows/core.yml`) builds every target on every pull request and every push to `main`. These jobs prove the bindings and the builds, not the platforms. The Apple, Android, and Windows and Linux tracks prove them on their own devices.

| Job | Runner | What it proves |
| --- | --- | --- |
| `linux` | Ubuntu | `make check-core`: fmt, clippy and tests for `core/`, `tools/` and `server/`, plus proof that the committed bindings and token constants match what the generators produce. The licence scan of every dependency. The search and browse budgets on 50,000 songs, with the timings in the log. Then it runs the library flow over real HTTP against the Jellyfin replay: first in Rust through `EncoreCore::new` (production page size and poll interval, plus a delta sync), then from Swift (Swift 6 mode), from Kotlin on a JVM with JNA and coroutines, and from Dart. The flow is check the address, sign in with Quick Connect, sync, open an album, play it, finish a song, get the next one, and search; then fetch a song's lyrics, make an album's palette and check its base is exactly `3D0000` in every language (the same bytes everywhere), and add a song to a playlist and undo it. Each language also checks typed errors, and Dart checks the events stream. |
| `apple` | macOS | Builds `libencore_core.a` for iOS (arm64), the iOS Simulator (arm64, x86_64) and macOS (arm64, x86_64) with the plan's minimums (iOS 17, macOS 14). Links the Swift bindings for all five in Swift 6 mode and runs them, library flow included, on macOS and in an iOS Simulator. The iOS device build is linked but not run, because CI has no iPhone. Runs the palette tests natively on Apple silicon (arm64), so the golden palette is checked on ARM as well as x86_64. Packages an XCFramework of the three platforms. |
| `android` | Ubuntu | Builds `libencore_core.so` for arm64-v8a, armeabi-v7a, x86_64 and x86 at API 28 (Android 9). Checks that every one is 16 KB page aligned, as Google Play will require for app updates from 1 February 2027 ([Android docs](https://developer.android.com/guide/practices/page-sizes)), and that it exports every symbol the Kotlin bindings call. Nothing runs, because that needs an emulator or a phone. |
| `windows` | Windows | Core and server tests, the Rust library flow over HTTP, then the Dart tests (library flow included) through `encore_dart.dll`. |

A run started by hand (Actions, `core`, "Run workflow") also uploads the XCFramework (`encore-core-apple`) and the Android `jniLibs` folder (`encore-core-android`) for 7 days. A platform track can try them before its own build is set up. `apple` and `android` are skipped when a change touches only `server/`.

The Jellyfin replay is `tools/ci/replay_jellyfin.py`, standard library only. It serves the recording on 127.0.0.1, so a platform track can run its own app against it with no server: `source tools/ci/replay.sh && start_replay` prints the address to type in, then approve any Quick Connect code. Its two differences from a real server are in the file's header.

Not proven by the core track: signing in on a device, gapless playback, the lock screen and media keys. The platform tracks prove those with their players.

## What the platform tracks can build now (Phase 2)

Everything below is in API version 1 and runs on a real Jellyfin 12.0.0 recording in CI.

- **Connect and sign in (boards 18 and 21).**
  - The address field with its rules: `check_address` adds the scheme and ports, gives the plain-http warning on the local network, refuses plain http elsewhere, and returns an untrusted certificate with its fingerprint for "trust this certificate".
  - Password sign-in and Quick Connect with its code and expiry.
  - Error messages are ready to show: each error carries `reason`.
- **Keychain or keystore.** `SignedIn` hands the app the token and the `credential_key` to store it under. `open_profile` takes it back at launch. The core never writes it to disk.
- **Library sync screen.** `sync_library` with `SyncProgress` events ("Indexing 3,200 of 12,480"), and the server row (`servers`) with its song count and last sync.
- **Offline browse.** These all answer straight from the mirror, synchronously and paged, under 50 ms per page on 50,000 songs:
  - albums (four sorts), songs (four sorts), artists;
  - an artist's page (`artist`, with their albums) and `artist_songs`, an album's songs in disc and track order;
  - playlists with missing songs marked;
  - genres, and recently added.
- **Search (board 07).** Top result, songs, albums, artists, playlists; accent-insensitive and typo-tolerant; recent searches.
- **Queue (boards 11 and 42).**
  - play, play next, add to Up Next, move, remove, clear, undo;
  - shuffle and repeat;
  - previous: it restarts the song after 3 s.
  - The app redraws on `QueueChanged` and `NowPlaying`.
- **Playback.**
  - `stream_for` gives a URL and header for the quality set with `set_stream_quality` (with an `ApiKey` URL for players that can't send headers).
  - The `Preload` event comes 20 s before the end, for gapless playback.
  - The player reports with `playback_started`, `playback_progress` and `playback_ended`, and `lyric_line_tapped` for lyric jumps. The core writes the play log.
- **Private listening and Guest.** `set_listening_mode`: nothing is written while it is on.
- **Artwork and lyrics.** `artwork` URLs at an exact size (and, from Phase 3, `lyrics` and palettes below).

## What the platform tracks can build now (Phase 3)

Everything below is in API version 2, tested in Rust on the owner's real Jellyfin recording, and called from Swift, Kotlin and Dart in CI.

- **The lyrics view (board 10).** `lyrics(song_id)` gives everything the view needs:
  - lines in order with filled-in end times; words with their own times for the word-by-word fill;
  - the second singer's side, background vocals, translations;
  - the gaps for the breathing dots, the instrumental flag, and the three states (word, line, unsynced).

  Tap a line: `lyric_line_tapped`. The owner's lyrics are line-synced today; word-timed lyrics need no app change when they arrive.
- **Album gradients (board 04).** `album_palette` and `playlist_palette`: the base, four stops, the text colour, the scheme (light art flips) and the contrast, the same on every device. Draw the moving field, the album and playlist page tint, and the mini player tint from it; with no cover, use the profile accent.
- **Swipes (board 42).**
  - `skip_next(Swipe)` and `skip_previous(Swipe, position)`, and the same calls for headsets, cars, the lock screen and keys;
  - `next_target` / `previous_target` to draw the neighbour or the rubber band at the end of the queue;
  - `playback_interrupted` (unplug, output switch, call, assistant) and `playback_seeked`.

  The core decides what each skip teaches, and songs passed over in quick swipes leave no trace.
- **Row swipes.** Play Next and Add to Queue (`add_to` with PlayNext or UpNext), Remove on your own playlists (`remove_from_playlist`), each with Undo (`undo`).
- **Drag and drop, and every menu twin (board 43).**
  - `drop_preview` for the tip, the count badge, "already there", the confirm over 100, the new playlist's name and the refusal reason;
  - then `add_to` with the whole list in one call: a playlist at the line, a new playlist, Liked Songs, Up Next at the line, or Play Next;
  - `move_in_playlist` to reorder, `rename_playlist` for the name field, `undo` for the toast and Ctrl/Command+Z.
- **Likes.** The heart (`are_liked`, `like`, `unlike`) and Liked Songs (`liked_songs`); likes are Jellyfin favourites, so other Jellyfin apps agree.
- **Offline edits.** All of the above work offline; `pending_edits` can drive a "Waiting to sync" note, and `send_edits` sends at once when the network returns.
- **Backup and restore.** `backup_profile` / `restore_profile` for a settings page and `export_profile` for Export (board 19).
- **Lyrics in search.** `SearchResults.lyrics` and `fetch_lyrics_ahead` after a sync (board 07: "lyrics included when the server has them").

## What the platform tracks can build now (Phase 4)

Everything below is in API version 3. It is tested in Rust, including two real cores against a real companion server and against its Docker image (amd64 and arm64), and called from Swift, Kotlin and Dart in CI.

- **The profile switcher** (board 14): the sheet from `profiles()`, switching with `switch_profile` (Keep playing or Stop and switch), PIN and biometric locks, Manage profiles, Remove from this device, Add account (the existing sign-in), and the "Sign in again" row.
- **Guest**: `start_guest`, the "Guest, nothing is saved" banner from `guest_session()`, and `end_guest`.
- **The companion server** (boards 18 and 19): Connect a companion server, the sync status row, Devices with sign-out, and Delete profile and its data.
- **Keychain handling**: the companion token (`source_id` 0) arrives and rotates through `CredentialChanged`.
- **The server**: anyone can run it next to Jellyfin with server/compose.yaml.

## What the platform tracks can build now (Phase 5)

Everything below is in API version 4. It is tested in Rust on the owner's recording, against generated listeners (docs/core/REPLAY-RESULTS.md) and end to end with the companion server, and called from Swift, Kotlin and Dart in CI.

- **Home and the Mixes tab** (boards 05, 15, 23): Made for you with every card state, Rediscover, Recently played, Deep Cuts, Recently added, Most played; mix pages with a reason per row; Pin, Hide, Regenerate, Save as playlist, Not in this mix.
- **The queue's Autoplay section** (board 11) and its switch; radio from any song, album, artist or playlist; Smart shuffle with the spark mark.
- **Why this song, More/Fewer like this, Not interested** with Undo and its Settings list (board 38).
- **Settings, Mixes and suggestions:** How adventurous, Learn from my listening (`set_listening_mode`), Reset suggestions, Clear listening history, How suggestions are doing.

Not available yet: handoff and the WebSocket (Phase 7), downloads and offline artwork (Phase 8), recaps (Phase 11), finding the companion on the network by itself (mDNS; the address is typed).
