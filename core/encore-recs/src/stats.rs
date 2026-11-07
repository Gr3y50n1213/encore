//! "How suggestions are doing" (board 38, note 7): a private, local page. Per
//! shelf over the last 4 weeks: the early-skip rate of app-chosen songs,
//! completion, distinct artists per 20 songs, and repeats. Nothing leaves the
//! device; the numbers come from the play log alone.

use serde::{Deserialize, Serialize};

use crate::{Catalog, DAY_MS, Outcome, Play};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShelfStats {
    /// Where the plays came from ("autoplay", "mix", "radio", "smartShuffle", ...).
    pub shelf: String,
    pub plays: u32,
    /// Early skips of app-chosen songs, per 1,000 plays (whole numbers keep the
    /// bytes the same everywhere).
    pub early_skips_per_1000: u32,
    pub finished_per_1000: u32,
    /// Distinct artists in each run of 20 songs, times 10, averaged.
    pub artists_per_20_x10: u32,
    /// Plays of a song already played on this shelf in the window, per 1,000.
    pub repeats_per_1000: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub since_utc_ms: i64,
    pub shelves: Vec<ShelfStats>,
}

/// `plays` with where each came from (the core's `started_from`).
pub fn stats(plays: &[(Play, String)], catalog: &Catalog, now_utc_ms: i64) -> Stats {
    let since = now_utc_ms - 28 * DAY_MS;
    let mut shelves: Vec<String> = plays
        .iter()
        .filter(|(p, _)| p.started_utc_ms >= since && p.app_chosen)
        .map(|(_, s)| s.clone())
        .collect();
    shelves.sort();
    shelves.dedup();
    let per_1000 = |n: usize, d: usize| (n * 1000).checked_div(d).unwrap_or(0) as u32;
    let out = shelves
        .into_iter()
        .map(|shelf| {
            let list: Vec<&Play> = plays
                .iter()
                .filter(|(p, s)| *s == shelf && p.started_utc_ms >= since && p.app_chosen)
                .map(|(p, _)| p)
                .collect();
            let counted: Vec<&&Play> = list.iter().filter(|p| p.outcome.is_some()).collect();
            let early = counted
                .iter()
                .filter(|p| p.outcome == Some(Outcome::SkippedEarly))
                .count();
            let finished = counted
                .iter()
                .filter(|p| p.outcome.is_some_and(Outcome::enjoyed))
                .count();
            let mut seen = std::collections::HashSet::new();
            let repeats = list
                .iter()
                .filter(|p| !seen.insert(p.song_key.clone()))
                .count();
            let artist = |p: &Play| {
                catalog
                    .find(&p.song_key)
                    .and_then(|i| catalog.get(i).artists.first().cloned())
                    .unwrap_or_default()
            };
            let windows: Vec<usize> = list
                .chunks(20)
                .filter(|c| c.len() == 20)
                .map(|c| {
                    let mut a: Vec<String> = c.iter().map(|p| artist(p)).collect();
                    a.sort();
                    a.dedup();
                    a.len()
                })
                .collect();
            let artists = if windows.is_empty() {
                0
            } else {
                (windows.iter().sum::<usize>() * 10 / windows.len()) as u32
            };
            ShelfStats {
                plays: list.len() as u32,
                early_skips_per_1000: per_1000(early, counted.len()),
                finished_per_1000: per_1000(finished, counted.len()),
                artists_per_20_x10: artists,
                repeats_per_1000: per_1000(repeats, list.len()),
                shelf,
            }
        })
        .collect();
    Stats {
        since_utc_ms: since,
        shelves: out,
    }
}
