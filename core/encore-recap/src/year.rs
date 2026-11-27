//! The counting rules (plan section 25) and one pass over the year's plays that
//! gathers every number the beats need. Integers only; every map is ordered.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::input::{Hemisphere, Play, RecapInput};
use crate::songs::group_keys;
use crate::time::{day_of, days_from_civil, hour_of, local_ms, weekday};

/// `recap.counting.minListenedSeconds`: plays under 2 s are not counted.
pub(crate) const MIN_LISTENED_MS: u64 = 2_000;
/// End reasons that never count: a handed-off part (the other device's play carries
/// the time on), an interruption (plan section 25) and a play that failed.
pub(crate) const EXCLUDED_END_REASONS: [&str; 3] = ["handedOff", "interrupted", "error"];
/// Offsets beyond this are not real time zones; such plays are not counted.
pub(crate) const MAX_OFFSET_MINUTES: i32 = 18 * 60;
/// A skip in the first 30 s, for the personality's "early skips".
pub(crate) const EARLY_SKIP_MS: u64 = 30_000;

/// Whether a play passes the counting rules that do not depend on the year.
pub(crate) fn counts(p: &Play) -> bool {
    p.listened_ms >= MIN_LISTENED_MS
        && p.utc_offset_minutes.abs() <= MAX_OFFSET_MINUTES
        && !p
            .end_reason
            .as_deref()
            .is_some_and(|r| EXCLUDED_END_REASONS.contains(&r))
}

/// A total order on plays over every field, so the input order never matters.
pub(crate) fn canonical_cmp(a: &Play, b: &Play) -> Ordering {
    (
        a.started_utc_ms,
        a.utc_offset_minutes,
        &a.song_key,
        a.listened_ms,
    )
        .cmp(&(
            b.started_utc_ms,
            b.utc_offset_minutes,
            &b.song_key,
            b.listened_ms,
        ))
        .then_with(|| {
            (&a.artist_key, &a.artist, &a.title, &a.album_key, &a.album).cmp(&(
                &b.artist_key,
                &b.artist,
                &b.title,
                &b.album_key,
                &b.album,
            ))
        })
        .then_with(|| {
            (
                a.duration_ms,
                &a.genre,
                a.explicit,
                &a.end_reason,
                &a.started_from,
            )
                .cmp(&(
                    b.duration_ms,
                    &b.genre,
                    b.explicit,
                    &b.end_reason,
                    &b.started_from,
                ))
        })
        .then_with(|| {
            let ja = a
                .lyric_jumps
                .iter()
                .map(|j| (j.line_index, j.line_start_ms));
            let jb = b
                .lyric_jumps
                .iter()
                .map(|j| (j.line_index, j.line_start_ms));
            ja.cmp(jb)
        })
}

/// The part of the day an hour falls in.
pub(crate) fn part_of_day(hour: u32) -> usize {
    match hour {
        5..=11 => PART_MORNING,
        12..=16 => PART_AFTERNOON,
        17..=21 => PART_EVENING,
        _ => PART_LATE_NIGHT,
    }
}
pub(crate) const PART_MORNING: usize = 0;
pub(crate) const PART_AFTERNOON: usize = 1;
pub(crate) const PART_EVENING: usize = 2;
pub(crate) const PART_LATE_NIGHT: usize = 3;
pub(crate) const PART_NAMES: [&str; 4] = ["morning", "afternoon", "evening", "lateNight"];

#[derive(Debug, Default)]
pub(crate) struct SongAgg<'a> {
    pub anchor: &'a str,
    /// The raw key played most (what a playlist should hold).
    pub key: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub album: Option<&'a str>,
    pub duration_ms: Option<u64>,
    pub plays: u64,
    pub ms: u64,
}

#[derive(Debug, Default)]
pub(crate) struct ArtistAgg<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub plays: u64,
    pub ms: u64,
    pub first_day: i64,
    pub first_utc: i64,
    pub months: [u64; 12],
}

#[derive(Debug, Default)]
pub(crate) struct AlbumAgg<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub artist: &'a str,
    pub plays: u64,
    pub ms: u64,
}

#[derive(Debug, Default)]
pub(crate) struct GenreAgg<'a> {
    pub name: &'a str,
    pub plays: u64,
}

