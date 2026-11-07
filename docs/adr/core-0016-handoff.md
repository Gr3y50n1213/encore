# ADR core-0016: The handoff protocol: snapshot, confirmation, counted once

Date: 7 November 2026
Status: accepted

## Context
Plan section 11 and board 12 define handoff:
1. The sender packages a snapshot.
2. The server relays it and the play is counted once.
3. The target preloads, starts at the position plus the network delay, and confirms.
4. Only then does the sender fade out. If the target fails, the sender keeps playing and shows why.

Phase 7's "done when" is *phone-to-desktop transfer starts in under 1 second for cached tracks*. The platform's player plays the audio, and the core owns the queue and the play log.

## Decision
- **The snapshot** (`crate::handoff::Snapshot`) holds:
  - History, the current song, Up Next and Autoplay, each entry by song key and the media server's item ID, never this device's track ID (each mirror numbers songs its own way);
  - shuffle, repeat, Autoplay on or off, the radio seed and the volume;
  - the position with the time it was true, by the server's clock;
  - whether it plays, and the listening time so far.
- **Push** (`hand_off_to`):
  1. The sender relays an `offer` and waits up to 5 s.
  2. The target maps the entries to its mirror, leaving out songs it doesn't have and counting them. If it lacks the current song it declines: "That song isn't in this device's library yet".
  3. The target closes whatever played there, puts the queue in place (`Queue::restore`, Up Next in its playing order), and sends `HandoffIncoming` to its platform.
  4. The platform loads the song and calls `handoff_ready`, which returns the position to start at now (position plus time since, by the server's clock) and sends `accept`.
  5. The sender then ends its part, sends `HandoffConfirmed` (fade out and pause), and returns.

  A `decline` (the platform's `handoff_failed`), an `undeliverable` or the timeout sends `HandoffFailed` with the reason. The sender has changed nothing, so it keeps playing.
- **Pull** (`pull_from`) asks the playing device to push here. The playing device's core answers by itself, because it has the queue and the position. A device that plays nothing declines with "Nothing is playing on …".
- **Counted once, with a change from the plan.** The plan's draft said the target continues the same play ID. But a play event belongs to the device that writes it (sync pushes only a device's own plays), and an ended play is never replaced (core-0010). So:
  - the sender's part ends with a new end reason, `handedOff`, read as "system": never a skip, never counted, never learned from. Readers leave it out: `recs_plays`, Most played, and the server's history;
  - the target's play starts with the listening time so far (`PlayStart.carried_listened_ms`), so its skip signal and its time cover the whole play.

  One counted play results, with no change to sync's rules. The recap (Phase 11) counts the same way.
- **Remote control:**
  - Transport commands (Play, Pause, Next, Previous, seek, volume) reach the playing device's platform as `RemoteCommand`, to do as if its own button was pressed, so the play log's rules stay in one place.
  - Queue edits (Play next, Add, Remove, Move, Jump to) are done by the playing device's core, mapping the controller's songs by key.
  - `remote_queue` reads its queue (empty under Private listening there).
  - `RemoteCommand` is one record with a kind and its fields, so Dart reads it too.
- **Presence and the banner** (board 12, item 1):
  - `ContinueAvailable` fires only while another device plays and this one doesn't;
  - once per session per device;
  - never transfers by itself.

  Under Private listening, presence says what state a device is in but not the song. Guest has no handoff.
- **Timing.** The core's part (relay, mapping, queue, confirmation) is measured end to end. The sound starting on a device is the platform's part.

## Consequences
- Measured in `server/tests/two_devices.rs` with two real cores, a real server and real WebSockets on loopback, and a player that is ready at once (a cached track): the tap to the confirmation took 15 to 19 ms in three runs, starting at 30.0 s for a song 30 s in.
- The same test covers:
  - one `handedOff` part and one finished play of 200 s;
  - the pull back;
  - remote Pause and Play next;
  - a refused handoff leaving the phone playing;
  - rename.
- The 1 s goal on real devices still includes the platform's own start and real networks; the platform tracks prove it on their devices.
