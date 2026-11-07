//! A listener with known tastes, so any pick can be scored (docs/adr/core-0014).
//!
//! Each song has a hidden appeal for the listener at each time of day: genre
//! taste, favourite and disliked artists, a genre's preferred time of day, and
//! per-song noise. Appeal sets the chance a song survives its first 30 s and the
//! chance it is played to the end. The history is generated from that listener:
//! songs they pick themselves (chosen by taste), plus app-chosen songs from plain
//! shuffle (where their early skips come from). The engine only ever sees the
//! history, never the tastes.

use encore_recs::det::Rng;
use encore_recs::{Catalog, DAY_MS, History, Outcome, Play, Song, bucket_of};

pub const START: i64 = 1_760_000_000_000; // October 2025

const GENRES: [&str; 12] = [
    "Ambient",
    "Folk",
    "Rock",
    "Hip Hop",
    "Pop",
    "Jazz",
    "Metal",
    "Soul",
    "House",
    "Classical",
    "Country",
    "Indie",
];

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: &'static str,
    pub artists: usize,
    pub days: i64,
    pub sessions_per_day: usize,
    pub songs_per_session: usize,
    /// Lower: the listener skips more of everything.
    pub base: f64,
    pub seed: u64,
}

pub const PROFILES: [Profile; 3] = [
    Profile {
        name: "light",
        artists: 120,
        days: 20,
        sessions_per_day: 1,
        songs_per_session: 10,
        base: 0.0,
        seed: 11,
    },
    Profile {
        name: "heavy",
        artists: 300,
        days: 365,
        sessions_per_day: 4,
        songs_per_session: 14,
        base: 0.0,
        seed: 22,
    },
    Profile {
        name: "many skips",
        artists: 200,
        days: 90,
        sessions_per_day: 3,
        songs_per_session: 12,
        base: -1.0,
        seed: 33,
    },
];

pub struct Listener {
    genre_taste: Vec<f64>,
    genre_bucket: Vec<u8>,
    artist_bias: Vec<f64>,
    base: f64,
}

pub struct World {
    pub catalog: Catalog,
    /// Per song (catalog order): artist index and genre index.
    pub song_artist: Vec<usize>,
    pub song_genre: Vec<usize>,
    pub listener: Listener,
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + encore_recs::det::exp(-x))
}

/// A number in [-0.5, 0.5) fixed per song, from its key.
fn noise(key: &str) -> f64 {
    (Rng::seed_of(key) >> 11) as f64 / (1u64 << 53) as f64 - 0.5
}

impl World {
    pub fn new(p: &Profile) -> World {
        let mut rng = Rng::new(p.seed);
        let mut songs = Vec::new();
        let mut artist_genre = Vec::new();
        for a in 0..p.artists {
            let g = rng.below(GENRES.len());
            artist_genre.push(g);
            let era = 1965 + rng.below(55) as i32;
            for al in 0..3 {
                for t in 0..10 {
                    songs.push(Song {
                        key: format!("{}-a{a:03}-{al}-{t}", p.name.replace(' ', "")),
                        title: format!("Song {t}"),
                        artists: vec![format!("Artist {a}")],
                        album: Some(format!("Album {a}-{al}")),
                        album_artist: Some(format!("Artist {a}")),
                        genres: vec![GENRES[g].to_owned()],
                        year: Some(era + al),
                        duration_ms: 210_000,
                        added_utc_ms: Some(START - 500 * DAY_MS),
                        server_play_count: 0,
                        server_last_played_utc_ms: None,
                        server_favorite: false,
                        explicit: false,
                    });
                }
            }
        }
        let catalog = Catalog::new(songs);
        let (song_artist, song_genre): (Vec<usize>, Vec<usize>) = catalog
            .songs()
            .iter()
            .map(|s| {
                let a: usize = s.artists[0]
                    .trim_start_matches("Artist ")
                    .parse()
                    .unwrap_or(0);
                (a, artist_genre[a])
            })
            .unzip();
        let listener = Listener {
            genre_taste: (0..GENRES.len()).map(|_| rng.unit() * 3.0 - 1.5).collect(),
            genre_bucket: (0..GENRES.len()).map(|_| rng.below(4) as u8).collect(),
            artist_bias: (0..p.artists)
                .map(|_| match rng.below(10) {
                    0 => 1.5,
                    1 | 2 => -1.5,
                    _ => rng.unit() - 0.5,
                })
                .collect(),
            base: p.base,
        };
        World {
            catalog,
            song_artist,
            song_genre,
            listener,
        }
    }

