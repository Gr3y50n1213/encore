//! Moods from what songs sound like (Phase 10; PLAN-PRODUCT section 7.1; board 07's Browse by
//! mood): regions of the library's own sound, each named only with words tied to what is
//! measured.
//!
//! A song's [`Sound`] carries energy, brightness (warm to bright), dynamics (how much the loudness
//! moves) and tempo. Energy, brightness and dynamics are placed in thirds *of this library* (low,
//! middle, high), so a quiet folk library still has its own "lively" third and a loud one its own
//! calm one; tempo is absolute (slow under 95 beats a minute, fast from 120). A mood is a rule
//! over those levels. A song can be in more than one mood. Nothing is guessed: a song without a
//! sound is in no mood, and the words never claim happy, sad or anything a measurement can't
//! show (the plan's "never faked").
//!
//! Pure and deterministic: the same library gives the same moods on every device and the server.

use crate::Catalog;
use crate::sound::Sound;

/// A mood is offered only with at least this many songs (board 07).
pub const MIN_SONGS: usize = 25;
/// Moods need this many analysed songs before thirds mean anything.
pub const MIN_ANALYSED: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Low,
    Mid,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pace {
    Slow,
    Steady,
    Fast,
    /// No steady beat was found.
    Free,
}

/// One song's place on the map.
#[derive(Debug, Clone, Copy)]
struct Place {
    energy: Level,
    brightness: Level,
    dynamics: Level,
    pace: Pace,
}

/// The moods, in the order they are shown: an id (stable, for links and pins) and a title of two
/// to four plain words (titles.rs's rules), and which places it takes.
type MoodRule = (&'static str, &'static str, fn(&Place) -> bool);

const MOODS: &[MoodRule] = &[
    ("calm-warm", "Calm and Warm", |p| {
        p.energy == Level::Low && p.brightness != Level::High
    }),
    ("quiet-bright", "Quiet and Bright", |p| {
        p.energy == Level::Low && p.brightness == Level::High
    }),
    ("slow-easy", "Slow and Easy", |p| {
        p.pace == Pace::Slow && p.energy != Level::High
    }),
    ("steady-groove", "Steady Groove", |p| {
        p.energy == Level::Mid && p.dynamics == Level::Low && p.pace == Pace::Steady
    }),
    ("bright-lively", "Bright and Lively", |p| {
        p.brightness == Level::High && p.energy != Level::Low
    }),
    ("upbeat-fast", "Upbeat and Fast", |p| {
        p.energy == Level::High && p.pace == Pace::Fast
    }),
    ("heavy-dark", "Heavy and Dark", |p| {
        p.energy == Level::High && p.brightness == Level::Low
    }),
    ("big-swings", "Big Swings", |p| p.dynamics == Level::High),
];

/// A mood with its songs (catalog indexes, in catalog order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mood {
    pub id: &'static str,
    pub title: &'static str,
    pub songs: Vec<usize>,
}

/// The cut points between the thirds of the analysed songs' values.
fn thirds(values: &mut [u8]) -> (u8, u8) {
    values.sort_unstable();
    let n = values.len();
    (values[n / 3], values[2 * n / 3])
}

fn level(v: u8, (lo, hi): (u8, u8)) -> Level {
    if v < lo {
        Level::Low
    } else if v >= hi {
        Level::High
    } else {
        Level::Mid
    }
}

fn pace(s: &Sound) -> Pace {
    match s.bpm_tenths {
        0 => Pace::Free,
        t if t < 950 => Pace::Slow,
        t if t >= 1200 => Pace::Fast,
        _ => Pace::Steady,
    }
}

/// Every mood this library has, with at least [`MIN_SONGS`] songs; empty until [`MIN_ANALYSED`]
/// songs are analysed.
pub fn moods(cat: &Catalog) -> Vec<Mood> {
    let analysed: Vec<(usize, &Sound)> = cat
        .songs()
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.sound.as_ref().map(|x| (i, x)))
        .collect();
    if analysed.len() < MIN_ANALYSED {
        return Vec::new();
    }
    let cuts = |f: fn(&Sound) -> u8| {
        let mut v: Vec<u8> = analysed.iter().map(|(_, s)| f(s)).collect();
        thirds(&mut v)
    };
    let (e, b, d) = (
        cuts(|s| s.energy),
        cuts(|s| s.brightness),
        cuts(|s| s.dynamics),
    );
    let places: Vec<(usize, Place)> = analysed
        .iter()
        .map(|(i, s)| {
            (
                *i,
                Place {
                    energy: level(s.energy, e),
                    brightness: level(s.brightness, b),
                    dynamics: level(s.dynamics, d),
                    pace: pace(s),
                },
            )
        })
        .collect();
    MOODS
        .iter()
        .map(|(id, title, takes)| Mood {
            id,
            title,
            songs: places
                .iter()
                .filter(|(_, p)| takes(p))
                .map(|(i, _)| *i)
                .collect(),
        })
        .filter(|m| m.songs.len() >= MIN_SONGS)
        .collect()
}

