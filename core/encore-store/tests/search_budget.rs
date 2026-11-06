//! Search must answer in under `search.budgetMs` (50 ms, board 07) on a large
//! library. This builds 50,000 songs and times typical queries.
//!
//! Ignored in ordinary runs (it takes a few seconds); CI runs it in release mode:
//!   cargo test -p encore-store --release --test search_budget -- --ignored --nocapture
//!
//! A CI runner is not a phone: this proves the query plan stays fast at this size,
//! and the platform tracks measure it again on devices (Phase 6).

use std::time::{Duration, Instant};

use encore_sources::{Album, Artist, ArtistCredit, LibraryDelta, Track};
use encore_store::{NewSource, SearchLimits, Store};

const SONGS: usize = 50_000;
const BUDGET: Duration = Duration::from_millis(50);

/// Deterministic made-up words, so every run searches the same library.
struct Words(u64);

impl Words {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn word(&mut self) -> String {
        const SYLLABLES: [&str; 24] = [
            "ka", "lo", "mi", "ren", "sol", "ta", "vee", "ni", "or", "bel", "cu", "dra", "en",
            "fo", "gal", "hu", "is", "jo", "mar", "nu", "pi", "qua", "ros", "ul",
        ];
        let n = 2 + (self.next() % 3) as usize;
        let mut w: String = (0..n)
            .map(|_| SYLLABLES[(self.next() % 24) as usize])
            .collect();
        if let Some(first) = w.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        w
    }

    fn title(&mut self) -> String {
        let n = 1 + (self.next() % 4) as usize;
        (0..n).map(|_| self.word()).collect::<Vec<_>>().join(" ")
    }
}

fn library() -> LibraryDelta {
    let mut w = Words(0x9E37_79B9_7F4A_7C15);
    let artists: Vec<Artist> = (0..2_000)
        .map(|i| {
            let name = w.title();
            Artist {
                source_item_id: format!("ar{i}"),
                sort_name: name.clone(),
                name,
                mbid: None,
            }
        })
        .collect();
    let albums: Vec<Album> = (0..4_000)
        .map(|i| {
            let title = w.title();
            let a = &artists[i % artists.len()];
            Album {
                source_item_id: format!("al{i}"),
                sort_title: title.clone(),
                title,
                album_artists: vec![ArtistCredit {
                    source_item_id: Some(a.source_item_id.clone()),
                    name: a.name.clone(),
                }],
                year: Some(1970 + (i % 55) as i32),
                mbid: None,
                genres: vec![],
                date_added_utc_ms: Some(i as i64),
                has_artwork: true,
                artwork_tag: None,
            }
        })
        .collect();
    let tracks: Vec<Track> = (0..SONGS)
        .map(|i| {
            let title = w.title();
            let album = &albums[i % albums.len()];
            let artist = &album.album_artists[0];
            Track {
                source_item_id: format!("t{i}"),
                sort_title: title.clone(),
                title,
                album_item_id: Some(album.source_item_id.clone()),
                album_title: Some(album.title.clone()),
                artists: vec![artist.clone()],
                album_artists: vec![artist.clone()],
                disc_number: Some(1),
                track_number: Some((i / albums.len()) as u32 + 1),
                duration_ms: 180_000 + (i % 120) as u64 * 1000,
                year: album.year,
                container: Some("flac".into()),
                bitrate: None,
                mbid_recording: None,
                // Every song is Ambient (the genre's deep page); one in 500 is
                // also Choral (a rare genre: most of the library is passed over).
                genres: if i % 500 == 0 {
                    vec!["Ambient".into(), "Choral".into()]
                } else {
                    vec!["Ambient".into()]
                },
                date_added_utc_ms: Some(i as i64),
                has_lyrics: false,
                track_gain_db: None,
                album_gain_db: None,
            }
        })
        .collect();
    LibraryDelta {
        full: true,
        artists,
        albums,
        tracks,
        playlists: vec![],
        removed_item_ids: vec![],
        favorite_item_ids: None,
        cursor: None,
    }
}

fn store_with_library() -> (Store, LibraryDelta) {
    let store = Store::open_in_memory().unwrap();
    let src = store
        .add_source(&NewSource {
            kind: "jellyfin",
            name: "Bench",
            base_url: "https://bench.example",
            server_id: "s",
            server_version: None,
            user_id: "u",
            user_name: "u",
            pinned_cert_sha256: None,
            created_utc_ms: 0,
        })
        .unwrap();
    let delta = library();
    let started = Instant::now();
    store.apply_delta(src, &delta).unwrap();
    println!(
        "indexed {SONGS} songs, 4000 albums, 2000 artists in {:.1} s",
        started.elapsed().as_secs_f64()
    );
    (store, delta)
}

/// Median of 7 runs, printed; returns the median.
fn time(label: &str, mut f: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..7)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    times.sort();
    let median = times[times.len() / 2];
    println!(
        "{label:<48} median {:.2} ms (max {:.2} ms)",
        median.as_secs_f64() * 1e3,
        times[6].as_secs_f64() * 1e3
    );
    median
}

