# ADR core-0008: Library edits, the outbox and undo

Date: 7 November 2026
Status: accepted

## Context
Plan section 26 and board 43: a drop is **one call in the core** with the whole list and the position, **one sync event** and **one undo entry**, even for 500 songs. It must work offline and sync later. Duplicates are found by the song, not the file. Albums refuse drops, Guest can't save, and more than 100 songs asks first. Undo is offered for 6 s in the toast and 10 s for the keyboard shortcut. The owner chose that a like is also a Jellyfin favourite.

Jellyfin 12 writes playlists one call at a time, by entry ID:
- `POST /Playlists`;
- `POST /Playlists/{id}/Items?ids=&position=`;
- `DELETE /Playlists/{id}/Items?entryIds=`;
- `POST /Playlists/{id}/Items/{entryId}/Move/{index}`;
- `POST /Playlists/{id}` (rename);
- `DELETE /Items/{id}`.

Favourites are `POST` or `DELETE /UserFavoriteItems/{id}`. The Phase 3 recording confirmed each one on the owner's server, including that Move takes the entry ID.

## Decision
- **One transaction per edit.** It holds the local change (the library shows it at once), one event in `events` (ULID, device, hybrid logical clock; the log Phase 4 syncs), one undo entry with its inverse, and one outbox entry with the server writes, as JSON.
- **Playlists are named by their row ID in the outbox,** so a playlist made offline (`local:` ID) can be written to before the server gives it an ID.
- **Sending (`flush`):**
  - in order, per server;
  - an unreachable server, a 5xx, 401 or 429 leaves the entry waiting, with backoff, and stops so order is kept;
  - a refusal (404, another 4xx) drops it, and the next sync brings back the server's version;
  - a partly sent entry is rewritten with what is left, so nothing is sent twice;
  - entry IDs missing at send time are resolved from the server's list: by position first, then by song;
  - after a playlist's writes are all accepted, its rows are read back with their entry IDs.
- **Pulls never overwrite unsent edits.** A playlist with waiting writes keeps its local rows on pull, and a `local:` playlist is never removed by a full sync.
- **Undo.** `playlist_items.edit_id` (migration 4) lets undo remove exactly the rows an edit added, even after other edits moved them. The window is `gestures.dragAndDrop.shortcutUndoMs` (10 s), the longest any Undo is offered; the toast may close sooner. Undoing a new playlist before it was sent cancels its writes, so the server never hears of it. An undo is one event too and can't itself be undone.
- **Likes are keyed by song key** (a song on two albums is liked once). Each like sets the favourite on the song's server; an unlike clears it on every copy. After each sync, favourites set or cleared elsewhere become likes or clear them, except for songs with a favourite write still waiting.
- **Up Next and Play Next** use the queue's own undo, with an edit ID like every other edit. Nothing is sent: the queue is this device's.

## Alternatives considered
- **Sending immediately, without an outbox:** fails offline and loses the edit when the app closes.
- **One outbox entry per song:** 500 rows for one drop; the plan asks for one.
- **Encore-only likes:** other Jellyfin apps wouldn't see them, and other devices would wait for Phase 4. The owner chose favourites.

## Consequences
- Phase 4 syncs the `events` table as it is.
- A refused write is lost by design: the server's version wins. The UI can show `pending_edits` and the next sync's result.
- The `edits/tests.rs` model server keeps playlists the way Jellyfin does. The recording shows the real server answers the same.
