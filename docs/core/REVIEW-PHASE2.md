# Phase 2 review: core and server track

Prompt 5, 6 November 2026. Phase 2's "done when" line (docs/ROADMAP.md) is *you can sign in, browse your whole library offline-fast, and listen to an album gaplessly*. The platform tracks draw the screens and play the audio; this checks the core's part, and the section 24 groups of `PLAN-CORE-SERVER.md` that this phase touched.

Rules: an item passes only with evidence from a test, a benchmark or a CI run. Nothing is passed from reading the code. Every CI reference is run **42296856834** (`core` workflow, commit "ci: start the replay before booting the iOS Simulator"), all five jobs green.

## The "done when" line, core part

| Item | Result | Evidence |
| --- | --- | --- |
| Sign in | **Passes**, in CI, not on a device | Quick Connect and password sign-in on the real Jellyfin 12.0.0 recording: `quick_connect_signs_in_after_the_code_is_approved`, `sign_in_sends_the_password_once_and_returns_a_session`, `quick_connect_creates_a_profile_and_hands_the_token_to_the_app`. Over real HTTP against the replay (`tools/ci/replay_jellyfin.py`), from Rust (`library_flow_over_http_against_the_replay_server`, Linux and Windows), Swift (Linux, macOS, iOS Simulator on an iPhone 17 Pro), Kotlin (JVM) and Dart (Linux, Windows): each printed or passed "library flow OK". |
| Browse the whole library offline | **Passes** | After one sync, every browse call reads the mirror with no network: `sync_then_browse_search_and_list_servers_offline`, `a_recorded_library_lands_in_the_mirror_and_can_be_browsed_offline`, and the store's `browse.rs` tests. |
| Offline-fast | **Passes on a CI runner**; not measured on a phone | 50,000 songs, release build, Linux runner. Slowest browse page median **21.0 ms** (a genre's last page), every sort's first and last page under 3 ms. Slowest search median **13.3 ms** (a typo). Budget 50 ms (`search_stays_under_budget_on_50000_songs`, `browse_pages_stay_under_budget_on_50000_songs`). There is no browse budget in tokens.json, so browse is held to `search.budgetMs`: that ceiling is my choice. A runner is not a phone: re-measure in Phase 6. |
| Listen to an album gaplessly | **Core part passes**; gapless playback itself **could not be checked** | The album plays in disc and track order (`an_album_plays_in_track_order`). The core asks the player to preload the next song 20 s before the end, once (`play_an_album_report_it_and_preload_the_next_song`). Gapless audio is the platform players' job and needs a device. |

## The plan's verification list (Phase 2 plan)

| Item | Result | Evidence |
| --- | --- | --- |
| `make check-core` in CI | **Passes** | Linux job: fmt, clippy, tests for core, server and tools, and generated bindings and tokens up to date. |
| Adapter tests pass without a server, from recorded and anonymized fixtures, with the privacy check | **Passes** | 7 `recorded_tests.rs` tests on `tools/fixtures/jellyfin/recorded.json` (59 exchanges, 150 songs), including `the_recording_carries_no_paths_previews_tokens_or_real_addresses`. The anonymizer is tested too (`replies_lose_names_ids_paths_tokens_and_previews`). |
| Section 5 fields, crash recovery, Private and Guest write nothing | **Passes** | `a_play_stores_every_section_5_field`, `every_field_round_trips`, `a_crash_leaves_history_that_is_closed_at_the_next_launch`, `checkpoints_are_written_every_ten_seconds`, `private_guest_and_learning_off_write_nothing_at_the_database_level` (rows counted in SQLite), `private_listening_records_nothing` (through the API). |
| Search timed on 50,000 songs and compared with 50 ms | **Passes** | 13.3 ms worst median, printed in the Linux job's log. |
| Four jobs green; Swift, Kotlin and Dart call the new API | **Passes** | Linux, Apple, Android and Windows green. Each language runs the library flow (above), typed errors, and in Dart the events stream. |
| Smoke tests "open a profile, run a queue, write a play event and search" | **Passes** (fixed in this review) | They didn't do this until the review: they now run the library flow over HTTP. |

## Section 24 groups this phase touched

### Smart picks

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Private listening, Guest and Learn from my listening off write zero events, checked at the database level | **Passes** | `private_guest_and_learning_off_write_nothing_at_the_database_level`; `switching_to_private_mid_song_closes_the_play_and_stops_recording`. |
| Interruptions, errors, seeks and plays under 2 s write no negative signal | **Could not be checked: not built yet** | Signals arrive in Phase 5. The play log already records what they need: the end reason (finished, skipped, paused and abandoned, interrupted, error, ended by the system) and the time listened (`a_play_stores_every_section_5_field`). |
| The other smart-picks items | Phase 5 | Nothing built yet. |

### Year recap

All Phase 11. What Phase 2 contributes is the data that can't be rebuilt later:
- each play's UTC start plus the UTC offset at that moment (`local_time_uses_the_offset_at_the_start`);
- the time listened and the end reason;
- a song key stable across albums (`fallback_key_is_frozen`, `composed_and_decomposed_accents_give_one_key`, `durations_within_the_tolerance_are_the_same_song`).

### Data, sync and security

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Versioned migrations tested forward (and backward where promised) | **Passes** | `a_new_database_reaches_the_latest_schema`, `migrating_twice_changes_nothing`, `a_version_1_database_upgrades_with_its_data` (migration 2, added in this review), `a_newer_schema_is_refused_rather_than_damaged`. ADR core-0003 promises forward only; a newer database is refused, not damaged. |
| A backup and restore test passes | **Fails: not built** | Phase 2 has no backup feature. ADR core-0003 makes a backup a copy of the profile folder, but no test proves a copied profile reopens. To build with the backup feature. |
| Sync conflict tests (like, playlist, Not interested) | Phase 4 | The hybrid logical clock that orders those events is built and tested (`orders_after_a_remote_event`, `never_goes_backwards_when_the_wall_clock_stalls_or_steps_back`). |
| No telemetry; a full session's network log shows only the user's servers | **Could not be checked** | Needs a network log from a device. The core has no telemetry code, but that comes from reading the code, so it is not marked passed. |
| Tokens in the keychain or keystore; nothing sensitive in logs, backups or screenshots | **Core part passes**; the keychain itself **could not be checked** | The token is handed to the app and isn't in the profile's database file or in any `Debug` output (`quick_connect_creates_a_profile_and_hands_the_token_to_the_app`). Stream URLs and headers never reach a log (`streams_use_the_quality_setting_and_never_log_the_token`). A profile reopens with the token the app gives back (`a_profile_reopens_with_the_token_from_the_keychain`). Storing it in the keychain is the platforms' job. |
| Dependency and licence scans run in CI and fail on a new unreviewed licence | **Passes** (built in this review) | Linux job step "Every dependency licence is reviewed": 263, 171 and 149 packages for core, server and tools, all on reviewed licences (`tools/ci/licences.json`). Self-test in CI. Checked locally that it fails, naming each crate, when a licence is taken off the list. |

## Found and fixed during this review

1. **Kotlin couldn't compile the error types.** A field named `message` clashed with `Throwable.message`. Errors now carry `reason` (CORE-API "1, revised").
2. **Dropping the core during a call panicked.** The runtime could be dropped from its own task. Dart hit this; Swift and Kotlin could if a screen closed mid-call. Fixed and tested (`dropping_the_core_inside_another_runtime_does_not_panic`; it failed before the fix).
3. **The Dart events stream could never end.** The core held the stream for its whole life, so cancelling waited forever. `stopEvents` / `clear_listener` now end it.
4. **The smoke tests didn't exercise the library.** They now sign in, sync, play and search over HTTP on every platform CI runs.
5. **Browsing a large library was slow.** On 50,000 songs, the last page of songs by artist took about 3.8 s on the owner's PC, songs by album 2 s, and a genre's last page 2.2 s. Migration 2 adds sort indexes and the genre query was rewritten; every page is now under 50 ms.
6. **The section 24 licence scan was missing.** It's built now.

## Unverified items added by this phase

| Item | Why it is open | Check against |
| --- | --- | --- |
| Preload 20 s before the end, checkpoint every 10 s, sync pages of 500 | Starting values in tokens.json | Gapless tests and battery use on devices |
| Browse held to 50 ms | My choice; the plan has no browse budget | The owner, and Phase 6 measurements |
| Search and browse times | Measured on a CI runner and this PC, not a phone | Phase 6, a mid-range phone |
| Android certificate checks | `EncoreTls.initialize` is exported and checked by symbol, but nothing has run on Android | Request `core-0001-android-tls-init` (open, to the Android track) |
| The iOS device build | Linked, not run: CI has no iPhone | The Apple track's devices |

## What the platform tracks can build now

See CORE-API.md, "What the platform tracks can build now". In short: the connect and sign-in screens, keychain storage, the library sync screen, all of offline browse and search, the queue, playback with preload and the play log, Private listening and Guest, artwork and raw lyrics. They can build against the Jellyfin replay with no server of their own.
