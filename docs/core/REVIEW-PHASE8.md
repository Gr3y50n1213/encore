# Phase 8 review: core track

Prompt 8, 8 November 2026. Phase 8's "done when" line is *a downloaded playlist plays in airplane mode and in a car*. The core track's part covers:
- downloads: storage, transfer, and what stays wanted;
- offline rules: what plays, what's refused, how the queue behaves;
- the rolling cache's keys;
- the storage page's data.

The plan is ADR core-0017; the API is CORE-API v6.

**The owner's answers (approved plan):**
- **Quality:** Original by default; High 320 and Normal 160 are offered.
- **Networks:** Wi-Fi only is on.
- **Storage:** no limit by default (2, 5, 10, 20 and 50 GB are offered). At the limit, songs wait and nothing is deleted.
- **Auto-remove after 30 days unplayed:** off, and it works per album or playlist.
- **Liked songs and Smart downloads:** off. Smart downloads use 2 GB, nightly, on Wi-Fi while charging.
- **The rolling cache:** 1 GB. Songs cached whole play offline.
- **Shared files:** profiles on one device share files by server, item and quality.
- **Libraries:** no new ones.

**Evidence:**
- **Local:** `cargo test` in `core/` (core, store, sources) and clippy.
- **CI run 42530396349,** every job green:
  - **Linux:** `make check-core`, then smoke tests in Swift, Kotlin and Dart. Each one downloads a song from the Jellyfin replay, plays it from its file and removes it.
  - **Windows:** core and server tests, and the Dart smoke test.
  - **Apple:** iOS, the Simulator and macOS.
  - **Android:** four ABIs, 16 KB aligned.
  - **The server image's end-to-end test.**
- **The replay** (`tools/ci/replay_jellyfin.py`) now serves 30 s of generated silent WAV for `/Audio/<id>/stream` and `/universal`, with byte ranges, so CI moves real bytes. No audio is committed.

## The "done when" line

| Item | Result | Evidence |
| --- | --- | --- |
| A downloaded collection plays with no network | **Passes (core)** | `an_album_downloads_plays_from_its_files_and_shows_its_cover_offline`: the album downloads; then, offline, `play` starts it, `stream_for` gives `file://` for every song, and `artwork` gives the saved cover. The Android emulators repeat this through the app with the network off (OfflineAndCarTest). |
| In a car | **Platform track** | The core gives the car the same lists and `play` calls as the app (the Android review covers Android Auto). |

## The phase's items

| Item | Result | Evidence |
| --- | --- | --- |
| Storage | **Passes** | Profile migration V8: `download_collections`, `download_members`, `download_songs`, `download_covers` and `download_signals`. Registry R2: `download_files` counts holders, never profiles. Store tests: `collections_members_and_songs_keep_their_state`, `covers_and_signals_are_kept`, and the registry's `download_file_row`. |
| Transfer | **Passes** | `Transport::download` streams to a `.part` file with a per-chunk timeout and a cancel flag. It resumes with `Range` where the server allows (206 appends; 200 starts again). `a_cut_transfer_resumes_where_it_stopped_and_a_stopped_run_carries_on`. |
| Failures | **Passes** | One try per song per run, given up after 3 (tokens); Retry puts failed songs back. `a_failing_song_is_tried_three_times_then_marked_and_can_be_retried`. |
| Waiting, never deleting | **Passes** | Wi-Fi only, the storage limit and low space (500 MB kept free) each stop the run with their reason, and nothing is deleted. `downloads_wait_for_wifi_the_limit_and_space_and_never_delete`. |
| Files shared between profiles | **Passes** | A file goes only when the last holder lets go. `two_profiles_share_one_file_and_it_goes_only_when_both_let_go`. Removing a profile or server runs the clean-up. |
| What stays wanted | **Passes** | Albums, playlists and mixes follow their members, Liked songs follow likes, and a song inside a downloaded album stays with it ("It's saved as part of …"). `liked_songs_follow_likes_and_a_song_inside_an_album_stays_with_it`. |
| Offline rules | **Passes** | Offline, only saved songs and songs cached whole play. `play` keeps the playable ones and refuses with "Not downloaded" when the chosen song can't play. The queue passes over the rest both ways, and Previous restarts when nothing behind can play. Autoplay and mixes use only playable songs. Tests: `offline_only_saved_and_cached_songs_play_and_the_queue_passes_over_the_rest`, `offline_the_queue_passes_over_songs_that_cannot_play`, `offline_with_nothing_playable_behind_previous_restarts`. |
| Guest and Search | **Passes** | Guest downloads nothing; `search_downloaded` finds only what is saved. `guest_downloads_nothing_and_search_finds_what_is_downloaded`. |
| Remove all and auto-remove | **Passes** | `remove_all_clears_every_file_and_unplayed_ones_stay_until_their_days_pass`. |
| Keys | **Passes** | A file key is a hash of server, item and quality, with no token. The cache key is `server/item/quality`. `keys_carry_their_quality_and_never_a_token`, `estimates_follow_the_quality`. |
| Signals | **Passes** | A download is the +1.5 "added" signal from plan section 21; automatic downloads don't count. The fingerprint includes the signals, so mixes refresh. |
| Events | **Passes** | `DownloadsChanged`, and `DownloadProgress` at most every 500 ms (tokens). |
| Bindings | **Passes** | Swift and Kotlin match CI's generated files byte for byte. Dart glue came from the regenerate run. All three smoke tests call the new API on every CI platform. |

## Found and fixed during this phase

1. **Lock order.** Two paths could take the connectivity lock and then the store lock, the reverse of every other path. The order is now playback, then store, then connectivity or registry, and transfers are read before the store.
2. **Candidate order.** The next song to download mixed positions across collections. It's now ordered by when its collection was added, then its position.
3. **Add to playlist and Undo** didn't update downloaded playlists; both reconcile now.
4. **Smoke tests:**
   - Swift can't `try` inside `precondition`.
   - Kotlin needed the `Network` import.
   - Dart's 64-bit lists hold `BigInt`.
   - CI now prints the Dart output when that step fails.

## Not claimed

- **Real networks:** downloads over a real phone network, the timing of a large library, and a weekend of Smart downloads. These are for the owner's devices.
- **Desktop and Apple apps:** they get downloads from the core when they're built; their screens and schedulers are their own tracks.
