# Phase 5 review: core and server track

Prompt 5, 7 November 2026. Phase 5's "done when" line (docs/ROADMAP.md) is *autoplay's early-skip rate is clearly lower than shuffle on your own history, and the smart-picks items in section 24 pass*. The core track's part covers:
- signal logging, candidate pools and a hand-tuned ranker;
- autoplay, radio, mixes and Smart shuffle;
- Why this song, Not interested and Private listening;
- skip-rate stats.

No app exists yet: the platforms draw these screens from CORE-API version 4.

Rules: an item passes only with evidence from a test, a replay, the recording or a CI run. Unless it says otherwise, CI evidence is run **42362558429** (all six jobs green; Swift, Kotlin and Dart call smart picks through the bindings), with every job green.

The owner's answers:
- "your own history" comes from Jellyfin's own data, in one read-only recording, and stays on the owner's PC;
- the mixes are built on the companion server, and by a device without one;
- I chose that devices send the song list.

## The "done when" line

| Item | Result | Evidence |
| --- | --- | --- |
| Autoplay's early-skip rate clearly lower than shuffle on **generated** histories | **Passes** | `tools/recs-replay --check` in CI (docs/core/REPLAY-RESULTS.md). Autoplay against shuffle, early skips, on the same random draws: **light 20.8% vs 60.6%** (0.34), **heavy 9.0% vs 49.7%** (0.18), **many skips 37.1% vs 81.5%** (0.46). The check fails above 0.60. |
| … **on your own history** | **Open: the skip rate can't be measured from what Jellyfin keeps** | Recorded 7 November 2026 (`fixture-recorder --phase5`, one read-only approval): 184 songs, 138 played, 686 plays. Playback Reporting is not installed, and Jellyfin keeps counts, not skips, so only the weaker hit-rate check could run (`recs-replay --private`; core-0014). Held out the 27 most recently played songs: **(a)** 5.2% of autoplay's picks were among them against 14.1% for shuffle. That check is biased against the engine: hiding their counts makes them look unplayed, and the engine prefers the songs it sees played. **(b)** The fair half, among songs the engine sees as unplayed: all 14 of autoplay's picks there were held-out songs, against 35.6% for shuffle (2.8×). That is evidence it follows what you play next, but it rests on 14 picks. The skip-rate line closes once an app logs plays with outcomes (Phase 6 onward) and `recs-replay` runs on that log. |
| The smart-picks items in section 24 | See below | |

## Section 24: Smart picks

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Offline replay on at least three test histories beats plain shuffle, numbers saved in CI | **Passes** | Above; the numbers are in the Linux job's output and REPLAY-RESULTS.md. |
| Interruptions, errors, seeks and plays under 2 s write no negative signal | **Passes** | Unchanged from Phase 3 (`every_section_21_rule` and the rest). The engine reads only counted outcomes; the others only mark a song as heard (`affinity_follows_what_was_finished_and_skipped`). |
| Private listening, Guest and Learning off write zero events | **Passes** | Unchanged from Phase 3. Plus: Private listening ends after 2 hours of silence (`private_listening_ends_after_two_hours_of_silence`); Guest can't give feedback; Learning off shows only fallbacks (`home_follows_…`). |
| Reset suggestions and Clear listening history behave differently, and both sync as tombstones another device can't undo | **Passes** | Through the API: `reset_and_clear_behave_differently` (Reset keeps 55 plays but learns from none; Clear deletes them). Sync: `reset_and_clear_are_tombstones_no_device_can_undo` (an older reset can't move the epoch back; a late older play stays cleared). On the server: `the_server_builds_mixes_from_the_log_and_the_catalog` (a pushed clear deletes older plays and drops a late one). |
| A cold profile, 49 plays and 50 plays show the right mixes, labels and fallbacks | **Passes** | `mixes_follow_the_cold_start_table`: 0 plays gives a Starter mix; 1 to 49 up to three family mixes, still learning; 50 or more under 30 days the two strongest; 30 days or more up to five. `home_follows_…`: NotEnoughPlays at 0 of 50, Fresh after 60, LearningOff with Most played. |
| Every smart surface has a working fallback with no model, the server off and no network | **Passes, except downloads** | No history: metadata-only picks (Guest and learning off build from the library alone). Server off: the device builds the same set (`the_mixes_come_from_the_companion_server_and_the_device_builds_them_without_it`). No network: everything reads the mirror. "Only downloaded songs" needs downloads (Phase 8): the engine takes the list (`only`), and nothing is downloaded yet. |
| Budgets: next song under 50 ms, mix build under 20 s and 150 MB | **Passes in CI; not on a phone** | On 20,010 songs and 20,160 plays, in CI: model plus five mixes **0.03 s**, **6.7 MB** at the peak; the next-song pick **5.1 ms** (20 ms on this busy PC). A phone is Phase 6's. |
| Licences of the audio embedding model and the nearest-neighbour library | Not yet applicable | Neither exists until audio analysis (Phase 10). No new crate this phase. |

