# ADR core-0006: One lyrics model, word timing ready

Date: 7 November 2026
Status: accepted

## Context
Plan section 4 asks for LRC, enhanced LRC and TTML read into one model: lines with start and end times, optional per-word timing, a singer per line, and background vocals. It also asks that word timing from other sources can be added later without the UI knowing where it came from.

Board 10 adds:
- word-by-word fill, with lines lit whole where only lines are timed;
- duet lines aligned to the other side;
- background vocals;
- breathing dots for gaps over 5 s;
- three states: synced by word, synced by line, unsynced.

The owner's library has no word-timed lyrics today (the Phase 3 recording: 40 of 40 songs line-synced, none with word cues). Word timing must still work end to end, so a later update only has to supply the timings.

Jellyfin 12 doesn't send lyric files. It sends its own parsed JSON: lines with a start in ticks, and `Cues` that are character ranges of the line with their own times. Its LRC parser neither applies nor reports `[offset:]`; I read `LrcLyricParser.cs` to confirm this.

## Decision
- **A pure crate.** `encore-lyrics` has no network, no clock and no files.
- **One model:**
  - lines, each with optional words, a singer index and a side;
  - background vocals with their own words;
  - a confidence (1 for anything from a file or a server);
  - optional translation and pronunciation;
  - the gaps, an instrumental flag, a sync level, the origin (media server, embedded tag, sidecar, provider, generated) and the format.
- **Words.** A word's text plus `space_after`. Syllables of one word have no space between them, and the line's text is always its words joined that way, so no platform needs character offsets in its own string encoding.
- **Parsers:**
  - TTML through quick-xml 0.42.0 (MIT, approved), matching attributes by local name. It handles agents, `x-bg`, inline and head translations, `itunes:timing`, `dur`, and times written relative to their line;
  - enhanced LRC and LRC: offset, repeated time tags, A2 voice markers, `[bg:]`, CR, CRLF and LF;
  - plain text;
  - Jellyfin JSON, with cues sliced as UTF-16, the way .NET counts them.
- **Shared rules, decided in the core:** end times, the last line never past the song, blank timed lines as line ends, sides (the first person main, other people opposite, groups main), gaps (`lyrics.instrumentalGapSeconds`), instrumental, and the sync level (word when at least one line has words).
- **Hardening.** Every parser returns a model or an error and never panics. Inputs are capped at 4 MB and 20,000 lines.
- **The API.** `lyrics(song_id)` keeps the parsed model per song, so lyrics work offline once fetched. It fetches again after `lyrics.refreshAfterDays` (7), or when the song's lyrics flag or length changes. When the server can't be reached it uses the kept copy, and an unreadable file counts as no lyrics.

## Alternatives considered
- **ttml_processor** (named in the plan): its newest version was 6 days old with about 3,600 downloads, and it brings four more crates on top of quick-xml. Its model is its own, so we would map twice.
- **Character offsets for words:** Swift, Kotlin and Dart index strings differently. Words with `space_after` avoid that.
- **Asking Jellyfin for the raw file:** there is no such endpoint; its JSON is what we get.

## Consequences
- Word-by-word lyrics need no core change, whether they come from a word-timed file, a provider or the companion server's forced alignment. Only the source changes, through `origin` and `confidence`.
- The unverified case: a server that reports `Metadata.Offset`. It is applied once, the LRC way. Nothing in the owner's library exercises it.
- Fixtures are original text written for the tests (`tools/fixtures/lyrics`); no copyrighted lyrics are committed.
