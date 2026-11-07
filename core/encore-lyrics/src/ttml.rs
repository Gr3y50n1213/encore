//! TTML lyrics: word timing, singers and background vocals.
//!
//! - `<tt itunes:timing="Word|Line|None">`; `<ttm:agent xml:id="v1" type="person">`
//!   with an optional `<ttm:name>` in the head.
//! - One `<p begin end ttm:agent>` per line; `<span begin end>` per word or syllable.
//!   Whitespace between spans separates words; spans with nothing between them are
//!   syllables of one word.
//! - `<span ttm:role="x-bg">` holds background vocals, with its own word spans.
//! - Translations and pronunciations: inline spans with `ttm:role="x-translation"`
//!   or `"x-roman"`, or head `<translation>` / `<transliteration>` entries whose
//!   `<text for="L1">` matches a line's `itunes:key="L1"`.
//! - Times are absolute in lyric files. When every word of a line starts before the
//!   line itself, the file wrote them relative to the line (as TTML allows), and
//!   they are moved.
//!
//! Attributes are matched by local name, so any prefix works.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::model::{
    Agent, AgentKind, Background, Format, Line, Lyrics, OPEN, Origin, SyncLevel, Word, words_text,
};
use crate::{LyricsError, MAX_LINES, time};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ctx {
    Other,
    AgentName(usize),
    Line,
    Word,
    Background,
    BackgroundWord,
    Translation,
    Pronunciation,
    HeadText,
}

#[derive(Default)]
struct WordBuild {
    text: String,
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(Default)]
struct WordsBuild {
    words: Vec<Word>,
    /// Untimed text waiting for the next word.
    prefix: String,
    /// Everything as written, for a line without word timing.
    plain: String,
}

impl WordsBuild {
    fn loose(&mut self, text: &str) {
        self.plain.push_str(text);
        if text.trim().is_empty() {
            if let Some(w) = self.words.last_mut() {
                w.space_after = true;
            } else if !self.prefix.is_empty() {
                self.prefix.push(' ');
            }
            return;
        }
        let leading = text.len() > text.trim_start().len();
        let trailing = text.len() > text.trim_end().len();
        let core = collapse(text.trim());
        match self.words.last_mut() {
            Some(w) if !leading && !w.space_after => {
                w.text.push_str(&core);
                w.space_after = trailing;
            }
            Some(w) => {
                w.space_after = true;
                self.prefix.push_str(&core);
                if trailing {
                    self.prefix.push(' ');
                }
            }
            None => {
                self.prefix.push_str(&core);
                if trailing {
                    self.prefix.push(' ');
                }
            }
        }
    }

    fn word(&mut self, w: WordBuild) {
        self.plain.push_str(&w.text);
        let Some(start) = w.start else {
            self.loose(&w.text);
            return;
        };
        if w.text.len() > w.text.trim_start().len()
            && let Some(prev) = self.words.last_mut()
        {
            prev.space_after = true;
        }
        let mut text = std::mem::take(&mut self.prefix);
        text.push_str(&collapse(w.text.trim()));
        if text.trim().is_empty() {
            return;
        }
        self.words.push(Word {
            text: text.trim_start().to_owned(),
            start_ms: start,
            end_ms: w.end.unwrap_or(OPEN),
            space_after: w.text.len() > w.text.trim_end().len(),
        });
    }

    fn text(&self) -> String {
        if self.words.is_empty() {
            return collapse(self.plain.trim());
        }
        let mut t = words_text(&self.words);
        if !self.prefix.trim().is_empty() {
            t.push(' ');
            t.push_str(self.prefix.trim());
        }
        t
    }
}

struct LineBuild {
    begin: Option<i64>,
    end: Option<i64>,
    agent: Option<String>,
    key: Option<String>,
    words: WordsBuild,
    background: Vec<Background>,
    bg: Option<(Option<i64>, Option<i64>, WordsBuild)>,
    current: Option<WordBuild>,
    translation: String,
    pronunciation: String,
}

/// Runs of whitespace (newlines in the file included) become one space.
fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An element's attributes by local name. A short list, not a map: elements have
/// a handful, and a word-timed song has thousands of elements.
struct Attrs {
    list: Vec<(String, String)>,
}