## The phase's items

| Item | Result | Evidence |
| --- | --- | --- |
| Signal logging | **Passes** | The section 21 classification since Phase 3; the engine's weights from `smartPicks`. Jellyfin's own play counts are now mirrored (a prior). |
| Candidate pools, hand-tuned ranker | **Passes** | 16 engine tests. Autoplay follows its seed, avoids early-skipped artists, never repeats an artist in a row, never explores first or twice in a row, and is deterministic. Not interested by scope; explicit hidden; offline lists. |
| Autoplay, radio, Smart shuffle | **Passes** | Through the API on the recording (`smart_picks_fill_autoplay_and_explain_themselves`): 10 app-chosen songs, radio holding its seed, one suggestion per 4. |
| Mixes | **Passes** | Rules per mix: titles pass the section 7.1 rules, 25 to 40 songs, 2 per artist, overlap, no artist twice in a row, stretch songs new, deterministic, hidden stays hidden, at most 30% change a day. Built on the companion server; the device falls back to its own build. |
| Why this song, Not interested, Private listening | **Passes** | At most two reasons, as checkable facts; Not interested with undo and a list, last write wins across devices offline (`not_interested_on_two_devices_offline_ends_the_same_everywhere`); Private's 2-hour end. |
| Skip-rate stats | **Passes** | `suggestion_stats` per shelf over 4 weeks (`shelves_and_stats`, the API test). |

## Found and fixed during this phase

1. **A self-deadlock.** `playback_started` took the playback lock it was already holding, and new code took the store lock before playback, the reverse of the rest of the core. Both hung the tests.
2. **Small libraries never got a mix.** The recording has 14 lead artists, and at 2 songs per artist that can't fill 25 songs. Mixes are now as long as the caps allow (at least 10); the caps are never relaxed.
3. **Most played was empty with learning off.** It came from the model, which learns nothing then; it now comes from the play log.
4. **The next-song pick took 180 ms** on 20,000 songs. Each artist's record and the fallback sort are now precomputed: 20 ms.
5. **The Phase 3 swipe test assumed no autoplay.** It now turns autoplay off (a new `set_autoplay` call, board 11's switch).

## Unverified and open

| Item | Why it is open | Check against |
| --- | --- | --- |
| Early skips on your own history | Jellyfin has no skip data and Playback Reporting isn't installed; the hit-rate check is small (14 fair picks) | The first app's play log, replayed (or install Playback Reporting and record again after a few weeks) |
| Every weight, threshold and title rule | Starting values; generated listeners are not real ones | Replay on real histories as apps log them |
| Phone budgets | CI and this PC only | Phase 6 devices |
| Audio-based moods, tempo and energy flow, model-written titles | Need audio analysis | Phase 10 |
| "Only downloaded songs" | Downloads don't exist yet | Phase 8 |
| Smart downloads | A Phase 8 surface | Phase 8 |
| Household learning, history import from Last.fm or ListenBrainz | Optional extras, not built | Later, if the owner wants them |

## What the platform tracks can build now

CORE-API.md version 4, "What the platform tracks can build now (Phase 5)":
- Home and the Mixes tab with every card state;
- the queue's Autoplay section and switch, radio and Smart shuffle;
- Why this song, More/Fewer like this, and Not interested with its list;
- Settings, Mixes and suggestions.