/// "Browse your whole library offline-fast" (Phase 2). tokens.json has no browse
/// budget, so each page is held to the search budget: a screen of the library
/// answers as fast as a search does. First and deep pages of every sort.
#[test]
#[ignore = "slow: run in release mode in CI"]
fn browse_pages_stay_under_budget_on_50000_songs() {
    use encore_store::{AlbumSort, Page, TrackSort};
    let (store, _) = store_with_library();
    let first = Page {
        offset: 0,
        limit: 100,
    };
    let deep = Page {
        offset: SONGS as u32 - 100,
        limit: 100,
    };
    let deep_albums = Page {
        offset: 3_900,
        limit: 100,
    };
    let album_id = store.albums(AlbumSort::Title, first).unwrap()[50].id;
    let artist_id = store.artists(first).unwrap()[50].id;
    let mut worst = Duration::ZERO;
    let mut run = |label: &str, f: &mut dyn FnMut()| worst = worst.max(time(label, f));
    for sort in [
        AlbumSort::Title,
        AlbumSort::Artist,
        AlbumSort::Year,
        AlbumSort::RecentlyAdded,
    ] {
        run(&format!("albums {sort:?}, first page"), &mut || {
            std::hint::black_box(store.albums(sort, first).unwrap());
        });
        run(&format!("albums {sort:?}, last page"), &mut || {
            std::hint::black_box(store.albums(sort, deep_albums).unwrap());
        });
    }
    for sort in [
        TrackSort::Title,
        TrackSort::Artist,
        TrackSort::Album,
        TrackSort::RecentlyAdded,
    ] {
        run(&format!("songs {sort:?}, first page"), &mut || {
            std::hint::black_box(store.tracks(sort, first).unwrap());
        });
        run(&format!("songs {sort:?}, last page"), &mut || {
            std::hint::black_box(store.tracks(sort, deep).unwrap());
        });
    }
    run("artists, first page", &mut || {
        std::hint::black_box(store.artists(first).unwrap());
    });
    run("an album's songs", &mut || {
        std::hint::black_box(store.album_tracks(album_id).unwrap());
    });
    run("an artist's albums", &mut || {
        std::hint::black_box(store.artist_albums(artist_id).unwrap());
    });
    run("an artist's songs, first page", &mut || {
        std::hint::black_box(store.artist_tracks(artist_id, first).unwrap());
    });
    run("genres with counts", &mut || {
        std::hint::black_box(store.genres().unwrap());
    });
    run("a genre's songs, first page", &mut || {
        std::hint::black_box(store.genre_tracks("Ambient", first).unwrap());
    });
    run("a genre's songs, last page", &mut || {
        std::hint::black_box(store.genre_tracks("Ambient", deep).unwrap());
    });
    run("a rare genre's songs (1 in 500)", &mut || {
        let songs = store.genre_tracks("choral", first).unwrap();
        assert_eq!(songs.len(), 100);
        std::hint::black_box(songs);
    });
    println!(
        "browse: slowest median {:.2} ms, budget {} ms",
        worst.as_secs_f64() * 1e3,
        BUDGET.as_millis()
    );
    assert!(
        worst < BUDGET,
        "a browse page took {worst:?}, over the {BUDGET:?} budget"
    );
}

#[test]
#[ignore = "slow: run in release mode in CI"]
fn search_stays_under_budget_on_50000_songs() {
    let (store, delta) = store_with_library();
    let probe = delta.tracks[SONGS / 2].title.clone();
    let word = probe.split(' ').next().unwrap().to_owned();
    let mut typo = word.to_lowercase();
    typo.remove(1); // a dropped letter
    let artist = delta.artists[7].name.clone();
    let queries = [
        ("whole word", word.to_lowercase()),
        ("part of a word", word[1..4].to_lowercase()),
        ("full title", probe.to_lowercase()),
        ("typo", typo),
        ("one letter", "k".to_owned()),
        ("two letters", "ma".to_owned()),
        ("artist", artist.to_lowercase()),
        ("no match", "zzzzqqqq".to_owned()),
    ];
    let limits = SearchLimits {
        songs: 20,
        albums: 10,
        artists: 10,
        playlists: 10,
    };
    let mut worst = Duration::ZERO;
    for (label, q) in &queries {
        let mut times: Vec<Duration> = (0..7)
            .map(|_| {
                let t = Instant::now();
                let r = store.search(q, &limits, 2).unwrap();
                std::hint::black_box(r);
                t.elapsed()
            })
            .collect();
        times.sort();
        let median = times[times.len() / 2];
        worst = worst.max(median);
        println!(
            "search {label:<15} {q:?}: median {:.2} ms (max {:.2} ms)",
            median.as_secs_f64() * 1e3,
            times[6].as_secs_f64() * 1e3
        );
    }
    println!(
        "slowest median {:.2} ms, budget {} ms",
        worst.as_secs_f64() * 1e3,
        BUDGET.as_millis()
    );
    assert!(
        worst < BUDGET,
        "search took {worst:?}, over the {BUDGET:?} budget"
    );
}
