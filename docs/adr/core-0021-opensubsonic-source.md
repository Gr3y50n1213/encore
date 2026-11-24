# ADR core-0021: Navidrome and other OpenSubsonic servers

Date: 24 November 2026
Status: accepted (the owner: "Go on", Phase 12 stage 1)

## Context
Phase 12's "done when" is that a Navidrome user can do everything a Jellyfin user can, with no UI changes. Navidrome, Gonic, Airsonic-Advanced and others speak the Subsonic API; [OpenSubsonic](https://opensubsonic.netlify.app/) is its maintained, extended form, and one adapter covers them all. The core already reaches its server through the `MediaSource` trait (PLAN-PRODUCT section 4), but four places are Jellyfin-shaped: the profile's source map holds `JellyfinSource`, sign-in and Quick Connect build a `JellyfinClient`, a saved sign-in is a Jellyfin token, and the companion checks sign-ins against Jellyfin only.

## Decision
1. **One adapter, `encore-sources/src/subsonic/`**, a `SubsonicSource` implementing `MediaSource`, and `SourceKind::Subsonic` ("subsonic" in the database). JSON responses (`f=json`) only.
2. **Finding out what a server is.** `check_address` asks Jellyfin's `/System/Info/Public` first and, if that isn't Jellyfin, `/rest/ping` (no sign-in needed to learn that it is Subsonic). `ServerCheck` gains `kind`; the sign-in screen shows which kind it found and otherwise doesn't change.
3. **Signing in without keeping a password.** Subsonic's token is `md5(password + salt)` with a random salt; the device keeps only the user name, that token and its salt (in the keychain, where the Jellyfin token goes), never the password. Where the server offers OpenSubsonic's API-key extension, the key is used instead. No Quick Connect: its capability flag is off, so the button doesn't show.
4. **A server ID.** Subsonic has none, so the source's ID is a hash of the server's address as the user typed it, made canonical (scheme, host, port, path). The same server reached by two addresses is two sources; the docs say so.
5. **The library mirror.** Artists from `getArtists`, albums from `getAlbumList2` (alphabetical, paged), songs from each album, playlists from `getPlaylists` and `getPlaylist`. Subsonic has no change feed, so every sync is a full one in pages (the mirror already handles full syncs and removals), started only when the server's `lastModified` from `getIndexes` moved or a pull-to-refresh asks. Song IDs are the server's own; a song's key (the cross-source identity, `song_key.rs`) is made as for Jellyfin.
6. **Playing.** `stream` with `maxBitRate` and `format` for the quality caps, the original file otherwise; `download` (with HTTP ranges) for sound analysis and Downloads. Covers from `getCoverArt` at the exact size.
7. **Lyrics.** OpenSubsonic `getLyricsBySongId` (synced lines, offsets, several languages) when the server has the `songLyrics` extension, `getLyrics` (plain) otherwise; both parse into the one lyrics model.
8. **Writes.** Playlists: `createPlaylist`, `updatePlaylist` (add, remove by index, rename), `deletePlaylist`; a move is done as the server allows it (remove and add at the end, then reorder by recreating the list if the server can't insert at a position), and the capability flags hide what a server can't do. Likes: `star` and `unstar`. Play counts and last-played come from the song records.
9. **The companion.** `ENCORE_JELLYFIN_URLS` becomes `ENCORE_MEDIA_URLS` (the old name still read), each address tried as Jellyfin then Subsonic. A Subsonic device signs in to the companion with its user name, token and salt (or key); the companion checks them with `ping` and `getUser` on a trusted address and keeps nothing of them.
10. **Testing.** Recorded fixtures for the adapter's unit tests; in CI, a real Navidrome in Docker on GitHub's runners (never on the owner's PC) with a few Creative Commons songs, driving the production path over HTTP as the Jellyfin replay does. The two-device companion test runs for both kinds.

## Consequences
- Sync on a large Subsonic library is a full listing each time it runs; it runs only when the server says something changed.
- A Subsonic token is as good as a password for that server (the protocol's design); it is kept only in the keychain and is replaced by an API key wherever the server offers one.
- The Dart bridge and the Apple app pick this up when their tracks rebuild against CORE-API.
