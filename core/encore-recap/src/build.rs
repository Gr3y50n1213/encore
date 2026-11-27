//! One build: count, gather, pick the story, then pack, seed, palette and words.

use crate::RecapError;
use crate::facts::{Candidate, Ctx, DETAIL_TOP};
use crate::input::{Play, RecapInput};
use crate::look::{choose_pack, palette, pick, pick_u32, seed_of};
use crate::output::{Beat, Detail, Kind, NoneReason, RecapData, Since, SinceReason, Totals};
use crate::personality::{Metrics, metrics};
use crate::time::{MS_PER_DAY, MS_PER_HOUR, day_of, days_from_civil, format_day, parse_day};
use crate::words::{self, TRAIT_PHRASES, capitalize, fill, say};
use crate::year::{Year, filter, gather};

/// The document's schema version.
pub const SCHEMA: u32 = 1;
/// The engine's rules version. Bump it whenever a rule changes the bytes.
pub const ENGINE: u32 = 1;

/// `recap.thresholds.noRecapUnderPlays`.
pub(crate) const NO_RECAP_UNDER_PLAYS: u64 = 10;
/// `recap.thresholds.fullStoryMinPlays` and `fullStoryMinHours`.
pub(crate) const FULL_MIN_PLAYS: u64 = 100;
pub(crate) const FULL_MIN_HOURS: u64 = 20;
/// `recap.story`: cards, fixed and rotating.
pub(crate) const CARDS_MAX: usize = 12;
pub(crate) const ROTATING_MIN: usize = 2;
pub(crate) const ROTATING_MAX: usize = 4;
/// `recap.story.hardMaxSeconds`, `cardSeconds` and `listCardSeconds`.
pub(crate) const HARD_MAX_SECONDS: u32 = 90;
pub(crate) const CARD_SECONDS_MIN: u32 = 6;
pub(crate) const LIST_CARD_SECONDS: u32 = 9;
/// `recap.data.maxRecapDataKb`.
pub(crate) const MAX_BYTES: usize = 100 * 1024;
/// Rotating-beat scoring (starting values): weights of distance and personalness
/// (tenths), the most the seed may add, the cost of each theme shared with a beat
/// already chosen, and the score below which no more than the minimum is taken.
pub(crate) const W_DISTANCE: i64 = 6;
pub(crate) const W_PERSONAL: i64 = 4;
pub(crate) const SEED_JITTER: usize = 41;
pub(crate) const SHARED_THEME_COST: i64 = 300;
pub(crate) const SCORE_FLOOR: i64 = 250;

/// The order of beats inside one arc role.
const ORDER: &[&str] = &[
    "intro",
    "minutes",
    "topSong",
    "topArtist",
    "topAlbum",
    "yourSound",
    "moodArc",
    "listeningClock",
    "longestStreak",
    "biggestDiscovery",
    "comeback",
    "obsessionWeek",
    "songOfEachSeason",
    "mostReplayedDay",
    "mostSungLine",
    "personality",
    "closer",
];

/// Layouts each beat type has (`recap.variation.layoutsPerBeat`, 3 to 4).
fn layout_count(id: &str) -> u32 {
    match id {
        "intro" | "minutes" | "topSong" | "topArtist" | "topAlbum" | "personality" => 4,
        _ => 3,
    }
}

