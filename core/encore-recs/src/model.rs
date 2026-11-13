//! What the engine knows, built once from the history and the catalog (board 37,
//! steps 1 to 3): per-song signals, affinity, sessions, the co-listen graph, the
//! time of day each song is played at, and what not to suggest.
//!
//! Signal rules (section 21; weights from `Params`):
//! - liked +3 (permanent), added to a playlist or downloaded +1.5 (removing it
//!   later doesn't undo it), More like this +1.5, Fewer like this -1;
//! - finished +1 (a little more when the app chose it), past 80% +0.5, skipped
//!   between 30 s and 80% -0.5, skipped early when the app chose it -1.5;
//! - a skip of a song the user picked counts (-0.2) only once the same song was
//!   skipped 3 times in 14 days;
//! - negatives fade with a 30-day half-life; plays and feedback before the
//!   learning epoch are not learned from;
//! - the media server's play counts and favourites are a prior for songs this
//!   profile hasn't played yet.

use std::collections::{HashMap, HashSet};

use crate::det::{decay, squash};
use crate::{
    Catalog, DAY_MS, History, NotInterestedReason, Outcome, Params, Scope, bucket_of, weekend,
};

/// Analysed songs the person must have a verdict on before their taste in sound is trusted.
const MIN_SOUND_VERDICTS: u32 = 8;
/// A taste cosine of 0.5 (songs that sound very much like what they like) is a full lean.
const SOUND_FIT_SCALE: f64 = 2.0;
/// How far what a song sounds like can move a never-played song's liking.
const SOUND_LEAN: f64 = 0.25;
/// How many never-played songs that fit the person's taste in sound join the candidates.
const SOUND_TASTE_PICKS: usize = 100;

/// One song's facts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SongFacts {
    pub plays: u32,
    pub finishes: u32,
    /// App-chosen plays and how many of them were skipped early, faded.
    pub app_plays: f64,
    pub app_early_skips: f64,
    pub last_heard_utc_ms: Option<i64>,
    /// Enjoyed plays per (time bucket, weekend): index bucket * 2 + weekend.
    pub when: [f64; 8],
    pub raw: f64,
    /// Anything learned about it at all (a play, a like, feedback, the server).
    pub known: bool,
    pub liked: bool,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub now_utc_ms: i64,
    pub utc_offset_minutes: i32,
    /// Plays learned from (this epoch).
    pub plays_counted: u32,
    pub first_play_utc_ms: Option<i64>,
    pub facts: Vec<SongFacts>,
    /// 0..1, 0.5 is "nothing known".
    pub affinity: Vec<f64>,
    artist_affinity: HashMap<String, f64>,
    genre_affinity: HashMap<String, f64>,
    /// Per song: (other song, weight), strongest first, at most 30.
    pub colisten: Vec<Vec<(usize, f64)>>,
    /// Sessions of enjoyed plays (song indexes, in order), oldest first, with the
    /// session's start time and offset.
    pub sessions: Vec<(i64, i32, Vec<usize>)>,
    blocked_songs: HashSet<usize>,
    blocked_artists: HashSet<String>,
    rested_until: HashMap<usize, i64>,
    /// (song, bucket) and (artist, bucket) not to suggest at that time of day.
    wrong_moment_songs: HashSet<(usize, u8)>,
    wrong_moment_artists: HashSet<(String, u8)>,
    artist_skips: HashMap<String, (f64, f64)>,
    /// Each artist's enjoyed plays per (time bucket, weekend).
    artist_when: HashMap<String, [f64; 8]>,
    /// Each artist's finished plays and (faded) early skips, for exploration.
    artist_record: HashMap<String, (u32, u32)>,
    /// Phase 10: the direction in sound space the person's plays point to, a unit vector over the
    /// centred projection: toward what they finished and liked, away from what they skipped.
    /// None until a few analysed songs have been learned from.
    sound_taste: Option<Vec<f64>>,
    /// The learned ranker, when this profile has one that earned its place (`learn::train`).
    ranker: Option<std::sync::Arc<crate::learn::Ranker>>,
    /// See `taste_pool`.
    taste_pool: Vec<usize>,
}

