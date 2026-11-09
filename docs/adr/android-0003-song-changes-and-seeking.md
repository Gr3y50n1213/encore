# ADR android-0003: The next song is loaded at once; a transcoded song seeks from the rolling cache

Date: 9 November 2026
Status: accepted

## Context
A live check on the owner's tablet (8 November 2026, LTE, a capped quality) found three things the emulator tests had never exercised, because no test let a song play to its end:

1. **Every natural song end skipped a song.** The player moved into the preloaded next song, reported `STATE_ENDED` for it a few milliseconds later, and the app took that as the new song finishing: the core moved on a second time, the skipped song went to History as played, and the one after loaded cold (0.5 to 19 s of silence). The app removed the finished item from the player's list inside ExoPlayer's own `onMediaItemTransition` callback.
2. **Next mid-song was always a cold start**, because the next song was loaded only 20 s before the end (`playback.preloadBeforeEndSeconds`).
3. **A capped quality couldn't seek.** Jellyfin sends a transcode as a progressive stream with no length. ExoPlayer can't seek it and treats it as live, so a seek restarted the song (and transcoded it again) and the lock screen, notification and car showed no position.

The owner left the seeking approach to Encore ("whatever works best for everyone"); it has to work with anyone's Jellyfin, with or without the plugin, and the owner's server is a 2-core Celeron.

## Decision
**The player's list is the current song, then the next one, and it is never edited inside ExoPlayer's callbacks.** `CorePlayback.tidy()` runs just after, from the main looper: it drops played items and makes the item after the current one match the core's next song (`next_target`, or the current song on Repeat One).

**A song counts as finished only when the player is really at its end** (within 3 s of its duration). An "ended" anywhere else is not believed: the song plays again from the start, and a second false end of the same song is treated as an error (skipped). `falseEnds` counts them; the emulator test requires zero.

**The next song is loaded as soon as a song starts**, not 20 s before the end. ExoPlayer loads it right after the current one is read through, so a natural change is gapless and Next is instant. The owner accepted the extra data (8 November 2026). The buffer is 64 MB, room for a CD-quality FLAC and the next song.

**A transcoded song is kept in the rolling cache.** ExoPlayer marks every progressive request "don't cache if the length is unknown" (`DataSpec.FLAG_DONT_CACHE_IF_LENGTH_UNKNOWN`, meant for endless radio), so until now a transcode was never cached: Phase 8's "a song played whole plays offline" silently didn't hold for capped qualities (mobile data by default). The resolving data source clears that flag; Encore plays songs, never endless streams. Android asks for AAC transcodes, whose frames stand alone, so a part kept from an interrupted play joins cleanly with the rest fetched later.

**A transcoded song seeks from the rolling cache.** The player already reads each song through into the rolling cache (ADR android-0002, the 15-minute load control), and when the cache reaches the end of a stream with no length it records the length. Opened again from the cache, the same song has a known length; with constant-bitrate seeking on in the extractors, ADTS (AAC) and MP3 then seek by their bitrate, and Ogg (Opus) by its pages.
- The song after the current one is opened again from the cache once it's whole (`RollingCache.isWhole`), before it plays: it plays seekable, with its length on the lock screen and in the car.
- A seek from Now Playing in the current transcoded song reopens it from the cache at that point; while it is still arriving, the seek waits for the rest (seconds).
- The check runs after each load and on the one-second tick, and never in the current song's last 8 s (the player may already be reading into the next song for a gapless change).
- The next song is added to the player only once the current one has settled (arrived whole, any waiting seek done). Any seek in the current song, or reopening it, makes ExoPlayer drop the next song's download and start it again (`seekToPeriodPosition` removes the periods after the playing one); on a capped quality each new start is another transcode on the server. On the owner's Jellyfin a reopen right as the next song started downloading asked for it three times. The player reads songs one after another anyway, so this costs no time.
- A song already whole in the cache (played before) opens from there at once.
- The address read from the cache is the stream's own with a `#whole` fragment: never sent to the server, and a different item to the player.

## Considered
- **HLS for capped qualities** (`master.m3u8`, Media3's HLS module). Seeks before the song has arrived and is what Jellyfin's web client uses. Rejected for now: a new library; the core can't tell from its library data which songs need a transcode (it stores the container, not the codec or bitrate, so an M4A could be ALAC or AAC), so every capped song would go through ffmpeg on the server, a real cost on a small server; HLS segments need their own cache keys, and "played whole, so playable offline" would need rethinking. Worth revisiting if a seek in the first seconds of a cold transcode matters.
- **Asking Jellyfin for an estimated length** (`EstimateContentLength`, byte-range seeking into a transcode): a DLNA-era option, not reliable across server versions.

## Consequences
- More data on mobile networks: one song ahead is always fetched, including songs that are then skipped.
- The lock screen, the car and Bluetooth can't seek the first transcoded song of a run (the one that loaded cold): the media session takes its available commands from ExoPlayer, which withholds seeking for a stream it can't seek, and `ForwardingPlayer` passes that list through. From the second song on they can, as those open from the cache. Fixing the first would mean a `ForwardingSimpleBasePlayer` for the session.
- A seek in the current song while the next one is still downloading (a few seconds after the current one arrived) still restarts the next one's download: one more transcode request on the server.
- With the rolling cache set to Off, a transcoded song can't seek (the seek waits and is dropped at the next song). Original quality, downloads and direct streams seek as before.
- If the cache evicts a song opened as `#whole`, its reads fall back to the server, which re-transcodes from the start (rare: the cache keeps the newest songs).
- Debug builds log every player event (`EncorePlayer`) and the app's decisions (`CorePlayback`); CI keeps both in `device-check/logcat-playback.txt`.
