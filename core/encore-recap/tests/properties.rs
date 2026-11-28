//! Property-style tests over many generated inputs (a small PRNG, no extra
//! crates): the same input always gives the same bytes, input order never matters,
//! the next year never repeats the pack, and the document stays small.

mod common;

use std::time::Instant;

use common::{Rng, Spec, input};
use encore_recap::{Kind, PastRecap, RecapInput, build_recap, build_recap_json, daily_rollups};

fn random_spec(r: &mut Rng, case: u64) -> Spec {
    let year = 2024 + r.below(8) as i32;
    let plays = [5, 30, 150, 400, 900][r.below(5) as usize];
    let mut s = Spec::new(r.next(), &format!("profile-{case:04}"), year, plays);
    s.artists = 2 + r.below(40) as usize;
    s.night = r.below(700);
    s.kid = r.chance(200);
    s.genres = r.chance(700);
    s.fingerprints = r.chance(600);
    s.lyric_jumps = r.chance(500);
    s.earlier_plays = [0, 50, 300][r.below(3) as usize];
    s.first_doy = 1 + r.below(200) as i64;
    s.last_doy = s.first_doy + r.below(165) as i64;
    if r.chance(150) {
        s.cleared_since = Some(format!("{year}-0{}-1{}", 3 + r.below(6), r.below(9)));
    }
    let all = [
        "editorial",
        "tide",
        "bloom",
        "glass",
        "contour",
        "aura",
        "neonGrid",
    ];
    s.packs = all[..2 + r.below(6) as usize].to_vec();
    s
}

fn json(i: &RecapInput) -> String {
    build_recap(i).expect("builds").to_json()
}

#[test]
fn the_same_input_gives_the_same_bytes_in_any_order() {
    let mut r = Rng(0x5EED_0001);
    for case in 0..60 {
        let s = random_spec(&mut r, case);
        let i = input(&s);
        let a = json(&i);
        assert_eq!(a, json(&i), "case {case}: two builds differ");
        let mut shuffled = i.clone();
        let mut r2 = Rng(case);
        r2.shuffle(&mut shuffled.plays);
        shuffled.packs.reverse();
        assert_eq!(
            a,
            json(&shuffled),
            "case {case}: input order changed the output"
        );
        // Through JSON, as a binding would call it.
        let via =
            build_recap_json(&serde_json::to_string(&i).expect("serializes")).expect("builds");
        assert_eq!(a, via, "case {case}: the JSON entry point differs");
        let d = build_recap(&i).expect("builds");
        assert!(d.beats.len() <= 12);
        assert!(d.beats.iter().map(|b| b.seconds).sum::<u32>() <= 90);
        match d.kind {
            Kind::Short => assert_eq!(d.beats.len(), 3),
            Kind::None => assert!(d.beats.is_empty()),
            Kind::Full => assert!(d.beats.len() >= 8),
        }
        for b in &d.beats {
            assert!(b.layout < b.layout_count && (3..=4).contains(&b.layout_count));
        }
        assert!(a.len() < 100 * 1024);
    }
}

#[test]
fn the_next_year_never_repeats_the_pack() {
    let mut r = Rng(0x5EED_0002);
    let mut checked = 0;
    for case in 0..60 {
        let mut s = random_spec(&mut r, case);
        s.plays = 400;
        s.first_doy = 1;
        s.last_doy = 365;
        s.cleared_since = None;
        let a = build_recap(&input(&s)).expect("builds");
        let Some(pack) = a.pack.clone() else { continue };
        let mut next = s.clone();
        next.year += 1;
        next.seed ^= 0xABCD;
        next.past = vec![PastRecap {
            year: s.year,
            pack: Some(pack.id.clone()),
            lead_hue: a.palette.as_ref().map(|p| p.lead_hue),
            listened_ms: Some(a.totals.listened_ms),
            personality_metric: None,
            personality_value: None,
        }];
        let b = build_recap(&input(&next)).expect("builds");
        let bp = b.pack.expect("a pack");
        assert_ne!(bp.id, pack.id, "case {case}: same pack two years running");
        let gap = b.palette.expect("palette").hue_gap_from_last_year;
        // A gap below 60 only when no album colour sits 60 degrees away.
        assert!(gap.is_some());
        checked += 1;
    }
    assert!(checked > 30, "too few cases had a recap: {checked}");
}

#[test]
fn rollups_are_rebuildable_and_order_free() {
    let mut s = Spec::new(77, "profile-rollups", 2026, 500);
    s.earlier_plays = 50;
    let i = input(&s);
    let a = daily_rollups(&i.plays);
    let mut shuffled = i.plays.clone();
    Rng(5).shuffle(&mut shuffled);
    assert_eq!(a, daily_rollups(&shuffled));
    let total: u64 = a.iter().map(|d| d.total.plays).sum();
    let counted = i
        .plays
        .iter()
        .filter(|p| p.listened_ms >= 2_000 && p.end_reason.as_deref() != Some("interrupted"))
        .count() as u64;
    assert_eq!(total, counted);
    assert!(a.windows(2).all(|w| w[0].day < w[1].day));
    for d in &a {
        assert_eq!(
            d.total.plays,
            d.songs.values().map(|t| t.plays).sum::<u64>()
        );
        assert_eq!(
            d.total.plays,
            d.hours.values().map(|t| t.plays).sum::<u64>()
        );
    }
}

fn hundred_thousand(long_names: bool) -> RecapInput {
    let mut s = Spec::new(99, "profile-huge", 2026, 100_000);
    s.artists = 5_000;
    s.earlier_plays = 0;
    s.long_names = long_names;
    input(&s)
}

#[test]
fn a_hundred_thousand_plays_stay_under_100_kb() {
    let i = hundred_thousand(true);
    let out = json(&i);
    assert!(out.len() < 100 * 1024, "{} bytes", out.len());
}

/// The build time over 100,000 plays. Run with
/// `cargo test -p encore-recap -- --ignored timing --nocapture`.
#[test]
#[ignore]
fn timing_100k_plays() {
    let i = hundred_thousand(false);
    let start = Instant::now();
    let out = json(&i);
    let took = start.elapsed();
    println!(
        "recap over {} plays: {} ms, {} bytes",
        i.plays.len(),
        took.as_millis(),
        out.len()
    );
}