pub(crate) fn build(input: &RecapInput) -> Result<RecapData, RecapError> {
    if !(1971..=9998).contains(&input.year) {
        return Err(RecapError::BadYear(input.year));
    }
    let day = |s: &Option<String>| -> Result<Option<i64>, RecapError> {
        s.as_deref()
            .map(|d| parse_day(d).ok_or_else(|| RecapError::BadDay(d.to_string())))
            .transpose()
    };
    let cleared = day(&input.cleared_since)?;
    let joined = day(&input.joined_day)?;
    let y0 = i64::from(input.year);
    let jan1 = days_from_civil(y0, 1, 1);
    let dec31 = days_from_civil(y0, 12, 31);
    // The year has ended everywhere once 31 Dec is over in UTC-12.
    let year_done_utc = days_from_civil(y0 + 1, 1, 1) * MS_PER_DAY + 12 * MS_PER_HOUR;
    let lock_utc = days_from_civil(y0 + 1, 1, 8) * MS_PER_DAY;
    let so_far = input.counted_through_utc_ms < year_done_utc;
    let locked = input.counted_through_utc_ms >= lock_utc;

    let since = [
        cleared.map(|d| (d, SinceReason::Cleared)),
        joined.map(|d| (d, SinceReason::Joined)),
    ]
    .into_iter()
    .flatten()
    .filter(|(d, _)| *d > jan1 && *d <= dec31)
    // The later day wins; on the same day, the clear.
    .max_by_key(|(d, r)| (*d, *r == SinceReason::Cleared));

    let empty = |reason: NoneReason, y: Option<&Year<'_>>, hash: String| RecapData {
        schema: SCHEMA,
        engine: ENGINE,
        year: input.year,
        kind: Kind::None,
        none_reason: Some(reason),
        so_far,
        locked,
        counted_through_utc_ms: input.counted_through_utc_ms,
        since: since.map(|(d, reason)| Since {
            day: format_day(d),
            reason,
        }),
        pack: None,
        seed: format!("{:016x}", seed_of(&input.profile_id, input.year, None)),
        palette: None,
        year_line: None,
        totals: y.map_or_else(zero_totals, totals),
        beats: Vec::new(),
        detail: None,
        log_hash: hash,
    };

    if input.recap_off {
        // Nothing is shown and nothing is counted.
        return Ok(empty(NoneReason::Off, None, log_hash(&[], &[])));
    }

    let f = filter(input, cleared);
    let hash = log_hash(
        &f.earlier,
        &f.this_year.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
    );
    let y = gather(input, f);
    if y.total_plays < NO_RECAP_UNDER_PLAYS {
        return Ok(empty(NoneReason::NotEnoughPlays, Some(&y), hash));
    }
    let full = y.total_plays >= FULL_MIN_PLAYS && y.total_ms >= FULL_MIN_HOURS * MS_PER_HOUR as u64;

    let pack = choose_pack(&input.profile_id, input.year, &input.packs, &input.past);
    let seed = seed_of(&input.profile_id, input.year, pack.as_ref());
    let last_year = input
        .past
        .iter()
        .filter(|p| p.year == input.year - 1)
        .min_by_key(|p| {
            // Several entries for one year: a stable choice.
            (p.pack.clone(), p.lead_hue, p.listened_ms)
        });
    let first_day = since.map_or(jan1, |(d, _)| d);
    let last_day = dec31
        .min(day_of(input.counted_through_utc_ms))
        .max(first_day);
    let baseline_known = !y.before.is_empty();
    let m = metrics(
        &y,
        baseline_known,
        last_day - first_day + 1,
        input.kid_profile,
    );
    let ctx = Ctx {
        input,
        y: &y,
        seed,
        baseline_known,
        first_day,
        last_day,
        last_year,
        so_far,
        metrics: m,
    };

    let album_colours: Vec<&[String]> = y
        .album_rank
        .iter()
        .filter_map(|&a| input.album_palettes.get(y.albums[a].key).map(Vec::as_slice))
        .take(crate::look::PALETTE_ALBUMS)
        .collect();
    let pal = palette(&album_colours, last_year.and_then(|p| p.lead_hue), seed);

    let (year_line, chosen) = if full {
        let line = year_line(&ctx);
        (
            Some(line.clone()),
            story(&ctx, &line, since.map(|(d, _)| d)),
        )
    } else {
        let short: Vec<Candidate> = [Some(ctx.minutes(true)), ctx.top_song(), ctx.top_artist()]
            .into_iter()
            .flatten()
            .collect();
        (None, short)
    };
    let beats = finish(chosen, seed);

    let mut doc = RecapData {
        schema: SCHEMA,
        engine: ENGINE,
        year: input.year,
        kind: if full { Kind::Full } else { Kind::Short },
        none_reason: None,
        so_far,
        locked,
        counted_through_utc_ms: input.counted_through_utc_ms,
        since: since.map(|(d, reason)| Since {
            day: format_day(d),
            reason,
        }),
        pack,
        seed: format!("{seed:016x}"),
        palette: Some(pal),
        year_line,
        totals: totals(&y),
        beats,
        detail: full.then(|| detail(&ctx, DETAIL_TOP)),
        log_hash: hash,
    };
    // Keep the document under 100 KB: shorten the detail lists if it is not.
    for n in [25, 10, 0] {
        if doc.to_json().len() <= MAX_BYTES {
            break;
        }
        if doc.detail.is_some() {
            doc.detail = Some(detail(&ctx, n));
        }
    }
    Ok(doc)
}

