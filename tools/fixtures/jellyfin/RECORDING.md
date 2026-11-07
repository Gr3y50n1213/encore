# Jellyfin fixtures

`recorded.json` is a recording of the Jellyfin adapter talking to a real Jellyfin 12.0.0 server,
made with `tools/fixture-recorder`. The adapter tests replay it with no server
(`core/encore-sources/src/jellyfin/recorded_tests.rs`).

- Recorded through the adapter's own code: the probe, Quick Connect, a first library sync,
a second (delta) sync, and lyrics for one song with lyrics and one without.
- Trimmed: each item type cut to the first 150 items, with `TotalRecordCount` cut to match.
- The ID listing that finds removals was answered from the recorded items, not the library.
- Anonymized before writing: names, titles, genres, IDs, image tags, provider IDs and the
server's address are replaced with consistent stand-ins of the same shape and script; file
paths, BlurHash previews, media stream details and tokens are removed. The key was random
and was not kept. A leak check refused to write if any original value was still present.

Recorded items: 26 artists, 38 albums, 150 songs, 1 playlists;
the delta sync found 0 changed songs and 0 removals.

## Phase 3: lyrics, covers and writes

Real covers were replaced in the fixture with board 04's own Golden Hour Static art. The
writes were made on a playlist named "Encore test - safe to delete" and on one favourite, and were undone in the
same session.

- Lyrics: 40 of 40 songs marked as having lyrics returned them; 0 with word cues.
- Covers: 38 of 38 came back as PNG (largest side 64 px); 0 in another format.
- Playlist: created the test playlist with 2 songs.
- Playlist: inserting at position 1 put the song second.
- Playlist: every entry has its own entry ID.
- Playlist: Move takes the entry ID (PlaylistItemId).
- Playlist: removing by entry ID removed exactly that song.
- Playlist: renamed.
- Playlist: deleted, and the server confirms it is gone.
- Favourite: 12 favourite songs before.
- Favourite: set, and listed as one.
- Favourite: cleared again; the favourites are as they were.