/// What a profile is known to have heard before this year, per artist key.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Before {
    pub plays: u64,
    pub last_utc: Option<i64>,
}

/// Every number of the year, gathered in one pass.
#[derive(Debug, Default)]
pub(crate) struct Year<'a> {
    pub songs: Vec<SongAgg<'a>>,
    pub artists: Vec<ArtistAgg<'a>>,
    pub albums: Vec<AlbumAgg<'a>>,
    pub genres: Vec<GenreAgg<'a>>,
    /// Indexes ranked by plays, then listened time, then key.
    pub song_rank: Vec<usize>,
    /// The inverse of `song_rank`: a song's place in it.
    pub song_pos: Vec<usize>,
    pub artist_rank: Vec<usize>,
    pub album_rank: Vec<usize>,
    pub genre_rank: Vec<usize>,
    pub total_plays: u64,
    pub total_ms: u64,
    pub month_plays: [u64; 12],
    pub month_ms: [u64; 12],
    /// Monday 00:00 first.
    pub hour_week_plays: Vec<u64>,
    pub part_ms: [u64; 4],
    pub early_morning_ms: u64,
    pub weekend_ms: u64,
    pub active_days: BTreeSet<i64>,
    pub genre_plays: u64,
    pub month_genre: BTreeMap<(u32, usize), u64>,
    pub month_genre_total: [u64; 12],
    pub week_artist: BTreeMap<(i64, usize), u64>,
    pub week_total: BTreeMap<i64, u64>,
    pub day_artist: BTreeMap<(i64, usize), u64>,
    pub day_song: BTreeMap<(i64, usize), u64>,
    /// Season index (0 winter .. 3 autumn) and song.
    pub season_song: BTreeMap<(usize, usize), u64>,
    pub season_plays: [u64; 4],
    pub started_known: u64,
    pub from_album: u64,
    pub from_radio: u64,
    pub ended_known: u64,
    pub early_skips: u64,
    /// Plays whose song has a fingerprint, and per month (count, energy, valence).
    pub fp_plays: u64,
    pub fp_energy_sum: u64,
    pub fp_month: [(u64, u64, u64); 12],
    /// (song, line) -> (jumps, start ms -> count).
    pub lyric_lines: BTreeMap<(usize, u32), (u64, BTreeMap<u64, u64>)>,
    /// Times each album was played start to finish (album index).
    pub play_throughs: BTreeMap<usize, u64>,
    /// Artist key -> what was heard before this year.
    pub before: BTreeMap<&'a str, Before>,
    /// Plays of earlier years that were used (for the log hash).
    pub earlier: Vec<&'a Play>,
}

fn bump<K: Ord>(m: &mut BTreeMap<K, u64>, k: K, by: u64) {
    *m.entry(k).or_insert(0) += by;
}

/// The key with the highest count; ties go to the smallest key.
fn most<K: Ord + Copy>(m: &BTreeMap<K, u64>) -> Option<K> {
    let mut best: Option<(K, u64)> = None;
    for (k, &n) in m {
        if best.is_none_or(|(_, b)| n > b) {
            best = Some((*k, n));
        }
    }
    best.map(|(k, _)| k)
}

/// Indexes sorted by (plays desc, listened desc, key asc).
fn rank<'k>(n: usize, key: impl Fn(usize) -> (u64, u64, &'k str)) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    v.sort_by(|&a, &b| {
        let (pa, ma, ka) = key(a);
        let (pb, mb, kb) = key(b);
        pb.cmp(&pa).then(mb.cmp(&ma)).then(ka.cmp(kb))
    });
    v
}

/// The season of a month: 0 winter, 1 spring, 2 summer, 3 autumn.
pub(crate) fn season_of(month: u32, hemisphere: Hemisphere) -> usize {
    let north = match month {
        12 | 1 | 2 => 0,
        3..=5 => 1,
        6..=8 => 2,
        _ => 3,
    };
    match hemisphere {
        Hemisphere::North => north,
        Hemisphere::South => (north + 2) % 4,
    }
}

/// The plays of the year and of earlier years, after every counting rule.
pub(crate) struct Filtered<'a> {
    pub this_year: Vec<(&'a Play, i64)>,
    pub earlier: Vec<&'a Play>,
}

