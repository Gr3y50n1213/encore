# Emby fixtures (invented)

Hand-written replies for the Emby flavour of the Jellyfin adapter (ADR core-0022,
`core/encore-sources/src/jellyfin/emby_fixture_tests.rs`). Nothing here was recorded
from a real server and nothing in them is real: the server name, the IDs, the user
"listener", the artists (Ember Cove, Salt & Lantern), the albums and the songs are all
made up. The shapes follow what a real Emby 4.10.2.0 answered in CI on 26 November 2026
(GitHub Actions, the `emby` job in `.github/workflows/core.yml`, on generated tone
files): no `ProductName` in `/System/Info/Public`, every path under `/emby`, small
numeric item IDs, lyrics as a `Subtitle` stream with codec `lrc`, lyric lines from
`Stream.js` with a byte order mark and start ticks, `PlaylistItemId`s that rise with the
position, `PlayCount: 0` beside `Played: true` in lists, and `/Sessions` entries.

The real server is exercised by `emby_flow_over_http` in CI; these files only let the
adapter's unit tests run with no server.

- `server.json`: `/System/Info/Public`, a sign-in, the user record, the sessions.
- `library.json`: one full sync (artists, albums, songs, a playlist, favourites, plays).
- `lyrics.json`: the item with its streams and the lyric lines.
