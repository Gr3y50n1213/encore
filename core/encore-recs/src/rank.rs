//! Candidates, the hand-tuned score, shaping and reasons (board 37, steps 4 to 7):
//! autoplay, radio, Smart shuffle and Why this song.
//!
//! Score, until a trained model exists (section 21, rule 2): 0.35 liking + 0.20
//! session fit + 0.15 time-of-day fit + 0.10 freshness - 0.20 recent skip rate in
//! similar contexts. Shaping: Thompson-sampled exploration slots (never the first,
//! never two in a row, only after a song the user is likely to finish), maximal
//! marginal relevance at `diversity_lambda`, and no artist twice within
//! `artist_spacing` songs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::det::Rng;
use crate::model::Model;
use crate::{Catalog, Params, bucket_of, weekend};

/// How many songs that sound most like each seed join the candidates (a starting value).
const SOUND_NEIGHBOURS: usize = 60;
/// How much a jump in tempo and energy from the song before counts against a pick, next to
/// the diversity trade-off (a starting value; the plan's "flow", section 21).
const FLOW_WEIGHT: f64 = 0.15;

/// What a request knows about the moment (the queue, the session).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    /// The current song first, then the last songs finished (song keys).
    pub seeds: Vec<String>,
    /// Never pick these (queued, upcoming, just played).
    pub exclude: Vec<String>,
    /// App-chosen songs skipped early in a row this session.
    pub early_skips_in_a_row: u32,
    /// Songs skipped early this session: the session moves away from them.
    pub pushed_away: Vec<String>,
    /// A song liked or replayed this session: the session moves toward it, and
    /// the next two picks come from its neighbours.
    pub pulled_toward: Option<String>,
    /// Exploration share in percent (Familiar 5, Balanced 10, Adventurous 20).
    pub exploration_percent: u32,
    /// Offline: only these songs can be picked.
    pub only: Option<Vec<String>>,
    pub hide_explicit: bool,
    pub seed: u64,
}

/// A reason, as facts for the app to word (board 38: plain, checkable facts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Reason {
    /// "You finished {song} {finished} of the last {of} times, and this sounds like it."
    FinishedOften {
        song_key: String,
        finished: u32,
        of: u32,
    },
    /// "You often play this with {song}."
    PlayedTogether { song_key: String },
    /// "You often play songs like this in the {bucket}." (0 night .. 3 evening)
    TimeOfDay { bucket: u8 },
    /// "You liked this."
    YouLikedIt,
    /// "You loved this and haven't played it for {days} days."
    Rediscover { days: u32 },
    /// "More from {artist}, whom you play often."
    SameArtist { artist: String },
    /// "Like the {genre} you play."
    SameGenre { genre: String },
    /// "Played {count} times in your other apps."
    PlayedElsewhere { count: u32 },
    /// "New to you: picked so Encore keeps learning."
    NewToYou,
    /// "Added to your library recently."
    RecentlyAdded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub song_key: String,
    /// One of the exploration slots.
    pub exploration: bool,
    /// The top two reasons, strongest first.
    pub reasons: Vec<Reason>,
}