pub(crate) fn filter<'a>(input: &'a RecapInput, clear_day: Option<i64>) -> Filtered<'a> {
    let first = days_from_civil(i64::from(input.year), 1, 1);
    let last = days_from_civil(i64::from(input.year), 12, 31);
    let mut sorted: Vec<&Play> = input.plays.iter().collect();
    sorted.sort_by(|a, b| canonical_cmp(a, b));
    let mut out = Filtered {
        this_year: Vec::new(),
        earlier: Vec::new(),
    };
    for p in sorted {
        if !counts(p) || p.started_utc_ms > input.counted_through_utc_ms {
            continue;
        }
        if input.kid_profile && p.explicit {
            continue;
        }
        let day = day_of(local_ms(p.started_utc_ms, p.utc_offset_minutes));
        if clear_day.is_some_and(|c| day < c) {
            continue;
        }
        if day < first {
            out.earlier.push(p);
        } else if day <= last {
            out.this_year.push((p, day));
        }
    }
    out
}

pub(crate) fn gather<'a>(input: &'a RecapInput, f: Filtered<'a>) -> Year<'a> {
    let mut y = Year {
        hour_week_plays: vec![0; 168],
        ..Year::default()
    };

    // What was heard before this year: the input's list, then earlier plays.
    for a in &input.artists_before {
        let e = y.before.entry(a.artist_key.as_str()).or_default();
        e.plays = e.plays.max(a.plays);
        e.last_utc = e.last_utc.max(a.last_played_utc_ms);
    }
    let mut earlier_counts: BTreeMap<&str, (u64, i64)> = BTreeMap::new();
    for p in &f.earlier {
        let e = earlier_counts
            .entry(p.artist_key.as_str())
            .or_insert((0, i64::MIN));
        e.0 += 1;
        e.1 = e.1.max(p.started_utc_ms);
    }
    for (k, (n, last)) in earlier_counts {
        let e = y.before.entry(k).or_default();
        e.plays = e.plays.max(n);
        e.last_utc = e.last_utc.max(Some(last));
    }
    y.earlier = f.earlier;

    // Interning: indexes follow key order, so they are the same everywhere.
    let groups = group_keys(f.this_year.iter().map(|(p, _)| p.song_key.as_str()));
    let anchors: BTreeSet<&str> = groups.values().copied().collect();
    let song_idx: BTreeMap<&str, usize> =
        anchors.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let artist_keys: BTreeSet<&str> = f
        .this_year
        .iter()
        .map(|(p, _)| p.artist_key.as_str())
        .collect();
    let artist_idx: BTreeMap<&str, usize> = artist_keys
        .iter()
        .enumerate()
        .map(|(i, k)| (*k, i))
        .collect();
    let album_keys: BTreeSet<&str> = f
        .this_year
        .iter()
        .filter_map(|(p, _)| p.album_key.as_deref())
        .collect();
    let album_idx: BTreeMap<&str, usize> = album_keys
        .iter()
        .enumerate()
        .map(|(i, k)| (*k, i))
        .collect();
    let genre_names: BTreeSet<&str> = f
        .this_year
        .iter()
        .filter_map(|(p, _)| genre_of(p))
        .collect();
    let genre_idx: BTreeMap<&str, usize> = genre_names
        .iter()
        .enumerate()
        .map(|(i, k)| (*k, i))
        .collect();

    y.songs = anchors
        .iter()
        .map(|a| SongAgg {
            anchor: a,
            ..SongAgg::default()
        })
        .collect();
    y.artists = artist_keys
        .iter()
        .map(|k| ArtistAgg {
            key: k,
            first_day: i64::MAX,
            first_utc: i64::MAX,
            ..ArtistAgg::default()
        })
        .collect();
    y.albums = album_keys
        .iter()
        .map(|k| AlbumAgg {
            key: k,
            ..AlbumAgg::default()
        })
        .collect();
    y.genres = genre_names
        .iter()
        .map(|g| GenreAgg {
            name: g,
            ..GenreAgg::default()
        })
        .collect();

    // Variant counts for the names shown.
    let mut song_keys: Vec<BTreeMap<&str, u64>> = vec![BTreeMap::new(); y.songs.len()];
    let mut song_names: Vec<BTreeMap<(&str, &str), u64>> = vec![BTreeMap::new(); y.songs.len()];
    let mut song_albums: Vec<BTreeMap<Option<&str>, u64>> = vec![BTreeMap::new(); y.songs.len()];
    let mut song_durations: Vec<BTreeMap<u64, u64>> = vec![BTreeMap::new(); y.songs.len()];
    let mut artist_names: Vec<BTreeMap<&str, u64>> = vec![BTreeMap::new(); y.artists.len()];
    let mut album_names: Vec<BTreeMap<&str, u64>> = vec![BTreeMap::new(); y.albums.len()];
    let mut album_artists: Vec<BTreeMap<&str, u64>> = vec![BTreeMap::new(); y.albums.len()];

    // Album play-throughs: a run of consecutive plays of one album.
    let mut run: Option<(usize, BTreeSet<usize>)> = None;

    for (p, day) in f.this_year {
        let local = local_ms(p.started_utc_ms, p.utc_offset_minutes);
        let hour = hour_of(local);
        let wd = weekday(day);
        let (_, month, _) = crate::time::civil_from_days(day);
        let song = song_idx[groups[p.song_key.as_str()]];
        let artist = artist_idx[p.artist_key.as_str()];
        let album = p.album_key.as_deref().map(|k| album_idx[k]);
        let genre = genre_of(p).map(|g| genre_idx[g]);
        let mi = (month - 1) as usize;
        let ms = p.listened_ms;

        y.total_plays += 1;
        y.total_ms += ms;
        y.month_plays[mi] += 1;
        y.month_ms[mi] += ms;
        y.hour_week_plays[(wd * 24 + hour) as usize] += 1;
        y.part_ms[part_of_day(hour)] += ms;
        if (5..=9).contains(&hour) {
            y.early_morning_ms += ms;
        }
        if wd >= 5 {
            y.weekend_ms += ms;
        }
        y.active_days.insert(day);

        let s = &mut y.songs[song];
        s.plays += 1;
        s.ms += ms;
        bump(&mut song_keys[song], p.song_key.as_str(), 1);
        bump(
            &mut song_names[song],
            (p.title.as_str(), p.artist.as_str()),
            1,
        );
        bump(&mut song_albums[song], p.album.as_deref(), 1);
        if let Some(d) = p.duration_ms {
            bump(&mut song_durations[song], d, 1);
        }

        let a = &mut y.artists[artist];
        a.plays += 1;
        a.ms += ms;
        a.months[mi] += 1;
        if p.started_utc_ms < a.first_utc {
            a.first_utc = p.started_utc_ms;
            a.first_day = day;
        }
        bump(&mut artist_names[artist], p.artist.as_str(), 1);

        if let Some(al) = album {
            y.albums[al].plays += 1;
            y.albums[al].ms += ms;
            if let Some(n) = p.album.as_deref() {
                bump(&mut album_names[al], n, 1);
            }
            bump(&mut album_artists[al], p.artist.as_str(), 1);
        }
        if let Some(g) = genre {
            y.genres[g].plays += 1;
            y.genre_plays += 1;
            bump(&mut y.month_genre, (month, g), 1);
            y.month_genre_total[mi] += 1;
        }

        let week = day - i64::from(wd);
        bump(&mut y.week_artist, (week, artist), 1);
        bump(&mut y.week_total, week, 1);
        bump(&mut y.day_artist, (day, artist), 1);
        bump(&mut y.day_song, (day, song), 1);
        let season = season_of(month, input.hemisphere);
        bump(&mut y.season_song, (season, song), 1);
        y.season_plays[season] += 1;

        if let Some(from) = p.started_from.as_deref() {
            y.started_known += 1;
            match from {
                "album" => y.from_album += 1,
                "radio" | "mix" => y.from_radio += 1,
                _ => {}
            }
        }
        if let Some(end) = p.end_reason.as_deref() {
            y.ended_known += 1;
            if end == "skipped" && ms < EARLY_SKIP_MS {
                y.early_skips += 1;
            }
        }

        let fp = input
            .fingerprints
            .get(p.song_key.as_str())
            .or_else(|| input.fingerprints.get(y.songs[song].anchor));
        if let Some(fp) = fp {
            let (e, v) = (
                u64::from(fp.energy.min(1000)),
                u64::from(fp.valence.min(1000)),
            );
            y.fp_plays += 1;
            y.fp_energy_sum += e;
            let m = &mut y.fp_month[mi];
            m.0 += 1;
            m.1 += e;
            m.2 += v;
        }

        for j in &p.lyric_jumps {
            let e = y.lyric_lines.entry((song, j.line_index)).or_default();
            e.0 += 1;
            bump(&mut e.1, j.line_start_ms, 1);
        }

        // Play-throughs: consecutive plays of one album, not skipped, covering as
        // many different songs as the album has.
        match (album, &mut run) {
            (Some(al), Some((ra, songs))) if *ra == al => {
                if p.end_reason.as_deref() != Some("skipped") {
                    songs.insert(song);
                }
            }
            (Some(al), _) => {
                let mut songs = BTreeSet::new();
                if p.end_reason.as_deref() != Some("skipped") {
                    songs.insert(song);
                }
                run = Some((al, songs));
            }
            (None, _) => run = None,
        }
        if let Some((ra, songs)) = &run {
            let need = input
                .album_track_counts
                .get(y.albums[*ra].key)
                .copied()
                .unwrap_or(0);
            if need > 0 && songs.len() as u64 >= u64::from(need) {
                bump(&mut y.play_throughs, *ra, 1);
                run = None;
            }
        }

    }

    for (i, s) in y.songs.iter_mut().enumerate() {
        s.key = most(&song_keys[i]).unwrap_or(s.anchor);
        let (t, a) = most(&song_names[i]).unwrap_or(("", ""));
        s.title = t;
        s.artist = a;
        s.album = most(&song_albums[i]).flatten();
        s.duration_ms = most(&song_durations[i]);
    }
    for (i, a) in y.artists.iter_mut().enumerate() {
        a.name = most(&artist_names[i]).unwrap_or(a.key);
    }
    for (i, al) in y.albums.iter_mut().enumerate() {
        al.name = most(&album_names[i]).unwrap_or(al.key);
        al.artist = most(&album_artists[i]).unwrap_or("");
    }

    y.song_rank = rank(y.songs.len(), |i| {
        (y.songs[i].plays, y.songs[i].ms, y.songs[i].anchor)
    });
    y.song_pos = vec![0; y.songs.len()];
    for (pos, &s) in y.song_rank.iter().enumerate() {
        y.song_pos[s] = pos;
    }
    y.artist_rank = rank(y.artists.len(), |i| {
        (y.artists[i].plays, y.artists[i].ms, y.artists[i].key)
    });
    y.album_rank = rank(y.albums.len(), |i| {
        (y.albums[i].plays, y.albums[i].ms, y.albums[i].key)
    });
    y.genre_rank = rank(y.genres.len(), |i| (y.genres[i].plays, 0, y.genres[i].name));
    y
}

/// A play's genre, trimmed; None when empty.
fn genre_of(p: &Play) -> Option<&str> {
    p.genre.as_deref().map(str::trim).filter(|g| !g.is_empty())
}

/// a * 1000 / b, rounded half up; 0 when b is 0.
pub(crate) fn permille(a: u64, b: u64) -> u32 {
    if b == 0 {
        return 0;
    }
    ((u128::from(a) * 2000 + u128::from(b)) / (u128::from(b) * 2)) as u32
}

/// Permille to whole percent, rounded half up.
pub(crate) fn percent(pm: u32) -> u32 {
    (pm + 5) / 10
}

impl Year<'_> {
    /// The song played most in a season; ties go to the higher-ranked song.
    pub(crate) fn top_song_of_season(&self, season: usize) -> Option<(usize, u64)> {
        let mut best: Option<(usize, u64)> = None;
        for (&(s, song), &n) in self.season_song.range((season, 0)..(season + 1, 0)) {
            debug_assert_eq!(s, season);
            let better = match best {
                None => true,
                Some((b, bn)) => n > bn || (n == bn && self.song_pos[song] < self.song_pos[b]),
            };
            if better {
                best = Some((song, n));
            }
        }
        best
    }
}