fn zero_totals() -> Totals {
    Totals {
        plays: 0,
        listened_ms: 0,
        minutes: 0,
        songs: 0,
        artists: 0,
        albums: 0,
        active_days: 0,
    }
}

fn totals(y: &Year<'_>) -> Totals {
    Totals {
        plays: y.total_plays,
        listened_ms: y.total_ms,
        minutes: y.total_ms / 60_000,
        songs: y.songs.len() as u64,
        artists: y.artists.len() as u64,
        albums: y.albums.len() as u64,
        active_days: y.active_days.len() as u64,
    }
}

fn detail(ctx: &Ctx<'_, '_>, n: usize) -> Detail {
    Detail {
        top_songs: ctx.top_songs(n),
        top_artists: ctx.top_artists(n),
        top_albums: ctx.top_albums(n),
        months: ctx.months(),
        personality: ctx.metrics.all(),
    }
}

/// The full story: the fixed beats that have data, then 2 to 4 rotating beats.
fn story(ctx: &Ctx<'_, '_>, year_line: &str, since: Option<i64>) -> Vec<Candidate> {
    let mut beats: Vec<Candidate> = [
        Some(ctx.intro(year_line, since)),
        Some(ctx.minutes(false)),
        ctx.top_song(),
        ctx.top_artist(),
        ctx.top_album(),
        ctx.your_sound(),
        Some(ctx.personality()),
        Some(ctx.closer()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let max_rot = ROTATING_MAX.min(CARDS_MAX.saturating_sub(beats.len()));
    let mut pool = ctx.rotating_pool();
    let mut chosen: Vec<Candidate> = Vec::new();
    while chosen.len() < max_rot && !pool.is_empty() {
        let score = |c: &Candidate| -> i64 {
            let shared = c
                .themes
                .iter()
                .filter(|t| chosen.iter().any(|x| x.themes.contains(t)))
                .count() as i64;
            (c.distance * W_DISTANCE + c.personal * W_PERSONAL) / 10
                + pick(ctx.seed, &format!("score/{}", c.id), SEED_JITTER) as i64
                - shared * SHARED_THEME_COST
        };
        let (bi, bs) = pool
            .iter()
            .enumerate()
            .map(|(i, c)| (i, score(c)))
            // Highest score; ties go to the beat named first.
            .max_by(|a, b| a.1.cmp(&b.1).then(pool[b.0].id.cmp(pool[a.0].id)))
            .unwrap_or((0, 0));
        if chosen.len() >= ROTATING_MIN && bs < SCORE_FLOOR {
            break;
        }
        let mut c = pool.remove(bi);
        c.score = Some(bs);
        chosen.push(c);
    }
    beats.extend(chosen);
    beats
}

/// Orders the beats by arc role, gives each its seconds, layout and shape seed.
fn finish(mut cands: Vec<Candidate>, seed: u64) -> Vec<Beat> {
    let pos = |id: &str| ORDER.iter().position(|o| *o == id).unwrap_or(ORDER.len());
    cands.sort_by_key(|c| (c.role, pos(c.id)));
    let mut seconds: Vec<u32> = cands
        .iter()
        .map(|c| match c.id {
            _ if c.list => LIST_CARD_SECONDS,
            "intro" => 6,
            "personality" | "closer" => 8,
            _ => 7,
        })
        .collect();
    // Never more than 90 s: shorten the latest cards that can go shorter.
    while seconds.iter().sum::<u32>() > HARD_MAX_SECONDS {
        match (0..seconds.len())
            .rev()
            .find(|&i| !cands[i].list && seconds[i] > CARD_SECONDS_MIN)
        {
            Some(i) => seconds[i] -= 1,
            None => match (0..seconds.len())
                .rev()
                .find(|&i| seconds[i] > CARD_SECONDS_MIN)
            {
                Some(i) => seconds[i] -= 1,
                None => break,
            },
        }
    }
    cands
        .into_iter()
        .zip(seconds)
        .map(|(c, s)| {
            let n = layout_count(c.id);
            Beat {
                id: c.id.into(),
                role: c.role,
                fixed: c.fixed,
                seconds: s,
                layout: pick(seed, &format!("layout/{}", c.id), n as usize) as u32,
                layout_count: n,
                shape_seed: pick_u32(seed, &format!("shapes/{}", c.id)),
                score: c.score,
                template: c.template,
                text: c.text,
                data: c.data,
                math: c.math,
            }
        })
        .collect()
}

/// The year line from the two strongest traits ("Late nights, loud choruses.").
/// A trait's strength is its metric over its threshold; ties go to the earlier
/// trait in `TRAIT_PHRASES`.
fn year_line(ctx: &Ctx<'_, '_>) -> String {
    let m: &Metrics = &ctx.metrics;
    let kid = ctx.input.kid_profile;
    let top_genre = ctx
        .y
        .genre_rank
        .first()
        .map(|&g| ctx.y.genres[g].name.to_string());
    let soft = m.energy.map(|e| 1000 - e.min(1000));
    let traits: [(&str, Option<u32>, u32); 11] = [
        ("lateNights", if kid { None } else { m.late_night }, 250),
        ("earlyMornings", m.early_morning, 250),
        ("loudChoruses", m.energy, 650),
        ("softEdges", soft, 650),
        ("wholeAlbums", m.from_album, 500),
        ("newNames", m.new_artists, 200),
        ("onRepeat", m.repeat, 150),
        ("everyDay", m.every_day, 700),
        ("weekends", m.weekend, 400),
        ("topGenre", m.top_genre.filter(|_| top_genre.is_some()), 250),
        ("fewFavourites", m.top_six, 300),
    ];
    let mut strong: Vec<(u64, usize)> = traits
        .iter()
        .enumerate()
        .filter_map(|(i, (_, v, t))| {
            let v = (*v)?;
            (v >= *t).then(|| (u64::from(v) * 1000 / u64::from(*t), i))
        })
        .collect();
    // Strongest first; ties keep the table order.
    strong.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let phrase = |i: usize| -> String {
        let (id, variants) = TRAIT_PHRASES
            .iter()
            .find(|(id, _)| *id == traits[i].0)
            .copied()
            .unwrap_or(("", &[""]));
        let v = variants[pick(ctx.seed, &format!("yearLine/{id}"), variants.len())];
        fill(v, &[("genre", top_genre.clone().unwrap_or_default())])
    };
    let (_, line) = match strong.as_slice() {
        [] => say(ctx.seed, &words::YEAR_LINE_NONE, &[]),
        [one] => {
            let a = phrase(one.1);
            say(
                ctx.seed,
                &words::YEAR_LINE_ONE,
                &[("a", a.clone()), ("A", capitalize(&a))],
            )
        }
        [one, two, ..] => {
            let a = phrase(one.1);
            say(
                ctx.seed,
                &words::YEAR_LINE_TWO,
                &[("A", capitalize(&a)), ("b", phrase(two.1))],
            )
        }
    };
    line
}

/// SHA-256 over every play the build used, earlier years first, each in canonical
/// order: fields joined by 0x1F, plays ended by 0x1E, all numbers in decimal.
pub(crate) fn log_hash(earlier: &[&Play], this_year: &[&Play]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"encore-recap/log/v1\n");
    for p in earlier.iter().chain(this_year) {
        let jumps: Vec<String> = p
            .lyric_jumps
            .iter()
            .map(|j| format!("{}:{}", j.line_index, j.line_start_ms))
            .collect();
        let opt = |o: &Option<String>| o.clone().map_or_else(|| "\u{0}".to_string(), |s| s);
        let fields: [String; 15] = [
            p.song_key.clone(),
            p.title.clone(),
            p.artist_key.clone(),
            p.artist.clone(),
            opt(&p.album_key),
            opt(&p.album),
            p.started_utc_ms.to_string(),
            p.utc_offset_minutes.to_string(),
            p.listened_ms.to_string(),
            p.duration_ms
                .map_or_else(|| "\u{0}".to_string(), |d| d.to_string()),
            opt(&p.genre),
            u8::from(p.explicit).to_string(),
            opt(&p.end_reason),
            opt(&p.started_from),
            jumps.join(","),
        ];
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                h.update([0x1F]);
            }
            h.update(f.as_bytes());
        }
        h.update([0x1E]);
    }
    let d = h.finalize();
    d.iter().map(|b| format!("{b:02x}")).collect()
}
