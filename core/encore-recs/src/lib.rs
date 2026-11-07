//! The smart-picks engine (plan sections 7, 7.1 and 21; boards 15, 23, 37, 38;
//! docs/adr/core-0012): signals, sessions and co-listening, candidate pools, the
//! hand-tuned ranker, shaping (exploration, diversity, caps), reasons, the daily
//! mixes and the stats page.
//!
//! **Pure and deterministic.** No I/O, no clock, no platform maths: the same
//! history, catalog, "now" and seed give the same picks, byte for byte, on a phone,
//! a desktop and the companion server, so the server's mixes and a device's own
//! build agree (docs/adr/core-0013). Randomness comes from a seeded PRNG
//! ([`det::Rng`]) and every exponential from [`det::exp`].
//!
//! Version 1 has no audio features (they arrive with the companion's analysis in
//! Phase 9): "sounds like" means played together, same artist, genre and era.

#![forbid(unsafe_code)]

pub mod det;
pub mod mixes;
pub mod model;
pub mod rank;
pub mod stats;
#[cfg(test)]
mod tests;
pub mod titles;

use serde::{Deserialize, Serialize};

pub use mixes::{Mix, MixSet, MixSlot, Previous, Shelves, build_mixes, build_shelves};
pub use model::Model;
pub use rank::{Context, Pick, Reason, autoplay, radio, score_of, smart_shuffle, sounds_like, why};
pub use stats::{ShelfStats, Stats, stats};

/// One song of the library, as the engine needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Song {
    /// The stable song key (MusicBrainz ID, or artist + title + duration).
    pub key: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genres: Vec<String>,
    pub year: Option<i32>,
    pub duration_ms: u64,
    pub added_utc_ms: Option<i64>,
    /// What the media server knows from other apps (a cold-start prior).
    pub server_play_count: u32,
    pub server_last_played_utc_ms: Option<i64>,
    pub server_favorite: bool,
    /// Tagged explicit (hidden when the profile asks).
    pub explicit: bool,
}

/// The library: songs by song key, one each (a song on two albums is one song).
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    songs: Vec<Song>,
    index: std::collections::HashMap<String, usize>,
}

impl Catalog {
    /// One song per key; songs are sorted by key, so the result doesn't depend on
    /// the order they arrived in.
    pub fn new(mut songs: Vec<Song>) -> Catalog {
        songs.sort_by(|a, b| a.key.cmp(&b.key));
        songs.dedup_by(|b, a| a.key == b.key);
        let index = songs
            .iter()
            .enumerate()
            .map(|(i, s)| (s.key.clone(), i))
            .collect();
        Catalog { songs, index }
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn get(&self, i: usize) -> &Song {
        &self.songs[i]
    }

    pub fn find(&self, key: &str) -> Option<usize> {
        self.index.get(key).copied()
    }
}

/// How a play ended, for the engine (the section 21 classification the core
/// stores as `signal`). Plays that never count (interruptions, errors, accidents,
/// too soon, ended by the system) come as `None` and only mark the song as heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Finished,
    /// Played past 80%, then skipped: mostly enjoyed.
    SkippedLate,
    /// Skipped between 30 s and 80%.
    SkippedMid,
    /// Skipped in the first 30 s, the app chose it: the key signal.
    SkippedEarly,
    /// Skipped, the user chose it: browsing.
    SkippedUserPick,
    /// Paused and never resumed.
    Abandoned,
}

impl Outcome {
    /// From the core's stored signal name; None for the ones that never count.
    pub fn from_signal(s: &str) -> Option<Outcome> {
        Some(match s {
            "finished" => Outcome::Finished,
            "skippedLate" => Outcome::SkippedLate,
            "skippedMid" => Outcome::SkippedMid,
            "skippedEarly" => Outcome::SkippedEarly,
            "skippedUserPick" => Outcome::SkippedUserPick,
            "pausedAbandoned" => Outcome::Abandoned,
            _ => return None,
        })
    }

    /// Played to the end, or nearly.
    pub fn enjoyed(self) -> bool {
        matches!(self, Outcome::Finished | Outcome::SkippedLate)
    }
}

/// One play from the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Play {
    pub song_key: String,
    pub started_utc_ms: i64,
    pub utc_offset_minutes: i32,
    pub listened_ms: u64,
    pub outcome: Option<Outcome>,
    /// The app chose it (autoplay, a mix, radio, Smart shuffle).
    pub app_chosen: bool,
    /// Queued by hand, searched then played, or replayed: strong intent.
    pub strong_intent: bool,
}

