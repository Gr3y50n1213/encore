# ADR core-0013: Mixes on the companion server, the catalog, the device fallback

Date: 7 November 2026
Status: accepted

## Context
Plan section 7.1 builds the five mixes nightly on the companion server, delivered so every device shows the same five. With no server, the device builds them with the same code. The owner chose the server for Phase 5, with the device building them when a profile has no server. The companion server deliberately keeps no media-server token (core-0009), so it can't read the library itself. The owner left the choice to me: the device sends the song list, or the server holds a Jellyfin API key.

## Decision
- **Devices send a catalog.** After a library sync, a device sends the profile's song list (`PUT /catalog`), but only when its SHA-256 hash changed (`GET /catalog/hash`).
  - It holds song keys, titles, artists, album, genres, year, length, date added, Jellyfin's play counts and favourites.
  - It holds no audio and no token. The server holds no new secret, and knows only what the profile's devices already know.
  - The API-key choice can wait for Phase 9's audio analysis, which needs to read audio anyway.
- **The server builds from the synced log** (`POST /recs/home`): the history from the profile's events (plays, likes and Not interested last-writer-wins, playlist adds, feedback, the epoch), plus the catalog, through `encore-recs`.
- **When it builds:**
  - at the first request after 06:00 in the profile's own time zone (the device sends its offset);
  - after 25 new plays;
  - on Regenerate;
  - never while a device plays one of the set's mixes (the device says which).

  Builds have a 20 s limit, and the last good set is kept on failure.
- **Device behaviour.** With a server that answers, Home uses the server's set (`from_companion`). With none, or one that can't be reached, the device builds the same set with the same code. If the server has no catalog yet, the device sends one and asks again.
- **Clear listening history on the server.** The pushed tombstone deletes the profile's older plays and lyric jumps and its mix set. An older play pushed later is accepted (so the device stops resending it) and dropped.
- **Pins and hides.**
  - Hidden mixes are sent with each request.
  - Pins are kept on the device: a pinned mix is a copy of its songs, so it needs nothing from the server.

## Consequences
- An end-to-end test: two cores and a real server; after 60 plays, the phone and the Mac show the same set from the server; with the server gone, the phone builds its own.
- A device and the server agree exactly only when their inputs agree. The engine is deterministic (core-0012), but a device's history can be ahead of the server's until it syncs.
- A large library's catalog is a few megabytes, once per change, on the home network. The route accepts up to 64 MB.
