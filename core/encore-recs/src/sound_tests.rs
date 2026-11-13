//! The engine with sound (Phase 10): a library whose sound families cut across its genres, so
//! "sounds like" can be told from "is tagged like". Artist a plays genre a % 6 (the generated
//! library's) and sounds like family a % 4: artists 0, 4, 8 share a family and differ in genre;
//! artists 0, 6, 12 share a genre and differ in family.

use crate::sound::{DIMS, Sound};
use crate::tests::{NOW, history, library};
use crate::*;

fn family_of(artist: usize) -> usize {
    artist % 4
}

fn artist_of(cat: &Catalog, i: usize) -> usize {
    cat.get(i).artists[0]
        .trim_start_matches("Artist ")
        .parse()
        .unwrap()
}

/// A song's sound: its family's pattern, a little different for every song, and a tempo and energy
/// that belong to the family.
fn sound_of(artist: usize, serial: usize) -> Sound {
    let f = family_of(artist);
    let projection = (0..DIMS)
        .map(|d| {
            let base: i16 = if d % 4 == f { 100 } else { -30 };
            let wobble = ((d * 31 + serial * 17 + artist * 7) % 17) as i16 - 8;
            base + wobble
        })
        .collect();
    Sound {
        projection,
        bpm_tenths: (800 + 200 * f) as u16,
        energy: (60 + 50 * f + serial % 7) as u8,
        brightness: 100,
        dynamics: 80,
        key: (f * 3) as u8,
    }
}

fn with_sound(everything: bool) -> Catalog {
    let mut songs = library().songs().to_vec();
    let mut serial = 0;
    for s in &mut songs {
        serial += 1;
        let a: usize = s.artists[0].trim_start_matches("Artist ").parse().unwrap();
        // Half the library unanalysed, when asked: the engine has to cope with a mix.
        if everything || a.is_multiple_of(2) {
            s.sound = Some(sound_of(a, serial));
        }
    }
    Catalog::new(songs)
}

fn ctx(seed_key: &str) -> Context {
    Context {
        seeds: vec![seed_key.into()],
        exploration_percent: 10,
        seed: 7,
        ..Context::default()
    }
}

#[test]
fn the_songs_that_sound_most_alike_are_found_across_genres() {
    let cat = with_sound(true);
    let me = cat.find("k00-0-0").unwrap();
    // 400 songs (ten artists) share artist 0's sound family; the 200 nearest are all of it.
    let near = cat.nearest_by_sound(me, 200);
    assert_eq!(near.len(), 200);
    assert!(
        near.iter().all(|&j| family_of(artist_of(&cat, j)) == 0),
        "only artists 0, 4, 8 ... sound like artist 0"
    );
    assert!(
        near.iter().any(|&j| !artist_of(&cat, j).is_multiple_of(6)),
        "and some of them are another genre: that is the point"
    );
    assert!(!near.contains(&me), "never itself");
    // The same answer every time.
    assert_eq!(near, cat.nearest_by_sound(me, 200));
}

#[test]
fn songs_without_a_sound_are_left_alone_not_guessed() {
    let cat = with_sound(false);
    assert!(cat.analysed() > 0 && cat.analysed() < cat.len());
    let analysed = (0..cat.len())
        .find(|&i| cat.get(i).sound.is_some())
        .unwrap();
    let bare = (0..cat.len())
        .find(|&i| cat.get(i).sound.is_none())
        .unwrap();
    assert!(cat.sound_similarity(analysed, bare).is_none());
    assert!(
        cat.nearest_by_sound(bare, 5).is_empty(),
        "nothing to compare, so nothing is claimed"
    );
    assert!(
        cat.nearest_by_sound(analysed, 5)
            .iter()
            .all(|&j| cat.get(j).sound.is_some())
    );
}

