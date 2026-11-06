//! LRC and enhanced LRC.
//!
//! - `[mm:ss.xx]text`, with several time tags on one line for a repeated line.
//! - Metadata tags (`[ar:]`, `[ti:]`, `[offset:]`, `[la:]`, ...). `[offset:+500]`
//!   shows the lyrics 500 ms earlier, as the LRC convention says.
//! - Enhanced word tags: `[00:12.00]<00:12.00>Hel<00:12.40>lo <00:12.90>world<00:13.50>`.
//! - Duets: a voice marker after the time tag, `v1:` / `v2:` (the A2 extension),
//!   or `M:` / `F:` / `D:` (D is both singers).
//! - Background vocals: a `[bg:...]` line belongs to the line before it.
//! - A file with no time tags at all is plain, unsynced text.

use crate::model::{
    Agent, AgentKind, Background, Format, Line, Lyrics, OPEN, Origin, SyncLevel, Word,
};
use crate::time;

/// The voice marker at the start of a line's text, if any, and the rest.
fn voice_marker(text: &str) -> (Option<(&'static str, AgentKind)>, &str) {
    let t = text.trim_start();
    for (marker, id, kind) in [
        ("v1:", "v1", AgentKind::Person),
        ("v2:", "v2", AgentKind::Person),
        ("v1000:", "v1000", AgentKind::Group),
        ("M:", "m", AgentKind::Person),
        ("F:", "f", AgentKind::Person),
        ("D:", "d", AgentKind::Group),
    ] {
        if let Some(rest) = t.strip_prefix(marker) {
            return (Some((id, kind)), rest);
        }
    }
    (None, text)
}

/// Split text with `<mm:ss.xx>` word tags into words. Returns the words (empty when
/// there are no tags) and the text without tags.
fn word_tags(body: &str) -> (Vec<Word>, String) {
    let mut words: Vec<Word> = Vec::new();
    let mut plain = String::new();
    let mut current: Option<i64> = None;
    let mut pending = String::new(); // text since the last tag
    let mut prefix = String::new(); // untimed text before the first tag
    let mut rest = body;
    let mut any_tag = false;

    let flush = |text: &mut String,
                 at: Option<i64>,
                 next: Option<i64>,
                 words: &mut Vec<Word>,
                 prefix: &mut String| {
        if text.is_empty() {
            return;
        }
        let Some(start) = at else {
            prefix.push_str(text);
            text.clear();
            return;
        };
        let trimmed_end = text.trim_end();
        let space_after = trimmed_end.len() < text.len();
        let leading_space = text.len() > text.trim_start().len();
        if leading_space && let Some(prev) = words.last_mut() {
            prev.space_after = true;
        }
        let word = trimmed_end.trim_start();
        if word.is_empty() {
            if let Some(prev) = words.last_mut() {
                prev.space_after |= space_after;
            }
        } else {
            let mut text_out = String::new();
            if words.is_empty() && !prefix.is_empty() {
                text_out.push_str(prefix);
                prefix.clear();
            }
            text_out.push_str(word);
            words.push(Word {
                text: text_out,
                start_ms: start,
                end_ms: next.unwrap_or(OPEN),
                space_after,
            });
        }
        text.clear();
    };

    while let Some(open) = rest.find('<') {
        let (before, after) = rest.split_at(open);
        let Some(close) = after.find('>') else { break };
        let inner = &after[1..close];
        match time::lrc(inner) {
            Some(t) => {
                any_tag = true;
                pending.push_str(before);
                plain.push_str(before);
                flush(&mut pending, current, Some(t), &mut words, &mut prefix);
                current = Some(t);
                rest = &after[close + 1..];
            }
            None => {
                // Not a time tag: a literal '<'.
                pending.push_str(&rest[..open + 1]);
                plain.push_str(&rest[..open + 1]);
                rest = &rest[open + 1..];
            }
        }
    }
    pending.push_str(rest);
    plain.push_str(rest);
    flush(&mut pending, current, None, &mut words, &mut prefix);
    if !any_tag {
        return (Vec::new(), plain);
    }
    // Untimed text at the very start with nothing timed after it.
    if !prefix.is_empty() && words.is_empty() {
        return (Vec::new(), plain);
    }
    (words, plain)
}

/// One `[key:value]` metadata tag, if the whole line is one.
fn metadata(line: &str) -> Option<(String, &str)> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = inner.split_once(':')?;
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphabetic() || c == '#') {
        return None;
    }
    Some((key.to_ascii_lowercase(), value.trim()))
}

