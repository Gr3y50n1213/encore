//! Golden files: fixed logs give exact expected recap data, byte for byte.
//!
//! Each case is `tests/golden/<name>.input.json` (an invented profile) and
//! `<name>.expected.json` (the exact bytes of `RecapData::to_json`). Every platform's
//! core and the server build must produce the same bytes from the same input.
//!
//! To regenerate after a deliberate rule change (and bump `ENGINE`):
//!
//!     UPDATE_GOLDEN=1 cargo test -p encore-recap --test golden
//!
//! That rewrites the inputs from the generator in `tests/common` and the expected
//! outputs from the engine. Read the diff before committing it.

mod common;

use std::path::PathBuf;

use common::{Spec, input, input_file, jan1_utc};
use encore_recap::{BeatData, Kind, PastRecap, RecapData, RecapInput, build_recap};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn updating() -> bool {
    std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1")
}

/// The invented profiles, by case name. `years-2027` reads `years-2026`'s output.
fn spec(name: &str) -> Spec {
    match name {
        "light" => {
            let mut s = Spec::new(11, "profile-light-0001", 2026, 40);
            s.artists = 8;
            s.first_doy = 32;
            s.last_doy = 90;
            s.fingerprints = false;
            s.lyric_jumps = false;
            s
        }
        "very-new" => {
            let mut s = Spec::new(12, "profile-new-0002", 2026, 6);
            s.artists = 3;
            s.first_doy = 300;
            s.last_doy = 310;
            s
        }
        "heavy" => {
            let mut s = Spec::new(13, "profile-heavy-0003", 2026, 1400);
            s.artists = 40;
            s.night = 380;
            s.earlier_plays = 300;
            s.past = vec![PastRecap {
                year: 2025,
                pack: Some("tide".into()),
                lead_hue: Some(200),
                listened_ms: Some(2_900 * 60_000),
                personality_metric: Some("topSix".into()),
                personality_value: Some(290),
            }];
            s
        }
        "kid" => {
            let mut s = Spec::new(14, "profile-kid-0004", 2026, 700);
            s.artists = 20;
            s.night = 600;
            s.kid = true;
            s
        }
        "cleared" => {
            let mut s = Spec::new(15, "profile-cleared-0005", 2026, 900);
            s.earlier_plays = 120;
            s.cleared_since = Some("2026-08-03".into());
            s
        }
        "no-fingerprints" => {
            let mut s = Spec::new(16, "profile-plain-0006", 2026, 800);
            s.fingerprints = false;
            s.lyric_jumps = false;
            s
        }
        "so-far" => {
            let mut s = Spec::new(17, "profile-december-0007", 2026, 900);
            s.last_doy = 345;
            // Counted through 12 December 2026.
            s.counted_through_utc_ms = Some(jan1_utc(2026) + 345 * common::DAY_MS);
            s
        }
        "years-2026" => {
            let mut s = Spec::new(18, "profile-years-0008", 2026, 900);
            s.earlier_plays = 200;
            s
        }
        "years-2027" => {
            let mut s = Spec::new(19, "profile-years-0008", 2027, 900);
            s.earlier_plays = 200;
            let prev = read_expected_doc("years-2026");
            s.past = vec![past_from(&prev, 2026)];
            s
        }
        other => panic!("no case {other}"),
    }
}

const CASES: &[&str] = &[
    "very-new",
    "light",
    "heavy",
    "kid",
    "cleared",
    "no-fingerprints",
    "so-far",
    "years-2026",
    "years-2027",
];

/// What an archived recap gives next year's build.
fn past_from(doc: &serde_json::Value, year: i32) -> PastRecap {
    let personality = doc["beats"]
        .as_array()
        .and_then(|b| b.iter().find(|b| b["id"] == "personality"))
        .map(|b| &b["data"]);
    PastRecap {
        year,
        pack: doc["pack"]["id"].as_str().map(String::from),
        lead_hue: doc["palette"]["leadHue"].as_u64().map(|h| h as u16),
        listened_ms: doc["totals"]["listenedMs"].as_u64(),
        personality_metric: personality
            .and_then(|p| p["metric"].as_str())
            .map(String::from),
        personality_value: personality
            .and_then(|p| p["value"].as_u64())
            .map(|v| v as u32),
    }
}

fn read_expected_doc(name: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(dir().join(format!("{name}.expected.json")))
        .expect("expected file (run with UPDATE_GOLDEN=1 once)");
    serde_json::from_str(&text).expect("expected file is JSON")
}

fn read_input(name: &str) -> RecapInput {
    let text = std::fs::read_to_string(dir().join(format!("{name}.input.json")))
        .expect("input file (run with UPDATE_GOLDEN=1 once)");
    serde_json::from_str(&text).expect("input file parses")
}

fn build(i: &RecapInput) -> RecapData {
    build_recap(i).expect("builds")
}

#[test]
fn golden_files_match_byte_for_byte() {
    if updating() {
        std::fs::create_dir_all(dir()).expect("golden dir");
        // In order: years-2027 reads years-2026's fresh output.
        for name in CASES {
            let i = input(&spec(name));
            std::fs::write(dir().join(format!("{name}.input.json")), input_file(&i))
                .expect("write input");
            let parsed = read_input(name);
            assert_eq!(parsed, i, "{name}: the input file round-trips");
            std::fs::write(
                dir().join(format!("{name}.expected.json")),
                build(&i).to_json(),
            )
            .expect("write expected");
        }
    }
    for name in CASES {
        let got = build(&read_input(name)).to_json();
        let want = std::fs::read_to_string(dir().join(format!("{name}.expected.json")))
            .expect("expected file");
        assert!(
            got == want,
            "{name}: output differs from the golden file (UPDATE_GOLDEN=1 regenerates)"
        );
    }
}

