//! Jellyfin's lyrics JSON (`GET /Audio/{id}/Lyrics`, a `LyricDto`): Jellyfin parses
//! LRC, enhanced LRC and text files itself and sends lines with start times in
//! ticks (100 ns), plus `Cues` for word timing: character ranges of the line's
//! text with their own start and end.
//!
//! Cue positions are .NET string indexes, which count UTF-16 code units, so the
//! text is sliced in UTF-16 to keep accents and emoji in the right word.
//! Jellyfin's LRC parser does not apply `[offset:]` and does not report it; when a
//! server does report `Metadata.Offset`, it is applied once, the LRC way (positive
//! shows the lyrics earlier).

use serde::Deserialize;

use crate::model::{Format, Line, Lyrics, OPEN, Origin, SyncLevel, Word};
use crate::{LyricsError, MAX_LINES};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Dto {
    metadata: Option<Metadata>,
    lyrics: Option<Vec<DtoLine>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    offset: Option<i64>,
    is_synced: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DtoLine {
    text: Option<String>,
    start: Option<i64>,
    cues: Option<Vec<Cue>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Cue {
    position: i64,
    end_position: i64,
    start: i64,
    end: Option<i64>,
}

const TICKS_PER_MS: i64 = 10_000;

fn words_from_cues(text: &str, cues: &[Cue], offset_ms: i64) -> Vec<Word> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let len = units.len() as i64;
    let mut words: Vec<Word> = Vec::new();
    for c in cues {
        let from = c.position.clamp(0, len) as usize;
        let to = c.end_position.clamp(0, len) as usize;
        if to <= from {
            continue;
        }
        let slice = String::from_utf16_lossy(&units[from..to]);
        if slice.len() > slice.trim_start().len()
            && let Some(prev) = words.last_mut()
        {
            prev.space_after = true;
        }
        let word = slice.trim();
        if word.is_empty() {
            continue;
        }
        // A space right after the range also separates words.
        let space_after = slice.len() > slice.trim_end().len()
            || units
                .get(to)
                .is_some_and(|u| char::from_u32(u32::from(*u)).is_some_and(char::is_whitespace));
        words.push(Word {
            text: word.to_owned(),
            start_ms: (c.start / TICKS_PER_MS - offset_ms).max(0),
            end_ms: c
                .end
                .map(|e| (e / TICKS_PER_MS - offset_ms).max(0))
                .unwrap_or(OPEN),
            space_after,
        });
    }
    words
}

pub(crate) fn parse(json: &str, origin: Origin) -> Result<Lyrics, LyricsError> {
    let dto: Dto = serde_json::from_str(json)
        .map_err(|e| LyricsError::Malformed(format!("Jellyfin lyrics: {e}")))?;
    let offset_ms = dto.metadata.as_ref().and_then(|m| m.offset).unwrap_or(0) / TICKS_PER_MS;
    let synced_flag = dto.metadata.as_ref().and_then(|m| m.is_synced);
    let raw = dto.lyrics.unwrap_or_default();
    if raw.len() > MAX_LINES {
        return Err(LyricsError::TooLarge);
    }
    let any_start = raw.iter().any(|l| l.start.is_some());
    let synced = synced_flag.unwrap_or(any_start) && any_start;
    let mut lines = Vec::with_capacity(raw.len());
    for l in raw {
        let text = l.text.unwrap_or_default();
        let start = if synced {
            l.start.map(|t| (t / TICKS_PER_MS - offset_ms).max(0))
        } else {
            None
        };
        let mut line = Line::new(text.trim().to_owned(), start);
        if synced && let Some(cues) = &l.cues {
            line.words = words_from_cues(&text, cues, offset_ms);
        }
        lines.push(line);
    }
    Ok(Lyrics {
        sync: if synced {
            SyncLevel::Line
        } else {
            SyncLevel::Unsynced
        },
        origin,
        format: Format::Jellyfin,
        lines,
        agents: Vec::new(),
        gaps: Vec::new(),
        instrumental: false,
        language: None,
    })
}

/// Whether the text is Jellyfin's lyrics JSON.
pub(crate) fn looks_like(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('{') && t.contains("\"Lyrics\"")
}