    /// How much the listener wants song i at this time of day.
    pub fn appeal(&self, i: usize, bucket: u8) -> f64 {
        let l = &self.listener;
        let g = self.song_genre[i];
        let tod = if l.genre_bucket[g] == bucket {
            0.8
        } else {
            -0.4
        };
        l.base
            + l.genre_taste[g]
            + l.artist_bias[self.song_artist[i]]
            + tod
            + noise(&self.catalog.get(i).key)
    }

    /// How a play of song i goes, from two uniforms (shared between conditions so
    /// they are compared on the same luck).
    pub fn outcome(&self, i: usize, bucket: u8, app_chosen: bool, u1: f64, u2: f64) -> Outcome {
        let a = self.appeal(i, bucket);
        if u1 > sigmoid(a * 1.5 + 0.3) {
            return if app_chosen {
                Outcome::SkippedEarly
            } else {
                Outcome::SkippedUserPick
            };
        }
        let finish = sigmoid(a + 0.8);
        if u2 < finish {
            Outcome::Finished
        } else if u2 < finish + (1.0 - finish) * 0.3 {
            Outcome::SkippedLate
        } else {
            Outcome::SkippedMid
        }
    }

    /// A song the listener picks themselves: the most wanted of a few they browse.
    pub fn user_pick(&self, rng: &mut Rng, bucket: u8) -> usize {
        (0..12)
            .map(|_| rng.below(self.catalog.len()))
            .max_by(|a, b| {
                self.appeal(*a, bucket)
                    .total_cmp(&self.appeal(*b, bucket))
                    .then(a.cmp(b))
            })
            .unwrap_or(0)
    }

    /// The generated history, and the moment it ends.
    pub fn history(&self, p: &Profile) -> (History, i64) {
        let mut rng = Rng::new(p.seed ^ 0xA5A5);
        let mut h = History::default();
        let mut finishes = vec![0u32; self.catalog.len()];
        for d in 0..p.days {
            for s in 0..p.sessions_per_day {
                let hour = [8i64, 13, 19, 23][(s + d as usize) % 4];
                let mut t = START + d * DAY_MS + hour * 3_600_000 + rng.below(3_000_000) as i64;
                let bucket = bucket_of(t, 0);
                for _ in 0..p.songs_per_session {
                    let app = rng.below(10) < 4;
                    let i = if app {
                        rng.below(self.catalog.len())
                    } else {
                        self.user_pick(&mut rng, bucket)
                    };
                    let o = self.outcome(i, bucket, app, rng.unit(), rng.unit());
                    if o == Outcome::Finished {
                        finishes[i] += 1;
                        if finishes[i] == 3 && rng.below(10) < 3 {
                            h.likes.push((self.catalog.get(i).key.clone(), t));
                        }
                    }
                    let listened = match o {
                        Outcome::Finished => 210_000,
                        Outcome::SkippedLate => 180_000,
                        Outcome::SkippedMid => 90_000,
                        _ => 12_000,
                    };
                    h.plays.push(Play {
                        song_key: self.catalog.get(i).key.clone(),
                        started_utc_ms: t,
                        utc_offset_minutes: 0,
                        listened_ms: listened,
                        outcome: Some(o),
                        app_chosen: app,
                        strong_intent: !app && rng.below(10) < 2,
                    });
                    t += listened as i64 + 2_000;
                }
            }
        }
        let end = START + p.days * DAY_MS;
        (h, end)
    }
}