impl Attrs {
    fn of(e: &BytesStart<'_>) -> Result<Attrs, LyricsError> {
        let mut list = Vec::new();
        for a in e.attributes() {
            let a = a.map_err(|e| LyricsError::Malformed(format!("attribute: {e}")))?;
            let local = a.key.local_name();
            let value = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| LyricsError::Malformed(format!("attribute value: {e}")))?;
            list.push((local.as_ref().to_owned(), value.into_owned()));
        }
        Ok(Attrs { list })
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.list
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// `begin` and `end` (or `begin` + `dur`).
    fn times(&self) -> (Option<i64>, Option<i64>) {
        let begin = self.get("begin").and_then(time::ttml);
        let end = self
            .get("end")
            .and_then(time::ttml)
            .or_else(|| Some(begin? + self.get("dur").and_then(time::ttml)?));
        (begin, end)
    }
}

fn shift_words(words: &mut [Word], by: i64) {
    for w in words {
        w.start_ms += by;
        if w.end_ms != OPEN {
            w.end_ms += by;
        }
    }
}

pub(crate) fn parse(text: &str, origin: Origin) -> Result<Lyrics, LyricsError> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text_start = false;
    reader.config_mut().trim_text_end = false;

    let mut stack: Vec<Ctx> = Vec::new();
    let mut saw_tt = false;
    let mut timing: Option<String> = None;
    let mut language: Option<String> = None;
    let mut agents: Vec<Agent> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut line: Option<LineBuild> = None;
    let mut head_text: HashMap<(bool, String), String> = HashMap::new();
    let mut head_kind: Option<bool> = None; // Some(true): translations, Some(false): transliterations
    let mut head_key: Option<String> = None;

    loop {
        let event = reader.read_event().map_err(|e| {
            LyricsError::Malformed(format!("{e} (at byte {})", reader.error_position()))
        })?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                let name = e.local_name();
                let name = name.as_ref();
                let attrs = Attrs::of(e)?;
                let top = stack.last().copied().unwrap_or(Ctx::Other);
                let ctx = match name {
                    "tt" => {
                        saw_tt = true;
                        timing = attrs.get("timing").map(str::to_ascii_lowercase);
                        language = attrs.get("lang").map(str::to_owned);
                        Ctx::Other
                    }
                    "agent" if line.is_none() => {
                        let kind = match attrs.get("type") {
                            Some("group") => AgentKind::Group,
                            Some("other") => AgentKind::Other,
                            _ => AgentKind::Person,
                        };
                        match attrs.get("id") {
                            Some(id) => {
                                agents.push(Agent {
                                    id: id.to_owned(),
                                    name: None,
                                    kind,
                                });
                                Ctx::Other
                            }
                            None => Ctx::Other,
                        }
                    }
                    "name" if !agents.is_empty() && line.is_none() => {
                        Ctx::AgentName(agents.len() - 1)
                    }
                    "translation" | "translations" => {
                        head_kind = Some(true);
                        Ctx::Other
                    }
                    "transliteration" | "transliterations" => {
                        head_kind = Some(false);
                        Ctx::Other
                    }
                    "text" if line.is_none() && head_kind.is_some() => {
                        head_key = attrs.get("for").map(str::to_owned);
                        Ctx::HeadText
                    }
                    "p" => {
                        if lines.len() >= MAX_LINES {
                            return Err(LyricsError::TooLarge);
                        }
                        let (begin, end) = attrs.times();
                        line = Some(LineBuild {
                            begin,
                            end,
                            agent: attrs.get("agent").map(str::to_owned),
                            key: attrs.get("key").map(str::to_owned),
                            words: WordsBuild::default(),
                            background: Vec::new(),
                            bg: None,
                            current: None,
                            translation: String::new(),
                            pronunciation: String::new(),
                        });
                        Ctx::Line
                    }
                    "span" if line.is_some() => {
                        let l = line.as_mut().expect("checked");
                        let (begin, end) = attrs.times();
                        match attrs.get("role") {
                            Some("x-bg") if top == Ctx::Line => {
                                l.bg = Some((begin, end, WordsBuild::default()));
                                Ctx::Background
                            }
                            Some("x-translation") => Ctx::Translation,
                            Some("x-roman" | "x-pronunciation" | "x-transliteration") => {
                                Ctx::Pronunciation
                            }
                            _ if matches!(top, Ctx::Line | Ctx::Background) => {
                                l.current = Some(WordBuild {
                                    text: String::new(),
                                    start: begin,
                                    end,
                                });
                                if top == Ctx::Background {
                                    Ctx::BackgroundWord
                                } else {
                                    Ctx::Word
                                }
                            }
                            _ => top,
                        }
                    }
                    "br" if line.is_some() => {
                        if let Some(l) = line.as_mut() {
                            l.words.loose(" ");
                        }
                        Ctx::Other
                    }
                    _ => match top {
                        // Unknown markup inside a word, a line or a head text keeps
                        // its text there.
                        Ctx::Word
                        | Ctx::BackgroundWord
                        | Ctx::Line
                        | Ctx::Background
                        | Ctx::HeadText
                        | Ctx::Translation
                        | Ctx::Pronunciation => top,
                        _ => Ctx::Other,
                    },
                };
                if !empty {
                    stack.push(ctx);
                } else if matches!(ctx, Ctx::Word | Ctx::BackgroundWord) {
                    // `<span begin end/>`: a timed word with no text adds nothing.
                    if let Some(l) = line.as_mut() {
                        l.current = None;
                    }
                }
            }
            Event::End(ref e) => {
                let ctx = stack.pop().unwrap_or(Ctx::Other);
                let name = e.local_name();
                let name = name.as_ref();
                match (ctx, name) {
                    (Ctx::Word, "span") => {
                        if let Some(l) = line.as_mut()
                            && let Some(w) = l.current.take()
                        {
                            l.words.word(w);
                        }
                    }
                    (Ctx::BackgroundWord, "span") => {
                        if let Some(l) = line.as_mut()
                            && let Some(w) = l.current.take()
                            && let Some((_, _, b)) = l.bg.as_mut()
                        {
                            b.word(w);
                        }
                    }
                    (Ctx::Background, "span") => {
                        if let Some(l) = line.as_mut()
                            && let Some((begin, end, b)) = l.bg.take()
                        {
                            l.background.push(Background {
                                text: b.text(),
                                start_ms: begin,
                                end_ms: end,
                                words: b.words,
                            });
                        }
                    }
                    (Ctx::Line, "p") => {
                        if let Some(l) = line.take() {
                            lines.push(finish_line(l, &mut agents));
                        }
                    }
                    (Ctx::HeadText, "text") => head_key = None,
                    (_, "translations" | "transliterations") => head_kind = None,
                    _ => {}
                }
            }
            Event::Text(t) => add_text(
                &t.xml10_content(),
                &stack,
                &mut line,
                &mut agents,
                head_kind,
                &head_key,
                &mut head_text,
            ),
            Event::CData(t) => add_text(
                &t.xml10_content(),
                &stack,
                &mut line,
                &mut agents,
                head_kind,
                &head_key,
                &mut head_text,
            ),
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => match r.xml10_content().as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "apos" => "'".into(),
                        "quot" => "\"".into(),
                        "nbsp" => "\u{a0}".into(),
                        _ => String::new(),
                    },
                };
                add_text(
                    &resolved,
                    &stack,
                    &mut line,
                    &mut agents,
                    head_kind,
                    &head_key,
                    &mut head_text,
                );
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_tt {
        return Err(LyricsError::Malformed(
            "not a TTML document (no <tt>)".into(),
        ));
    }

    for l in &mut lines {
        // A key placeholder from finish_line: look the line up in the head.
        let key = l
            .translation
            .as_deref()
            .and_then(|t| t.strip_prefix('\u{1}'))
            .map(str::to_owned);
        if let Some(key) = key {
            l.translation = head_text.get(&(true, key.clone())).map(|s| collapse(s));
            if l.pronunciation.is_none() {
                l.pronunciation = head_text.get(&(false, key)).map(|s| collapse(s));
            }
        }
    }

    match timing.as_deref() {
        Some("none") => {
            for l in &mut lines {
                l.start_ms = None;
                l.end_ms = None;
                l.words.clear();
                for b in &mut l.background {
                    b.words.clear();
                    b.start_ms = None;
                    b.end_ms = None;
                }
            }
        }
        Some("line") => {
            for l in &mut lines {
                l.words.clear();
                for b in &mut l.background {
                    b.words.clear();
                }
            }
        }
        _ => {}
    }

    Ok(Lyrics {
        sync: SyncLevel::Line,
        origin,
        format: Format::Ttml,
        lines,
        agents,
        gaps: Vec::new(),
        instrumental: false,
        language,
    })
}

