# Phase 4 review: core and server track

Prompt 5, 7 November 2026. Phase 4's "done when" line (docs/ROADMAP.md) is *two people share an iPad with fully separate history, and a like on the phone shows on the Mac within a second*. The core track's part (PLAN-CORE-SERVER section 3) is the Docker image, the auth exchange, event log sync, and profiles with the switcher state. The platforms draw the switcher and the sync status row.

Rules: an item passes only with evidence from a test, the recording or a CI run, never from reading the code. Unless it says otherwise, CI evidence is run **CI_RUN**, with every job green.

The owner's answers for this phase:
- long-poll now, with the WebSocket in Phase 7;
- only crates already in the build;
- no mDNS yet;
- one read-only recording;
- the server built and run in CI only, never on the owner's PC.

## The "done when" line, core part

| Item | Result | Evidence |
| --- | --- | --- |
| Two people share an iPad with fully separate history | **Passes** | `two_people_share_an_ipad_with_fully_separate_history`: one core, one data folder, two profiles (the recording and a stand-in second user with the same library). Alex starts with an empty queue, none of Sam's likes, and no plays in an export. Switching back loads Sam's queue paused, with Sam's play and like. On the server, two profiles from one device ID never see each other's events (`two_people_on_one_ipad_never_see_each_others_events`). |
| A like on the phone shows on the Mac within a second | **Passes, on one machine** | Two real cores (each with its own data folder) and a real `encore-server`; the like arrives as a `RemoteChange`, timed and asserted under 1 s. **118 and 135 ms** with the server in process on the owner's busy PC (`a_like_on_the_phone_shows_on_the_mac_within_a_second`); **106.8 and 107.2 ms** through the amd64 Docker image and **118.5 and 127.9 ms** through the arm64 image under QEMU in CI (runs 42347709222 and 42348565517) (`through_the_docker_image`). Never over Wi-Fi or on phones: that is Phase 6's. |

## The phase's items (PLAN-CORE-SERVER section 3)

| Item | Result | Evidence |
| --- | --- | --- |
| Docker image | **Passes** | `server/Dockerfile` (non-root uid 10001, one `/data` volume, a health check), built in CI for amd64 (91 MB) and cross-compiled for arm64 (113 MB). Each ran its health check, the two-core test and an online backup. `server/compose.yaml` runs it next to Jellyfin. |
| Auth exchange | **Passes** | Server tests on stand-ins and on the owner's real replies (`the_jellyfin_check_on_the_recorded_server`: the token names its user; after sign-out it is refused). Also tested:<br>- an untrusted server is 403, a bad token 401, Jellyfin down 503;<br>- tokens are stored only as hashes;<br>- refresh rotates, and reuse revokes the device, except a retry within 2 minutes;<br>- the rate limit;<br>- the status page only from the home network. |
| Event-log sync | **Passes** | Store tests:<br>- plays are sent at start and again once ended, and another device's ended version replaces its start;<br>- likes merge last-writer-wins, with a deterministic tie, and the losing device reasserts the winner to Jellyfin;<br>- playlist events name the playlist to re-read;<br>- odd events are skipped.<br>End to end: like, unlike, a play joining the other device's history, a profile reopened with its kept token catching up, revocation, and delete everywhere. |
| Profiles and the switcher state | **Passes** | The iPad test above, plus:<br>- `a_locked_profile_opens_only_after_its_pin` (five wrong tries lock it for 30 s and even the right PIN waits; biometrics need the PIN once after they change);<br>- `guest_saves_nothing_and_gives_the_queue_back` (no play recorded, likes refused, the queue given back, the idle end);<br>- the registry's order, names and avatars;<br>- the PIN rules' unit tests. |

## Section 24: Data, sync and security

| Item | Result | Evidence or reason |
| --- | --- | --- |
| Versioned migrations tested forward | **Passes** | Profile migration 6 (`like_clock`, `saved_queue`, send state) in the migration test chain; the server's own schema with a newer-schema refusal; the device registry's version 1. |
| A backup and restore test passes | **Passes** | The server's: `a_backup_restores_every_row`, and `encore-server backup` ran inside both images in CI. The core's from Phase 3, still green. |
| Sync conflict tests: two devices editing a like, a playlist and Not interested at the same time while offline | **Like and playlist pass; Not interested is Phase 5** | Like: `likes_merge_last_writer_wins_and_the_loser_reasserts_the_winner`. Playlist: `two_devices_edit_one_playlist_offline_and_both_edits_land` (both land on the media server, and both devices show its list). Not interested doesn't exist until Phase 5. |
| Tokens in the keychain only; nothing sensitive in logs or backups | **Core part passes** | Media-server and companion tokens reach the core only from the app's keychain, and back only through `CredentialChanged`. The server keeps hashes. Debug output of every token-holding type prints no token. Exports and backups hold none (Phase 3 tests). |
| No telemetry; only the user's servers | **Could not be checked** | Needs a device's network log. The only new destination is the companion server the user typed. |
| Licence scan in CI | **Passes** | No new package: core 271, server 181 and tools 149 packages, all on reviewed licences (search on 50,000 songs with 400,000 lyric lines: slowest median 7.84 ms, browse 33.24 ms; budget 50 ms). |

## Found and fixed during this phase

1. **A sign-in's unlock outlived the opening.** A PIN set later didn't lock the profile. Found by the PIN test; an unlock is now used by the next opening either way.
2. **Refresh-token rotation could lock the owner out.** A crash between the server rotating and the app saving would have looked like theft. A 2-minute retry window now applies while the replacement is unused.
3. **Lyrics search was too slow in its first two designs** (Phase 3 tidy-up, before this phase): fixed by a word index, 7.58 ms in CI.
4. **The recorder would have repeated Phase 3's writes.** A read-only `--phase4` mode records to its own file.
5. **axum's query extractor would have added a library.** The pull query is parsed with the url crate.

## Unverified and open

| Item | Why it is open | Check against |
| --- | --- | --- |
| The like over Wi-Fi, on phones | Measured on one machine and in CI | Phase 6 devices on the owner's network |
| arm64 speed | Ran under QEMU in CI | A Raspberry Pi 5 or arm64 NAS |
| Avatar size | The owner's Jellyfin sent a 2.7 MB original for a 256 px request | A device loading avatars |
| A delete queued while the companion is offline ("Waiting to finish on server", board 19) | Not built: the core refuses and says so | A later phase, if wanted |
| Biometrics | The core trusts the app's report | The platforms |
| The companion's own address checks | No certificate pinning for it yet | Before release |

## What the platform tracks can build now

CORE-API.md version 3, "What the platform tracks can build now (Phase 4)":
- the profile switcher, with locks, Manage profiles and Remove;
- Guest;
- Connect a companion server and the sync status row;
- Devices with sign-out;
- Delete profile and its data;
- the companion token in the keychain.