/// Every pool, then the hard filters (section 21, rule 1). Indexes, sorted.
fn candidates(
    model: &Model,
    cat: &Catalog,
    ctx: &Context,
    p: &Params,
    rng: &mut Rng,
) -> Vec<usize> {
    let bucket = bucket_of(model.now_utc_ms, model.utc_offset_minutes);
    // A song the session was pulled toward counts as a seed for the pools.
    let seeds: Vec<usize> = ctx
        .seeds
        .iter()
        .chain(ctx.pulled_toward.iter())
        .filter_map(|k| cat.find(k))
        .collect();
    let mut pool: Vec<usize> = Vec::new();
    for &s in &seeds {
        pool.extend(model.colisten[s].iter().map(|(j, _)| *j));
    }
    let seed_artists: HashSet<&str> = seeds
        .iter()
        .flat_map(|&s| cat.get(s).artists.iter().map(String::as_str))
        .collect();
    let seed_genres: HashSet<String> = seeds
        .iter()
        .flat_map(|&s| cat.get(s).genres.iter().map(|g| g.to_lowercase()))
        .collect();
    let mut same_genre = Vec::new();
    for (i, s) in cat.songs().iter().enumerate() {
        if s.artists.iter().any(|a| seed_artists.contains(a.as_str())) {
            pool.push(i);
        } else if s
            .genres
            .iter()
            .any(|g| seed_genres.contains(&g.to_lowercase()))
        {
            same_genre.push(i);
        }
        let f = &model.facts[i];
        let rediscover = f.liked
            && f.last_heard_utc_ms
                .is_none_or(|t| model.now_utc_ms - t > p.rediscover_after_days * crate::DAY_MS);
        let recent = s
            .added_utc_ms
            .is_some_and(|t| model.now_utc_ms - t < 30 * crate::DAY_MS);
        if rediscover || recent {
            pool.push(i);
        }
    }
    // A sample of the same genres (a big library has thousands).
    rng.shuffle(&mut same_genre);
    pool.extend(same_genre.into_iter().take(p.candidates / 2));
    // Phase 10: what sounds like the session, whatever it is tagged (exact scan, no randomness).
    for &s in &seeds {
        pool.extend(cat.nearest_by_sound(s, SOUND_NEIGHBOURS));
    }
    if seeds.is_empty() || pool.len() < 50 {
        // Nothing to go on: what the user likes most, then the rest by liking.
        // Liking computed once per song, not once per comparison.
        let mut by_liking: Vec<(f64, usize)> =
            (0..cat.len()).map(|i| (model.liking(cat, i), i)).collect();
        by_liking.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        pool.extend(by_liking.into_iter().take(p.candidates).map(|x| x.1));
    }
    let exclude: HashSet<&str> = ctx
        .exclude
        .iter()
        .chain(&ctx.seeds)
        .map(String::as_str)
        .collect();
    let only: Option<HashSet<&str>> = ctx
        .only
        .as_ref()
        .map(|o| o.iter().map(String::as_str).collect());
    pool.sort_unstable();
    pool.dedup();
    pool.retain(|&i| {
        let s = cat.get(i);
        let refused = exclude.contains(s.key.as_str())
            || model.excluded(cat, i, bucket)
            || (ctx.hide_explicit && s.explicit)
            || only.as_ref().is_some_and(|o| !o.contains(s.key.as_str()))
            || (!model.facts[i].liked && model.heard_within(i, p.recent_hours_filter));
        !refused
    });
    pool.truncate(p.candidates.max(1) * 4);
    pool
}

struct Scored {
    i: usize,
    score: f64,
    fit: f64,
    best_seed: Option<usize>,
}

fn session_fit(model: &Model, cat: &Catalog, ctx: &Context, i: usize) -> (f64, Option<usize>) {
    let mut best = 0.0;
    let mut best_seed = None;
    for (rank, k) in ctx.seeds.iter().enumerate() {
        if let Some(s) = cat.find(k) {
            let w = 1.0 / (1.0 + rank as f64 * 0.25);
            let v = model.similarity(cat, s, i) * w;
            if v > best {
                best = v;
                best_seed = Some(s);
            }
        }
    }
    let mut fit = best;
    if let Some(s) = ctx.pulled_toward.as_ref().and_then(|k| cat.find(k)) {
        fit += 0.3 * model.similarity(cat, s, i);
    }
    for s in ctx.pushed_away.iter().filter_map(|k| cat.find(k)) {
        fit -= 0.3 * model.similarity(cat, s, i);
    }
    (fit.clamp(0.0, 1.0), best_seed)
}

fn score_all(model: &Model, cat: &Catalog, ctx: &Context, pool: &[usize]) -> Vec<Scored> {
    let bucket = bucket_of(model.now_utc_ms, model.utc_offset_minutes);
    let wk = weekend(model.now_utc_ms, model.utc_offset_minutes);
    let mut out: Vec<Scored> = pool
        .iter()
        .map(|&i| {
            let (fit, best_seed) = session_fit(model, cat, ctx, i);
            let time = (model.time_fit(cat, i, bucket, wk) * 2.0).min(1.0);
            let score =
                0.35 * model.liking(cat, i) + 0.20 * fit + 0.15 * time + 0.10 * model.freshness(i)
                    - 0.20 * model.skip_rate(cat, i);
            Scored {
                i,
                score,
                fit,
                best_seed,
            }
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.i.cmp(&b.i)));
    out
}

fn shares_artist(cat: &Catalog, a: usize, b: usize) -> bool {
    cat.get(a)
        .artists
        .iter()
        .any(|x| cat.get(b).artists.contains(x))
}

