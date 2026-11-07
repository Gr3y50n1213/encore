//! The daily mixes and the Home shelves (plan section 7.1 version 1, board 15 and
//! 23, the cold-start table of section 21).
//!
//! Version 1 finds moods from how the user listens, not from audio (that comes in
//! Phase 10): each enjoyed song is placed by when it is played (time of day,
//! weekday or weekend), the mood family of its genres and its era, and those
//! points are clustered. Slots: the two strongest moods, then (after 30 days of
//! history) the time-of-day mood, the mood the user has drifted toward in the last
//! 30 days, and a stretch mix with about 30% songs new to the user. Fewer distinct
//! moods make fewer mixes, down to two. Each mix is 25 to 40 songs, at most 2 per
//! artist and 4 per album, the user's most played kept to 40%, overlapping
//! another mix by at most 20%, ordered so no artist comes twice within 3 songs.
//!
//! Cold start: 0 plays gives a Starter mix from the media server's favourites and
//! most played; 1 to 49 plays gives up to three mixes by mood family; 50 or more
//! the full set.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::det::Rng;
use crate::model::Model;
use crate::rank::{Pick, Reason, reasons};
use crate::titles;
use crate::{Catalog, DAY_MS, Params, bucket_of, weekend};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MixSlot {
    Strongest,
    Second,
    TimeOfDay,
    Drift,
    Stretch,
    /// 1 to 49 plays: a mix by mood family.
    Family,
    /// 0 plays: from the media server's favourites and most played.
    Starter,
}

