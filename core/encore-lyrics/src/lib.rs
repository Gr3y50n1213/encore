//! Lyrics in every format the plan names (LRC, enhanced LRC, TTML, plain text, and
//! Jellyfin's own JSON) read into one model with optional per-word timing, singers,
//! background vocals and the gaps for the breathing dots (plan section 4, board 10).
//!
//! Pure: no network, no clock, no files. Every parser returns a model or an error
//! and never panics, whatever the input (tests feed it garbage, truncated files and
//! huge ones). See docs/adr/core-0006-lyrics.md.

#![forbid(unsafe_code)]

mod finish;
mod jellyfin;
mod lrc;
mod model;
mod time;
mod ttml;

pub use finish::Rules;
pub use model::{
    Agent, AgentKind, Background, Format, Gap, Line, Lyrics, Origin, Side, SyncLevel, Word,
};

/// Larger than any real lyrics file (a long song's word-timed TTML is under 200 KB).
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
/// More lines than any song has.
pub const MAX_LINES: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LyricsError {
    #[error("the lyrics file is too large")]
    TooLarge,
    #[error("the lyrics file couldn't be read: {0}")]
    Malformed(String),
}

/// Read lyrics in whatever format `text` is in: TTML, Jellyfin JSON, LRC (plain or
/// enhanced), else plain text. Returns None when there are no lyrics in it.
pub fn parse(text: &str, origin: Origin, rules: &Rules) -> Result<Option<Lyrics>, LyricsError> {
    if text.len() > MAX_BYTES {
        return Err(LyricsError::TooLarge);
    }
    // One kind of line break: Windows (CRLF) and old Mac (CR) files read the same.
    let normalized;
    let mut text = text.trim_start_matches('\u{feff}');
    if text.contains('\r') {
        normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        text = &normalized;
    }
    let raw = if ttml::looks_like(text) {
        ttml::parse(text, origin)?
    } else if jellyfin::looks_like(text) {
        jellyfin::parse(text, origin)?
    } else if lrc::looks_like(text) {
        lrc::parse(text, origin)
    } else {
        plain(text, origin)
    };
    if raw.lines.len() > MAX_LINES {
        return Err(LyricsError::TooLarge);
    }
    let finished = finish::finish(raw, rules);
    if finished.lines.is_empty() && !finished.instrumental {
        return Ok(None);
    }
    Ok(Some(finished))
}

/// Plain text: one line per line, unsynced.
fn plain(text: &str, origin: Origin) -> Lyrics {
    Lyrics {
        sync: SyncLevel::Unsynced,
        origin,
        format: Format::Plain,
        lines: text
            .lines()
            .map(|l| model::Line::new(l.trim().to_owned(), None))
            .collect(),
        agents: Vec::new(),
        gaps: Vec::new(),
        instrumental: false,
        language: None,
    }
}

#[cfg(test)]
mod tests;
