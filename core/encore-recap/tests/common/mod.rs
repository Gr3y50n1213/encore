//! Invented listening histories for the tests. Every artist, song and album name is
//! made of random syllables, so no fixture names a real person or recording; every
//! profile ID is made up. Nothing here comes from anyone's real listening.

#![allow(dead_code, unreachable_pub)]

use std::collections::BTreeMap;

use encore_recap::{
    ArtistBefore, Fingerprint, Hemisphere, LyricJump, PackInfo, PastRecap, Play, RecapInput,
};

pub const DAY_MS: i64 = 86_400_000;

/// SplitMix64: small and the same everywhere.
#[derive(Debug, Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    pub fn chance(&mut self, permille: u64) -> bool {
        self.below(1000) < permille
    }
    /// Index 0..n weighted roughly by 1 / (i + 1): a few favourites, a long tail.
    pub fn zipf(&mut self, n: usize) -> usize {
        let total: u64 = (0..n as u64).map(|i| 100_000 / (i + 1)).sum();
        let mut x = self.below(total);
        for i in 0..n {
            let w = 100_000 / (i as u64 + 1);
            if x < w {
                return i;
            }
            x -= w;
        }
        n - 1
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

const ONSETS: &[&str] = &[
    "b", "d", "f", "g", "k", "l", "m", "n", "p", "r", "s", "t", "v", "z", "br", "dr", "kl", "st",
    "th", "vr",
];
const VOWELS: &[&str] = &["a", "e", "i", "o", "u", "ae", "ou", "y"];
const CODAS: &[&str] = &["", "", "n", "l", "r", "s", "th", "x", "nd"];

/// A made-up word of 2 or 3 syllables.
pub fn word(r: &mut Rng) -> String {
    let n = 2 + r.below(2);
    let mut w = String::new();
    for _ in 0..n {
        w.push_str(ONSETS[r.below(ONSETS.len() as u64) as usize]);
        w.push_str(VOWELS[r.below(VOWELS.len() as u64) as usize]);
    }
    w.push_str(CODAS[r.below(CODAS.len() as u64) as usize]);
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
        None => w,
    }
}

pub fn name(r: &mut Rng, words: u64) -> String {
    (0..words).map(|_| word(r)).collect::<Vec<_>>().join(" ")
}

pub const GENRES: &[&str] = &[
    "indie folk",
    "electronic",
    "ambient",
    "jazz",
    "rock",
    "hip hop",
    "classical",
    "soul",
    "pop",
    "metal",
];

#[derive(Debug, Clone)]
pub struct Song {
    pub key: String,
    pub title: String,
    pub secs: u64,
    pub explicit: bool,
    pub fingerprint: Fingerprint,
    /// The same recording also on a second album, one second longer there.
    pub twin_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub key: String,
    pub name: String,
    pub songs: Vec<Song>,
}

#[derive(Debug, Clone)]
pub struct Artist {
    pub key: String,
    pub name: String,
    pub genre: &'static str,
    pub albums: Vec<Album>,
}

/// A whole invented library.
pub fn library(r: &mut Rng, artists: usize, long_names: bool) -> Vec<Artist> {
    (0..artists)
        .map(|a| {
            let n = 1 + r.below(2);
            let mut aname = name(r, n);
            if long_names {
                while aname.chars().count() < 300 {
                    aname.push(' ');
                    aname.push_str(&word(r));
                }
            }
            let genre = GENRES[(a * 7 + r.below(3) as usize) % GENRES.len()];
            let albums = (0..1 + r.below(2))
                .map(|b| {
                    let songs = (0..4 + r.below(6))
                        .map(|_| {
                            let hash = format!("{:016x}{:016x}", r.next(), r.next());
                            let secs = 150 + r.below(180);
                            let energy = match genre {
                                "electronic" | "metal" | "rock" => 600 + r.below(350),
                                "ambient" | "classical" => 100 + r.below(300),
                                _ => 300 + r.below(400),
                            } as u16;
                            let twin = r.chance(120);
                            let n = 1 + r.below(3);
                            let mut title = name(r, n);
                            if long_names {
                                title = format!("{title} {}", "La ".repeat(100));
                            }
                            Song {
                                key: format!("t1:{hash}:{secs}"),
                                twin_key: twin.then(|| format!("t1:{hash}:{}", secs + 1)),
                                title,
                                secs,
                                explicit: r.chance(80),
                                fingerprint: Fingerprint {
                                    energy,
                                    valence: (200 + r.below(700)) as u16,
                                },
                            }
                        })
                        .collect();
                    let n = 1 + r.below(2);
                    Album {
                        key: format!("al-{a}-{b}"),
                        name: name(r, n),
                        songs,
                    }
                })
                .collect();
            Artist {
                key: format!("ar-{a}"),
                name: aname,
                genre,
                albums,
            }
        })
        .collect()
}

