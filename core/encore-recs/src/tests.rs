//! The engine on a generated library and a listener with known tastes.

use crate::det::Rng;
use crate::mixes::Previous;
use crate::*;

pub(crate) const NOW: i64 = 1_790_000_000_000; // a Wednesday in 2026, UTC

const GENRES: [&str; 6] = ["Ambient", "Folk", "Rock", "Hip Hop", "Pop", "Jazz"];

/// 40 artists, 4 albums each, 10 songs per album; artist a plays genre a % 6.
pub(crate) fn library() -> Catalog {
    let mut songs = Vec::new();
    for a in 0..40 {
        for al in 0..4 {
            for t in 0..10 {
                songs.push(Song {
                    key: format!("k{a:02}-{al}-{t}"),
                    title: format!("Song {t}"),
                    artists: vec![format!("Artist {a}")],
                    album: Some(format!("Album {a}-{al}")),
                    album_artist: Some(format!("Artist {a}")),
                    genres: vec![GENRES[a % 6].to_owned()],
                    year: Some(1970 + (a as i32 % 5) * 10 + al),
                    duration_ms: 200_000,
                    added_utc_ms: Some(NOW - 400 * DAY_MS),
                    server_play_count: 0,
                    server_last_played_utc_ms: None,
                    server_favorite: false,
                    explicit: a == 39,
                    sound: None,
                });
            }
        }
    }
    Catalog::new(songs)
}

