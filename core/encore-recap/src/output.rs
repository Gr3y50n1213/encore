//! What the engine writes: the recap data document (plan section 25), camelCase
//! JSON under 100 KB. Struct fields serialize in declaration order and every map is
//! a `BTreeMap`, so the bytes are the same everywhere.

use serde::Serialize;

/// The recap data document for one profile and year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapData {
    /// The document's schema version.
    pub schema: u32,
    /// The engine's rules version: a different engine may give different bytes.
    pub engine: u32,
    pub year: i32,
    pub kind: Kind,
    /// Why there is no recap (`kind` = none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub none_reason: Option<NoneReason>,
    /// Built before the year had ended in every time zone: the story is marked
    /// "so far".
    pub so_far: bool,
    /// Built on or after 8 January of the next year: the caller archives it and
    /// never builds this year again.
    pub locked: bool,
    pub counted_through_utc_ms: i64,
    /// Counted only since a clear or since the profile joined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<Since>,
    pub pack: Option<PackRef>,
    /// 16 hex digits: the seed behind layouts, shapes and words.
    pub seed: String,
    pub palette: Option<Palette>,
    /// "Late nights, loud choruses." Opens the story and titles every share card.
    pub year_line: Option<String>,
    pub totals: Totals,
    pub beats: Vec<Beat>,
    /// The detail page after the story (full story only).
    pub detail: Option<Detail>,
    /// SHA-256 (hex) over the canonical encoding of every play the build used.
    pub log_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    None,
    Short,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoneReason {
    /// Recap off in Settings.
    Off,
    /// Under 10 plays.
    NotEnoughPlays,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Since {
    /// "YYYY-MM-DD".
    pub day: String,
    pub reason: SinceReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SinceReason {
    Cleared,
    Joined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRef {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// `albums` (from the top three covers) or `seed` (no cover palettes given).
    pub source: String,
    /// "#rrggbb", the lead first, then the rest in album and cluster order.
    pub colours: Vec<String>,
    pub lead: String,
    pub lead_hue: u16,
    /// Degrees between this lead hue and last year's, when last year's is known.
    pub hue_gap_from_last_year: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub plays: u64,
    pub listened_ms: u64,
    pub minutes: u64,
    pub songs: u64,
    pub artists: u64,
    pub albums: u64,
    pub active_days: u64,
}

/// One card of the story.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Beat {
    /// The beat type: `intro`, `minutes`, `topSong`, ...
    pub id: String,
    pub role: Role,
    /// One of the eight that always appear (when their data exists).
    pub fixed: bool,
    /// Seconds on screen (6 to 8, a list card 9).
    pub seconds: u32,
    /// Which of `layoutCount` layouts the pack draws, from the seed.
    pub layout: u32,
    pub layout_count: u32,
    /// For placing the pack's shapes, from the seed.
    pub shape_seed: u32,
    /// The rotating beat's score (beats chosen by interest only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i64>,
    /// The template the sentence came from, and the sentence.
    pub template: String,
    pub text: String,
    /// The numbers the card shows.
    pub data: BeatData,
    /// Show the math: the counts and the rule.
    pub math: Math,
}

/// Where a beat sits in the arc; the story is ordered by these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Opener,
    BigNumbers,
    Turn,
    Surprise,
    Identity,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Math {
    pub counts: Vec<Count>,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    pub label: String,
    pub value: i64,
    /// `plays`, `minutes`, `ms`, `days`, `permille`, `count`, `degrees`.
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongEntry {
    pub rank: u32,
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub plays: u64,
    pub listened_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistEntry {
    pub rank: u32,
    pub key: String,
    pub name: String,
    pub plays: u64,
    pub listened_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumEntry {
    pub rank: u32,
    pub key: String,
    pub name: String,
    pub artist: String,
    pub plays: u64,
    pub listened_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenreEntry {
    pub rank: u32,
    pub name: String,
    pub plays: u64,
    /// Share of the plays that have a genre.
    pub permille: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthTally {
    /// 1 to 12.
    pub month: u32,
    pub plays: u64,
    pub minutes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub id: String,
    pub label: String,
    pub value: u32,
    /// `permille` or `count`.
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    /// `winter`, `spring`, `summer`, `autumn`.
    pub id: String,
    pub months: Vec<u32>,
    pub song: SongEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPoint {
    pub month: u32,
    /// None: too few fingerprinted plays that month.
    pub energy: Option<u32>,
    pub valence: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    pub genre: String,
    /// The last month before the drift (the sound moved "after" it).
    pub after_month: u32,
    /// Share after minus share before, in permille points.
    pub rise_permille: u32,
}

/// The numbers of one beat, by beat type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum BeatData {
    #[serde(rename_all = "camelCase")]
    Intro {
        year: i32,
        year_line: String,
        /// The top albums whose covers open the story.
        album_keys: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Minutes {
        minutes: u64,
        /// The same time as whole days and the hours left over.
        days: u64,
        hours: u64,
        months: Vec<MonthTally>,
        last_year_minutes: Option<u64>,
        /// Percent more (positive) or less (negative) than last year.
        change_percent: Option<i64>,
        /// The part of the day with the most listening: `lateNight` (22:00 to
        /// 04:59), `morning`, `afternoon` or `evening`. Never `lateNight` for kids.
        peak_part: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    TopSong {
        top: Vec<SongEntry>,
        /// Where the snippet starts: the line jumped to most, else None (30% in).
        snippet_start_ms: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    TopArtist { top: Vec<ArtistEntry> },
    #[serde(rename_all = "camelCase")]
    TopAlbum {
        top: Vec<AlbumEntry>,
        /// Times the top album was played start to finish (needs its track count).
        play_throughs: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    YourSound {
        top: Vec<GenreEntry>,
        /// Per month, the share of each of the top genres (same order as `top`, up
        /// to four) in permille; months with no genre plays are all zero.
        months: Vec<Vec<u32>>,
        drift: Option<Drift>,
    },
    #[serde(rename_all = "camelCase")]
    Personality {
        archetype: String,
        name: String,
        metric: String,
        value: u32,
        threshold: u32,
        /// `permille` or `count`.
        unit: String,
        last_year_value: Option<u32>,
        /// The supporting numbers ("New artists 9%").
        metrics: Vec<Metric>,
    },
    #[serde(rename_all = "camelCase")]
    Closer {
        title: String,
        song_keys: Vec<String>,
        count: u64,
        /// Sum of the known song lengths.
        duration_ms: u64,
        duration_known: u64,
    },
    #[serde(rename_all = "camelCase")]
    ListeningClock {
        /// 0 = Monday.
        peak_weekday: u32,
        peak_hour: u32,
        /// 168 cells, Monday 00:00 first: plays per hour of the week.
        grid: Vec<u64>,
        peak_part: String,
        top_weekday: u32,
    },
    #[serde(rename_all = "camelCase")]
    BiggestDiscovery {
        artist: ArtistEntry,
        first_day: String,
        first_month: u32,
        /// Plays per month, January first.
        months: Vec<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Comeback {
        artist: ArtistEntry,
        plays_before: u64,
        gap_days: u64,
    },
    #[serde(rename_all = "camelCase")]
    ObsessionWeek {
        artist: ArtistEntry,
        /// The Monday that starts the week.
        week_start: String,
        /// The artist's plays on each day, Monday first.
        days: Vec<u64>,
        week_plays: u64,
        week_share_permille: u32,
    },
    #[serde(rename_all = "camelCase")]
    LongestStreak {
        days: u64,
        first_day: String,
        last_day: String,
    },
    #[serde(rename_all = "camelCase")]
    SongOfEachSeason { seasons: Vec<Season> },
    #[serde(rename_all = "camelCase")]
    MostReplayedDay {
        song: SongEntry,
        day: String,
        plays: u64,
    },
    #[serde(rename_all = "camelCase")]
    MoodArc {
        months: Vec<SoundPoint>,
        first_half_energy: u32,
        second_half_energy: u32,
    },
    #[serde(rename_all = "camelCase")]
    MostSungLine {
        song: SongEntry,
        line_index: u32,
        line_start_ms: u64,
        jumps: u64,
    },
}

/// The detail page: top lists, months and every number behind the personality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub top_songs: Vec<SongEntry>,
    pub top_artists: Vec<ArtistEntry>,
    pub top_albums: Vec<AlbumEntry>,
    pub months: Vec<MonthTally>,
    pub personality: Vec<Metric>,
}

impl RecapData {
    /// The canonical bytes: compact JSON, fields in declaration order.
    pub fn to_json(&self) -> String {
        // Serializing plain structs, Vecs and strings cannot fail.
        serde_json::to_string(self).expect("recap data serializes")
    }
}
