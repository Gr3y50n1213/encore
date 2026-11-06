//! The decisions every format shares, made once here so no platform has to:
//! line and word end times, the gaps that get the breathing dots, duet sides, the
//! sync level and the instrumental flag (board 10).

use crate::model::{AgentKind, Gap, Line, Lyrics, OPEN, Side, SyncLevel, Word, words_text};

/// Starting values from `design/tokens.json` (`lyrics`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// A silence at least this long between lines gets the breathing dots.
    pub instrumental_gap_ms: i64,
    /// How long the last line stays lit when nothing says when it ends.
    pub last_line_ms: i64,
    /// The song's length, when known: the last line never runs past it.
    pub song_duration_ms: Option<i64>,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            instrumental_gap_ms: 5_000,
            last_line_ms: 5_000,
            song_duration_ms: None,
        }
    }
}

fn is_instrumental_marker(text: &str) -> bool {
    let t: String = text
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    t == "instrumental"
}

fn is_blank(line: &Line) -> bool {
    line.text.trim().is_empty() && line.words.is_empty() && line.background.is_empty()
}

/// Give every word an end and keep it after its start: an open end runs to the
/// next word's start, the last one to `end`.
fn close_words(words: &mut [Word], end: i64) {
    for i in 0..words.len() {
        let next = words.get(i + 1).map(|w| w.start_ms).unwrap_or(end);
        let w = &mut words[i];
        if w.end_ms == OPEN || w.end_ms < w.start_ms {
            w.end_ms = next.max(w.start_ms);
        }
    }
}

