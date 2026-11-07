# ADR core-0010: Event-log sync and merge rules

Date: 7 November 2026
Status: accepted

## Context
Plan section 5:
- every change is written locally first as an event with a ULID, the device ID and a hybrid logical clock;
- devices push unsent events and pull everything after their cursor;
- play events append, and mutable fields merge last-writer-wins per field by the clock;
- the transport is HTTPS plus one WebSocket per device for live updates.

Phase 4's "done when" asks that a like on the phone shows on the Mac within a second. The owner chose long-poll now, with the WebSocket coming with handoff in Phase 7.

Jellyfin already holds playlists and favourites, and Phase 3's outbox writes them there.

## Decision
- **What syncs.**
  - Plays, sent twice: when they start (version 0) and once ended (version 1). `play_events.synced` counts which went (0, 1, 2).
  - Lyric jumps, after their play.
  - The edit events Phase 3 writes.

  Guest, Private and Learning-off write no plays, so there is nothing of theirs to send.
- **The server log.** One row per (profile, event ID), with a sequence number as the cursor. A newer `version` replaces a row and gives it a new sequence number, so devices that pulled the start also get the end. Pushes are idempotent.
- **Pull with long-poll.** `GET /sync/pull?cursor=&wait=25` answers at once when there is something, otherwise when one of the profile's other devices pushes (a per-profile `Notify`), or after 25 s. The core keeps one pull open; its pusher wakes on each edit (and checks every 30 s for plays), then debounces for 100 ms. A device never gets its own events back, but its cursor moves past them.
- **Merging.**
  - **Plays** join the log as another device's, already synced. An ended version replaces one still going; nothing else replaces a play.
  - **Likes** are last-writer-wins per song key by (hybrid clock, device ID). The device ID breaks a tie the same way everywhere. `like_clock` remembers who decided each song. A remote like or unlike that wins is applied to the likes and the mirror's favourites without a new Jellyfin write, because the sending device made it. If this device had decided differently (`from_here`), it also sends the winner to Jellyfin, so Jellyfin ends where the clock says even when this device's own write reached it last. Undoing a like is an unlike, and the reverse.
  - **Playlists.** Jellyfin is the truth. A playlist event names its media server and the playlist's ID there. The receiving device re-reads that playlist from Jellyfin, and suggests a library sync for a playlist it doesn't have yet. Two offline edits to one playlist both land: adds are kept, and removes go by entry ID.
  - Unknown kinds are skipped (a newer app may send more), and so is anything malformed.
- **Failure.** Unreachable, 5xx, 408 or 429 means `Offline`: back off from 1 s to 60 s and retry; changes wait locally. 401 means refresh once, then `SignedOut`. The status and the count still waiting reach the UI as `SyncStatusChanged`, and other devices' changes as `RemoteChange`.
- **Switching profiles** pushes what's waiting for up to `profiles.switchFlushSeconds` (3 s); anything left goes the next time that profile opens.

## Consequences
- Measured: a like reached the other device in 106 to 135 ms in tests, through an in-process server, the amd64 image and the arm64 image under QEMU.
- History from every device feeds the same play log, which Phase 5's smart picks and Phase 11's recap read.
- The like-versus-unlike conflict is tested. Not interested and the tombstones of Clear listening history and Reset suggestions come with Phase 5, which adds them.