/// One mood by its id, if this library has it.
pub fn mood(cat: &Catalog, id: &str) -> Option<Mood> {
    moods(cat).into_iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::det::Rng;
    use crate::{Song, sound::DIMS};

    fn song(i: usize, energy: u8, brightness: u8, dynamics: u8, bpm: u16) -> Song {
        Song {
            key: format!("k{i}"),
            title: format!("Song {i}"),
            artists: vec![format!("Artist {}", i % 30)],
            album: Some(format!("Album {}", i % 50)),
            album_artist: None,
            genres: vec!["Rock".into()],
            year: Some(2000),
            duration_ms: 200_000,
            added_utc_ms: Some(1),
            server_play_count: 0,
            server_last_played_utc_ms: None,
            server_favorite: false,
            explicit: false,
            sound: Some(Sound {
                projection: vec![0; DIMS],
                bpm_tenths: bpm,
                energy,
                brightness,
                dynamics,
                key: 255,
            }),
        }
    }

    fn library(n: usize, seed: u64) -> Catalog {
        let mut rng = Rng::new(seed);
        let mut byte = || (rng.unit() * 255.0) as u8;
        let songs = (0..n)
            .map(|i| {
                let (e, b, d) = (byte(), byte(), byte());
                song(i, e, b, d, 600 + u16::from(byte()) * 4)
            })
            .collect();
        Catalog::new(songs)
    }

    #[test]
    fn moods_come_from_the_library_and_need_enough_songs() {
        assert!(
            moods(&library(MIN_ANALYSED - 1, 1)).is_empty(),
            "too few analysed"
        );
        let cat = library(600, 1);
        let all = moods(&cat);
        assert!(
            all.len() >= 6,
            "{:?}",
            all.iter().map(|m| m.id).collect::<Vec<_>>()
        );
        assert!(all.iter().all(|m| m.songs.len() >= MIN_SONGS));
        // Calm means the library's quietest third: every calm song is quieter than every
        // upbeat one.
        let calm = mood(&cat, "calm-warm").unwrap();
        let upbeat = mood(&cat, "upbeat-fast").unwrap();
        let max_calm = calm
            .songs
            .iter()
            .map(|&i| cat.get(i).sound.as_ref().unwrap().energy)
            .max();
        let min_up = upbeat
            .songs
            .iter()
            .map(|&i| cat.get(i).sound.as_ref().unwrap().energy)
            .min();
        assert!(max_calm < min_up);
        assert!(
            upbeat
                .songs
                .iter()
                .all(|&i| cat.get(i).sound.as_ref().unwrap().bpm_tenths >= 1200)
        );
    }

    #[test]
    fn a_quiet_library_still_has_its_own_lively_third_and_songs_without_sound_have_no_mood() {
        // Every song quiet in absolute terms: the moods are relative to this library.
        let mut songs: Vec<Song> = (0..300)
            .map(|i| {
                song(
                    i,
                    (i % 60) as u8,
                    (i * 7 % 60) as u8,
                    (i * 13 % 60) as u8,
                    1000,
                )
            })
            .collect();
        songs.push(Song {
            sound: None,
            ..song(999, 0, 0, 0, 0)
        });
        let cat = Catalog::new(songs);
        let all = moods(&cat);
        assert!(all.iter().any(|m| m.id == "bright-lively"), "{all:?}");
        let unanalysed = cat.find("k999").unwrap();
        assert!(all.iter().all(|m| !m.songs.contains(&unanalysed)));
    }

    #[test]
    fn titles_follow_the_title_rules_and_ids_are_unique() {
        let mut ids = std::collections::HashSet::new();
        for (id, title, _) in MOODS {
            assert!(ids.insert(*id), "{id} twice");
            let words = title.split(' ').count();
            assert!((2..=4).contains(&words), "{title}");
            assert_eq!(crate::titles::check(title, &[], &[]), Ok(()), "{title}");
        }
    }

    #[test]
    fn a_mood_mix_keeps_to_the_mood_and_the_caps() {
        let cat = library(600, 3);
        let p = crate::Params::default();
        let model = crate::Model::build(&crate::History::default(), &cat, &p, 1_800_000_000_000, 0);
        let mix = crate::build_mood_mix(&model, &cat, &p, "calm-warm").expect("a mix");
        assert_eq!(mix.id, "mood-calm-warm");
        assert_eq!(mix.slot, crate::MixSlot::Mood);
        assert_eq!(mix.title, "Calm and Warm");
        assert!(
            (p.mix_min_songs..=p.mix_max_songs).contains(&mix.songs.len()),
            "{}",
            mix.songs.len()
        );
        let calm = mood(&cat, "calm-warm").unwrap();
        let mut per_artist = std::collections::HashMap::new();
        for pick in &mix.songs {
            let i = cat.find(&pick.song_key).unwrap();
            assert!(calm.songs.contains(&i), "{} is not calm", pick.song_key);
            *per_artist.entry(cat.get(i).artists[0].clone()).or_insert(0) += 1;
        }
        assert!(per_artist.values().all(|n| *n <= p.per_artist_in_mix));
        assert_eq!(
            crate::build_mood_mix(&model, &cat, &p, "calm-warm"),
            Some(mix),
            "the same every time"
        );
        assert!(crate::build_mood_mix(&model, &cat, &p, "no-such-mood").is_none());
    }

    #[test]
    fn the_same_library_gives_the_same_moods() {
        assert_eq!(moods(&library(400, 7)), moods(&library(400, 7)));
    }
}