/// The top two reasons for a pick.
pub(crate) fn reasons(
    model: &Model,
    cat: &Catalog,
    i: usize,
    seed: Option<usize>,
    exploration: bool,
) -> Vec<Reason> {
    let mut out = Vec::new();
    let f = &model.facts[i];
    if exploration {
        out.push(Reason::NewToYou);
    }
    if let Some(s) = seed {
        let sf = &model.facts[s];
        if model.colisten_weight(s, i) > 0.2 {
            out.push(Reason::PlayedTogether {
                song_key: cat.get(s).key.clone(),
            });
        } else if sf.plays >= 3 && sf.finishes * 10 >= sf.plays * 7 {
            out.push(Reason::FinishedOften {
                song_key: cat.get(s).key.clone(),
                finished: sf.finishes.min(10),
                of: sf.plays.min(10),
            });
        } else if shares_artist(cat, s, i) {
            out.push(Reason::SameArtist {
                artist: cat.get(i).artists.first().cloned().unwrap_or_default(),
            });
        } else if let Some(g) = cat
            .get(i)
            .genres
            .iter()
            .find(|g| cat.get(s).genres.contains(g))
        {
            out.push(Reason::SameGenre { genre: g.clone() });
        }
    }
    if f.liked {
        let days = f
            .last_heard_utc_ms
            .map(|t| ((model.now_utc_ms - t).max(0) / crate::DAY_MS) as u32);
        match days {
            Some(d) if d >= 60 => out.push(Reason::Rediscover { days: d }),
            _ => out.push(Reason::YouLikedIt),
        }
    }
    let bucket = bucket_of(model.now_utc_ms, model.utc_offset_minutes);
    let total: f64 = f.when.iter().sum();
    if total >= 3.0
        && (f.when[usize::from(bucket) * 2] + f.when[usize::from(bucket) * 2 + 1]) / total > 0.5
    {
        out.push(Reason::TimeOfDay { bucket });
    }
    let s = cat.get(i);
    if f.plays == 0 && s.server_play_count > 0 {
        out.push(Reason::PlayedElsewhere {
            count: s.server_play_count,
        });
    }
    if s.added_utc_ms
        .is_some_and(|t| model.now_utc_ms - t < 30 * crate::DAY_MS)
    {
        out.push(Reason::RecentlyAdded);
    }
    out.truncate(2);
    out
}

/// Pick `n` songs for this moment: the shared core of autoplay and radio.
fn pick(model: &Model, cat: &Catalog, ctx: &Context, n: usize, p: &Params) -> Vec<Pick> {
    let mut rng = Rng::new(ctx.seed);
    let pool = candidates(model, cat, ctx, p, &mut rng);
    let scored = score_all(model, cat, ctx, &pool);
    let mut chosen: Vec<usize> = Vec::new();
    let mut picks: Vec<Pick> = Vec::new();
    let mut used = vec![false; scored.len()];
    // In-session adjustment: three early skips in a row stop exploring.
    let explore_pct = if ctx.early_skips_in_a_row >= 3 {
        0
    } else {
        ctx.exploration_percent
    };
    let explore_every = if explore_pct == 0 {
        usize::MAX
    } else {
        (100 / explore_pct.max(1)) as usize
    };
    let pulled = ctx.pulled_toward.as_ref().and_then(|k| cat.find(k));
    let recent = |chosen: &[usize], i: usize| {
        chosen
            .iter()
            .rev()
            .take(p.artist_spacing.saturating_sub(1))
            .any(|&c| shares_artist(cat, c, i))
    };
    while picks.len() < n {
        let slot = picks.len();
        let prev_likely = picks
            .last()
            .is_some_and(|_| chosen.last().is_some_and(|&c| model.liking(cat, c) >= 0.55));
        let explore = slot > 0
            && prev_likely
            && !picks.last().is_some_and(|x| x.exploration)
            && slot % explore_every == explore_every / 2;
        let mut best: Option<(usize, f64)> = None;
        for (x, s) in scored.iter().enumerate() {
            if used[x] || recent(&chosen, s.i) {
                continue;
            }
            let value = if explore {
                // Uncertain and not disliked: Thompson-sample its artist's record.
                let f = &model.facts[s.i];
                if f.plays > 1 || model.liking(cat, s.i) < 0.45 {
                    continue;
                }
                let (wins, losses) = model.artist_record(cat, s.i);
                rng.beta(1 + wins, 1 + losses) * 0.5 + s.fit * 0.5
            } else if slot < 2
                && let Some(pt) = pulled
            {
                model.similarity(cat, pt, s.i) + s.score * 0.01
            } else {
                let redundancy = chosen
                    .iter()
                    .rev()
                    .take(10)
                    .map(|&c| model.similarity(cat, c, s.i))
                    .fold(0.0, f64::max);
                // Flow: no jump in tempo or energy from the song before (when both are known).
                let jump = chosen
                    .last()
                    .and_then(|&c| {
                        let (a, b) = (cat.get(c).sound.as_ref()?, cat.get(s.i).sound.as_ref()?);
                        crate::sound::flow_cost(a, b)
                    })
                    .unwrap_or(0.0);
                p.diversity_lambda * s.score
                    - (1.0 - p.diversity_lambda) * redundancy
                    - FLOW_WEIGHT * jump
            };
            if best.is_none_or(|(_, v)| value > v) {
                best = Some((x, value));
            }
        }
        let Some((x, _)) = best.or_else(|| {
            // Spacing can run out in a small library: relax it rather than stop.
            scored
                .iter()
                .enumerate()
                .find(|(x, _)| !used[*x])
                .map(|(x, s)| (x, s.score))
        }) else {
            break;
        };
        used[x] = true;
        let s = &scored[x];
        chosen.push(s.i);
        picks.push(Pick {
            song_key: cat.get(s.i).key.clone(),
            exploration: explore,
            reasons: reasons(model, cat, s.i, s.best_seed, explore),
        });
    }
    picks
}

