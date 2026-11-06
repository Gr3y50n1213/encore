//! Search over the on-device mirror (board 07): instant, offline, typo tolerant.
//!
//! - Three or more characters: the trigram full-text index finds names containing
//!   the query anywhere ("hou" finds "Golden Hour").
//! - One or two characters: names or words that start with them.
//! - Too few hits and four or more characters: a typo pass looks at names sharing
//!   trigrams with the query and keeps those within `max_edits` edits.
//!
//! Ranking: exact name, then name prefix, then word prefix, then anywhere, then typo
//! matches by distance; ties go to the shorter, then alphabetical name.

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::browse::{AlbumRow, ArtistRow, PlaylistRow, TrackRow};
use crate::mirror::{KIND_ALBUM, KIND_ARTIST, KIND_PLAYLIST, KIND_TRACK};
use crate::{Result, Store};

/// Fold text for matching: accents removed, lower case, whitespace collapsed.
/// "Beyoncé" and "BEYONCE" both become "beyonce".
pub fn fold_for_search(s: &str) -> String {
    let stripped: String = s.nfkd().filter(|c| !is_combining_mark(*c)).collect();
    stripped
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// How many results each section returns (`search.limits` in tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLimits {
    pub songs: u32,
    pub albums: u32,
    pub artists: u32,
    pub playlists: u32,
}

/// The single best match, shown above the sections.
#[derive(Debug, Clone, PartialEq)]
pub enum TopResult {
    Song(TrackRow),
    Album(AlbumRow),
    Artist(ArtistRow),
    Playlist(PlaylistRow),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SearchResults {
    pub top: Option<TopResult>,
    pub songs: Vec<TrackRow>,
    pub albums: Vec<AlbumRow>,
    pub artists: Vec<ArtistRow>,
    pub playlists: Vec<PlaylistRow>,
}

/// Match quality; lower is better.
fn score(name: &str, q: &str) -> Option<u32> {
    if name == q {
        Some(0)
    } else if name.starts_with(q) {
        Some(1)
    } else if name.split(' ').any(|w| w.starts_with(q)) {
        Some(2)
    } else if name.contains(q) {
        Some(3)
    } else {
        None
    }
}

/// Levenshtein distance, giving up (None) past `max`.
fn edit_distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        let mut row_min = cur[0];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
            row_min = row_min.min(cur[j + 1]);
        }
        if row_min > max {
            return None;
        }
        prev = cur;
    }
    (prev[b.len()] <= max).then_some(prev[b.len()])
}

/// The smallest edit distance between the query and any run of whole words in the
/// name ("golden hour" against "the golden hour static").
fn typo_distance(name: &str, q: &str, max: usize) -> Option<usize> {
    let qc: Vec<char> = q.chars().collect();
    let words: Vec<&str> = name.split(' ').collect();
    let mut best: Option<usize> = None;
    for start in 0..words.len() {
        for end in start + 1..=words.len() {
            let span = words[start..end].join(" ");
            let sc: Vec<char> = span.chars().collect();
            if sc.len() > qc.len() + max {
                break;
            }
            if let Some(d) = edit_distance(&sc, &qc, max) {
                best = Some(best.map_or(d, |b: usize| b.min(d)));
            }
        }
    }
    best
}

/// (kind, id) -> (score, folded name). Rows are only read for the hits returned.
type Hits = HashMap<(i64, i64), (u32, String)>;

impl Store {
    /// Search the profile's mirror. `max_edits` is `search.typoMaxEdits`.
    pub fn search(
        &self,
        query: &str,
        limits: &SearchLimits,
        max_edits: u32,
    ) -> Result<SearchResults> {
        let q = fold_for_search(query);
        if q.is_empty() {
            return Ok(SearchResults::default());
        }
        let qlen = q.chars().count();
        let mut hits: Hits = HashMap::new();
        if qlen >= 3 {
            self.substring_hits(&q, &mut hits)?;
        } else {
            self.prefix_hits(&q, &mut hits)?;
        }
        // Typo matches are for when nothing matched cleanly: an exact, prefix or
        // word-prefix hit means the user typed it right, and the typo pass is the
        // expensive part (it ranks every name sharing a trigram).
        let clean_hit = hits.values().any(|(s, _)| *s <= 2);
        let wanted = (limits.songs + limits.albums + limits.artists + limits.playlists) as usize;
        if qlen >= 4 && !clean_hit && hits.len() < wanted {
            // Short words tolerate one typo, longer ones up to max_edits.
            let max = if qlen <= 5 { 1 } else { max_edits as usize };
            self.typo_hits(&q, max, &mut hits)?;
        }
        self.collect(hits, limits)
    }