fn add_text(
    text: &str,
    stack: &[Ctx],
    line: &mut Option<LineBuild>,
    agents: &mut [Agent],
    head_kind: Option<bool>,
    head_key: &Option<String>,
    head_text: &mut HashMap<(bool, String), String>,
) {
    match stack.last().copied().unwrap_or(Ctx::Other) {
        Ctx::AgentName(i) => {
            let name = agents[i].name.get_or_insert_with(String::new);
            name.push_str(text);
        }
        Ctx::HeadText => {
            if let (Some(kind), Some(key)) = (head_kind, head_key) {
                head_text
                    .entry((kind, key.clone()))
                    .or_default()
                    .push_str(text);
            }
        }
        ctx => {
            let Some(l) = line.as_mut() else { return };
            match ctx {
                Ctx::Word | Ctx::BackgroundWord => {
                    if let Some(w) = l.current.as_mut() {
                        w.text.push_str(text);
                    }
                }
                Ctx::Background => {
                    if let Some((_, _, b)) = l.bg.as_mut() {
                        b.loose(text);
                    }
                }
                Ctx::Translation => l.translation.push_str(text),
                Ctx::Pronunciation => l.pronunciation.push_str(text),
                Ctx::Line => l.words.loose(text),
                _ => {}
            }
        }
    }
}