#[test]
fn sounds_like_hears_what_the_tags_do_not_say() {
    let p = Params::default();
    let h = history(60, 20);
    // The listener finished artists 0, 1, 6, 7 (at night) and 2, 3, 4, 8 (mornings).
    let heard = with_sound(true);
    let m = Model::build(&h, &heard, &p, NOW, 0);
    let alike = sounds_like(&m, &heard, "k00-1-3", 2);
    assert_eq!(alike.len(), 2);
    for key in &alike {
        let a = artist_of(&heard, heard.find(key).unwrap());
        assert_eq!(
            family_of(a),
            0,
            "{key} (artist {a}) does not sound like artist 0"
        );
    }
    // Without a sound the same call falls back on tags and what is played together.
    let tagged = library();
    let m1 = Model::build(&h, &tagged, &p, NOW, 0);
    assert_eq!(sounds_like(&m1, &tagged, "k00-1-3", 2).len(), 2);
}

#[test]
fn autoplay_stays_in_the_sound_it_is_seeded_with() {
    let p = Params::default();
    let h = history(60, 20);
    let share = |cat: &Catalog, seed: &str| -> f64 {
        let m = Model::build(&h, cat, &p, NOW, 0);
        let picks = autoplay(&m, cat, &ctx(seed), 12, &p);
        let seed_family = family_of(artist_of(cat, cat.find(seed).unwrap()));
        let same = picks
            .iter()
            .filter(|x| family_of(artist_of(cat, cat.find(&x.song_key).unwrap())) == seed_family)
            .count();
        same as f64 / picks.len() as f64
    };
    // Seeded by artist 4 (Pop, family 0): with the sound the picks lean to family 0; the tags alone
    // would lean to Pop, which is spread over all four families.
    let (heard, plain) = (with_sound(true), library());
    let (with, without) = (share(&heard, "k04-0-2"), share(&plain, "k04-0-2"));
    eprintln!(
        "share of picks in the seed's sound family: {with:.2} with the sound, {without:.2} without"
    );
    assert!(with >= without, "{with} vs {without}");
    assert!(with >= 0.4, "{with}");
}

#[test]
fn picks_move_smoothly_in_tempo_and_energy() {
    let p = Params::default();
    let cat = with_sound(true);
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let picks = autoplay(&m, &cat, &ctx("k08-0-1"), 12, &p);
    let sounds: Vec<&Sound> = picks
        .iter()
        .map(|x| {
            cat.get(cat.find(&x.song_key).unwrap())
                .sound
                .as_ref()
                .unwrap()
        })
        .collect();
    let steps: Vec<f64> = sounds
        .windows(2)
        .map(|w| crate::sound::flow_cost(w[0], w[1]).unwrap())
        .collect();
    let picked: f64 = steps.iter().sum::<f64>() / steps.len() as f64;
    // Against a pick order that ignores sound: the whole library in key order.
    let all: Vec<&Sound> = cat
        .songs()
        .iter()
        .step_by(37)
        .take(12)
        .map(|s| s.sound.as_ref().unwrap())
        .collect();
    let steps: Vec<f64> = all
        .windows(2)
        .map(|w| crate::sound::flow_cost(w[0], w[1]).unwrap())
        .collect();
    let arbitrary: f64 = steps.iter().sum::<f64>() / steps.len() as f64;
    eprintln!("mean jump between picks {picked:.3}, between arbitrary songs {arbitrary:.3}");
    assert!(picked < arbitrary, "{picked} vs {arbitrary}");
}

#[test]
fn picks_are_the_same_every_time_with_sound() {
    let p = Params::default();
    let cat = with_sound(false);
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let a = autoplay(&m, &cat, &ctx("k00-0-1"), 10, &p);
    assert_eq!(a, autoplay(&m, &cat, &ctx("k00-0-1"), 10, &p));
    assert_eq!(a.len(), 10);
}

#[test]
fn a_catalog_from_an_older_device_without_sound_still_loads() {
    // The server and old devices send songs with no "sound" key at all.
    let json = serde_json::json!({
        "key": "k1", "title": "T", "artists": ["A"], "album": null, "album_artist": null,
        "genres": [], "year": null, "duration_ms": 1000, "added_utc_ms": null,
        "server_play_count": 0, "server_last_played_utc_ms": null, "server_favorite": false,
        "explicit": false
    });
    let song: Song = serde_json::from_value(json).unwrap();
    assert!(song.sound.is_none());
    // And a song with no sound is not written with a null.
    assert!(!serde_json::to_string(&song).unwrap().contains("sound"));
}
