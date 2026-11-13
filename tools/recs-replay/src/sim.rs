//! A listener with known tastes, so any pick can be scored (docs/adr/core-0014).
//!
//! Each song has a hidden appeal for the listener at each time of day: genre
//! taste, favourite and disliked artists, a genre's preferred time of day, and
//! per-song noise. Appeal sets the chance a song survives its first 30 s and the
//! chance it is played to the end. The history is generated from that listener:
//! songs they pick themselves (chosen by taste), plus app-chosen songs from plain
//! shuffle (where their early skips come from). The engine only ever sees the
//! history, never the tastes.
//!
//! Phase 10 (docs/adr/core-0020): every song also has a hidden sound, a point in an
//! 8-dimensional space made from its artist's sound, its genre's typical sound and a
//! little of its own, and the listener has a taste for a direction in that space that
//! no tag says. The version 2 engine is given each song's sound as `encore_analysis`
//! would make it (a noisy 128-number view of that point, and a tempo and an energy);
//! version 1 is given nothing, and the two are compared on the same luck.

use encore_recs::det::Rng;
use encore_recs::{Catalog, DAY_MS, History, Outcome, Play, Song, Sound, bucket_of};

/// The dimensions of the hidden sound.
const LATENT: usize = 8;
/// How much the listener's taste for a sound direction moves a song's appeal, next to
/// genre taste (about +-1.5) and favourite artists (+-1.5).
const SOUND_TASTE: f64 = 1.6;

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
    /// The share of artists whose genre tag is wrong (a real library's tags are messy: missing,
    /// wrong, or a label's idea). The listener's taste follows the true genre; the engine sees the tag.
    pub tag_noise: f64,
}

const fn profile(
    name: &'static str,
    artists: usize,
    days: i64,
    sessions_per_day: usize,
    songs_per_session: usize,
    base: f64,
    seed: u64,
    tag_noise: f64,
) -> Profile {
    Profile {
        name,
        artists,
        days,
        sessions_per_day,
        songs_per_session,
        base,
        seed,
        tag_noise,
    }
}

/// Three listeners with clean tags (the Phase 5 set), and the same three with half the artists
/// mis-tagged: where sound has something to add.
pub const PROFILES: [Profile; 6] = [
    profile("light", 120, 20, 1, 10, 0.0, 11, 0.0),
    profile("heavy", 300, 365, 4, 14, 0.0, 22, 0.0),
    profile("many skips", 200, 90, 3, 12, -1.0, 33, 0.0),
    profile("light, messy tags", 120, 20, 1, 10, 0.0, 11, 0.5),
    profile("heavy, messy tags", 300, 365, 4, 14, 0.0, 22, 0.5),
    profile("many skips, messy tags", 200, 90, 3, 12, -1.0, 33, 0.5),
];
pub struct Listener {
    genre_taste: Vec<f64>,
    genre_bucket: Vec<u8>,
    artist_bias: Vec<f64>,
    /// The direction in sound space the listener likes (a unit vector).
    sound_taste: [f64; LATENT],
    base: f64,
}

pub struct World {
    /// The library as version 1 sees it: no sound.
    pub catalog: Catalog,
    /// Per song (catalog order): artist index and genre index.
    pub song_artist: Vec<usize>,
    pub song_genre: Vec<usize>,
    /// Per song (catalog order): its hidden sound, a unit vector.
    song_latent: Vec<[f64; LATENT]>,
    pub listener: Listener,
}

fn unit_vector(rng: &mut Rng) -> [f64; LATENT] {
    let mut v = [0f64; LATENT];
    for x in &mut v {
        *x = rng.unit() * 2.0 - 1.0;
    }
    normalised(v)
}

