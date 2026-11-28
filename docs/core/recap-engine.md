# Year recap (Phase 11, engine only)

The engine's input and output, field by field. The core's calls that use it are in CORE-API.md, "Year recap" (API 12). Design: plan sections 10 and 25, boards 39 to 41, ADR core-0025.

## Calls (crate `encore-recap`)

| Call | What it does |
| --- | --- |
| `build_recap(&RecapInput) -> Result<RecapData, RecapError>` | Builds one profile's recap for one year. Pure and deterministic: the same input gives the same bytes on every platform. Run it in the background; it never blocks playback. |
| `build_recap_json(&str) -> Result<String, RecapError>` | The same, from camelCase JSON input to the canonical JSON output. |
| `RecapData::to_json() -> String` | The canonical bytes (compact, fields in a fixed order). Cache and compare these. |
| `daily_rollups(&[Play]) -> Vec<DailyRollup>` | Per local day: total, per raw song key, per artist key and per hour. Rebuildable from the log at any time; keep none when Recap is off. |

`RecapError` is `BadDay` (a `clearedSince` or `joinedDay` that is not `YYYY-MM-DD`), `BadYear` or `Json`. Thin or missing data is never an error: it gives a shorter recap.

`SCHEMA` (1) is the document's schema version; `ENGINE` (1) is the rules version, bumped whenever a rule changes the bytes.

## Input: `RecapInput` (camelCase JSON)

| Field | Type | Meaning |
| --- | --- | --- |
| `year` | i32 | The recap year. |
| `profileId` | string | Seeds the pack, layouts and words; never written to the output. |
| `kidProfile` | bool | Drops explicit plays and the late-night beats, metric and words. |
| `recapOff` | bool | Kind `none`, nothing counted. |
| `countedThroughUtcMs` | i64 | The build's "now": later plays are not counted; sets `soFar` and `locked`. |
| `clearedSince` | "YYYY-MM-DD"? | The local day of a Clear listening history. Earlier plays are not counted; the story says "Since 3 Aug". |
| `joinedDay` | "YYYY-MM-DD"? | The day the profile was made, for "Since 14 Jun". |
| `packs` | [{id, version}] | Packs every device of the profile from the last 90 days has. |
| `past` | [{year, pack?, leadHue?, listenedMs?, personalityMetric?, personalityValue?}] | Archived recaps of earlier years (fill it from their documents: `pack.id`, `palette.leadHue`, `totals.listenedMs`, the personality beat's `metric` and `value`). |
| `albumPalettes` | {albumKey: ["#rrggbb"]} | The core's cover palettes, most prominent first; only the top three albums are read. |
| `albumTrackCounts` | {albumKey: n} | For "played start to finish". |
| `fingerprints` | {songKey: {energy, valence}} | Permille, from the sound analysis; for the mood arc and two year-line traits. |
| `artistsBefore` | [{artistKey, plays, lastPlayedUtcMs?}] | Artists heard before this year (for discovery and comeback), merged with any earlier-year plays in `plays`. |
| `hemisphere` | "north" \| "south" | For the seasons. |
| `plays` | [Play] | The log, any order. |

`Play`: `songKey`, `title`, `artistKey`, `artist`, `albumKey?`, `album?`, `startedUtcMs`, `utcOffsetMinutes`, `listenedMs`, `durationMs?`, `genre?`, `explicit` (default false), `endReason?` (the store's values), `startedFrom?`, `lyricJumps` ([{lineIndex, lineStartMs}]). These map one to one onto `PlayEventRecord` and `LyricJumpRecord` in `encore-store` (the artist key is the caller's normalized artist name; the genre and explicit flag come from the library).

## Output: `RecapData` (camelCase JSON, under 100 KB)

| Field | Meaning |
| --- | --- |
| `schema`, `engine` | 1 and 1. |
| `year`, `kind` | `none`, `short` (three cards) or `full`. |
| `noneReason` | `off` or `notEnoughPlays` (only with kind `none`). |
| `soFar` | Built before 31 December ended everywhere: show the "So far" tag. |
| `locked` | Built on or after 8 January: archive it and stop building this year. |
| `countedThroughUtcMs` | Shown on every card. |
| `since` | {day, reason: `cleared` \| `joined`}, when counting started inside the year. |
| `pack` | {id, version}; null with kind `none`. An app without it draws the pack's fallback with "Shown in a simpler look on this device". |
| `seed` | 16 hex digits. |
| `palette` | {source: `albums` \| `seed`, colours, lead, leadHue, hueGapFromLastYear}. The pack fits these to its ground and checks contrast. |
| `yearLine` | "Late nights, loud choruses." (full story only). |
| `totals` | plays, listenedMs, minutes, songs, artists, albums, activeDays. |
| `beats[]` | In story order. Each: `id`, `role` (opener, bigNumbers, turn, surprise, identity, close), `fixed`, `seconds`, `layout` of `layoutCount`, `shapeSeed`, `score` (rotating beats), `template` (id#variant), `text`, `data` (tagged by `type`, the numbers the card draws), `math` ({counts: [{label, value, unit}], rule}). |
| `detail` | Top 50 songs, artists and albums, months, every personality metric (full story only). |
| `logHash` | SHA-256 of the plays used; two builds from the same log have the same hash. |

Beat ids: `intro`, `minutes`, `topSong`, `topArtist`, `topAlbum`, `yourSound`, `personality`, `closer` (fixed); `listeningClock`, `biggestDiscovery`, `comeback`, `obsessionWeek`, `longestStreak`, `songOfEachSeason`, `mostReplayedDay`, `moodArc`, `mostSungLine` (rotating). The household deep cut is not built.

Units in `math.counts`: `plays`, `minutes`, `days`, `count`, `permille`. A permille shown as a percent is rounded half up.

## For the lead: wiring notes
- **Server:** run on 1 December, 1 January and on new plays until 8 January; store the bytes of `to_json()` per profile and year; serve them at `GET /recap/{year}`.
- **Devices:** the same call when the server is off; the newest `countedThroughUtcMs` wins. Pass the same `packs` list the server uses, or the pack can differ.
- **"N more plays since you last looked":** compare `totals.plays` with the cached document.
- **Parity:** the golden inputs in `core/encore-recap/tests/golden/` are the parity fixtures; every platform's build and the server must give the expected bytes.