fn finish_line(mut l: LineBuild, agents: &mut Vec<Agent>) -> Line {
    // Relative times: every word starts before the line does.
    if let Some(pb) = l.begin
        && pb > 0
    {
        let words = &l.words.words;
        let all_before = !words.is_empty() && words.iter().all(|w| w.start_ms < pb);
        let fits = match l.end {
            Some(pe) => words
                .iter()
                .map(|w| {
                    if w.end_ms == OPEN {
                        w.start_ms
                    } else {
                        w.end_ms
                    }
                })
                .max()
                .is_some_and(|m| m <= pe - pb + 100),
            None => true,
        };
        if all_before && fits {
            shift_words(&mut l.words.words, pb);
            for b in &mut l.background {
                shift_words(&mut b.words, pb);
                b.start_ms = b.start_ms.map(|t| t + pb);
                b.end_ms = b.end_ms.map(|t| t + pb);
            }
        }
    }
    let agent = l
        .agent
        .as_deref()
        .map(|id| match agents.iter().position(|a| a.id == id) {
            Some(i) => i as u32,
            None => {
                agents.push(Agent {
                    id: id.to_owned(),
                    name: None,
                    kind: AgentKind::Person,
                });
                (agents.len() - 1) as u32
            }
        });
    let mut out = Line::new(l.words.text(), l.begin);
    out.end_ms = l.end;
    out.words = l.words.words;
    out.agent = agent;
    out.background = l.background;
    let inline_translation = collapse(l.translation.trim());
    out.translation = if !inline_translation.is_empty() {
        Some(inline_translation)
    } else {
        // Looked up in the head once the whole document is read.
        l.key.map(|k| format!("\u{1}{k}"))
    };
    let pronunciation = collapse(l.pronunciation.trim());
    out.pronunciation = (!pronunciation.is_empty()).then_some(pronunciation);
    out
}

/// Whether the text is a TTML document.
pub(crate) fn looks_like(text: &str) -> bool {
    let head: String = text.chars().take(2048).collect();
    head.trim_start().starts_with('<') && head.contains("<tt")
}