    fn substring_hits(&self, q: &str, hits: &mut Hits) -> Result<()> {
        let phrase = format!("\"{}\"", q.replace('"', "\"\""));
        let mut st = self.conn.prepare_cached(
            "SELECT rowid, text FROM search_index WHERE search_index MATCH ?1 LIMIT 2000",
        )?;
        let rows = st.query_map([phrase], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (rowid, text) = row?;
            let name = fold_for_search(&text);
            if let Some(s) = score(&name, q) {
                hits.insert((rowid % 4, rowid / 4), (s, name));
            }
        }
        Ok(())
    }

    fn prefix_hits(&self, q: &str, hits: &mut Hits) -> Result<()> {
        let escaped = q
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let starts = format!("{escaped}%");
        let word = format!("% {escaped}%");
        for (table, kind) in [
            ("tracks", KIND_TRACK),
            ("albums", KIND_ALBUM),
            ("artists", KIND_ARTIST),
            ("playlists", KIND_PLAYLIST),
        ] {
            let mut st = self.conn.prepare_cached(&format!(
                "SELECT id, search_name FROM {table}
                 WHERE search_name LIKE ?1 ESCAPE '\\' OR search_name LIKE ?2 ESCAPE '\\' LIMIT 500"
            ))?;
            let rows = st.query_map(params![starts, word], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (id, name) = row?;
                if let Some(s) = score(&name, q) {
                    hits.insert((kind, id), (s, name));
                }
            }
        }
        Ok(())
    }

