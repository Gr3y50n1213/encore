# Encore export format, version 1

`export_profile` writes this file (board 19, item 5). It is plain UTF-8 JSON, one object, so any tool can read it without Encore. It holds everything a profile keeps about its listener. It never holds a token or a password: those live in the device's keychain, not the profile's database.

Times are milliseconds since 1970 in UTC (`*_utc_ms`). A field the profile doesn't know is `null`, never left out. New versions add fields and never rename or remove one; a reader ignores fields it doesn't know.

## The object

| Field | What it holds |
| --- | --- |
| `format` | Always `"encore-export"`. |
| `version` | `1`. |
| `exported_utc_ms` | When the file was made. |
| `app` | The core that made it, such as `"Encore core 0.1.0"`. |
| `profile` | `id`, and `name` (the user name on the first server). |
| `settings` | The profile's settings, a key to text value. |
| `servers` | The media servers added: `kind` (`"jellyfin"`), `name`, `base_url`, `server_id`, `server_version`, `user_name`, `last_sync_utc_ms`, `added_utc_ms`. |
| `likes` | Oldest first: `song_key`, `liked_utc_ms`, and the song as the library has it now: `title`, `artist`, `album`, `musicbrainz_recording`. Those four are `null` for a song no longer in the library; the like is kept. |
| `plays` | The play log, oldest first (below). |
| `lyric_jumps` | Lyric lines tapped: `play_id` (a `plays[].id`), `line_index`, `line_start_ms`, `at_utc_ms`. |
| `recent_searches` | Newest first: `query`, `used_utc_ms`. |
| `playlists` | By name: `name`, `server_item_id` (`null` for one made offline and not yet sent), `added_utc_ms`, and `songs` in order, each with `server_item_id`, `song_key`, `title`, `artist`, `album` (the last four `null` when the song left the library). |
| `recaps` | Empty until recaps exist (Phase 11). |

## A play

| Field | What it holds |
| --- | --- |
| `id` | A ULID, unique across devices. |
| `device_id`, `hlc` | The device that played it and its hybrid logical clock, for merging logs. |
| `song_key` | The MusicBrainz recording ID when tagged, otherwise artist, title and duration (PLAN-PRODUCT section 8). |
| `title`, `artist`, `album`, `duration_ms` | The song as it was when played. |
| `started_utc_ms`, `utc_offset_minutes` | When it started, and the clock's offset from UTC there and then. |
| `listened_ms` | How much was actually heard. |
| `end_reason` | `finished`, `skipped`, `pausedAbandoned`, `interrupted`, `error`, `endedBySystem`, or `null` while playing. |
| `signal` | How a skip counts for smart picks (PLAN-PRODUCT section 21), one of `finished`, `skippedLate`, `skippedMid`, `skippedEarly`, `skippedUserPick`, `interruption`, `error`, `accident`, `tooSoon`, `system`, `pausedAbandoned`; `null` when not classified (plays from before Phase 3, or still playing). |
| `origin` | What skipped it: `button`, `swipe`, `headset`, `car`, `lockScreen`, `keyboard`, `auto`, or `null`. |
| `seeks` | How many times the listener moved within the song. |
| `started_from`, `context_id`, `chosen_by` | Where playback started (`album`, `playlist`, `artist`, `songs`, `genre`, `search`, `radio`, `mix`, `autoplay`, `queue`, `history`, `other`), that item's ID, and whether the listener (`user`) or the app (`app`) chose the song. |
| `source_kind`, `source_item_id` | The server kind and the song's ID there. |

## Reading it

The file is written in sections, one row per line, so a long history can be read as a stream. It is written to `<name>.partial` and renamed when complete, so a file with the right name is always whole.
