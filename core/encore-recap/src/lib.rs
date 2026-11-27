//! The deterministic year recap engine: the same log, year and seed give the same bytes on the server and on every device.
//!
//! Plan sections 10 and 25, boards 39 to 41, ADR core-0025. Built in Phase 11. The
//! server uses this same crate; never fork it.
//!
//! [`build_recap`] takes a [`RecapInput`] (the play log and the profile's settings)
//! and returns [`RecapData`], whose [`RecapData::to_json`] bytes are identical on
//! every platform. [`daily_rollups`] gives the small per-day rollups the core keeps.
//!
//! **Determinism.** No floating point, no clock, no locale, no hash-map order and
//! no randomness except the seed (SHA-256 of the profile, the year and the pack):
//! every count is an integer, every share is permille in integers, every map is a
//! `BTreeMap`, every sort has a total tie-break, and the plays are put in one
//! canonical order before anything is counted, so input order never matters.

#![forbid(unsafe_code)]

mod build;
mod facts;
mod input;
mod look;
mod output;
mod personality;
mod rollup;
mod songs;
mod time;
mod words;
mod year;

pub use build::{ENGINE, SCHEMA};
pub use input::{
    ArtistBefore, Fingerprint, Hemisphere, LyricJump, PackInfo, PastRecap, Play, RecapInput,
};
pub use output::{
    AlbumEntry, ArtistEntry, Beat, BeatData, Count, Detail, Drift, GenreEntry, Kind, Math, Metric,
    MonthTally, NoneReason, PackRef, Palette, RecapData, Role, Season, Since, SinceReason,
    SongEntry, SoundPoint, Totals,
};
pub use rollup::{DailyRollup, Tally, daily_rollups};

/// Why a build could not run. Everything else (thin data, missing fields) is a
/// smaller recap, never an error.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RecapError {
    #[error("not a day (YYYY-MM-DD): {0}")]
    BadDay(String),
    #[error("year out of range: {0}")]
    BadYear(i32),
    #[error("input is not valid recap JSON: {0}")]
    Json(String),
}

/// Builds the recap data for one profile and year. Pure: the same input gives the
/// same document, byte for byte, everywhere.
pub fn build_recap(input: &RecapInput) -> Result<RecapData, RecapError> {
    build::build(input)
}

/// [`build_recap`] from and to JSON (camelCase), for callers across a binding.
pub fn build_recap_json(input_json: &str) -> Result<String, RecapError> {
    let input: RecapInput =
        serde_json::from_str(input_json).map_err(|e| RecapError::Json(e.to_string()))?;
    Ok(build_recap(&input)?.to_json())
}