/// The ranker's score for one song in this context (for the offline replay's
/// ranking check; picks use the same score). None when the song isn't known.
pub fn score_of(model: &Model, cat: &Catalog, ctx: &Context, key: &str) -> Option<f64> {
    let i = cat.find(key)?;
    score_all(model, cat, ctx, &[i]).first().map(|s| s.score)
}

/// Autoplay: `n` songs seeded by the current song and the last ones finished
/// (board 37: 10, refilled when fewer than 5 are upcoming).
pub fn autoplay(model: &Model, cat: &Catalog, ctx: &Context, n: usize, p: &Params) -> Vec<Pick> {
    pick(model, cat, ctx, n, p)
}

/// Radio: autoplay with the seed held for the whole session (the seed first among
/// the seeds, whatever played since).
pub fn radio(
    model: &Model,
    cat: &Catalog,
    seed_key: &str,
    ctx: &Context,
    n: usize,
    p: &Params,
) -> Vec<Pick> {
    let mut c = ctx.clone();
    c.seeds.retain(|k| k != seed_key);
    c.seeds.insert(0, seed_key.to_owned());
    c.seeds.truncate(3);
    pick(model, cat, &c, n, p)
}

/// Smart shuffle: the list shuffled, with one suggestion after every
/// `smart_shuffle_every` songs, each seeded by the songs around it. Returns
/// (song key, suggested).
pub fn smart_shuffle(
    model: &Model,
    cat: &Catalog,
    list: &[String],
    ctx: &Context,
    p: &Params,
) -> Vec<(String, bool)> {
    let mut rng = Rng::new(ctx.seed ^ 0x5348_5546);
    let mut order: Vec<String> = list.to_vec();
    rng.shuffle(&mut order);
    let mut exclude: Vec<String> = ctx.exclude.clone();
    exclude.extend(order.iter().cloned());
    let mut out = Vec::new();
    for (x, k) in order.iter().enumerate() {
        out.push((k.clone(), false));
        if (x + 1) % p.smart_shuffle_every.max(1) == 0 {
            let c = Context {
                seeds: order[x.saturating_sub(2)..=x]
                    .iter()
                    .rev()
                    .cloned()
                    .collect(),
                exclude: exclude.clone(),
                exploration_percent: 0,
                seed: ctx.seed.wrapping_add(x as u64),
                ..ctx.clone()
            };
            if let Some(s) = pick(model, cat, &c, 1, p).into_iter().next() {
                exclude.push(s.song_key.clone());
                out.push((s.song_key, true));
            }
        }
    }
    out
}

/// Why this song (board 38): one or two reasons for a song in this context.
pub fn why(model: &Model, cat: &Catalog, key: &str, ctx: &Context) -> Vec<Reason> {
    let Some(i) = cat.find(key) else {
        return Vec::new();
    };
    let (_, seed) = session_fit(model, cat, ctx, i);
    reasons(model, cat, i, seed, false)
}

/// Up to `n` songs that "sound like" this one: the enjoyed songs most alike it.
pub fn sounds_like(model: &Model, cat: &Catalog, key: &str, n: usize) -> Vec<String> {
    let Some(i) = cat.find(key) else {
        return Vec::new();
    };
    let mut alike: Vec<(usize, f64)> = (0..cat.len())
        .filter(|&j| j != i && (model.facts[j].finishes > 0 || model.facts[j].liked))
        .map(|j| (j, model.similarity(cat, i, j)))
        .filter(|(_, s)| *s > 0.1)
        .collect();
    alike.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    alike
        .into_iter()
        .take(n)
        .map(|(j, _)| cat.get(j).key.clone())
        .collect()
}