pub(crate) fn parse(text: &str, origin: Origin) -> Lyrics {
    let mut lines: Vec<Line> = Vec::new();
    let mut untimed: Vec<Line> = Vec::new();
    let mut agents: Vec<Agent> = Vec::new();
    let mut offset_ms: i64 = 0;
    let mut language = None;
    let mut enhanced = false;

    for raw in text.lines() {
        let raw = raw.trim_end_matches('\r');
        let line = raw.trim();
        if line.is_empty() {
            untimed.push(Line::new(String::new(), None));
            continue;
        }
        // Background vocals for the line before: [bg:<00:01.00>word ...].
        if let Some(inner) = line.strip_prefix("[bg:").and_then(|s| s.strip_suffix(']')) {
            let (words, plain) = word_tags(inner);
            enhanced |= !words.is_empty();
            if let Some(prev) = lines.last_mut() {
                prev.background.push(Background {
                    text: plain.trim().to_owned(),
                    start_ms: words.first().map(|w| w.start_ms),
                    end_ms: None,
                    words,
                });
            }
            continue;
        }
        if let Some((key, value)) = metadata(line)
            && time::lrc(&format!("{key}:{value}")).is_none()
        {
            match key.as_str() {
                "offset" => offset_ms = value.trim_start_matches('+').parse().unwrap_or(0),
                "la" | "lang" | "language" if !value.is_empty() => {
                    language = Some(value.to_owned())
                }
                _ => {}
            }
            continue;
        }
        // Leading time tags.
        let mut times = Vec::new();
        let mut rest = line;
        while let Some(after) = rest.strip_prefix('[') {
            let Some(close) = after.find(']') else { break };
            match time::lrc(&after[..close]) {
                Some(t) => {
                    times.push(t);
                    rest = &after[close + 1..];
                }
                None => break,
            }
        }
        if times.is_empty() {
            untimed.push(Line::new(line.to_owned(), None));
            continue;
        }
        let (marker, body) = voice_marker(rest);
        let agent = marker.map(|(id, kind)| match agents.iter().position(|a| a.id == id) {
            Some(i) => i as u32,
            None => {
                agents.push(Agent {
                    id: id.to_owned(),
                    name: None,
                    kind,
                });
                (agents.len() - 1) as u32
            }
        });
        let (words, plain) = word_tags(body);
        enhanced |= !words.is_empty();
        let first = times[0];
        for t in times {
            let mut l = Line::new(plain.trim().to_owned(), Some(t));
            l.agent = agent;
            // A repeated line's words move with it.
            l.words = words
                .iter()
                .map(|w| Word {
                    start_ms: w.start_ms - first + t,
                    end_ms: if w.end_ms == OPEN {
                        OPEN
                    } else {
                        w.end_ms - first + t
                    },
                    ..w.clone()
                })
                .collect();
            lines.push(l);
        }
    }

    if lines.is_empty() {
        // No time tags anywhere: plain text, kept as written.
        return Lyrics {
            sync: SyncLevel::Unsynced,
            origin,
            format: Format::Plain,
            lines: untimed,
            agents: Vec::new(),
            gaps: Vec::new(),
            instrumental: false,
            language,
        };
    }

    if offset_ms != 0 {
        let shift = |t: i64| {
            if t == OPEN {
                OPEN
            } else {
                (t - offset_ms).max(0)
            }
        };
        for l in &mut lines {
            l.start_ms = l.start_ms.map(shift);
            for w in &mut l.words {
                w.start_ms = shift(w.start_ms);
                w.end_ms = shift(w.end_ms);
            }
            for b in &mut l.background {
                b.start_ms = b.start_ms.map(shift);
                for w in &mut b.words {
                    w.start_ms = shift(w.start_ms);
                    w.end_ms = shift(w.end_ms);
                }
            }
        }
    }

    Lyrics {
        sync: SyncLevel::Line,
        origin,
        format: if enhanced {
            Format::EnhancedLrc
        } else {
            Format::Lrc
        },
        lines,
        agents,
        gaps: Vec::new(),
        instrumental: false,
        language,
    }
}

/// The metadata tags LRC writers use; a file with only these has no lyrics.
const KNOWN_TAGS: [&str; 13] = [
    "ar", "ti", "al", "au", "by", "offset", "length", "re", "ve", "tool", "la", "lang", "#",
];

/// Whether the text looks like LRC: a line starting with a time tag, or a known
/// metadata tag (so a file of tags alone reads as LRC with no lyrics).
pub(crate) fn looks_like(text: &str) -> bool {
    text.lines().take(200).any(|l| {
        let l = l.trim();
        let timed = l
            .strip_prefix('[')
            .and_then(|s| s.find(']').map(|c| time::lrc(&s[..c]).is_some()))
            .unwrap_or(false);
        timed || metadata(l).is_some_and(|(k, _)| KNOWN_TAGS.contains(&k.as_str()))
    })
}
