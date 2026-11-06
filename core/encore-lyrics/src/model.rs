//! The one lyrics model every format maps into (plan section 4, board 10).
//!
//! Word timing is optional everywhere: a line has words when its source timed
//! them, and lights up whole when it didn't. Nothing here says where word timings
//! came from beyond `origin` and `confidence`, so a later source (a word-timed file,
//! or timings generated on the companion server) fills the same fields.
//!
//! Times are milliseconds from the start of the song, never negative.

use serde::{Deserialize, Serialize};

/// How finely the lyrics are timed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncLevel {
    /// At least one line has per-word timing (the rest light up line by line).
    Word,
    /// Lines have start times; no words are timed.
    Line,
    /// No timing: a plain page that scrolls, with a "Not synced" tag.
    Unsynced,
}

/// Where the lyrics came from (plan section 4, item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// The media server (Jellyfin reads `.lrc` sidecars and embedded tags itself).
    MediaServer,
    EmbeddedTag,
    Sidecar,
    /// An online provider, through the companion server, if the user turned it on.
    Provider,
    /// Word timings made by the companion server from line-synced lyrics.
    Generated,
}

/// The format the lyrics were read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// Jellyfin's own parsed JSON (`/Audio/{id}/Lyrics`).
    Jellyfin,
    Lrc,
    /// LRC with `<mm:ss.xx>` word tags.
    EnhancedLrc,
    Ttml,
    Plain,
}

/// Which side a line sits on. A second singer's lines align to the opposite side
/// (board 10, item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Main,
    Opposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentKind {
    Person,
    /// Several singers at once (a duet's shared lines).
    Group,
    Other,
}

/// A singer, as the source names them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    /// The source's ID ("v1", "v2" in TTML and A2 LRC).
    pub id: String,
    pub name: Option<String>,
    pub kind: AgentKind,
}

/// One timed piece of a line: a word or a syllable. `space_after` says whether a
/// space follows it, so syllables of one word join up: the line's text is every
/// word's text, with a space after those that have one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub space_after: bool,
}

/// Background vocals sung under a line: smaller and dimmer (board 10, item 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub text: String,
    /// None only when the lyrics are unsynced.
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    /// Empty when the line is timed as a whole.
    pub words: Vec<Word>,
    /// Index into `Lyrics::agents`.
    pub agent: Option<u32>,
    pub side: Side,
    pub background: Vec<Background>,
    /// How sure the timing is, 0 to 1. 1 for anything read from a file or a
    /// server; lower only for generated timings, which are used only when high.
    pub confidence: f32,
    pub translation: Option<String>,
    pub pronunciation: Option<String>,
}

/// A stretch with no singing long enough for the breathing dots (board 10, item 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gap {
    /// The line the gap follows; None for a gap before the first line.
    pub after_line: Option<u32>,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub sync: SyncLevel,
    pub origin: Origin,
    pub format: Format,
    pub lines: Vec<Line>,
    pub agents: Vec<Agent>,
    pub gaps: Vec<Gap>,
    /// The source says the song is instrumental: show the dots and the title only.
    pub instrumental: bool,
    pub language: Option<String>,
}

impl Line {
    pub(crate) fn new(text: String, start_ms: Option<i64>) -> Line {
        Line {
            text,
            start_ms,
            end_ms: None,
            words: Vec::new(),
            agent: None,
            side: Side::Main,
            background: Vec::new(),
            confidence: 1.0,
            translation: None,
            pronunciation: None,
        }
    }
}

/// Join words into the text they spell.
pub(crate) fn words_text(words: &[Word]) -> String {
    let mut s = String::new();
    for w in words {
        s.push_str(&w.text);
        if w.space_after {
            s.push(' ');
        }
    }
    s.trim_end().to_owned()
}

/// A time not known yet; `finish` replaces it.
pub(crate) const OPEN: i64 = -1;