/// How one invented profile listens.
#[derive(Debug, Clone)]
pub struct Spec {
    pub seed: u64,
    pub profile_id: String,
    pub year: i32,
    pub plays: usize,
    pub artists: usize,
    /// Local days of the year (1-based day of year) the plays fall in.
    pub first_doy: i64,
    pub last_doy: i64,
    /// Permille of plays between 22:00 and 02:00.
    pub night: u64,
    pub kid: bool,
    pub genres: bool,
    pub fingerprints: bool,
    pub lyric_jumps: bool,
    /// Plays in the year before, for "new to you" and the comeback.
    pub earlier_plays: usize,
    pub cleared_since: Option<String>,
    pub counted_through_utc_ms: Option<i64>,
    pub past: Vec<PastRecap>,
    pub long_names: bool,
    pub packs: Vec<&'static str>,
}

impl Spec {
    pub fn new(seed: u64, profile_id: &str, year: i32, plays: usize) -> Spec {
        Spec {
            seed,
            profile_id: profile_id.into(),
            year,
            plays,
            artists: 30,
            first_doy: 1,
            last_doy: 365,
            night: 150,
            kid: false,
            genres: true,
            fingerprints: true,
            lyric_jumps: true,
            earlier_plays: 0,
            cleared_since: None,
            counted_through_utc_ms: None,
            past: Vec::new(),
            long_names: false,
            packs: vec!["editorial", "tide", "bloom", "glass"],
        }
    }
}

pub fn jan1_utc(year: i32) -> i64 {
    // Days from 1970 to 1 Jan of the year (Gregorian).
    let y = i64::from(year) - 1;
    let days =
        y * 365 + y / 4 - y / 100 + y / 400 - (1969 * 365 + 1969 / 4 - 1969 / 100 + 1969 / 400);
    days * DAY_MS
}

/// One play of a song by an artist at a local day-of-year and hour.
fn play(r: &mut Rng, s: &Spec, lib: &[Artist], ai: usize, year: i32, doy: i64, hour: i64) -> Play {
    let artist = &lib[ai];
    let album_i = r.below(artist.albums.len() as u64) as usize;
    let album = &artist.albums[album_i];
    let si = r.zipf(album.songs.len());
    let song = &album.songs[si];
    // Summer time in the middle of the year, a trip west in October.
    let offset: i32 = if (280..290).contains(&doy) {
        -300
    } else if (88..300).contains(&doy) {
        120
    } else {
        60
    };
    let minute = r.below(60) as i64;
    let local = jan1_utc(year) + (doy - 1) * DAY_MS + hour * 3_600_000 + minute * 60_000;
    let started_utc_ms = local - i64::from(offset) * 60_000;
    let dur = song.secs * 1000;
    let roll = r.below(1000);
    let (listened_ms, end) = if roll < 30 {
        (r.below(1900), "skipped") // under 2 s: never counted
    } else if roll < 170 {
        (2_000 + r.below(28_000), "skipped")
    } else if roll < 180 {
        (dur / 2, "interrupted")
    } else if roll < 220 {
        (dur / 3, "pausedAbandoned")
    } else {
        (dur, "finished")
    };
    let from = match r.below(10) {
        0..=5 => "album",
        6 | 7 => "playlist",
        8 => "radio",
        _ => "search",
    };
    let twin = song.twin_key.as_ref().filter(|_| r.chance(500));
    let mut jumps = Vec::new();
    if s.lyric_jumps && si == 0 && r.chance(300) {
        jumps.push(LyricJump {
            line_index: 7,
            line_start_ms: 41_250,
        });
    }
    Play {
        song_key: twin.cloned().unwrap_or_else(|| song.key.clone()),
        title: song.title.clone(),
        artist_key: artist.key.clone(),
        artist: artist.name.clone(),
        album_key: Some(if twin.is_some() {
            format!("{}-deluxe", album.key)
        } else {
            album.key.clone()
        }),
        album: Some(album.name.clone()),
        started_utc_ms,
        utc_offset_minutes: offset,
        listened_ms,
        duration_ms: Some(dur),
        genre: s.genres.then(|| artist.genre.to_string()),
        explicit: song.explicit,
        end_reason: Some(end.into()),
        started_from: Some(from.into()),
        lyric_jumps: jumps,
    }
}

fn hour(r: &mut Rng, night: u64) -> i64 {
    if r.chance(night) {
        [22, 23, 0, 1][r.below(4) as usize]
    } else {
        7 + r.below(14) as i64
    }
}