fn add_pair(pairs: &mut HashMap<(usize, usize), f64>, a: usize, b: usize, w: f64) {
    if a != b {
        *pairs.entry((a.min(b), a.max(b))).or_default() += w;
    }
}

impl Model {
    pub fn build(
        history: &History,
        catalog: &Catalog,
        p: &Params,
        now_utc_ms: i64,
        utc_offset_minutes: i32,
    ) -> Model {
        let n = catalog.len();
        let mut facts = vec![SongFacts::default(); n];
        let age_days = |t: i64| (now_utc_ms - t) as f64 / DAY_MS as f64;
        let fade = |t: i64| decay(age_days(t), p.half_life_days);
        let learn = |t: i64| t >= history.epoch_start_utc_ms;

        for (key, _) in &history.likes {
            if let Some(i) = catalog.find(key) {
                facts[i].liked = true;
                facts[i].known = true;
                facts[i].raw += p.w_liked;
            }
        }
        for (key, at) in &history.added {
            if let Some(i) = catalog.find(key)
                && learn(*at)
            {
                facts[i].known = true;
                facts[i].raw += p.w_added;
            }
        }

        // User-picked skips count only after enough of them in the window.
        let mut user_skips: HashMap<usize, Vec<i64>> = HashMap::new();
        let mut plays_counted = 0;
        let mut first_play = None;
        let mut artist_skips: HashMap<String, (f64, f64)> = HashMap::new();
        for play in &history.plays {
            let Some(i) = catalog.find(&play.song_key) else {
                continue;
            };
            let f = &mut facts[i];
            let t = play.started_utc_ms;
            f.last_heard_utc_ms = Some(f.last_heard_utc_ms.map_or(t, |l| l.max(t)));
            if !learn(t) {
                continue;
            }
            let Some(outcome) = play.outcome else {
                continue;
            };
            plays_counted += 1;
            first_play = Some(first_play.map_or(t, |x: i64| x.min(t)));
            f.known = true;
            f.plays += 1;
            let slot = usize::from(bucket_of(t, play.utc_offset_minutes)) * 2
                + usize::from(weekend(t, play.utc_offset_minutes));
            match outcome {
                Outcome::Finished => {
                    f.finishes += 1;
                    f.raw += p.w_finished * if play.app_chosen { 1.2 } else { 1.0 };
                    f.when[slot] += 1.0;
                }
                Outcome::SkippedLate => {
                    f.raw += p.w_skipped_late;
                    f.when[slot] += 0.5;
                }
                Outcome::SkippedMid => f.raw += p.w_skipped_mid * fade(t),
                Outcome::SkippedEarly if play.app_chosen => f.raw += p.w_early_skip_app * fade(t),
                Outcome::SkippedEarly | Outcome::SkippedUserPick => {
                    user_skips.entry(i).or_default().push(t);
                }
                Outcome::Abandoned => {}
            }
            if play.strong_intent
                && !matches!(outcome, Outcome::SkippedEarly | Outcome::SkippedUserPick)
            {
                f.raw += p.w_strong_intent;
            }
            if play.app_chosen {
                let w = fade(t);
                f.app_plays += w;
                let early = outcome == Outcome::SkippedEarly;
                if early {
                    f.app_early_skips += w;
                }
                for a in &catalog.get(i).artists {
                    let e = artist_skips.entry(a.clone()).or_default();
                    e.1 += w;
                    if early {
                        e.0 += w;
                    }
                }
            }
        }
        for (i, times) in &user_skips {
            let window = p.user_pick_days * DAY_MS;
            let recent = times.iter().filter(|t| now_utc_ms - **t <= window).count() as u32;
            if recent >= p.user_pick_skips {
                facts[*i].raw += p.w_skipped_user_pick * f64::from(recent);
            }
        }

        // Sessions and the co-listen graph, from enjoyed plays.
        let gap = p.session_gap_minutes * 60_000;
        let mut sessions: Vec<(i64, i32, Vec<usize>)> = Vec::new();
        let mut last_t: Option<i64> = None;
        for play in &history.plays {
            if !learn(play.started_utc_ms) {
                continue;
            }
            let Some(i) = catalog.find(&play.song_key) else {
                continue;
            };
            let new = last_t.is_none_or(|l| play.started_utc_ms - l > gap);
            last_t = Some(play.started_utc_ms);
            if new {
                sessions.push((play.started_utc_ms, play.utc_offset_minutes, Vec::new()));
            }
            let enjoyed = play.outcome.is_some_and(Outcome::enjoyed) || play.strong_intent;
            if enjoyed && let Some(s) = sessions.last_mut() {
                s.2.push(i);
            }
        }
        sessions.retain(|s| !s.2.is_empty());
        let mut pairs: HashMap<(usize, usize), f64> = HashMap::new();
        for (t, _, songs) in &sessions {
            let w = decay(age_days(*t), p.half_life_days * 6.0);
            for (x, a) in songs.iter().enumerate() {
                for (d, b) in songs.iter().enumerate().skip(x + 1).take(5) {
                    add_pair(&mut pairs, *a, *b, w / (d - x) as f64);
                }
            }
        }
        let mut colisten: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut sorted: Vec<((usize, usize), f64)> = pairs.into_iter().collect();
        sorted.sort_by_key(|x| x.0);
        for ((a, b), w) in sorted {
            colisten[a].push((b, w));
            colisten[b].push((a, w));
        }
        for list in &mut colisten {
            list.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
            list.truncate(30);
        }

        // Feedback and Not interested.
        let mut blocked_songs = HashSet::new();
        let mut blocked_artists = HashSet::new();
        let mut rested_until = HashMap::new();
        let mut wrong_moment_songs = HashSet::new();
        let mut wrong_moment_artists = HashSet::new();
        let mut spread: Vec<(usize, f64)> = Vec::new();
        for fb in &history.feedback {
            let Some(i) = catalog.find(&fb.song_key) else {
                continue;
            };
            if !learn(fb.at_utc_ms) {
                continue;
            }
            let w = if fb.more {
                p.w_more_like_this
            } else {
                p.w_fewer_like_this * fade(fb.at_utc_ms)
            };
            facts[i].known = true;
            facts[i].raw += w;
            for (j, _) in colisten[i].iter().take(5) {
                spread.push((*j, w / 2.0));
            }
        }
        for ni in &history.not_interested {
            match (&ni.scope, ni.reason) {
                (Scope::Song(k), reason) => {
                    let Some(i) = catalog.find(k) else { continue };
                    match reason {
                        NotInterestedReason::NotMyTaste => {
                            blocked_songs.insert(i);
                            for (j, _) in colisten[i].iter().take(5) {
                                spread.push((*j, p.w_fewer_like_this / 2.0));
                            }
                        }
                        NotInterestedReason::HeardTooMuch => {
                            rested_until
                                .insert(i, ni.at_utc_ms + p.heard_too_much_rest_days * DAY_MS);
                        }
                        NotInterestedReason::WrongMoment => {
                            wrong_moment_songs.insert((i, ni.bucket));
                        }
                    }
                }
                (Scope::Artist(a), NotInterestedReason::WrongMoment) => {
                    wrong_moment_artists.insert((a.clone(), ni.bucket));
                }
                (Scope::Artist(a), _) => {
                    blocked_artists.insert(a.clone());
                }
            }
        }
        for (j, w) in spread {
            facts[j].raw += w;
            facts[j].known = true;
        }

        // The media server's own counts, for songs this profile hasn't played.
        for (i, s) in catalog.songs().iter().enumerate() {
            if facts[i].plays == 0 && s.server_play_count > 0 {
                let c = f64::from(s.server_play_count);
                facts[i].raw += 0.8 * c / (c + 3.0);
                facts[i].known = true;
                if facts[i].last_heard_utc_ms.is_none() {
                    facts[i].last_heard_utc_ms = s.server_last_played_utc_ms;
                }
            }
        }

        let affinity: Vec<f64> = facts
            .iter()
            .map(|f| (squash(f.raw / 2.0) + 1.0) / 2.0)
            .collect();
        let mut artist_sum: HashMap<String, (f64, u32)> = HashMap::new();
        let mut genre_sum: HashMap<String, (f64, u32)> = HashMap::new();
        for (i, s) in catalog.songs().iter().enumerate() {
            if !facts[i].known {
                continue;
            }
            for a in &s.artists {
                let e = artist_sum.entry(a.clone()).or_default();
                e.0 += affinity[i];
                e.1 += 1;
            }
            for g in &s.genres {
                let e = genre_sum.entry(g.to_lowercase()).or_default();
                e.0 += affinity[i];
                e.1 += 1;
            }
        }
        let mut artist_when: HashMap<String, [f64; 8]> = HashMap::new();
        for (i, s) in catalog.songs().iter().enumerate() {
            if facts[i].when.iter().sum::<f64>() > 0.0 {
                for a in &s.artists {
                    let e = artist_when.entry(a.clone()).or_insert([0.0; 8]);
                    for (slot, v) in facts[i].when.iter().enumerate() {
                        e[slot] += v;
                    }
                }
            }
        }
        let mut artist_record: HashMap<String, (u32, u32)> = HashMap::new();
        for (i, s) in catalog.songs().iter().enumerate() {
            let f = &facts[i];
            if f.finishes > 0 || f.app_early_skips > 0.0 {
                for a in &s.artists {
                    let e = artist_record.entry(a.clone()).or_default();
                    e.0 += f.finishes;
                    e.1 += f.app_early_skips.round() as u32;
                }
            }
        }
        // Shrunk toward 0.5: one liked song doesn't make an artist a favourite.
        let shrink = |(s, c): (f64, u32)| (s + 0.5 * 2.0) / (f64::from(c) + 2.0);
        // The taste in sound space (Phase 10): every analysed song the person has a verdict on
        // pulls the direction toward it (finished, liked) or away from it (skipped), by that
        // verdict's strength, each song as a unit vector so a song played a hundred times does not
        // outweigh a hundred songs. Songs nobody has played are then judged by it (`liking`).
        let sound_taste: Option<Vec<f64>> = {
            let mut t = vec![0f64; crate::sound::DIMS];
            let mut learned = 0;
            for (i, f) in facts.iter().enumerate() {
                let Some(c) = catalog.centred(i) else {
                    continue;
                };
                if !f.known || f.raw == 0.0 || c.norm2 == 0 {
                    continue;
                }
                let w = f.raw.clamp(-3.0, 3.0) / c.norm;
                for (a, v) in t.iter_mut().zip(&c.v) {
                    *a += w * f64::from(*v);
                }
                learned += 1;
            }
            let norm = t.iter().map(|x| x * x).sum::<f64>().sqrt();
            (learned >= MIN_SOUND_VERDICTS && norm > 0.0)
                .then(|| t.iter().map(|x| x / norm).collect())
        };
        // The never-played songs that sound most like what they like.
        let taste_pool: Vec<usize> = match &sound_taste {
            Some(t) => {
                let mut fits: Vec<(f64, usize)> = (0..n)
                    .filter(|&i| !facts[i].known)
                    .filter_map(|i| {
                        let c = catalog.centred(i).filter(|c| c.norm2 > 0)?;
                        let dot: f64 = t.iter().zip(&c.v).map(|(a, v)| a * f64::from(*v)).sum();
                        Some((dot / c.norm, i))
                    })
                    .collect();
                let best =
                    |a: &(f64, usize), b: &(f64, usize)| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1));
                if fits.len() > SOUND_TASTE_PICKS {
                    fits.select_nth_unstable_by(SOUND_TASTE_PICKS, best);
                    fits.truncate(SOUND_TASTE_PICKS);
                }
                fits.sort_by(best);
                fits.into_iter().map(|x| x.1).collect()
            }
            None => Vec::new(),
        };
        Model {
            now_utc_ms,
            utc_offset_minutes,
            plays_counted,
            first_play_utc_ms: first_play,
            facts,
            affinity,
            artist_affinity: artist_sum
                .into_iter()
                .map(|(k, v)| (k, shrink(v)))
                .collect(),
            genre_affinity: genre_sum.into_iter().map(|(k, v)| (k, shrink(v))).collect(),
            colisten,
            sessions,
            blocked_songs,
            blocked_artists,
            rested_until,
            wrong_moment_songs,
            wrong_moment_artists,
            artist_skips,
            artist_when,
            artist_record,
            taste_pool,
            sound_taste,
            ranker: None,
        }
    }

    /// This model with a learned ranker (or none): picks are scored by it when it is usable.
    pub fn with_ranker(mut self, ranker: Option<std::sync::Arc<crate::learn::Ranker>>) -> Model {
        self.ranker = ranker;
        self
    }

    pub fn ranker(&self) -> Option<&crate::learn::Ranker> {
        self.ranker.as_deref()
    }

    /// How well a song's sound fits what the person likes, -1 to 1 (the cosine with their taste
    /// direction); None when the song has no sound or the taste is not known yet.
    pub fn sound_fit(&self, catalog: &Catalog, i: usize) -> Option<f64> {
        let t = self.sound_taste.as_ref()?;
        let c = catalog.centred(i)?;
        if c.norm2 == 0 {
            return None;
        }
        let dot: f64 = t.iter().zip(&c.v).map(|(a, v)| a * f64::from(*v)).sum();
        Some(dot / c.norm)
    }

    /// The never-played songs that fit the person's taste in sound best, best first (made once
    /// with the model): candidates that let the engine look past the few songs it has heard.
    pub fn taste_pool(&self) -> &[usize] {
        &self.taste_pool
    }

    /// The best guess of how much the user likes a song, 0..1: what was learned
    /// about it, else its artist and genres.
    pub fn liking(&self, catalog: &Catalog, i: usize) -> f64 {
        if self.facts[i].known {
            return self.affinity[i];
        }
        // A song nobody has played, that has been analysed: what it sounds like says as much as
        // its tags, and when they disagree both count (starting values, docs/adr/core-0020).
        if let Some(fit) = self.sound_fit(catalog, i) {
            let tags = self.tag_liking(catalog, i);
            let sound = 0.5 + 0.5 * (fit * SOUND_FIT_SCALE).clamp(-1.0, 1.0);
            // A guess never outranks what the person has shown: kept within what a song they
            // finished once or twice earns.
            return (0.5 + 0.6 * (tags - 0.5) + SOUND_LEAN * (sound - 0.5)).clamp(0.2, 0.68);
        }
        self.tag_liking(catalog, i)
    }

    /// What the artist and genres say about a song nobody has played, 0..1.
    fn tag_liking(&self, catalog: &Catalog, i: usize) -> f64 {
        let s = catalog.get(i);
        let artist = s
            .artists
            .iter()
            .filter_map(|a| self.artist_affinity.get(a))
            .fold(None, |acc: Option<f64>, x| {
                Some(acc.map_or(*x, |a| a.max(*x)))
            })
            .unwrap_or(0.5);
        let genre = s
            .genres
            .iter()
            .filter_map(|g| self.genre_affinity.get(&g.to_lowercase()))
            .fold(None, |acc: Option<f64>, x| {
                Some(acc.map_or(*x, |a| a.max(*x)))
            })
            .unwrap_or(0.5);
        0.5 + 0.6 * (artist - 0.5) + 0.4 * (genre - 0.5)
    }

    /// Finished plays and early skips of a song's artists (for Thompson sampling).
    pub fn artist_record(&self, catalog: &Catalog, i: usize) -> (u32, u32) {
        catalog
            .get(i)
            .artists
            .iter()
            .filter_map(|a| self.artist_record.get(a))
            .fold((0, 0), |(w, l), (a, b)| (w + a, l + b))
    }

    /// How well a song fits a time of day, 0..1 (0.25 is "no idea": four buckets).
    pub fn time_fit(&self, catalog: &Catalog, i: usize, bucket: u8, weekend: bool) -> f64 {
        let f = &self.facts[i];
        let slot = usize::from(bucket) * 2 + usize::from(weekend);
        let total: f64 = f.when.iter().sum();
        if total > 0.0 {
            let here = f.when[slot] + f.when[usize::from(bucket) * 2 + usize::from(!weekend)] * 0.5;
            return ((here + 0.25) / (total + 1.0)).min(1.0);
        }
        // The artist's habits, else nothing known.
        let mut here = 0.0;
        let mut all = 0.0;
        for a in &catalog.get(i).artists {
            if let Some(w) = self.artist_when.get(a) {
                here += w[slot];
                all += w.iter().sum::<f64>();
            }
        }
        if all > 0.0 {
            ((here + 0.25) / (all + 1.0)).min(1.0)
        } else {
            0.25
        }
    }

    /// 0..1: how long since it was last heard (a song heard today is 0).
    pub fn freshness(&self, i: usize) -> f64 {
        match self.facts[i].last_heard_utc_ms {
            None => 0.8,
            Some(t) => {
                let days = (self.now_utc_ms - t).max(0) as f64 / DAY_MS as f64;
                1.0 - crate::det::exp(-days / 7.0)
            }
        }
    }

    /// The recent early-skip rate in similar contexts: the song's own, or its
    /// artist's (app-chosen plays, faded), with a prior of no skips.
    pub fn skip_rate(&self, catalog: &Catalog, i: usize) -> f64 {
        let f = &self.facts[i];
        let own = f.app_early_skips / (f.app_plays + 2.0);
        let artist = catalog
            .get(i)
            .artists
            .iter()
            .filter_map(|a| self.artist_skips.get(a))
            .map(|(e, t)| e / (t + 3.0))
            .fold(0.0, f64::max);
        own.max(artist * 0.7)
    }

    /// Never suggested now: Not interested (song, artist, resting, wrong moment).
    pub fn excluded(&self, catalog: &Catalog, i: usize, bucket: u8) -> bool {
        if self.blocked_songs.contains(&i) || self.wrong_moment_songs.contains(&(i, bucket)) {
            return true;
        }
        if self
            .rested_until
            .get(&i)
            .is_some_and(|u| *u > self.now_utc_ms)
        {
            return true;
        }
        catalog.get(i).artists.iter().any(|a| {
            self.blocked_artists.contains(a)
                || self.wrong_moment_artists.contains(&(a.clone(), bucket))
        })
    }

    /// Heard within the last `hours`.
    pub fn heard_within(&self, i: usize, hours: i64) -> bool {
        self.facts[i]
            .last_heard_utc_ms
            .is_some_and(|t| self.now_utc_ms - t < hours * 3_600_000)
    }

    /// The co-listen weight from a to b, 0..1 of a's strongest.
    pub fn colisten_weight(&self, a: usize, b: usize) -> f64 {
        let list = &self.colisten[a];
        let Some(top) = list.first().map(|x| x.1) else {
            return 0.0;
        };
        list.iter()
            .find(|(j, _)| *j == b)
            .map_or(0.0, |(_, w)| w / top)
    }

    /// How alike two songs are, 0..1: played together, same artist, genres, era, and, when both
    /// have been analysed, how they sound (Phase 10). "Sounds like" is defined here and nowhere
    /// else: candidates, diversity, the session's pull, radio and Why this song all ask this.
    pub fn similarity(&self, catalog: &Catalog, a: usize, b: usize) -> f64 {
        if a == b {
            return 1.0;
        }
        let (x, y) = (catalog.get(a), catalog.get(b));
        let co = self.colisten_weight(a, b).max(self.colisten_weight(b, a));
        let artist = if x.artists.iter().any(|n| y.artists.contains(n)) {
            1.0
        } else {
            0.0
        };
        let gx: HashSet<String> = x.genres.iter().map(|g| g.to_lowercase()).collect();
        let gy: HashSet<String> = y.genres.iter().map(|g| g.to_lowercase()).collect();
        let union = gx.union(&gy).count();
        let genre = if union == 0 {
            0.0
        } else {
            gx.intersection(&gy).count() as f64 / union as f64
        };
        let era = match (x.year, y.year) {
            (Some(p), Some(q)) => (1.0 - f64::from((p - q).abs()) / 10.0).max(0.0),
            _ => 0.0,
        };
        match catalog.sound_similarity(a, b) {
            // Starting values (docs/adr/core-0020): sound leads, what the person plays together
            // is next; genre tags matter less once the sound is known.
            Some(sound) => 0.45 * sound + 0.25 * co + 0.10 * artist + 0.10 * genre + 0.10 * era,
            None => 0.5 * co + 0.2 * artist + 0.2 * genre + 0.1 * era,
        }
    }
}
