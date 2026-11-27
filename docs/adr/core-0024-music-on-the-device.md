# ADR core-0024: Music files on the device

Date: 26 November 2026
Status: proposed (Phase 12 stage 4; mock-up 49 waits for the owner's approval)

## Context
Phase 12's last source is the listener's own music files on the device, next to a media server in the same profile (plan section 4: "one profile can attach several sources at once", duplicates merged by MusicBrainz ID, then by artist, title and length). Unlike every other source there is no server: nothing to sign in to, no API to page through, nothing to stream from. The files have to be found by the platform, which alone can open them (Android's content addresses, iOS security-scoped bookmarks, desktop folders), and read by the core, which alone knows the library model. The store already keys every song, album and artist by source (`sources` table, `source_id` on each row), so a profile with two sources needs no new schema for the library itself.

## Decision
1. **Reading a file is the core's; finding it is the platform's.** `encore-sources/src/local/` reads a file it is handed as a stream (`Read + Seek`): its tags, length, embedded cover and lyrics (`read_tags`), through Symphonia's format readers. Symphonia is already in the workspace for audio analysis (MPL-2.0); no new library. No audio is decoded. The core never walks a file system or keeps a path.
2. **What is read** (mock-up 49): title, artists (a tag holding "A; B" or "A / B" is split; "AC/DC" is not), album, album artist, track and disc numbers, year (from a year or a date), genres, length (from the container, or counted from its packets when an MP3 has no Xing or VBRI frame), the MusicBrainz recording ID, lyrics, and the front cover (or else the first picture). A file with no title is named from its file name ("03 - Artist - Title.mp3"; `name_from_file`). Formats: MP3, FLAC, M4A (AAC and ALAC), Ogg Vorbis and WAV.
3. **Pending the owner's choice (mock-up 49, "To approve"):**
   - How the platform reaches the files. Recommended: folders picked in the system's folder picker (Android's Storage Access Framework), which also lets Encore read `.lrc` lyric files and `cover.jpg` beside the music and needs no permission. The alternative is Android's "Music and audio" permission and MediaStore.
   - Which copy plays when a song is both on the server and on the device. Recommended: the device's (no data, works offline).
4. **Not decided yet (after the owner's choices):** the call the platform makes for each file and how a changed or removed file is noticed, how a second source joins an open profile, how duplicates are matched and shown once, and the Settings and Connect screens.

## Testing
`local::tags` tests read one-second sine tones with tags (`encore-sources/tests/fixtures/local/`, made by `make.sh` with ffmpeg, so they carry no one's music): an MP3 with ID3v2 tags and a cover, an untagged MP3 with no Xing frame, a FLAC with Vorbis comments, lyrics and a picture block, an M4A and an Ogg Vorbis file.
