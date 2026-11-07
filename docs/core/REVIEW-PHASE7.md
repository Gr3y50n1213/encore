# Phase 7 review: core and server track

Prompt 7, 7 November 2026. Phase 7's "done when" line is *phone-to-desktop transfer starts in under 1 second for cached tracks*. The core track's part is presence, push, pull, remote control and the server WebSocket (plan section 11, board 12).

The owner's answers:
- tokio-tungstenite approved;
- shared household devices later;
- build it in full.

**Evidence:** local runs, then CI run 42393953953 with every job green. Its Linux job runs the two-core handoff end-to-end test, and Swift, Kotlin and Dart call `output_devices` and `hand_off_to` on Linux, Windows and macOS. The Docker image's end-to-end test still covers sync only; `/ws` through the image is a follow-up.

## The "done when" line

| Item | Result | Evidence |
| --- | --- | --- |
| The core's part of a phone-to-desktop transfer, under 1 s | **Passes locally: 15 to 19 ms** | `server/tests/two_devices.rs` runs `playback_moves_between_devices_and_is_counted_once` with two real cores, a real server, real WebSockets on loopback, and a "player" ready at once (a cached track). From `hand_off_to` to the confirmation: 18.5, 18.4 and 15.0 ms in three runs. The Mac started at 30.0 s for a song 30 s in. |
| The sound starting on a device in under 1 s | **Platform tracks** | The platform's player start and real Wi-Fi come on top of the core's time; measured on devices. |

## The phase's items

| Item | Result | Evidence |
| --- | --- | --- |
| The server WebSocket | **Passes** | `GET /ws` (server/src/live.rs), with server tests over a real socket. `the_live_link_carries_presence_relays_and_nudges_within_one_profile_only` covers welcome, presence, relay, undeliverable (another profile), pong, nudge, devices and rename, sign-out closing the link, and a revoked token refused. `the_live_link_is_bounded` covers 256 KB, 40 a second, garbage and replacement. |
| Presence | **Passes** | Published from the playback calls, and again when the position drifts over 2 s. The other device sees Playing, the song and the position (e2e). Private listening shares no song; Guest has no handoff. |
| Push | **Passes** | The snapshot goes by song key and item ID. The target puts the queue in place in its playing order (`a_handed_off_queue_arrives_as_it_was`) and starts at the position by the server's clock. The sender stops only after the confirmation, and a refused handoff leaves it playing with the reason (e2e). |
| Counted once | **Passes** | One `handedOff` part and one finished play of 200 s (30 s on the phone plus 170 s on the Mac), after sync (e2e). `handedOff` is left out of the engine's plays, Most played and the server's history. |
| Pull | **Passes** | The phone pulls the Mac's next song back; the Mac fades out (e2e). |
| Remote control | **Passes** | Pause reaches the phone's player as `RemoteCommand`; `remote_queue` reads its queue; Play next is done by the phone's core (e2e). |
| The banner rules (board 12, item 1) | **Passes** | `the_banner_follows_board_12`, and `ContinueAvailable` in the e2e test. |
| The output picker's states | **Passes** | `output_devices`: Ready in the e2e test, NoCompanion in the smoke tests, Unreachable while the link is down, NotAvailable in Guest. |
| Bindings | **Passes** | The Dart glue was regenerated in CI (run 42389936835). The smoke tests call `output_devices` and `hand_off_to` in Swift, Kotlin and Dart on every CI platform (run 42393953953), and on the owner's Android phones. |
| Licences | **Passes** | Every new crate is on a reviewed licence (`check_licences.py`: the server has 203 dependencies, all reviewed). |

## Found and fixed during this phase

1. **The plan's "same play ID" couldn't work.** A play belongs to the device that writes it, and an ended play is never replaced. Counting once is now "the sender's part is `handedOff`, the target's play carries the time" (core-0016).
2. **Positions drifted.** Progress reports updated the position only locally, so another device estimated from an old one after a jump. Presence now goes out again on a drift over 2 s.
3. **Two e2e tests could share a temp folder.** They started in the same instant with the same device names. The folder names now carry a counter.

## Not claimed

- Sound in under 1 s on real devices, and handoff over real Wi-Fi (the platform tracks').
- Shared household devices across profiles (later, by the owner's choice).
- `NSUserActivity` and the system Handoff icon (the Apple track's).
- `/ws` through the Docker image (the in-process server is tested; the image's end-to-end test covers sync).
