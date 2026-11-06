# Lyrics fixtures

Every lyric here was written for these tests: no line is from a real song. They exercise what the parsers in `core/encore-lyrics` must handle.

| File | What it covers |
| --- | --- |
| `duet.ttml` | Word timing; syllables of one word ("light" + "house"); two singers and a group line; background vocals; a translation and a pronunciation from the head; an inline translation; `&amp;` and `&#8217;`; a 12 s instrumental gap |
| `relative.ttml` | Word times written relative to their line; `dur` instead of `end`; a line with no words |
| `line.ttml` | `itunes:timing="Line"`: words are ignored |
| `enhanced.lrc` | Word tags, `[offset:+200]`, the `v1:`/`v2:`/`D:` duet markers, a `[bg:]` line |
| `simple.lrc` | Metadata tags, one line at two times, a blank timed line that ends the line before, an untimed credit line, CRLF line endings |
| `plain.txt` | Unsynced text with verse breaks |
| `jellyfin-cues.json` | Jellyfin's `LyricDto` with word cues over text with an accent and an emoji (UTF-16 positions), written from the Jellyfin 12.0.0 OpenAPI document (a stand-in, not a recording) |