/// The invented profile as engine input.
pub fn input(s: &Spec) -> RecapInput {
    let mut r = Rng(s.seed);
    let lib = library(&mut r, s.artists, s.long_names);
    let mut plays = Vec::with_capacity(s.plays + s.earlier_plays);
    // Artists 3 and 4 are new this year (a discovery); artist 5 was loved two years
    // ago and comes back; the rest were heard last year too.
    let earlier_artists: Vec<usize> = (0..lib.len()).filter(|a| *a != 3 && *a != 4).collect();
    for _ in 0..s.earlier_plays {
        let ai = earlier_artists[r.zipf(earlier_artists.len())];
        let doy = 1 + r.below(360) as i64;
        let (year, doy) = if ai == 5 {
            (s.year - 2, doy.min(150))
        } else {
            (s.year - 1, doy)
        };
        let h = hour(&mut r, s.night);
        plays.push(play(&mut r, s, &lib, ai, year, doy, h));
    }
    let span = (s.last_doy - s.first_doy + 1).max(1) as u64;
    for i in 0..s.plays {
        let mut ai = r.zipf(lib.len());
        let doy = s.first_doy + r.below(span) as i64;
        // An obsession: the second artist owns one week in March.
        if (68..75).contains(&doy) && r.chance(700) {
            ai = 1.min(lib.len() - 1);
        }
        // The sound drifts: electronic artists get more plays after August.
        if s.genres && doy > 243 && r.chance(250) {
            if let Some(e) = lib.iter().position(|a| a.genre == "electronic") {
                ai = e;
            }
        }
        // The comeback artist returns in the second half; the newcomers join in.
        if doy > 180 && i % 23 == 0 {
            ai = 5.min(lib.len() - 1);
        }
        if doy > 60 && i % 11 == 0 {
            ai = 3.min(lib.len() - 1);
        }
        let h = hour(&mut r, s.night);
        plays.push(play(&mut r, s, &lib, ai, s.year, doy, h));
        // A replayed song on one day.
        if i % 97 == 0 && r.chance(200) {
            for k in 0..6 {
                let mut p = plays.last().cloned().expect("a play");
                p.started_utc_ms += (k + 1) * 300_000;
                p.listened_ms = p.duration_ms.unwrap_or(200_000);
                p.end_reason = Some("finished".into());
                plays.push(p);
            }
        }
    }
    r.shuffle(&mut plays);

    let mut album_palettes = BTreeMap::new();
    let mut album_track_counts = BTreeMap::new();
    let mut fingerprints = BTreeMap::new();
    for a in &lib {
        for al in &a.albums {
            let cols: Vec<String> = (0..5)
                .map(|_| format!("#{:06x}", r.below(0x100_0000)))
                .collect();
            album_palettes.insert(al.key.clone(), cols);
            album_track_counts.insert(al.key.clone(), al.songs.len() as u32);
            if s.fingerprints {
                for sg in &al.songs {
                    fingerprints.insert(sg.key.clone(), sg.fingerprint);
                }
            }
        }
    }
    let mut artists_before = Vec::new();
    if s.earlier_plays > 0 {
        // What an archive might add: one artist heard long ago, count unknown.
        artists_before.push(ArtistBefore {
            artist_key: format!("ar-{}", lib.len() + 1),
            plays: 0,
            last_played_utc_ms: None,
        });
    }
    let first_day_after_year = jan1_utc(s.year + 1);
    RecapInput {
        year: s.year,
        profile_id: s.profile_id.clone(),
        kid_profile: s.kid,
        recap_off: false,
        counted_through_utc_ms: s
            .counted_through_utc_ms
            .unwrap_or(first_day_after_year + 2 * DAY_MS),
        cleared_since: s.cleared_since.clone(),
        joined_day: None,
        packs: s
            .packs
            .iter()
            .map(|p| PackInfo {
                id: (*p).into(),
                version: 1,
            })
            .collect(),
        past: s.past.clone(),
        album_palettes,
        album_track_counts,
        fingerprints,
        artists_before,
        hemisphere: Hemisphere::North,
        plays,
    }
}

/// The input as a JSON file: settings pretty-printed, one play per line.
pub fn input_file(i: &RecapInput) -> String {
    let mut head = i.clone();
    head.plays.clear();
    let pretty = serde_json::to_string_pretty(&head).expect("input serializes");
    let plays: Vec<String> = i
        .plays
        .iter()
        .map(|p| format!("    {}", serde_json::to_string(p).expect("play serializes")))
        .collect();
    let list = if plays.is_empty() {
        "[]".to_string()
    } else {
        format!("[\n{}\n  ]", plays.join(",\n"))
    };
    let out = pretty.replacen("\"plays\": []", &format!("\"plays\": {list}"), 1);
    out + "\n"
}
