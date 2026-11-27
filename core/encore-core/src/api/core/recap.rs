//! The year recap (Phase 11, plan sections 10 and 25, boards 39 to 41, ADR core-0025).
//! The engine is `encore-recap`, a pure function; this module gathers its input from
//! the profile's store (every device's plays arrive there through sync), keeps a year
//! once it is locked (8 January), and hands back the recap data document as JSON for
//! each app to draw natively.

use std::collections::BTreeMap;

use encore_recap::{
    ArtistBefore, Hemisphere, LyricJump, PackInfo, PastRecap, Play, RecapInput, build_recap,
};
use encore_store::Store;

use super::{EncoreCore, lock, now_ms};
use crate::api::types::*;
use crate::playlog::ListeningMode;

/// Where a year's locked recap is kept (`recs_state`), so it never changes again and
/// stays in the archive. Clear listening history keeps it (plan section 25).
fn kept_key(year: i32) -> String {
    format!("recap.year.{year}")
}

/// The version of every pack this core knows; the app names the packs it draws.
const PACK_VERSION: u32 = 1;

/// The span of plays a year's recap reads: the year in every time zone (UTC-14 to UTC+14).
fn year_span(year: i32) -> (i64, i64) {
    const FOURTEEN_HOURS_MS: i64 = 14 * 3_600_000;
    (
        utc_ms_of_day(year, 1, 1) - FOURTEEN_HOURS_MS,
        utc_ms_of_day(year + 1, 1, 1) + FOURTEEN_HOURS_MS,
    )
}

/// Midnight UTC at the start of a calendar day (proleptic Gregorian).
fn utc_ms_of_day(year: i32, month: u32, day: u32) -> i64 {
    // Howard Hinnant's days_from_civil.
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * 86_400_000
}