impl MixSlot {
    fn name(self) -> &'static str {
        match self {
            MixSlot::Strongest => "strongest",
            MixSlot::Second => "second",
            MixSlot::TimeOfDay => "timeOfDay",
            MixSlot::Drift => "drift",
            MixSlot::Stretch => "stretch",
            MixSlot::Family => "family",
            MixSlot::Starter => "starter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    /// Stable for the day and slot, so pins and hides find it.
    pub id: String,
    pub slot: MixSlot,
    pub title: String,
    pub subtitle: String,
    /// (bucket, weekend) for a time-of-day mix.
    pub moment: Option<(u8, bool)>,
    pub songs: Vec<Pick>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixSet {
    pub made_utc_ms: i64,
    /// The local day number (days since 1970) it was made for.
    pub day: i64,
    pub plays_counted: u32,
    /// Under `full_from_plays`: "Encore is still learning".
    pub learning: bool,
    pub mixes: Vec<Mix>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelves {
    /// Loved songs not heard for 60+ days (12), else random liked songs.
    pub rediscover: Vec<Pick>,
    /// Never played, matching the user's taste.
    pub deep_cuts: Vec<Pick>,
}

/// What the previous build gives the next: yesterday's set (for the 30% stability
/// rule), today's hidden mixes and the titles of the last 14 days.
#[derive(Debug, Clone, Default)]
pub struct Previous<'a> {
    pub set: Option<&'a MixSet>,
    pub hidden: &'a [String],
    pub recent_titles: &'a [String],
    /// Regenerate: ignore the stability rule.
    pub regenerate: bool,
}

const DIMS: usize = 8 + 7 + 10;
const FAMILIES: [&str; 7] = ["calm", "warm", "bright", "loud", "groove", "dark", "any"];

fn local_day(now: i64, offset: i32) -> i64 {
    (now + i64::from(offset) * 60_000).div_euclid(DAY_MS)
}

fn song_family(cat: &Catalog, i: usize) -> &'static str {
    let g: Vec<&str> = cat.get(i).genres.iter().map(String::as_str).collect();
    titles::dominant_family(&g)
}

fn vector(model: &Model, cat: &Catalog, i: usize) -> [f64; DIMS] {
    let mut v = [0.0; DIMS];
    let when = &model.facts[i].when;
    let total: f64 = when.iter().sum();
    if total > 0.0 {
        for (d, w) in when.iter().enumerate() {
            v[d] = w / total;
        }
    } else {
        for x in v.iter_mut().take(8) {
            *x = 0.125;
        }
    }
    let fam = song_family(cat, i);
    if let Some(f) = FAMILIES.iter().position(|x| *x == fam) {
        v[8 + f] = 0.6;
    }
    if let Some(y) = cat.get(i).year {
        let decade = ((y - 1950) / 10).clamp(0, 9) as usize;
        v[15 + decade] = 0.3;
    }
    v
}

fn dist(a: &[f64; DIMS], b: &[f64; DIMS]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

struct Cluster {
    centre: [f64; DIMS],
    members: Vec<usize>,
    weight: f64,
}

/// Deterministic k-means: the first centre is the best-liked song, each next one
/// the point farthest from every centre so far, then 15 rounds.
fn cluster(points: &[(usize, [f64; DIMS], f64)], k: usize) -> Vec<Cluster> {
    let mut centres: Vec<[f64; DIMS]> = vec![points[0].1];
    while centres.len() < k {
        let far = points
            .iter()
            .enumerate()
            .map(|(x, p)| {
                (
                    x,
                    centres
                        .iter()
                        .map(|c| dist(c, &p.1))
                        .fold(f64::INFINITY, f64::min),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
        match far {
            Some((x, d)) if d > 1e-9 => centres.push(points[x].1),
            _ => break,
        }
    }
    let mut assign = vec![0usize; points.len()];
    for _ in 0..15 {
        for (x, p) in points.iter().enumerate() {
            assign[x] = (0..centres.len())
                .min_by(|a, b| {
                    dist(&centres[*a], &p.1)
                        .total_cmp(&dist(&centres[*b], &p.1))
                        .then(a.cmp(b))
                })
                .unwrap_or(0);
        }
        for (c, centre) in centres.iter_mut().enumerate() {
            let mine: Vec<&[f64; DIMS]> = points
                .iter()
                .zip(&assign)
                .filter(|(_, a)| **a == c)
                .map(|(p, _)| &p.1)
                .collect();
            if mine.is_empty() {
                continue;
            }
            let mut m = [0.0; DIMS];
            for v in &mine {
                for d in 0..DIMS {
                    m[d] += v[d];
                }
            }
            for x in m.iter_mut() {
                *x /= mine.len() as f64;
            }
            *centre = m;
        }
    }
    let mut out: Vec<Cluster> = centres
        .into_iter()
        .enumerate()
        .map(|(c, centre)| {
            let mut members: Vec<(usize, f64, f64)> = points
                .iter()
                .zip(&assign)
                .filter(|(_, a)| **a == c)
                .map(|(p, _)| (p.0, dist(&centre, &p.1), p.2))
                .collect();
            members.sort_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(b.2.total_cmp(&a.2))
                    .then(a.0.cmp(&b.0))
            });
            let weight = members.iter().map(|m| m.2).sum();
            Cluster {
                centre,
                members: members.into_iter().map(|m| m.0).collect(),
                weight,
            }
        })
        .filter(|c| !c.members.is_empty())
        .collect();
    out.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then(a.members[0].cmp(&b.members[0]))
    });
    out
}

/// Fill a mix from ranked songs, keeping the caps, the most-played share and the
/// overlap limit; then order it so no artist repeats within the spacing.
#[allow(clippy::too_many_arguments)]
fn fill(
    model: &Model,
    cat: &Catalog,
    p: &Params,
    ranked: &[usize],
    keep: &[usize],
    used: &[HashSet<usize>],
    most_played: &HashSet<usize>,
    target: usize,
) -> Vec<usize> {
    let bucket = bucket_of(model.now_utc_ms, model.utc_offset_minutes);
    let mut out: Vec<usize> = Vec::new();
    let mut per_artist: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut per_album: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let max_most = target * p.most_played_max_percent as usize / 100;
    let mut most = 0;
    for &i in keep.iter().chain(ranked) {
        if out.len() >= target {
            break;
        }
        if out.contains(&i) || model.excluded(cat, i, bucket) || model.skip_rate(cat, i) > 0.4 {
            continue;
        }
        // Overlap: a song already in another mix only while under the limit.
        let shared = out
            .iter()
            .filter(|s| used.iter().any(|u| u.contains(s)))
            .count();
        if used.iter().any(|u| u.contains(&i))
            && (shared + 1) * 100 > target * p.mix_max_overlap_percent as usize
        {
            continue;
        }
        let s = cat.get(i);
        let artist = s.artists.first().map_or("", String::as_str);
        if per_artist.get(artist).copied().unwrap_or(0) >= p.per_artist_in_mix {
            continue;
        }
        let album = s.album.as_deref().unwrap_or("");
        if !album.is_empty() && per_album.get(album).copied().unwrap_or(0) >= p.per_album_in_mix {
            continue;
        }
        if most_played.contains(&i) {
            if most >= max_most {
                continue;
            }
            most += 1;
        }
        *per_artist.entry(artist).or_default() += 1;
        if !album.is_empty() {
            *per_album.entry(album).or_default() += 1;
        }
        out.push(i);
    }
    order(cat, p, out)
}

/// Flow, version 1: by era (no tempo or energy until audio arrives), with no
/// artist twice within the spacing.
fn order(cat: &Catalog, p: &Params, mut songs: Vec<usize>) -> Vec<usize> {
    songs.sort_by(|a, b| {
        cat.get(*a)
            .year
            .unwrap_or(0)
            .cmp(&cat.get(*b).year.unwrap_or(0))
            .then(a.cmp(b))
    });
    let near = |a: usize, b: usize| {
        cat.get(a)
            .artists
            .iter()
            .any(|x| cat.get(b).artists.contains(x))
    };
    let gap = p.artist_spacing.saturating_sub(1);
    // Whether song i fits at position k of `out` (inserted there).
    let fits = |out: &[usize], k: usize, i: usize| {
        let before = out[k.saturating_sub(gap)..k].iter().any(|&o| near(o, i));
        let after = out[k..(k + gap).min(out.len())].iter().any(|&o| near(o, i));
        !before && !after
    };
    let mut out: Vec<usize> = Vec::new();
    // Songs with nowhere to go yet wait, and are tried again after each placement.
    let mut waiting: Vec<usize> = Vec::new();
    let place = |out: &mut Vec<usize>, i: usize| -> bool {
        if fits(out, out.len(), i) {
            out.push(i);
            true
        } else if let Some(k) = (0..out.len()).find(|&k| fits(out, k, i)) {
            out.insert(k, i);
            true
        } else {
            false
        }
    };
    for i in songs {
        if place(&mut out, i) {
            let mut x = 0;
            while x < waiting.len() {
                if place(&mut out, waiting[x]) {
                    waiting.remove(x);
                    x = 0;
                } else {
                    x += 1;
                }
            }
        } else {
            waiting.push(i);
        }
    }
    // Whatever still can't fit (one artist only, say) goes at the end.
    out.extend(waiting);
    out
}

fn picks(model: &Model, cat: &Catalog, songs: &[usize], new_to_you: &HashSet<usize>) -> Vec<Pick> {
    songs
        .iter()
        .map(|&i| Pick {
            song_key: cat.get(i).key.clone(),
            exploration: new_to_you.contains(&i),
            reasons: reasons(model, cat, i, None, new_to_you.contains(&i)),
        })
        .collect()
}

/// The daily mixes (Home's Made for you and the Mixes tab).
pub fn build_mixes(model: &Model, cat: &Catalog, p: &Params, prev: &Previous<'_>) -> MixSet {
    let day = local_day(model.now_utc_ms, model.utc_offset_minutes);
    let mut set = MixSet {
        made_utc_ms: model.now_utc_ms,
        day,
        plays_counted: model.plays_counted,
        learning: model.plays_counted < p.full_from_plays,
        mixes: Vec::new(),
    };
    let n = cat.len();
    if n == 0 {
        return set;
    }
    let liking: Vec<f64> = (0..n).map(|i| model.liking(cat, i)).collect();
    let by_liking = |mut v: Vec<usize>| {
        v.sort_by(|a, b| liking[*b].total_cmp(&liking[*a]).then(a.cmp(b)));
        v
    };
    let mut row: Vec<String> = Vec::new();
    let mut used: Vec<HashSet<usize>> = Vec::new();
    // A small library can't fill 25 songs at 2 per artist: its mixes are as long
    // as the caps allow (at least 10), never made by relaxing the caps.
    let mut per_artist: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for s in cat.songs() {
        if let Some(a) = s.artists.first() {
            *per_artist.entry(a.as_str()).or_default() += 1;
        }
    }
    let reachable: usize = per_artist
        .values()
        .map(|c| (*c).min(p.per_artist_in_mix))
        .sum();
    let min_songs = p.mix_min_songs.min(reachable).max(10);
    let no_new = HashSet::new();
    let push = |set: &mut MixSet,
                slot: MixSlot,
                moment,
                family: &str,
                songs: Vec<usize>,
                new: &HashSet<usize>,
                row: &mut Vec<String>,
                used: &mut Vec<HashSet<usize>>| {
        if songs.len() < min_songs {
            return;
        }
        let id = format!("mix-{day}-{}", slot.name());
        if prev.hidden.contains(&id) {
            return;
        }
        let title = titles::make(family, moment, row, prev.recent_titles, Rng::seed_of(&id));
        row.push(title.clone());
        let artists: Vec<&str> = songs
            .iter()
            .filter_map(|&i| cat.get(i).artists.first().map(String::as_str))
            .collect();
        used.push(songs.iter().copied().collect());
        set.mixes.push(Mix {
            subtitle: titles::subtitle(&artists),
            songs: picks(model, cat, &songs, new),
            id,
            slot,
            title,
            moment,
        });
    };

    // Yesterday's songs for a slot (stability: change at most 30%).
    let kept = |slot: MixSlot| -> Vec<usize> {
        if prev.regenerate {
            return Vec::new();
        }
        let Some(old) = prev
            .set
            .and_then(|s| s.mixes.iter().find(|m| m.slot == slot))
        else {
            return Vec::new();
        };
        let keep = old.songs.len() * (100 - p.shelf_change_max_percent as usize) / 100;
        old.songs
            .iter()
            .filter_map(|s| cat.find(&s.song_key))
            .take(keep)
            .collect()
    };

    if model.plays_counted == 0 {
        // Starter: the media server's favourites and most played.
        let mut seeds: Vec<usize> = (0..n)
            .filter(|&i| cat.get(i).server_favorite || cat.get(i).server_play_count > 0)
            .collect();
        seeds.sort_by(|a, b| {
            let (x, y) = (cat.get(*a), cat.get(*b));
            (y.server_favorite, y.server_play_count)
                .cmp(&(x.server_favorite, x.server_play_count))
                .then(a.cmp(b))
        });
        // Then the rest of the library by liking: the server's counts make its
        // favourite artists' other songs rank high.
        let seen: HashSet<usize> = seeds.iter().copied().collect();
        seeds.extend(by_liking((0..n).filter(|i| !seen.contains(i)).collect()));
        let mostp: HashSet<usize> = HashSet::new();
        let songs = fill(
            model,
            cat,
            p,
            &seeds,
            &kept(MixSlot::Starter),
            &used,
            &mostp,
            p.mix_max_songs,
        );
        push(
            &mut set,
            MixSlot::Starter,
            None,
            "any",
            songs,
            &no_new,
            &mut row,
            &mut used,
        );
        return set;
    }

    if set.learning {
        // Up to three mixes by mood family, from what the user has liked so far.
        let mut families: Vec<(&str, Vec<usize>)> = Vec::new();
        for i in by_liking((0..n).collect()) {
            let f = song_family(cat, i);
            match families.iter_mut().find(|(k, _)| *k == f) {
                Some((_, v)) => v.push(i),
                None => families.push((f, vec![i])),
            }
        }
        families.sort_by(|a, b| {
            let wa: f64 = a.1.iter().take(40).map(|i| liking[*i]).sum();
            let wb: f64 = b.1.iter().take(40).map(|i| liking[*i]).sum();
            wb.total_cmp(&wa).then(a.0.cmp(b.0))
        });
        for (x, (fam, list)) in families
            .into_iter()
            .filter(|(f, _)| *f != "any")
            .take(3)
            .enumerate()
        {
            let slot = MixSlot::Family;
            let mostp = HashSet::new();
            // The family's songs first, then the rest of the library by liking.
            let mut list = list;
            let seen: HashSet<usize> = list.iter().copied().collect();
            list.extend(by_liking((0..n).filter(|i| !seen.contains(i)).collect()));
            let songs = fill(
                model,
                cat,
                p,
                &list,
                &[],
                &used,
                &mostp,
                p.mix_max_songs.min(30),
            );
            let mut tmp = MixSet {
                mixes: Vec::new(),
                ..set.clone()
            };
            push(
                &mut tmp, slot, None, fam, songs, &no_new, &mut row, &mut used,
            );
            if let Some(mut m) = tmp.mixes.pop() {
                m.id = format!("mix-{day}-family-{x}");
                set.mixes.push(m);
            }
        }
        return set;
    }

    // The full set: cluster the songs the user enjoys.
    let mut enjoyed: Vec<usize> = (0..n)
        .filter(|&i| model.facts[i].finishes > 0 || model.facts[i].liked)
        .collect();
    enjoyed = by_liking(enjoyed);
    enjoyed.truncate(2000);
    if enjoyed.len() < min_songs {
        return set;
    }
    let points: Vec<(usize, [f64; DIMS], f64)> = enjoyed
        .iter()
        .map(|&i| (i, vector(model, cat, i), liking[i]))
        .collect();
    let k = (enjoyed.len() / 15).clamp(2, 8);
    let clusters = cluster(&points, k);
    let mut plays_sorted: Vec<(u32, usize)> =
        enjoyed.iter().map(|&i| (model.facts[i].plays, i)).collect();
    plays_sorted.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let most_played: HashSet<usize> = plays_sorted
        .iter()
        .take(enjoyed.len() / 10 + 1)
        .map(|x| x.1)
        .collect();

    // Distinct moods only: a cluster too close to one already chosen is skipped.
    let mut chosen: Vec<usize> = Vec::new();
    let distinct = |c: usize, chosen: &[usize]| {
        chosen
            .iter()
            .all(|&o| dist(&clusters[o].centre, &clusters[c].centre) > 0.05)
    };
    let family_of_cluster = |c: &Cluster| {
        let g: Vec<&str> = c
            .members
            .iter()
            .take(40)
            .flat_map(|&i| cat.get(i).genres.iter().map(String::as_str))
            .collect();
        titles::dominant_family(&g)
    };
    let target = p
        .mix_max_songs
        .min((enjoyed.len() / 2).max(p.mix_min_songs));
    let extend = |c: &Cluster| -> Vec<usize> {
        // The cluster's songs, then the nearest other enjoyed songs, then the rest of
        // the library in the same mood family by liking (the caps of 2 per artist
        // need more artists than a narrow taste has enjoyed).
        let mut list = c.members.clone();
        let mut rest: Vec<(usize, f64)> = points
            .iter()
            .filter(|p| !c.members.contains(&p.0))
            .map(|p| (p.0, dist(&c.centre, &p.1)))
            .collect();
        rest.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        list.extend(rest.into_iter().map(|r| r.0));
        let fam = family_of_cluster(c);
        let seen: HashSet<usize> = list.iter().copied().collect();
        list.extend(by_liking(
            (0..n)
                .filter(|i| !seen.contains(i) && song_family(cat, *i) == fam)
                .collect(),
        ));
        // Last, anything else by liking: a small library may not have enough
        // artists in one mood family for the caps.
        let seen: HashSet<usize> = list.iter().copied().collect();
        list.extend(by_liking((0..n).filter(|i| !seen.contains(i)).collect()));
        list
    };
    for slot in [MixSlot::Strongest, MixSlot::Second] {
        if let Some(c) = (0..clusters.len()).find(|&c| !chosen.contains(&c) && distinct(c, &chosen))
        {
            chosen.push(c);
            let songs = fill(
                model,
                cat,
                p,
                &extend(&clusters[c]),
                &kept(slot),
                &used,
                &most_played,
                target,
            );
            push(
                &mut set,
                slot,
                None,
                family_of_cluster(&clusters[c]),
                songs,
                &no_new,
                &mut row,
                &mut used,
            );
        }
    }
    let month_old = model
        .first_play_utc_ms
        .is_some_and(|t| model.now_utc_ms - t >= p.drift_from_days * DAY_MS);
    if month_old {
        // The time of day this kind of day is listened to most.
        let wk = weekend(model.now_utc_ms, model.utc_offset_minutes);
        let mut slots = [0.0f64; 4];
        for i in &enjoyed {
            for (b, slot) in slots.iter_mut().enumerate() {
                *slot += model.facts[*i].when[b * 2 + usize::from(wk)];
            }
        }
        let b = (0..4)
            .max_by(|x, y| slots[*x].total_cmp(&slots[*y]).then(y.cmp(x)))
            .unwrap_or(3);
        let dim = b * 2 + usize::from(wk);
        if let Some(c) = (0..clusters.len())
            .filter(|&c| !chosen.contains(&c) && distinct(c, &chosen))
            .max_by(|x, y| {
                clusters[*x].centre[dim]
                    .total_cmp(&clusters[*y].centre[dim])
                    .then(y.cmp(x))
            })
        {
            chosen.push(c);
            let songs = fill(
                model,
                cat,
                p,
                &extend(&clusters[c]),
                &kept(MixSlot::TimeOfDay),
                &used,
                &most_played,
                target,
            );
            push(
                &mut set,
                MixSlot::TimeOfDay,
                Some((b as u8, wk)),
                family_of_cluster(&clusters[c]),
                songs,
                &no_new,
                &mut row,
                &mut used,
            );
        }
        // Drift: the cluster whose songs were heard most in the last 30 days, compared
        // with before.
        let recent = |i: usize| {
            model.facts[i]
                .last_heard_utc_ms
                .is_some_and(|t| model.now_utc_ms - t < p.drift_from_days * DAY_MS)
        };
        if let Some(c) = (0..clusters.len())
            .filter(|&c| !chosen.contains(&c) && distinct(c, &chosen))
            .map(|c| {
                let m = &clusters[c].members;
                (
                    c,
                    m.iter().filter(|&&i| recent(i)).count() as f64 / m.len() as f64,
                )
            })
            .filter(|(_, share)| *share > 0.3)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|(c, _)| c)
        {
            chosen.push(c);
            let songs = fill(
                model,
                cat,
                p,
                &extend(&clusters[c]),
                &kept(MixSlot::Drift),
                &used,
                &most_played,
                target,
            );
            push(
                &mut set,
                MixSlot::Drift,
                None,
                family_of_cluster(&clusters[c]),
                songs,
                &no_new,
                &mut row,
                &mut used,
            );
        }
        // Stretch: the strongest mood, with about 30% songs new to the user (never
        // played, or not heard for 60+ days), nearest the mood by liking prior.
        let c = chosen.first().copied().unwrap_or(0);
        let fam = family_of_cluster(&clusters[c]);
        let mut fresh: Vec<usize> = (0..n)
            .filter(|&i| {
                let f = &model.facts[i];
                (f.plays == 0
                    || f.last_heard_utc_ms
                        .is_some_and(|t| model.now_utc_ms - t > p.rediscover_after_days * DAY_MS))
                    && song_family(cat, i) == fam
            })
            .collect();
        fresh = by_liking(fresh);
        let new_count = target * p.stretch_new_percent as usize / 100;
        let new: HashSet<usize> = fresh.iter().take(new_count).copied().collect();
        let mut list: Vec<usize> = fresh.into_iter().take(new_count).collect();
        list.extend(extend(&clusters[c]));
        let songs = fill(
            model,
            cat,
            p,
            &list,
            &kept(MixSlot::Stretch),
            &used,
            &most_played,
            target,
        );
        push(
            &mut set,
            MixSlot::Stretch,
            None,
            fam,
            songs,
            &new,
            &mut row,
            &mut used,
        );
    }
    set.mixes.truncate(p.mix_count);
    set
}