/// Why something is "not interested" (board 38, note 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotInterestedReason {
    /// Lowers the song and its neighbours. Also the default.
    NotMyTaste,
    /// Rests the song for 30 days only.
    HeardTooMuch,
    /// Lowers it at that time of day only.
    WrongMoment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    Song(String),
    Artist(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotInterested {
    pub scope: Scope,
    pub reason: NotInterestedReason,
    pub at_utc_ms: i64,
    /// The time-of-day bucket it was said in (for Wrong moment).
    pub bucket: u8,
}

/// More like this (+) or Fewer like this (-) on a song, in a time-of-day bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub song_key: String,
    pub more: bool,
    pub at_utc_ms: i64,
    pub bucket: u8,
}

/// Everything the engine learns from, already filtered by the core: nothing from
/// Private listening, Guest or Learn-from-my-listening-off is in it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// Oldest first.
    pub plays: Vec<Play>,
    /// Liked songs (song key, when).
    pub likes: Vec<(String, i64)>,
    /// Songs added to a playlist or downloaded (song key, when).
    pub added: Vec<(String, i64)>,
    pub feedback: Vec<Feedback>,
    pub not_interested: Vec<NotInterested>,
    /// Reset suggestions starts a new learning epoch: plays and feedback before
    /// this are not learned from (likes, playlists and Not interested still count).
    pub epoch_start_utc_ms: i64,
}

/// Every tuning number, from the `smartPicks` tokens (encore-core fills it from
/// tokens.json; `Params::default()` is the same values, and a test in encore-core
/// checks the two agree).
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    pub w_liked: f64,
    pub w_strong_intent: f64,
    pub w_added: f64,
    pub w_finished: f64,
    pub w_skipped_late: f64,
    pub w_skipped_mid: f64,
    pub w_early_skip_app: f64,
    pub w_skipped_user_pick: f64,
    pub w_more_like_this: f64,
    pub w_fewer_like_this: f64,
    pub half_life_days: f64,
    pub user_pick_skips: u32,
    pub user_pick_days: i64,
    pub rediscover_after_days: i64,
    pub candidates: usize,
    pub diversity_lambda: f64,
    pub heard_too_much_rest_days: i64,
    pub full_from_plays: u32,
    pub drift_from_days: i64,
    pub mix_count: usize,
    pub shelf_change_max_percent: u32,
    pub smart_shuffle_every: usize,
    pub session_gap_minutes: i64,
    pub mix_min_songs: usize,
    pub mix_max_songs: usize,
    pub mix_max_overlap_percent: u32,
    pub per_artist_in_mix: usize,
    pub per_album_in_mix: usize,
    pub artist_spacing: usize,
    pub stretch_new_percent: u32,
    pub most_played_max_percent: u32,
    pub rediscover_count: usize,
    pub early_skip_ms: u64,
    pub recent_hours_filter: i64,
}

impl Default for Params {
    /// The tokens' values (smartPicks; plan sections 7.1 and 21).
    fn default() -> Params {
        Params {
            w_liked: 3.0,
            w_strong_intent: 1.5,
            w_added: 1.5,
            w_finished: 1.0,
            w_skipped_late: 0.5,
            w_skipped_mid: -0.5,
            w_early_skip_app: -1.5,
            w_skipped_user_pick: -0.2,
            w_more_like_this: 1.5,
            w_fewer_like_this: -1.0,
            half_life_days: 30.0,
            user_pick_skips: 3,
            user_pick_days: 14,
            rediscover_after_days: 60,
            candidates: 500,
            diversity_lambda: 0.7,
            heard_too_much_rest_days: 30,
            full_from_plays: 50,
            drift_from_days: 30,
            mix_count: 5,
            shelf_change_max_percent: 30,
            smart_shuffle_every: 4,
            session_gap_minutes: 30,
            mix_min_songs: 25,
            mix_max_songs: 40,
            mix_max_overlap_percent: 20,
            per_artist_in_mix: 2,
            per_album_in_mix: 4,
            artist_spacing: 3,
            stretch_new_percent: 30,
            most_played_max_percent: 40,
            rediscover_count: 12,
            early_skip_ms: 30_000,
            recent_hours_filter: 24,
        }
    }
}

/// The time-of-day bucket of a local hour: 0 night (0-5), 1 morning (6-11),
/// 2 afternoon (12-17), 3 evening (18-23).
pub fn bucket_of(utc_ms: i64, utc_offset_minutes: i32) -> u8 {
    let local = utc_ms + i64::from(utc_offset_minutes) * 60_000;
    let hour = local.rem_euclid(DAY_MS) / 3_600_000;
    (hour / 6) as u8
}

/// Whether a local time falls on Saturday or Sunday (1970-01-01 was a Thursday).
pub fn weekend(utc_ms: i64, utc_offset_minutes: i32) -> bool {
    let local = utc_ms + i64::from(utc_offset_minutes) * 60_000;
    let day = local.div_euclid(DAY_MS);
    matches!((day + 4).rem_euclid(7), 0 | 6)
}

pub const DAY_MS: i64 = 86_400_000;