fn beats(d: &RecapData) -> Vec<&str> {
    d.beats.iter().map(|b| b.id.as_str()).collect()
}

#[test]
fn a_very_new_profile_has_no_recap() {
    let d = build(&read_input("very-new"));
    assert_eq!(d.kind, Kind::None);
    assert!(d.beats.is_empty() && d.pack.is_none());
}

#[test]
fn a_light_profile_gets_the_short_three_cards() {
    let d = build(&read_input("light"));
    assert_eq!(d.kind, Kind::Short);
    assert_eq!(beats(&d), ["minutes", "topSong", "topArtist"]);
    assert!(d.totals.plays >= 10 && d.totals.plays < 100);
    assert!(d.detail.is_none());
}

#[test]
fn a_heavy_profile_gets_the_full_story() {
    let d = build(&read_input("heavy"));
    assert_eq!(d.kind, Kind::Full);
    let ids = beats(&d);
    assert!((10..=12).contains(&ids.len()), "{ids:?}");
    for fixed in [
        "intro",
        "minutes",
        "topSong",
        "topArtist",
        "topAlbum",
        "yourSound",
        "personality",
        "closer",
    ] {
        assert!(ids.contains(&fixed), "{fixed} missing from {ids:?}");
    }
    assert_eq!(ids.first(), Some(&"intro"));
    assert_eq!(ids.last(), Some(&"closer"));
    let rotating = d.beats.iter().filter(|b| !b.fixed).count();
    assert!((2..=4).contains(&rotating));
    assert!(d.beats.iter().map(|b| b.seconds).sum::<u32>() <= 90);
    assert!(
        d.beats.windows(2).all(|w| w[0].role <= w[1].role),
        "ordered by arc role"
    );
    assert_ne!(
        d.pack.as_ref().map(|p| p.id.as_str()),
        Some("tide"),
        "never last year's pack"
    );
    let gap = d
        .palette
        .as_ref()
        .and_then(|p| p.hue_gap_from_last_year)
        .expect("a gap");
    assert!(gap >= 60);
    assert!(d.year_line.is_some());
    // The minutes beat compares with last year.
    let minutes = d.beats.iter().find(|b| b.id == "minutes").expect("minutes");
    assert!(matches!(
        minutes.data,
        BeatData::Minutes {
            change_percent: Some(_),
            ..
        }
    ));
    // No household deep cut, ever.
    assert!(!ids.contains(&"householdDeepCut"));
}

#[test]
fn kid_profiles_skip_late_nights_and_explicit_songs() {
    let i = read_input("kid");
    let d = build(&i);
    let json = d.to_json();
    assert!(!json.contains("\"peakPart\":\"lateNight\""));
    assert!(!json.contains("10 pm"));
    assert!(!beats(&d).contains(&"listeningClock") || !json.contains("lateNight"));
    let explicit: Vec<&str> = i
        .plays
        .iter()
        .filter(|p| p.explicit)
        .map(|p| p.song_key.as_str())
        .collect();
    assert!(!explicit.is_empty(), "the fixture has explicit songs");
    for b in &d.beats {
        if let BeatData::TopSong { top, .. } = &b.data {
            assert!(top.iter().all(|s| !explicit.contains(&s.key.as_str())));
        }
    }
    let year_line = d.year_line.unwrap_or_default().to_ascii_lowercase();
    assert!(!year_line.contains("late") && !year_line.contains("after-dark"));
}

#[test]
fn a_cleared_history_counts_from_the_day_of_the_clear() {
    let d = build(&read_input("cleared"));
    let since = d.since.as_ref().expect("since");
    assert_eq!(since.day, "2026-08-03");
    let json = d.to_json();
    assert!(json.contains("\"reason\":\"cleared\""));
    if let Some(BeatData::Minutes { months, .. }) =
        d.beats.iter().find(|b| b.id == "minutes").map(|b| &b.data)
    {
        assert!(
            months.iter().take(7).all(|m| m.plays == 0),
            "nothing before August"
        );
    }
}

#[test]
fn without_fingerprints_the_mood_arc_is_left_out() {
    let d = build(&read_input("no-fingerprints"));
    assert_eq!(d.kind, Kind::Full);
    assert!(!beats(&d).contains(&"moodArc"));
    assert!(!beats(&d).contains(&"mostSungLine"));
}

#[test]
fn a_december_build_is_marked_so_far() {
    let d = build(&read_input("so-far"));
    assert!(d.so_far && !d.locked);
    let final_build = build(&read_input("heavy"));
    assert!(!final_build.so_far && !final_build.locked);
}

#[test]
fn consecutive_years_change_pack_and_lead_hue() {
    let a = build(&read_input("years-2026"));
    let b = build(&read_input("years-2027"));
    assert_ne!(
        a.pack.as_ref().expect("pack").id,
        b.pack.as_ref().expect("pack").id
    );
    let (ha, hb) = (
        a.palette.as_ref().expect("palette").lead_hue,
        b.palette.as_ref().expect("palette").lead_hue,
    );
    let gap = ha.abs_diff(hb) % 360;
    assert!(gap.min(360 - gap) >= 60, "{ha} vs {hb}");
    assert_ne!(a.seed, b.seed);
}

#[test]
fn every_golden_output_is_small() {
    for name in CASES {
        let d = build(&read_input(name));
        assert!(d.to_json().len() < 100 * 1024, "{name}");
    }
}
