//! What the engine is given: plain data, deserializable from camelCase JSON.
//!
//! Everything optional defaults to "not known". A beat that needs data the input
//! does not have is left out, never faked (plan section 25).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Everything one build needs. The same value gives the same bytes anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapInput {
    /// The calendar year of the recap (plays count by their own local day).
    pub year: i32,
    /// The profile's stable ID. It seeds the pack, layouts and words; it is never
    /// written to the output.
    pub profile_id: String,
    /// Kid profiles drop explicit plays and skip the late-night beats.
    #[serde(default)]
    pub kid_profile: bool,
    /// Recap off in Settings: the build returns kind `none` and nothing else.
    #[serde(default)]
    pub recap_off: bool,
    /// The build's "now": plays that started after it are not counted, and a build
    /// before the year has ended everywhere is marked "so far".
    pub counted_through_utc_ms: i64,
    /// Clear listening history: the local day ("YYYY-MM-DD") of the clear. Plays on
    /// earlier days are not counted (they should be gone already), and the story
    /// says "Since 3 Aug".
    #[serde(default)]
    pub cleared_since: Option<String>,
    /// The local day ("YYYY-MM-DD") the profile was made, for "Since 14 Jun" when it
    /// joined mid-year. Imported history before it still counts.
    #[serde(default)]
    pub joined_day: Option<String>,
    /// The packs this build may pick from: the ones every device the profile used
    /// in the last 90 days has (the caller works that out).
    pub packs: Vec<PackInfo>,
    /// Recaps of earlier years (from the archive), for "never the same pack",
    /// the 60 degree hue gap and the comparisons with last year.
    #[serde(default)]
    pub past: Vec<PastRecap>,
    /// Album key -> its palette as "#rrggbb" colours, most prominent first (the
    /// core's own sampling, plan section 16). Only the top three albums are read.
    #[serde(default)]
    pub album_palettes: BTreeMap<String, Vec<String>>,
    /// Album key -> how many songs it has, for "played start to finish".
    #[serde(default)]
    pub album_track_counts: BTreeMap<String, u32>,
    /// Song key -> a summary of its sound fingerprint, for the mood arc and the
    /// "loud choruses" trait. Songs without one simply do not count there.
    #[serde(default)]
    pub fingerprints: BTreeMap<String, Fingerprint>,
    /// What is known about artists before this year (from rollups or the archive).
    /// Plays of earlier years in `plays` are added to it.
    #[serde(default)]
    pub artists_before: Vec<ArtistBefore>,
    /// Which way the seasons run for "song of each season".
    #[serde(default)]
    pub hemisphere: Hemisphere,
    /// The play log: this year's plays, and any earlier ones the caller has (they
    /// only feed "new to you" and the comeback). Order does not matter.
    pub plays: Vec<Play>,
}

/// One play event, as the store keeps it (plan section 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    /// The stable song key: `mb:<id>` or `t1:<hash>:<seconds>`.
    pub song_key: String,
    pub title: String,
    /// A stable key for the artist (the core's normalized name or ID).
    pub artist_key: String,
    /// The artist name as shown.
    pub artist: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    pub started_utc_ms: i64,
    /// The UTC offset at the moment the play started. The play lands on the
    /// calendar day of this offset.
    pub utc_offset_minutes: i32,
    /// Milliseconds actually listened (not the track length).
    pub listened_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// Tagged explicit. Kid profiles do not count these plays.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub explicit: bool,
    /// The store's end reason (`finished`, `skipped`, `interrupted`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_reason: Option<String>,
    /// What started it (`album`, `playlist`, `radio`, `mix`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_from: Option<String>,
    /// Lyric-line taps during this play.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lyric_jumps: Vec<LyricJump>,
}

/// A tap on a lyric line (plan section 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricJump {
    pub line_index: u32,
    pub line_start_ms: u64,
}

/// A design pack the build may choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    /// `editorial`, `tide`, `bloom`, `glass`, ...
    pub id: String,
    /// The pack's own version; a new version gives new layouts.
    pub version: u32,
}

/// What the build needs from an archived recap of an earlier year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastRecap {
    pub year: i32,
    #[serde(default)]
    pub pack: Option<String>,
    /// The lead hue of that year's palette, in degrees.
    #[serde(default)]
    pub lead_hue: Option<u16>,
    /// Total listened time that year.
    #[serde(default)]
    pub listened_ms: Option<u64>,
    /// That year's personality metric and its value in permille, for "In 2025 it
    /// was 29%" (only shown when this year uses the same metric).
    #[serde(default)]
    pub personality_metric: Option<String>,
    #[serde(default)]
    pub personality_value: Option<u32>,
}

/// A song's sound summary, both values in permille (0 to 1000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    pub energy: u16,
    /// How bright (major, upbeat) the music sounds. Never read as the listener's
    /// mood.
    pub valence: u16,
}

/// An artist heard before this year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistBefore {
    pub artist_key: String,
    /// Plays before this year, if known (0 = heard, count unknown).
    #[serde(default)]
    pub plays: u64,
    /// The last play before this year, if known.
    #[serde(default)]
    pub last_played_utc_ms: Option<i64>,
}

/// Which months make which season.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Hemisphere {
    #[default]
    North,
    South,
}
