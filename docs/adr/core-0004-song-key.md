# ADR core-0004: The stable song key

Date: 6 November 2026
Status: accepted

## Context
Plan section 5: every play event stores "a stable song key (the MusicBrainz ID when tagged, otherwise artist + title + duration)". Section 4: duplicates across servers "merge by MusicBrainz ID, then by artist + title + duration". Section 25 counts the year recap by it ("the same song on two albums counts once"). The key outlives every library and every server, so it can never change for a song.

## Decision
- With a MusicBrainz **recording** ID: `mb:<the ID in lower case>`. In Jellyfin 12.0 this is `ProviderIds.MusicBrainzRecording`, read from the `MUSICBRAINZ_TRACKID` tag. `MusicBrainzTrack` holds the release-track ID instead (Jellyfin `AudioFileProber.cs`, lines 533 to 557, read on 6 November 2026).
- Without one: `t1:<hash>:<seconds>`. The hash is the first 32 hex digits of SHA-256 over the normalized artist, a 0x1F separator and the normalized title. The seconds are the duration rounded to the nearest whole second. The artist is the first credited artist, else the first album artist.
- Normalization: Unicode NFKC, lower case, whitespace trimmed and collapsed. Punctuation, accents and scripts are kept, because "Song (Live)" is not "Song" and "Ölmez" is not "Olmez".
- Two `t1` keys name the same song when their hashes match and the durations differ by no more than `recap.counting.sameSongDurationToleranceSeconds` (2 s).
- A test freezes one key's exact value, checked against Python's `hashlib`.

## Alternatives considered
- The server's item ID: it changes when a file is re-imported or the library moves to another server.
- Folding accents or dropping brackets: more matches, but it would merge different recordings, and the history could not be split again.
- Putting the duration inside the hash: then a 2 s tolerance could not be applied at all.

## Consequences
- The rules are frozen. A better key needs a new prefix (`t2:`) and a migration that keeps the old keys readable.
- The raw artist, title and duration are stored with each play, so a future key version can be computed from history.