/// A listener who loves artists 0-4 (Ambient and Folk at night, Rock in the
/// morning), skips artists 30-34 early, and listens for `days`.
pub(crate) fn history(days: i64, per_day: usize) -> History {
    let mut rng = Rng::new(42);
    let mut plays = Vec::new();
    for d in 0..days {
        let day = NOW - (days - d) * DAY_MS;
        for s in 0..per_day {
            let night = s % 2 == 0;
            let start =
                day + if night { 22 * 3_600_000 } else { 8 * 3_600_000 } + s as i64 * 240_000;
            let (artist, outcome, app) = match rng.below(10) {
                0..=5 => {
                    let a = if night {
                        [0usize, 1, 6, 7][rng.below(4)]
                    } else {
                        [2usize, 3, 4, 8][rng.below(4)]
                    };
                    (a, Outcome::Finished, rng.below(2) == 0)
                }
                6 | 7 => (30 + rng.below(5), Outcome::SkippedEarly, true),
                _ => (10 + rng.below(10), Outcome::SkippedMid, true),
            };
            plays.push(Play {
                song_key: format!("k{artist:02}-{}-{}", rng.below(4), rng.below(10)),
                started_utc_ms: start,
                utc_offset_minutes: 0,
                listened_ms: if outcome == Outcome::Finished {
                    200_000
                } else {
                    10_000
                },
                outcome: Some(outcome),
                app_chosen: app,
                strong_intent: false,
            });
        }
    }
    History {
        plays,
        likes: vec![
            ("k00-0-0".into(), NOW - 90 * DAY_MS),
            ("k01-1-1".into(), NOW - 200 * DAY_MS),
        ],
        ..History::default()
    }
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
fn affinity_follows_what_was_finished_and_skipped() {
    let cat = library();
    let p = Params::default();
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let liking_of_artist = |a: usize| {
        let ids: Vec<usize> = (0..cat.len())
            .filter(|&i| cat.get(i).artists[0] == format!("Artist {a}"))
            .collect();
        ids.iter().map(|&i| m.liking(&cat, i)).sum::<f64>() / ids.len() as f64
    };
    assert!(liking_of_artist(0) > 0.6, "{}", liking_of_artist(0));
    assert!(liking_of_artist(31) < 0.45, "{}", liking_of_artist(31));
    assert!(
        liking_of_artist(25) > 0.45 && liking_of_artist(25) < 0.55,
        "unknown stays neutral"
    );
    assert!(m.plays_counted > 1000);
}

#[test]
fn autoplay_follows_the_seed_and_avoids_early_skips_and_repeats_artists_rarely() {
    let cat = library();
    let p = Params::default();
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let picks = autoplay(&m, &cat, &ctx("k00-1-3"), 10, &p);
    assert_eq!(picks.len(), 10);
    let artists: Vec<String> = picks
        .iter()
        .map(|x| cat.get(cat.find(&x.song_key).unwrap()).artists[0].clone())
        .collect();
    assert!(
        !artists
            .iter()
            .any(|a| a == "Artist 30" || a == "Artist 31" || a == "Artist 32"),
        "{artists:?}"
    );
    for w in artists.windows(2) {
        assert_ne!(w[0], w[1], "no artist twice in a row");
    }
    assert!(!picks[0].exploration, "never the first slot");
    assert!(
        picks
            .windows(2)
            .all(|w| !(w[0].exploration && w[1].exploration))
    );
    assert!(picks.iter().all(|x| x.reasons.len() <= 2));
    // The same inputs, the same picks.
    assert_eq!(picks, autoplay(&m, &cat, &ctx("k00-1-3"), 10, &p));
}

#[test]
fn not_interested_hard_filters_and_reasons() {
    let cat = library();
    let p = Params::default();
    let mut h = history(60, 20);
    h.not_interested.push(NotInterested {
        scope: Scope::Artist("Artist 1".into()),
        reason: NotInterestedReason::NotMyTaste,
        at_utc_ms: NOW - DAY_MS,
        bucket: 3,
    });
    let m = Model::build(&h, &cat, &p, NOW, 0);
    let picks = autoplay(&m, &cat, &ctx("k00-0-1"), 30, &p);
    assert!(
        picks.iter().all(|x| !x.song_key.starts_with("k01-")),
        "Artist 1 never suggested"
    );
    // Explicit songs hidden when asked.
    let mut c = ctx("k39-0-0");
    c.hide_explicit = true;
    assert!(
        autoplay(&m, &cat, &c, 20, &p)
            .iter()
            .all(|x| !x.song_key.starts_with("k39-"))
    );
    // Offline: only downloaded songs.
    let mut c = ctx("k00-0-1");
    c.only = Some(vec!["k02-0-0".into(), "k03-0-0".into()]);
    let picks = autoplay(&m, &cat, &c, 5, &p);
    assert!(
        picks
            .iter()
            .all(|x| x.song_key == "k02-0-0" || x.song_key == "k03-0-0")
    );
}

#[test]
fn three_early_skips_stop_exploring_and_a_like_pulls_toward_it() {
    let cat = library();
    let p = Params::default();
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let mut c = ctx("k00-0-1");
    c.exploration_percent = 20;
    c.early_skips_in_a_row = 3;
    assert!(
        autoplay(&m, &cat, &c, 20, &p)
            .iter()
            .all(|x| !x.exploration)
    );
    let mut c = ctx("k00-0-1");
    c.pulled_toward = Some("k02-2-2".into());
    let picks = autoplay(&m, &cat, &c, 2, &p);
    for x in &picks {
        let i = cat.find(&x.song_key).unwrap();
        assert!(
            m.similarity(&cat, cat.find("k02-2-2").unwrap(), i) > 0.2,
            "{}",
            x.song_key
        );
    }
}

#[test]
fn smart_shuffle_adds_one_suggestion_per_four_and_radio_holds_its_seed() {
    let cat = library();
    let p = Params::default();
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let list: Vec<String> = (0..10).map(|t| format!("k02-0-{t}")).collect();
    let out = smart_shuffle(&m, &cat, &list, &ctx(""), &p);
    assert_eq!(out.iter().filter(|x| x.1).count(), 2);
    assert_eq!(out.iter().filter(|x| !x.1).count(), 10);
    assert!(out.iter().filter(|x| x.1).all(|x| !list.contains(&x.0)));
    let picks = radio(&m, &cat, "k03-0-0", &ctx("k20-0-0"), 10, &p);
    let near = picks
        .iter()
        .filter(|x| {
            m.similarity(
                &cat,
                cat.find("k03-0-0").unwrap(),
                cat.find(&x.song_key).unwrap(),
            ) > 0.15
        })
        .count();
    assert!(near >= 6, "radio stays near its seed: {near} of 10");
    assert!(
        !why(&m, &cat, &picks[0].song_key, &ctx("k03-0-0")).is_empty()
            || picks[0].reasons.is_empty()
    );
    assert!(sounds_like(&m, &cat, "k00-0-0", 2).len() <= 2);
}

#[test]
fn mixes_follow_the_cold_start_table() {
    let cat = library();
    let p = Params::default();
    // 0 plays: a Starter mix from the server's favourites and most played.
    let mut songs: Vec<Song> = cat.songs().to_vec();
    for s in songs.iter_mut().take(60) {
        s.server_play_count = 5;
    }
    let cold = Catalog::new(songs);
    let m = Model::build(&History::default(), &cold, &p, NOW, 0);
    let set = build_mixes(&m, &cold, &p, &Previous::default());
    assert_eq!(set.mixes.len(), 1);
    assert_eq!(set.mixes[0].slot, MixSlot::Starter);
    assert!(set.learning);
    // 1 to 49 plays: at most three, still learning.
    let m = Model::build(&history(2, 20), &cat, &p, NOW, 0);
    assert!(m.plays_counted < 50);
    let set = build_mixes(&m, &cat, &p, &Previous::default());
    assert!((1..=3).contains(&set.mixes.len()), "{}", set.mixes.len());
    assert!(set.learning);
    // 50 or more, but under 30 days: the two strongest moods only.
    let m = Model::build(&history(10, 20), &cat, &p, NOW, 0);
    let set = build_mixes(&m, &cat, &p, &Previous::default());
    assert!(!set.learning);
    assert!((2..=2).contains(&set.mixes.len()), "{}", set.mixes.len());
    // 30 days or more: up to five.
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let set = build_mixes(&m, &cat, &p, &Previous::default());
    assert!(set.mixes.len() >= 4, "{}", set.mixes.len());
}

#[test]
fn every_mix_keeps_the_rules() {
    let cat = library();
    let p = Params::default();
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let set = build_mixes(&m, &cat, &p, &Previous::default());
    let titles: Vec<String> = set.mixes.iter().map(|x| x.title.clone()).collect();
    for (x, mix) in set.mixes.iter().enumerate() {
        let others: Vec<String> = titles
            .iter()
            .enumerate()
            .filter(|(y, _)| *y != x)
            .map(|(_, t)| t.clone())
            .collect();
        assert!(
            titles::check(&mix.title, &others, &[]).is_ok(),
            "{}",
            mix.title
        );
        assert!(
            (p.mix_min_songs..=p.mix_max_songs).contains(&mix.songs.len()),
            "{} songs",
            mix.songs.len()
        );
        let artists: Vec<String> = mix
            .songs
            .iter()
            .map(|s| cat.get(cat.find(&s.song_key).unwrap()).artists[0].clone())
            .collect();
        for a in &artists {
            assert!(artists.iter().filter(|b| *b == a).count() <= p.per_artist_in_mix);
        }
        for w in artists.windows(2) {
            assert_ne!(w[0], w[1], "{}: no artist twice in a row", mix.title);
        }
        for other in set.mixes.iter().skip(x + 1) {
            let shared = mix
                .songs
                .iter()
                .filter(|s| other.songs.iter().any(|o| o.song_key == s.song_key))
                .count();
            assert!(
                shared * 100 <= mix.songs.len() * p.mix_max_overlap_percent as usize + 100,
                "overlap {shared}"
            );
        }
    }
    let stretch = set
        .mixes
        .iter()
        .find(|x| x.slot == MixSlot::Stretch)
        .expect("a stretch mix");
    assert!(
        stretch.songs.iter().any(|s| s.exploration),
        "the stretch mix has new songs"
    );
    // Deterministic, and a hidden mix stays hidden.
    assert_eq!(set, build_mixes(&m, &cat, &p, &Previous::default()));
    let hidden = vec![set.mixes[0].id.clone()];
    let again = build_mixes(
        &m,
        &cat,
        &p,
        &Previous {
            hidden: &hidden,
            ..Previous::default()
        },
    );
    assert!(again.mixes.iter().all(|x| x.id != hidden[0]));
}

#[test]
fn tomorrows_mixes_change_by_at_most_a_third_unless_regenerated() {
    let cat = library();
    let p = Params::default();
    let m = Model::build(&history(60, 20), &cat, &p, NOW, 0);
    let today = build_mixes(&m, &cat, &p, &Previous::default());
    let mut h = history(60, 20);
    h.plays.truncate(h.plays.len() - 200);
    let m2 = Model::build(&h, &cat, &p, NOW + DAY_MS, 0);
    let tomorrow = build_mixes(
        &m2,
        &cat,
        &p,
        &Previous {
            set: Some(&today),
            ..Previous::default()
        },
    );
    let a = &today.mixes[0];
    let b = tomorrow.mixes.iter().find(|x| x.slot == a.slot).unwrap();
    let kept = b
        .songs
        .iter()
        .filter(|s| a.songs.iter().any(|o| o.song_key == s.song_key))
        .count();
    assert!(
        kept * 100 >= a.songs.len() * 60,
        "kept {kept} of {}",
        a.songs.len()
    );
}

#[test]
fn shelves_and_stats() {
    let cat = library();
    let p = Params::default();
    let h = history(60, 20);
    let m = Model::build(&h, &cat, &p, NOW, 0);
    let s = build_shelves(&m, &cat, &p);
    assert!(!s.rediscover.is_empty() && s.rediscover.len() <= p.rediscover_count);
    assert!(
        s.deep_cuts
            .iter()
            .all(|x| m.facts[cat.find(&x.song_key).unwrap()].plays == 0)
    );
    let plays: Vec<(Play, String)> = h
        .plays
        .iter()
        .cloned()
        .map(|x| (x, "autoplay".to_owned()))
        .collect();
    let st = stats(&plays, &cat, NOW);
    assert_eq!(st.shelves.len(), 1);
    assert!(
        st.shelves[0].early_skips_per_1000 > 200,
        "{:?}",
        st.shelves[0]
    );
}

#[test]
fn time_buckets_and_weekends() {
    assert_eq!(bucket_of(0, 0), 0); // 1970-01-01 00:00 UTC
    assert_eq!(bucket_of(7 * 3_600_000, 0), 1);
    assert_eq!(bucket_of(13 * 3_600_000, 0), 2);
    assert_eq!(bucket_of(20 * 3_600_000, 0), 3);
    assert_eq!(
        bucket_of(20 * 3_600_000, 300),
        0,
        "UTC+5 at 20:00 UTC is 01:00"
    );
    assert!(!weekend(0, 0), "a Thursday");
    assert!(weekend(2 * DAY_MS, 0), "a Saturday");
}