/// The Home shelves that aren't mixes.
pub fn build_shelves(model: &Model, cat: &Catalog, p: &Params) -> Shelves {
    let n = cat.len();
    let mut rng = Rng::new(Rng::seed_of(&format!(
        "shelves-{}",
        local_day(model.now_utc_ms, model.utc_offset_minutes)
    )));
    let liked: Vec<usize> = (0..n).filter(|&i| model.facts[i].liked).collect();
    let old = |i: &usize| {
        model.facts[*i]
            .last_heard_utc_ms
            .is_none_or(|t| model.now_utc_ms - t > p.rediscover_after_days * DAY_MS)
    };
    let mut rediscover: Vec<usize> = liked.iter().copied().filter(old).collect();
    if rediscover.is_empty() {
        // Fallback (section 21): random liked songs.
        rediscover = liked.clone();
    }
    rng.shuffle(&mut rediscover);
    rediscover.truncate(p.rediscover_count);
    let mut deep: Vec<usize> = (0..n)
        .filter(|&i| {
            model.facts[i].plays == 0 && cat.get(i).server_play_count == 0 && !model.facts[i].liked
        })
        .collect();
    deep.sort_by(|a, b| {
        model
            .liking(cat, *b)
            .total_cmp(&model.liking(cat, *a))
            .then(a.cmp(b))
    });
    deep.truncate(p.mix_min_songs);
    let none = HashSet::new();
    let all: HashSet<usize> = deep.iter().copied().collect();
    Shelves {
        rediscover: rediscover
            .iter()
            .map(|&i| Pick {
                song_key: cat.get(i).key.clone(),
                exploration: false,
                reasons: {
                    let days = model.facts[i]
                        .last_heard_utc_ms
                        .map(|t| ((model.now_utc_ms - t).max(0) / DAY_MS) as u32);
                    match days {
                        Some(d) if d >= 60 => vec![Reason::Rediscover { days: d }],
                        _ => vec![Reason::YouLikedIt],
                    }
                },
            })
            .collect(),
        deep_cuts: picks(
            model,
            cat,
            &deep,
            if deep.is_empty() { &none } else { &all },
        ),
    }
}
