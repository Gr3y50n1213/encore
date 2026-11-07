# Phase 3 review: core and server track

Prompt 5, 7 November 2026. Phase 3's "done when" line (docs/ROADMAP.md) is *side by side with Apple Music, the screens feel equally smooth at 120 Hz, and lyrics sync word by word*. The platform tracks draw the screens; this checks the core's part. The part is the four items in `PLAN-CORE-SERVER.md` section 3: lyrics parsers, the artwork palette, the add-to-playlist call, and the skip signal for swipes. It also covers the section 24 groups this phase touched, and the Phase 2 item left open.

Rules: an item passes only with evidence from a test, a benchmark, the recording or a CI run, never from reading the code. Unless it says otherwise, CI evidence is run **42330146848**, the last of the phase, with all four jobs green.

## The "done when" line, core part

| Item | Result | Evidence |
| --- | --- | --- |
| Lyrics sync word by word | **Core part passes**; no real word-timed lyrics exist to check against | Word-timed TTML and enhanced LRC fixtures give words with their own times, syllables joined, two singers on their sides, background vocals and translations (`ttml_word_timing_singers_background_and_translations`, `enhanced_lrc_words_offset_duets_and_background`). Jellyfin's word cues are sliced by UTF-16, accents and emoji included (`jellyfin_cues_are_utf16_ranges`). The owner's library has none: in the recording, 40 of 40 songs with lyrics came back line-synced, none with cues. Line-synced lyrics are tested on that recording, through Rust and through Swift, Kotlin and Dart over HTTP in CI. |
| Smooth at 120 Hz (the core's share: nothing slow on the UI's path) | **Passes** in CI; one margin on the owner's PC | Synchronous calls stay under 50 ms (on 50,000 songs in CI: slowest search median 10.33 ms, slowest browse page 16.25 ms; budget 50 ms). A long word-timed song (400 lines, 3,200 timed words) parses in **1.80 ms** and reads back from its kept copy in **0.58 ms** in CI (medians of 7, release); one 120 Hz frame is 8.3 ms. On the owner's PC, busy and short on memory at the time, the first parse took about 9 ms and the kept copy about 1.8 ms, so my plan's "well under one frame" holds in CI but not on a loaded desktop. The first parse is async, off the main thread, and once per song, and CORE-API tells apps to fetch lyrics at `Preload`, so it can't stutter a frame. Palettes are made once per cover, async. The 120 Hz feel itself is the platforms' to prove. |

## The phase's four items (PLAN-CORE-SERVER section 3)