/// The calendar day ("YYYY-MM-DD") of an instant at a UTC offset.
fn day_of(utc_ms: i64, offset_minutes: i32) -> String {
    let days = (utc_ms + i64::from(offset_minutes) * 60_000).div_euclid(86_400_000);
    // civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The name the recap groups an artist by, the same on every device and the server.
fn artist_key(artist: &str) -> String {
    artist.trim().to_lowercase()
}

fn album_key(artist: &str, album: &str) -> String {
    format!("{}|{}", artist_key(artist), album.trim().to_lowercase())
}

/// What the engine needs from the store, for one profile and year.
#[allow(clippy::too_many_arguments)]
fn input_of(
    store: &Store,
    profile_id: &str,
    year: i32,
    packs: &[String],
    southern: bool,
    utc_offset_minutes: i32,
    guest: bool,
    now: i64,
) -> Result<RecapInput, EncoreError> {
    let (from, to) = year_span(year);
    let rows = store.recap_plays(from, to)?;
    let mut album_palettes = BTreeMap::new();
    let mut album_track_counts = BTreeMap::new();
    let mut plays = Vec::with_capacity(rows.len());
    for r in rows {
        let key = r.album.as_deref().map(|a| album_key(&r.artist, a));
        if let (Some(k), Some(id)) = (&key, r.album_id)
            && !album_track_counts.contains_key(k)
        {
            album_track_counts.insert(k.clone(), store.album_song_count(id)?);
            if let Some(colours) = palette_colours(store, id)? {
                album_palettes.insert(k.clone(), colours);
            }
        }
        plays.push(Play {
            artist_key: artist_key(&r.artist),
            song_key: r.song_key,
            title: r.title,
            artist: r.artist,
            album_key: key,
            album: r.album,
            started_utc_ms: r.started_utc_ms,
            utc_offset_minutes: r.utc_offset_minutes,
            listened_ms: r.listened_ms,
            duration_ms: (r.duration_ms > 0).then_some(r.duration_ms),
            genre: r.genre,
            explicit: false,
            end_reason: Some(r.end_reason),
            started_from: Some(r.started_from),
            lyric_jumps: r
                .lyric_jumps
                .into_iter()
                .map(|(line_index, line_start_ms)| LyricJump {
                    line_index,
                    line_start_ms,
                })
                .collect(),
        });
    }
    // Artists heard before the year, merged by their recap name.
    let mut before: BTreeMap<String, (u64, i64)> = BTreeMap::new();
    for a in store.recap_artists_before(from)? {
        let e = before.entry(artist_key(&a.artist)).or_insert((0, i64::MIN));
        e.0 += a.plays;
        e.1 = e.1.max(a.last_played_utc_ms);
    }
    let artists_before = before
        .into_iter()
        .map(|(artist_key, (plays, last))| ArtistBefore {
            artist_key,
            plays,
            last_played_utc_ms: (last != i64::MIN).then_some(last),
        })
        .collect();
    // Earlier years kept on this device.
    let mut past = Vec::new();
    for y in (year - 6)..year {
        if let Some(json) = store.recs_state(&kept_key(y))? {
            past.push(past_of(y, &json));
        }
    }
    let cleared_since = store
        .state_number(encore_store::CLEARED_KEY)?
        .filter(|&t| t > from)
        .map(|t| day_of(t, utc_offset_minutes));
    Ok(RecapInput {
        year,
        profile_id: profile_id.to_owned(),
        kid_profile: false,
        // Guest records nothing, so it has no recap (board 41, states).
        recap_off: guest,
        counted_through_utc_ms: now,
        cleared_since,
        joined_day: None,
        packs: packs
            .iter()
            .map(|id| PackInfo {
                id: id.clone(),
                version: PACK_VERSION,
            })
            .collect(),
        past,
        album_palettes,
        album_track_counts,
        fingerprints: BTreeMap::new(),
        artists_before,
        hemisphere: if southern {
            Hemisphere::South
        } else {
            Hemisphere::North
        },
        plays,
    })
}

/// An album's cover colours as the palette sampled them, largest first ("#rrggbb").
fn palette_colours(store: &Store, album_id: i64) -> Result<Option<Vec<String>>, EncoreError> {
    let Some(cached) = store.cached_palette("album", album_id)? else {
        return Ok(None);
    };
    let Some(json) = cached.palette_json else {
        return Ok(None);
    };
    let Ok(p) = serde_json::from_str::<crate::palette::Palette>(&json) else {
        return Ok(None);
    };
    let colours: Vec<String> = p
        .sampled
        .iter()
        .map(|s| format!("#{}", s.colour.trim_start_matches('#').to_ascii_lowercase()))
        .collect();
    Ok((!colours.is_empty()).then_some(colours))
}

/// What a kept year's document tells the next year (its pack, lead hue, minutes and
/// personality), read leniently: a field that is missing is simply not known.
fn past_of(year: i32, json: &str) -> PastRecap {
    let doc: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let personality = doc["beats"]
        .as_array()
        .and_then(|b| b.iter().find(|b| b["id"] == "personality"))
        .map(|b| &b["data"]);
    PastRecap {
        year,
        pack: doc["pack"]["id"].as_str().map(String::from),
        lead_hue: doc["palette"]["leadHue"].as_u64().map(|h| h as u16),
        listened_ms: doc["totals"]["listenedMs"].as_u64(),
        personality_metric: personality
            .and_then(|p| p["metric"].as_str())
            .map(String::from),
        personality_value: personality
            .and_then(|p| p["value"].as_u64())
            .map(|v| v as u32),
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// The open profile's recap of `year` (Phase 11): the recap data document as JSON
    /// (schema 1, `docs/core/recap-engine.md`). `packs` are the design packs this app
    /// draws ("editorial", "tide", "bloom", "glass"); `southern` turns the seasons
    /// round; `utc_offset_minutes` (the device's now) dates a Clear listening history.
    /// A year built on or after 8 January is locked: kept and given back unchanged from
    /// then on. Async: a year of plays is read and counted off the caller's thread.
    pub async fn recap(
        &self,
        year: i32,
        packs: Vec<String>,
        southern: bool,
        utc_offset_minutes: i32,
    ) -> Result<String, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let guest = lock(&p.playback)?.log.mode() == ListeningMode::Guest;
            let input = {
                let store = lock(&p.store)?;
                if let Some(kept) = store.recs_state(&kept_key(year))? {
                    return Ok(kept);
                }
                input_of(
                    &store,
                    &p.id,
                    year,
                    &packs,
                    southern,
                    utc_offset_minutes,
                    guest,
                    now_ms(),
                )?
            };
            let data = build_recap(&input).map_err(EncoreError::internal)?;
            let json = data.to_json();
            if data.locked && !guest {
                lock(&p.store)?.set_recs_state(&kept_key(year), Some(&json))?;
            }
            Ok(json)
        })
        .await
    }

    /// The years with a kept (locked) recap on this device, newest first.
    pub fn recap_years(&self) -> Result<Vec<i32>, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        let this_year: i32 = day_of(now_ms(), 0)[..4].parse().unwrap_or(2026);
        let mut years = Vec::new();
        for y in (2020..=this_year).rev() {
            if store.recs_state(&kept_key(y))?.is_some() {
                years.push(y);
            }
        }
        Ok(years)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_spans_are_calendar_exact() {
        assert_eq!(utc_ms_of_day(1970, 1, 1), 0);
        assert_eq!(utc_ms_of_day(2026, 1, 1), 1_767_225_600_000);
        assert_eq!(day_of(1_767_225_600_000, 0), "2026-01-01");
        // An hour before midnight UTC, in a zone two hours ahead: the next day.
        assert_eq!(day_of(1_767_225_600_000 - 3_600_000, 120), "2026-01-01");
        assert_eq!(day_of(1_767_225_600_000 - 3_600_000, 0), "2025-12-31");
        assert_eq!(day_of(utc_ms_of_day(2028, 2, 29), 0), "2028-02-29");
        let (from, to) = year_span(2026);
        assert_eq!(to - from, 365 * 86_400_000 + 28 * 3_600_000);
    }

    #[test]
    fn artist_and_album_keys_ignore_case_and_spaces() {
        assert_eq!(artist_key(" Marigold Ave "), "marigold ave");
        assert_eq!(
            album_key("Marigold Ave", "Slow Burn"),
            "marigold ave|slow burn"
        );
    }
}
