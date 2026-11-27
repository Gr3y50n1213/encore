# Plex fixtures (invented)

Everything here is made up by hand: the artists, albums, songs, playlists, the link code,
the account number and name, the server names, machine IDs, addresses and tokens. Nothing
was recorded from a real account or a real plex.tv.

- `server.json`: a Plex Media Server's replies (`/identity`, `/library/sections`, the
  artist, album and track listings, playlists, a track with lyric streams, the photo
  transcoder, playlist and rating writes), in the shapes a real, unclaimed Plex Media
  Server 1.43.5 answered in CI (`.github/workflows/core.yml`, job `plex`).
- `plex-tv.json`: a stand-in plex.tv (the PIN link, `/api/v2/user`, `/api/v2/resources`),
  from plex.tv's documented replies. The tests serve it through the fixture transport; no
  test or CI job contacts the real plex.tv.

The addresses use the documentation ranges (192.168.7.0/24 stands for a home network,
203.0.113.0/24 and 198.51.100.0/24 for the internet). See docs/adr/core-0023-plex-source.md.