    /// Names within `max` edits. A typo breaks at most a few of the query's
    /// trigrams, so candidates come from the rarest trigrams that exist in the
    /// index (a trigram the typo made up exists nowhere and is skipped). Ranking
    /// every name that shares a common trigram is what made this slow.
    fn typo_hits(&self, q: &str, max: usize, hits: &mut Hits) -> Result<()> {
        let chars: Vec<char> = q.chars().collect();
        let mut grams: Vec<String> = chars
            .windows(3)
            .map(|w| w.iter().collect::<String>())
            .collect();
        grams.sort();
        grams.dedup();
        let mut counted: Vec<(i64, String)> = Vec::new();
        {
            let mut vocab = self
                .conn
                .prepare_cached("SELECT doc FROM search_vocab WHERE term = ?1")?;
            for g in grams {
                let docs: i64 = vocab.query_row([&g], |r| r.get(0)).optional()?.unwrap_or(0);
                if docs > 0 {
                    counted.push((docs, g));
                }
            }
        }
        counted.sort();
        let rarest: Vec<String> = counted.into_iter().take(3).map(|(_, g)| g).collect();
        if rarest.is_empty() {
            return Ok(());
        }
        let expr = rarest
            .iter()
            .map(|g| format!("\"{}\"", g.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut st = self.conn.prepare_cached(
            "SELECT rowid, text FROM search_index WHERE search_index MATCH ?1 LIMIT 3000",
        )?;
        let rows = st.query_map([expr], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (rowid, text) = row?;
            let key = (rowid % 4, rowid / 4);
            if hits.contains_key(&key) {
                continue;
            }
            let name = fold_for_search(&text);
            if let Some(d) = typo_distance(&name, q, max) {
                hits.insert(key, (4 + d as u32, name));
            }
        }
        Ok(())
    }

    fn collect(&self, hits: Hits, limits: &SearchLimits) -> Result<SearchResults> {
        // Per kind: best score first, then the shorter name, then alphabetical.
        let mut by_kind: HashMap<i64, Vec<(u32, String, i64)>> = HashMap::new();
        for ((kind, id), (s, name)) in hits {
            by_kind.entry(kind).or_default().push((s, name, id));
        }
        for list in by_kind.values_mut() {
            list.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then(a.1.len().cmp(&b.1.len()))
                    .then(a.1.cmp(&b.1))
            });
        }
        let take = |kind: i64, n: u32| -> Vec<(u32, i64)> {
            by_kind
                .get(&kind)
                .map(|l| {
                    l.iter()
                        .take(n as usize)
                        .map(|(s, _, id)| (*s, *id))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut out = SearchResults::default();
        let mut best: Vec<(u32, u8, TopResult)> = Vec::new();
        for (s, id) in take(KIND_TRACK, limits.songs.max(1)) {
            if let Some(t) = self.track_row(id)? {
                if best.iter().all(|b| b.1 != 2) {
                    best.push((s, 2, TopResult::Song(t.clone())));
                }
                out.songs.push(t);
            }
        }
        for (s, id) in take(KIND_ALBUM, limits.albums.max(1)) {
            if let Some(a) = self.album_row(id)? {
                if best.iter().all(|b| b.1 != 1) {
                    best.push((s, 1, TopResult::Album(a.clone())));
                }
                out.albums.push(a);
            }
        }
        for (s, id) in take(KIND_ARTIST, limits.artists.max(1)) {
            if let Some(a) = self.artist_row(id)? {
                if best.iter().all(|b| b.1 != 0) {
                    best.push((s, 0, TopResult::Artist(a.clone())));
                }
                out.artists.push(a);
            }
        }
        for (s, id) in take(KIND_PLAYLIST, limits.playlists.max(1)) {
            if let Some(p) = self.playlist_row(id)? {
                if best.iter().all(|b| b.1 != 3) {
                    best.push((s, 3, TopResult::Playlist(p.clone())));
                }
                out.playlists.push(p);
            }
        }
        // Top result: the best score; on a tie an artist beats an album, an album a
        // song, and a song a playlist.
        out.top = best
            .into_iter()
            .min_by_key(|(s, rank, _)| (*s, *rank))
            .map(|(_, _, t)| t);
        out.songs.truncate(limits.songs as usize);
        out.albums.truncate(limits.albums as usize);
        out.artists.truncate(limits.artists as usize);
        out.playlists.truncate(limits.playlists as usize);
        Ok(out)
    }

    // ------------------------------------------------------------- recent searches

    /// Save a query the user ran (the return key, board 07). Keeps the newest `max`.
    pub fn add_recent_search(&self, query: &str, at_utc_ms: i64, max: u32) -> Result<()> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO recent_searches (query, used_utc_ms) VALUES (?1, ?2)
             ON CONFLICT (query) DO UPDATE SET used_utc_ms = excluded.used_utc_ms",
            params![q, at_utc_ms],
        )?;
        self.conn.execute(
            "DELETE FROM recent_searches WHERE query NOT IN
                 (SELECT query FROM recent_searches ORDER BY used_utc_ms DESC LIMIT ?1)",
            [max],
        )?;
        Ok(())
    }

    /// Newest first.
    pub fn recent_searches(&self) -> Result<Vec<String>> {
        let mut st = self
            .conn
            .prepare("SELECT query FROM recent_searches ORDER BY used_utc_ms DESC")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn remove_recent_search(&self, query: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM recent_searches WHERE query = ?1",
            [query.trim()],
        )?;
        Ok(())
    }

    pub fn clear_recent_searches(&self) -> Result<()> {
        self.conn.execute("DELETE FROM recent_searches", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::tests::{sample_delta, track};
    use crate::test_support::store_with_source;
    use encore_sources::{Artist, LibraryDelta};

    const LIMITS: SearchLimits = SearchLimits {
        songs: 20,
        albums: 10,
        artists: 10,
        playlists: 10,
    };

    fn store() -> crate::Store {
        let (store, src) = store_with_source();
        let mut d = sample_delta();
        d.tracks.push(track("t4", "Beyoncé Interlude", "al1", 4));
        d.artists.push(Artist {
            source_item_id: "ar2".into(),
            name: "東京 Night Club".into(),
            sort_name: "東京 Night Club".into(),
            mbid: None,
        });
        store.apply_delta(src, &d).unwrap();
        store
    }

    fn titles(r: &SearchResults) -> Vec<String> {
        r.songs.iter().map(|t| t.title.clone()).collect()
    }

    #[test]
    fn folding_removes_accents_case_and_extra_spaces() {
        assert_eq!(
            fold_for_search("  Beyoncé  INTERLUDE "),
            "beyonce interlude"
        );
        assert_eq!(fold_for_search("ＡＢＣ"), "abc");
    }

    #[test]
    fn finds_a_name_by_any_part_of_it() {
        let r = store().search("lantern", &LIMITS, 2).unwrap();
        assert_eq!(titles(&r), vec!["Paper Lanterns"]);
    }

    #[test]
    fn ignores_accents_and_case() {
        let r = store().search("BEYONCE", &LIMITS, 2).unwrap();
        assert_eq!(titles(&r), vec!["Beyoncé Interlude"]);
    }

    #[test]
    fn short_queries_match_the_start_of_words() {
        let r = store().search("sl", &LIMITS, 2).unwrap();
        assert_eq!(titles(&r), vec!["Slow Tide"]);
        let r = store().search("t", &LIMITS, 2).unwrap();
        assert!(titles(&r).contains(&"Count the Hours".to_string()));
    }

    #[test]
    fn tolerates_typos() {
        let r = store().search("lantrens", &LIMITS, 2).unwrap();
        assert_eq!(titles(&r), vec!["Paper Lanterns"]);
        let r = store().search("slow tdie", &LIMITS, 2).unwrap();
        assert_eq!(titles(&r), vec!["Slow Tide"]);
    }

    #[test]
    fn non_latin_names_are_found() {
        let r = store().search("東京", &LIMITS, 2).unwrap();
        assert_eq!(r.artists.len(), 1);
        assert_eq!(r.artists[0].name, "東京 Night Club");
    }

    #[test]
    fn an_exact_artist_is_the_top_result() {
        let r = store().search("ember cove", &LIMITS, 2).unwrap();
        assert!(matches!(r.top, Some(TopResult::Artist(ref a)) if a.name == "Ember Cove"));
        let r = store().search("golden hour static", &LIMITS, 2).unwrap();
        assert!(matches!(r.top, Some(TopResult::Album(ref a)) if a.title == "Golden Hour Static"));
    }

    #[test]
    fn nothing_matches_gives_empty_sections() {
        let r = store().search("zzzzqqqq", &LIMITS, 2).unwrap();
        assert!(
            r.top.is_none() && r.songs.is_empty() && r.albums.is_empty() && r.artists.is_empty()
        );
        assert_eq!(
            store().search("   ", &LIMITS, 2).unwrap(),
            SearchResults::default()
        );
    }

    #[test]
    fn quotes_and_wildcards_in_the_query_are_plain_text() {
        let s = store();
        assert!(s.search("\"", &LIMITS, 2).unwrap().songs.is_empty());
        assert!(s.search("%", &LIMITS, 2).unwrap().songs.is_empty());
        assert!(s.search("a\"b OR c", &LIMITS, 2).is_ok());
    }

    #[test]
    fn limits_cut_each_section() {
        let (store, src) = store_with_source();
        let mut d = LibraryDelta::default();
        for i in 0..30 {
            d.tracks
                .push(track(&format!("x{i}"), &format!("Tide {i:02}"), "none", i));
        }
        store.apply_delta(src, &d).unwrap();
        let r = store.search("tide", &LIMITS, 2).unwrap();
        assert_eq!(r.songs.len(), 20);
    }

    #[test]
    fn recent_searches_are_newest_first_and_capped() {
        let s = store();
        for (i, q) in ["one", "two", "three", "two"].iter().enumerate() {
            s.add_recent_search(q, i as i64, 2).unwrap();
        }
        assert_eq!(s.recent_searches().unwrap(), vec!["two", "three"]);
        s.remove_recent_search("two").unwrap();
        assert_eq!(s.recent_searches().unwrap(), vec!["three"]);
        s.clear_recent_searches().unwrap();
        assert!(s.recent_searches().unwrap().is_empty());
    }
}