| Item | Result | Evidence |
| --- | --- | --- |
| Lyrics parsers: LRC, enhanced LRC, TTML (and plain text, and Jellyfin's JSON) | **Passes** | 18 tests in `encore-lyrics`:<br>- every format on written fixtures (original text, `tools/fixtures/lyrics`);<br>- every prefix of every fixture, 2,000 noise inputs and out-of-range numbers, none panicking (`hostile_input_never_panics`);<br>- the size limits (`malformed_and_oversized_input_is_an_error`);<br>- the real recording (`the_recorded_jellyfin_lyrics_read_as_line_synced`).<br>Three bugs found by these tests and fixed: a tag-only LRC read as text, an open last word ending too early, old-Mac line breaks. |
| Artwork palette for gradients | **Passes** | Board 04's four covers (the project's own art) give the board's schemes, and every colour the board used has one of ours within 0.038 OKLab, against a bound of 0.05 (`the_board_covers_give_the_boards_palettes`). The same bytes every run, locked by a golden value (`the_same_cover_gives_the_same_bytes`) and checked on x86_64 (Linux, Windows) and arm64 (the macOS job on Apple silicon). Awkward covers: solid, one pixel, transparent, wide, broken files. Over HTTP, every smoke test checks the base is exactly `3D0000` in Swift, Kotlin and Dart. On the owner's 38 real covers (kept on the owner's PC only): 33 dark, 5 light, every one readable (`real_covers_keep_text_readable`, by hand). |
| The add-to-playlist call (one call, one sync event, one undo entry) | **Passes** | 500 songs give 1 event, 1 undo entry and 1 outbox entry, and the server ends identical (`five_hundred_songs_are_one_event_one_undo_and_one_outbox_entry`). 13 edit tests against a model Jellyfin (offline then online; a playlist made offline; undo inside and outside the window; refusals; likes) and the API test on the recording. The recording made real writes on the owner's server and undid them: create, add at a position, move by entry ID, remove, rename, delete, a favourite set and cleared, each checked by reading it back. Swift, Kotlin and Dart add and undo over HTTP in CI. |
| The skip signal for swipes | **Passes** | A table test of every section 21 rule with its boundaries (`every_section_21_rule`); interruptions (`a_skip_right_after_an_interruption_is_not_a_skip`); too soon; songs passed over by quick swipes leave no row (`songs_passed_over_by_quick_swipes_leave_no_trace`); the whole flow through the API with the real clock, swipe, headset, interruption, end of queue and back (`swipes_headsets_and_interruptions_through_the_api`). |

## Section 24 groups this phase touched

### Smart picks

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Interruptions, errors, seeks and plays under 2 s write no negative signal | **Passes** (open in Phase 2) | `every_section_21_rule` (errors, accidents under 2 s, system ends), `a_skip_right_after_an_interruption_is_not_a_skip`, and `signals_too_soon_interruptions_and_seeks`: a seek is counted on the play and never ends it, and a crash reads as system. |
| Private listening, Guest and Learn from my listening off write zero events, at the database level | **Passes** | Unchanged from Phase 2; still green. A guest may queue but not save (`refusals_confirmations_and_names`, `playlist_drops_likes_and_undo_through_the_api`). |
| The rest of the group | Phase 5 | Weights, mixes and replays need the engine. |

### Swipes and drag and drop (PLAN-UX section 24): the core's share

| Item | Result | Evidence or reason |
| --- | --- | --- |
| A completed swipe writes exactly the Next button's signal; two swipes within 600 ms write a signal only for the song landed on; the end of the queue rubber-bands | **Core part passes** | Every origin runs `skip_next` (`swipes_headsets_and_interruptions_through_the_api`); `songs_passed_over_by_quick_swipes_leave_no_trace`; `next_target` reports the end (`swipe_targets_and_the_ends_of_the_queue`). The gesture itself, pause state and the edge zone are the platforms'. |
| Headset, car and lock screen buttons run the same code | **Core part passes** | One call, `skip_next(origin)` and `skip_previous(origin, position)`, for every origin; the origin is stored on the play. |
| Songs go in list order; duplicates by song key with Add anyway; over 100 asks first; 500 songs make one call, one event, one undo entry | **Passes** | `songs_go_in_the_order_shown_at_the_line`, `duplicates_are_found_by_song_not_file`, `refusals_confirmations_and_names`, `five_hundred_songs_are_one_event_one_undo_and_one_outbox_entry`. Copy versus move is the platforms' (a move is add, then remove). |
| Albums refuse with the reason; Guest sees playlist targets refused with the reason | **Passes** | `drop_preview` gives `AlbumsTakeNoSongs` and `GuestCannotSave` with sentences (`refusals_confirmations_and_names`, the API test). Escape, menus and screen readers are the platforms'. |
| Row-swipe arm points; Undo for 5 s | **Core part passes** | Remove, Play Next and Add to Queue each give an `edit_id` with Undo up to 10 s (`undo_removes_exactly_that_edit_within_its_window`); the toast closes at 5 s. The arm points are the platforms'. |

### Data, sync and security

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Versioned migrations tested forward | **Passes** | Migrations 3 and 4: `a_version_2_database_with_a_library_and_history_upgrades` (2 to 4, rows kept, new defaults, the triggers), `a_version_1_database_upgrades_with_its_data`; a copy is kept before every upgrade (`a_copy_is_kept_before_migrating_and_old_copies_are_pruned`); a failing migration changes nothing (`a_failing_migration_changes_nothing_and_the_copy_stays`). |
| A backup and restore test passes | **Passes** (failed in Phase 2: not built) | `a_backup_taken_while_in_use_restores_every_row` (writes still in the WAL included), `a_backup_taken_while_playing_restores_on_another_device` (every play, no token in the file), refusals of damaged, foreign and newer files, and `restoring_waits_for_a_background_task_to_let_go`. That last test found a real Windows bug: restoring while a background send held the database failed with "Access is denied". Fixed, and it failed before the fix on the owner's PC. |
| Sync conflict tests | Phase 4 | Edits now write the event log Phase 4 syncs (ULID, device, hybrid clock). |
| No telemetry; only the user's servers | **Could not be checked** | Needs a device's network log. The new writes go only to the media server through the outbox, but that comes from reading the code. |
| Tokens never in logs or backups | **Core part passes** | A backup holds no token (`a_backup_taken_while_playing_restores_on_another_device`); the recording's leak checks passed. |
| Licence scan in CI | **Passes** | quick-xml and png (and png's flate2, fdeflate, crc32fast, miniz_oxide) are on reviewed licences; the licence texts were read (THIRD_PARTY_LICENSES.md); in CI 271 (core), 181 (server) and 149 (tools) packages, all on reviewed licences. |

## Found and fixed during this phase

1. **Restoring on Windows while a background send held the database failed.** Restore now waits for it.
2. **The recorder's anonymizer scrambled Jellyfin's own words.** `format=Png`, `maxWidth`, `filters`, `position` would have made replays never match. The dry run found it before the owner's approval.
3. **Three lyrics bugs** (above).
4. **The palette missed board 04's light field** (Paper Moon 0.078 off). Fixed by the board's rule, not by tuning.
5. **`sync` is reserved in Dart.** The field is `sync_level`.
6. **Smoke-test mistakes:**
   - a Swift `try` inside `precondition`;
   - a check that counted distinct songs instead of playlist entries.

## Unverified and open

| Item | Why it is open | Check against |
| --- | --- | --- |
| Word-by-word lyrics on real files | The owner has none; tested on written fixtures | The owner's first word-timed files, or a provider |
| A first lyrics parse of a very long word-timed song on a loaded machine | 1.8 ms in CI, 9 ms on the owner's busy PC; off the main thread and once per song | Phones in Phase 6; optimise if a real song gets near it |
| `Metadata.Offset` in Jellyfin lyrics | Nothing reports it | A server that does |
| Palette and signal numbers | Starting values | The look on devices; replays in Phase 5 |
| Lyrics search | Not built (it was not in Phase 3's row); lyrics are kept only once opened | A later phase, with offline lyrics |
| Export in an open format (board 19) | Backup and restore are built; export is not | The privacy page's phase |

## What the platform tracks can build now

See CORE-API.md version 2, "What the platform tracks can build now (Phase 3)":
- the lyrics view;
- album gradients;
- swipes on the mini player and Now Playing;
- row swipes;
- drag and drop with every menu twin;
- likes;
- offline edits;
- backup and restore.