fn normalised(mut v: [f64; LATENT]) -> [f64; LATENT] {
    let n = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-9);
    for x in &mut v {
        *x /= n;
    }
    v
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
            // What the library says the genre is: the truth, unless the tag is wrong. (No draw
            // when the tags are clean, so those listeners keep the streams they always had.)
            let tag = if p.tag_noise > 0.0 && rng.unit() < p.tag_noise {
                rng.below(GENRES.len())
            } else {
                g
            };
            let era = 1965 + rng.below(55) as i32;
            for al in 0..3 {
                for t in 0..10 {
                    songs.push(Song {
                        key: format!("{}-a{a:03}-{al}-{t}", p.name.replace(' ', "")),
                        title: format!("Song {t}"),
                        artists: vec![format!("Artist {a}")],
                        album: Some(format!("Album {a}-{al}")),
                        album_artist: Some(format!("Artist {a}")),
                        genres: vec![GENRES[tag].to_owned()],
                        year: Some(era + al),
                        duration_ms: 210_000,
                        added_utc_ms: Some(START - 500 * DAY_MS),
                        server_play_count: 0,
                        server_last_played_utc_ms: None,
                        server_favorite: false,
                        explicit: false,
                        sound: None,
                    });
                }
            }
        }
        let catalog = Catalog::new(songs);
        // The hidden sounds: an artist's own, a genre's typical, a little per song.
        let genre_sound: Vec<[f64; LATENT]> =
            (0..GENRES.len()).map(|_| unit_vector(&mut rng)).collect();
        let artist_sound: Vec<[f64; LATENT]> =
            (0..p.artists).map(|_| unit_vector(&mut rng)).collect();
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
        let song_latent: Vec<[f64; LATENT]> = catalog
            .songs()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut own = Rng::new(Rng::seed_of(&s.key));
                let wobble = unit_vector(&mut own);
                let mut z = [0f64; LATENT];
                for d in 0..LATENT {
                    z[d] = 0.6 * artist_sound[song_artist[i]][d]
                        + 0.5 * genre_sound[song_genre[i]][d]
                        + 0.25 * wobble[d];
                }
                normalised(z)
            })
            .collect();
        let listener = Listener {
            sound_taste: unit_vector(&mut rng),
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
            song_latent,
            listener,
        }
    }

    /// The same library as version 2 sees it: every song has been analysed. The sound is a
    /// noisy 128-number view of the hidden point (the way a trained model hears a song), a
    /// tempo and an energy that follow it loosely.
    pub fn catalog_with_sound(&self) -> Catalog {
        let mut rng = Rng::new(0x50_55_4E_44);
        // A fixed random view: 8 hidden dimensions seen through 128 numbers.
        let view: Vec<[f64; LATENT]> = (0..encore_recs::sound::DIMS)
            .map(|_| {
                let mut v = [0f64; LATENT];
                for x in &mut v {
                    *x = rng.unit() * 2.0 - 1.0;
                }
                v
            })
            .collect();
        let songs = self
            .catalog
            .songs()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let z = &self.song_latent[i];
                let mut own = Rng::new(Rng::seed_of(&s.key) ^ 0x5011);
                let projection = view
                    .iter()
                    .map(|row| {
                        let signal: f64 = row.iter().zip(z).map(|(a, b)| a * b).sum();
                        // Noise about a fifth of the signal's size: the model is not exact.
                        (120.0 * signal + (own.unit() - 0.5) * 40.0).round() as i16
                    })
                    .collect();
                let clamp = |x: f64| x.round().clamp(0.0, 255.0) as u8;
                let mut s = s.clone();
                s.sound = Some(Sound {
                    projection,
                    bpm_tenths: (1_200.0 + 500.0 * z[0] + (own.unit() - 0.5) * 200.0).round()
                        as u16,
                    energy: clamp(128.0 + 100.0 * z[1] + (own.unit() - 0.5) * 40.0),
                    brightness: clamp(128.0 + 100.0 * z[2]),
                    dynamics: 80,
                    key: 255,
                });
                s
            })
            .collect();
        Catalog::new(songs)
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
        let like_this_sound: f64 = l
            .sound_taste
            .iter()
            .zip(&self.song_latent[i])
            .map(|(a, b)| a * b)
            .sum();
        l.base
            + l.genre_taste[g]
            + l.artist_bias[self.song_artist[i]]
            + SOUND_TASTE * like_this_sound
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
