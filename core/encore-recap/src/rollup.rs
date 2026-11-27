//! Small daily rollups (plan section 25): plays, listened time, per song, per artist
//! and per hour, for each local day. They are a pure function of the log, so they
//! can be dropped and rebuilt at any time, and two devices build the same ones.
//!
//! Keys are the raw song and artist keys: merging songs across albums (the 2 s
//! tolerance) happens when a recap is built, so a rollup never has to change when a
//! new key of the same song appears. Kid-profile filtering also happens at build
//! time. With Recap off the caller keeps no rollups.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::input::Play;
use crate::time::{day_of, format_day, hour_of, local_ms, weekday};
use crate::year::{canonical_cmp, counts};

/// Plays and listened time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tally {
    pub plays: u64,
    pub listened_ms: u64,
}

impl Tally {
    fn add(&mut self, ms: u64) {
        self.plays += 1;
        self.listened_ms += ms;
    }
}

/// One local day of one profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyRollup {
    /// "YYYY-MM-DD", the day in each play's own time zone.
    pub day: String,
    /// 0 = Monday. With the hour, this is the hour of the week.
    pub weekday: u32,
    pub total: Tally,
    /// Raw song key -> tally.
    pub songs: BTreeMap<String, Tally>,
    /// Artist key -> tally.
    pub artists: BTreeMap<String, Tally>,
    /// Local hour 0-23 -> tally (hours with no plays are left out).
    pub hours: BTreeMap<u32, Tally>,
}

/// The daily rollups of a log, oldest day first. The same counting rules as the
/// recap: listened time, plays under 2 s and excluded end reasons left out, each
/// play on the day of its own offset. Input order does not matter.
pub fn daily_rollups(plays: &[Play]) -> Vec<DailyRollup> {
    let mut sorted: Vec<&Play> = plays.iter().filter(|p| counts(p)).collect();
    sorted.sort_by(|a, b| canonical_cmp(a, b));
    let mut days: BTreeMap<i64, DailyRollup> = BTreeMap::new();
    for p in sorted {
        let local = local_ms(p.started_utc_ms, p.utc_offset_minutes);
        let day = day_of(local);
        let r = days.entry(day).or_insert_with(|| DailyRollup {
            day: format_day(day),
            weekday: weekday(day),
            total: Tally::default(),
            songs: BTreeMap::new(),
            artists: BTreeMap::new(),
            hours: BTreeMap::new(),
        });
        let ms = p.listened_ms;
        r.total.add(ms);
        r.songs.entry(p.song_key.clone()).or_default().add(ms);
        r.artists.entry(p.artist_key.clone()).or_default().add(ms);
        r.hours.entry(hour_of(local)).or_default().add(ms);
    }
    days.into_values().collect()
}