pub(crate) fn finish(mut l: Lyrics, rules: &Rules) -> Lyrics {
    for line in &mut l.lines {
        line.text = line.text.trim().to_owned();
        if line.text.is_empty() && !line.words.is_empty() {
            line.text = words_text(&line.words);
        }
    }

    // Instrumental: the source says so.
    let sung: Vec<&Line> = l.lines.iter().filter(|x| !is_blank(x)).collect();
    if l.instrumental || (sung.len() == 1 && is_instrumental_marker(&sung[0].text)) {
        l.instrumental = true;
        l.lines.clear();
        l.gaps.clear();
        l.sync = SyncLevel::Unsynced;
        return l;
    }

    let timed = l
        .lines
        .iter()
        .any(|x| x.start_ms.is_some() || x.words.iter().any(|w| w.start_ms != OPEN));
    if !timed {
        // A plain page: keep blank lines as the gaps between verses, but not at the
        // ends and not two in a row.
        for line in &mut l.lines {
            line.start_ms = None;
            line.end_ms = None;
            line.words.clear();
            for b in &mut line.background {
                b.start_ms = None;
                b.end_ms = None;
                b.words.clear();
            }
        }
        let mut kept: Vec<Line> = Vec::with_capacity(l.lines.len());
        for line in l.lines.drain(..) {
            if is_blank(&line) && kept.last().is_none_or(is_blank) {
                continue;
            }
            kept.push(line);
        }
        while kept.last().is_some_and(is_blank) {
            kept.pop();
        }
        l.lines = kept;
        l.sync = SyncLevel::Unsynced;
        l.gaps.clear();
        assign_sides(&mut l);
        return l;
    }

    // Synced. A line timed only through its words starts with its first word; a
    // line with no time at all (credits in a synced file) is dropped.
    for line in &mut l.lines {
        if let Some(first) = line
            .words
            .iter()
            .map(|w| w.start_ms)
            .filter(|t| *t != OPEN)
            .min()
        {
            line.start_ms = Some(line.start_ms.map_or(first, |s| s.min(first)));
        }
    }
    l.lines.retain(|x| x.start_ms.is_some());
    l.lines.sort_by_key(|x| x.start_ms.unwrap_or(0));

    // A blank timed line marks where the line before it ends.
    let mut lines: Vec<Line> = Vec::with_capacity(l.lines.len());
    for line in l.lines.drain(..) {
        if is_blank(&line) {
            if let Some(prev) = lines.last_mut() {
                let at = line.start_ms.unwrap_or(0);
                if prev.end_ms.is_none_or(|e| e > at) {
                    prev.end_ms = Some(at);
                }
            }
            continue;
        }
        lines.push(line);
    }

    let n = lines.len();
    for i in 0..n {
        let start = lines[i].start_ms.unwrap_or(0);
        let next_start = lines
            .get(i + 1)
            .and_then(|x| x.start_ms)
            .filter(|t| *t > start);
        // The words say when the line ends only if the last one has an end.
        let last_word_end = match lines[i].words.last() {
            Some(last) if last.end_ms != OPEN => lines[i]
                .words
                .iter()
                .map(|w| w.end_ms)
                .filter(|t| *t != OPEN)
                .max(),
            _ => None,
        };
        // Nothing says when the line ends: it stays lit for `last_line_ms`, but never
        // past the end of the song.
        let fallback = match rules.song_duration_ms {
            Some(d) if d > start => (start + rules.last_line_ms).min(d),
            _ => start + rules.last_line_ms,
        };
        let end = lines[i]
            .end_ms
            .filter(|e| *e >= start)
            .or(last_word_end.filter(|e| *e > start))
            .or(next_start)
            .unwrap_or(fallback);
        let line = &mut lines[i];
        line.end_ms = Some(end.max(start));
        close_words(&mut line.words, end);
        for b in &mut line.background {
            let b_end = b.end_ms.filter(|e| *e != OPEN).unwrap_or(end);
            if let Some(first) = b
                .words
                .iter()
                .map(|w| w.start_ms)
                .filter(|t| *t != OPEN)
                .min()
            {
                b.start_ms = Some(b.start_ms.map_or(first, |s| s.min(first)));
            }
            close_words(&mut b.words, b_end);
            if b.text.trim().is_empty() {
                b.text = words_text(&b.words);
            }
            b.end_ms = Some(b_end.max(b.start_ms.unwrap_or(start)));
            if b.start_ms.is_none() {
                b.start_ms = Some(start);
            }
        }
        line.background.retain(|b| !b.text.trim().is_empty());
    }

    // Gaps long enough for the dots. Duet lines can overlap, so compare with the
    // latest end so far, not just the line before.
    l.gaps.clear();
    let gap = rules.instrumental_gap_ms;
    if let Some(first) = lines.first().and_then(|x| x.start_ms)
        && first >= gap
    {
        l.gaps.push(Gap {
            after_line: None,
            start_ms: 0,
            end_ms: first,
        });
    }
    let mut latest_end = i64::MIN;
    for i in 0..lines.len().saturating_sub(1) {
        latest_end = latest_end.max(lines[i].end_ms.unwrap_or(0));
        let next = lines[i + 1].start_ms.unwrap_or(0);
        if next - latest_end >= gap {
            l.gaps.push(Gap {
                after_line: Some(i as u32),
                start_ms: latest_end,
                end_ms: next,
            });
        }
    }

    l.sync = if lines.iter().any(|x| !x.words.is_empty()) {
        SyncLevel::Word
    } else {
        SyncLevel::Line
    };
    l.lines = lines;
    assign_sides(&mut l);
    l
}

/// The first person to sing sits on the main side; every other person sings from
/// the opposite side; a group (everyone together) and lines with no singer stay
/// on the main side. Unused agents are dropped and indexes renumbered.
fn assign_sides(l: &mut Lyrics) {
    let mut used: Vec<u32> = Vec::new();
    for line in &l.lines {
        if let Some(a) = line.agent
            && (a as usize) < l.agents.len()
            && !used.contains(&a)
        {
            used.push(a);
        }
    }
    let agents: Vec<_> = used.iter().map(|&i| l.agents[i as usize].clone()).collect();
    let first_person = agents.iter().position(|a| a.kind == AgentKind::Person);
    for line in &mut l.lines {
        let new_index = line.agent.and_then(|a| used.iter().position(|u| *u == a));
        line.agent = new_index.map(|i| i as u32);
        line.side = match new_index {
            Some(i) if agents[i].kind == AgentKind::Person && Some(i) != first_person => {
                Side::Opposite
            }
            _ => Side::Main,
        };
    }
    l.agents = agents;
}
