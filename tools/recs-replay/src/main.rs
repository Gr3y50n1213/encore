//! Offline replay of the smart-picks engine (Spike 7; plan section 24, Smart
//! picks: "offline replay on at least three test histories (light, heavy, one
//! with many skips) beats plain shuffle on skip rate of app-chosen songs, with the
//! numbers saved in CI"; docs/adr/core-0014).
//!
//!   cargo run --release -p recs-replay -- [--out docs/core/REPLAY-RESULTS.md] [--check]
//!   cargo run --release -p recs-replay -- --private target/private-history/history.json
//!
//! For each generated listener: after their history, in 120 listening sessions
//! the listener picks a song, and then autoplay picks 10, or plain shuffle picks
//! 10; each pick's outcome comes from the listener's hidden tastes, with the same
//! random draws for both, so they are compared on the same luck. `--check` fails
//! unless autoplay's early-skip rate is at most 60% of shuffle's on every
//! listener, and the budgets hold (a 20,000-song library and 20,000 plays: model
//! and five mixes under 20 s and 150 MB; the next-song pick under 50 ms).

mod private;
mod sim;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use encore_recs::det::Rng;
use encore_recs::{
    Context, DAY_MS, Model, Outcome, Params, Previous, autoplay, bucket_of, build_mixes,
};

/// Counts the bytes allocated, for the memory budget.
struct Counting;
static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator unchanged; only adds counting.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let n = NOW.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        NOW.fetch_sub(l.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Listening sessions after the history, per listener. 120 gave 1,200 picks and an error of about
/// a point on a 15% rate, too coarse to compare two versions; 480 brings it near half a point.
const SESSIONS: usize = 480;

/// Version 1's autoplay must have at most this share of shuffle's early-skip rate.
const MAX_RATIO: f64 = 0.6;
/// Version 2 may be at most this much worse than version 1 on any one listener (about three standard
/// errors of the rate over this many picks).
const MAX_WORSE: f64 = 0.02;

struct Result {
    name: &'static str,
    plays: usize,
    library: usize,
    /// Version 1: what the engine does without knowing what songs sound like.
    v1_early: f64,
    /// Version 2: the same engine given every song's sound (Phase 10).
    v2_early: f64,
    /// Version 2's sound without the learned ranker (to tell what each part adds).
    v2s_early: f64,
    shuffle_early: f64,
    v1_finished: f64,
    v2_finished: f64,
    shuffle_finished: f64,
    /// The learned ranker, when it passed its gate: examples trained on, its AUC and the hand-tuned score's on held-back plays.
    ranker: Option<(u32, f32, f32)>,
    /// Labelled plays available to train on.
    examples: usize,
}

fn replay(p: &sim::Profile) -> Result {
    let world = sim::World::new(p);
    let (history, end) = world.history(p);
    let params = Params::default();
    // The same songs in the same order, with and without what they sound like.
    let cat = &world.catalog;
    let heard = world.catalog_with_sound();
    // Version 2 is the engine with sound, and the learned ranker when this listener's history earns one.
    let examples = encore_recs::learn::examples(&history, &heard, &params, end, 0);
    let ranker = encore_recs::learn::judge(&examples).map(std::sync::Arc::new);
    let mut rng = Rng::new(p.seed ^ 0x5EED);
    let (mut e1, mut e2, mut es, mut e3) = (0, 0, 0, 0);
    let (mut f1, mut f2, mut fs, mut n) = (0, 0, 0, 0);
    for s in 0..SESSIONS {
        let t = end + (s as i64 % 14) * DAY_MS + [8i64, 13, 19, 23][s % 4] * 3_600_000;
        let bucket = bucket_of(t, 0);
        let model1 = Model::build(&history, cat, &params, t, 0);
        let model3 = Model::build(&history, &heard, &params, t, 0);
        let model2 = model3.clone().with_ranker(ranker.clone());
        let seed = world.user_pick(&mut rng, bucket);
        let ctx = Context {
            seeds: vec![cat.get(seed).key.clone()],
            exploration_percent: 10,
            seed: s as u64,
            ..Context::default()
        };
        let picks1 = autoplay(&model1, cat, &ctx, 10, &params);
        let picks2 = autoplay(&model2, &heard, &ctx, 10, &params);
        let picks3 = autoplay(&model3, &heard, &ctx, 10, &params);
        let mut shuffled: Vec<usize> = (0..cat.len()).filter(|&i| i != seed).collect();
        rng.shuffle(&mut shuffled);
        for k in 0..picks1.len().min(picks2.len()) {
            // The same luck for all three.
            let (u1, u2) = (rng.unit(), rng.unit());
            let a = cat.find(&picks1[k].song_key).unwrap_or(0);
            let b = cat.find(&picks2[k].song_key).unwrap_or(0);
            let (o1, o2) = (
                world.outcome(a, bucket, true, u1, u2),
                world.outcome(b, bucket, true, u1, u2),
            );
            let os = world.outcome(shuffled[k], bucket, true, u1, u2);
            let c = cat
                .find(&picks3[k.min(picks3.len() - 1)].song_key)
                .unwrap_or(0);
            e3 += usize::from(world.outcome(c, bucket, true, u1, u2) == Outcome::SkippedEarly);
            e1 += usize::from(o1 == Outcome::SkippedEarly);
            e2 += usize::from(o2 == Outcome::SkippedEarly);
            es += usize::from(os == Outcome::SkippedEarly);
            f1 += usize::from(o1.enjoyed());
            f2 += usize::from(o2.enjoyed());
            fs += usize::from(os.enjoyed());
            n += 1;
        }
    }
    let rate = |x: usize| x as f64 / n.max(1) as f64;
    Result {
        name: p.name,
        plays: history.plays.len(),
        library: cat.len(),
        v1_early: rate(e1),
        v2_early: rate(e2),
        v2s_early: rate(e3),
        shuffle_early: rate(es),
        v1_finished: rate(f1),
        v2_finished: rate(f2),
        shuffle_finished: rate(fs),
        ranker: ranker
            .as_ref()
            .map(|r| (r.examples, r.holdout_auc, r.hand_auc)),
        examples: examples.len(),
    }
}

struct Budgets {
    songs: usize,
    plays: usize,
    build: Duration,
    mixes: Duration,
    memory_mb: f64,
    next_song: Duration,
    /// The same library with every song analysed (Phase 10): the model's build, its extra memory, the next-song pick.
    sound_build: Duration,
    sound_memory_mb: f64,
    sound_next_song: Duration,
}

fn budgets() -> Budgets {
    let p = sim::Profile {
        name: "budget",
        artists: 667,
        days: 360,
        sessions_per_day: 4,
        songs_per_session: 14,
        base: 0.0,
        seed: 99,
        tag_noise: 0.0,
    };
    let world = sim::World::new(&p);
    let (history, end) = world.history(&p);
    let params = Params::default();
    let before = NOW.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let t = Instant::now();
    let model = Model::build(&history, &world.catalog, &params, end, 0);
    let build = t.elapsed();
    let t = Instant::now();
    let set = build_mixes(&model, &world.catalog, &params, &Previous::default());
    let mixes = t.elapsed();
    let memory_mb = (PEAK.load(Ordering::Relaxed) - before) as f64 / 1_048_576.0;
    assert!(set.mixes.len() >= 2, "the budget library makes mixes");
    let ctx = Context {
        seeds: vec![world.catalog.get(0).key.clone()],
        exploration_percent: 10,
        seed: 1,
        ..Context::default()
    };
    let mut times: Vec<Duration> = (0..7)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(autoplay(&model, &world.catalog, &ctx, 10, &params));
            t.elapsed()
        })
        .collect();
    times.sort();
    // And with every song's sound.
    let heard = world.catalog_with_sound();
    let before = NOW.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let t = Instant::now();
    let model2 = Model::build(&history, &heard, &params, end, 0);
    let sound_build = t.elapsed();
    let sound_memory_mb = (PEAK.load(Ordering::Relaxed) - before) as f64 / 1_048_576.0;
    let mut times2: Vec<Duration> = (0..7)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(autoplay(&model2, &heard, &ctx, 10, &params));
            t.elapsed()
        })
        .collect();
    times2.sort();
    Budgets {
        songs: world.catalog.len(),
        plays: history.plays.len(),
        build,
        mixes,
        memory_mb,
        next_song: times[3],
        sound_build,
        sound_memory_mb,
        sound_next_song: times2[3],
    }
}

fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The owner's own history, on the owner's PC only (fixture-recorder --phase5).
    if let Some(i) = args.iter().position(|a| a == "--private") {
        let path = args
            .get(i + 1)
            .map_or("../target/private-history/history.json", String::as_str);
        return match private::run(path) {
            Ok(report) => {
                print!("{report}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    let check = args.iter().any(|a| a == "--check");
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1).cloned());
    let results: Vec<Result> = sim::PROFILES.iter().map(replay).collect();
    let b = budgets();
    let mut failures = Vec::new();
    let mut md = String::from(
        "# Smart picks: offline replay results\n\n\
         Made by `tools/recs-replay` (docs/adr/core-0014 and core-0020). Each listener is generated with hidden tastes: genres, favourite and disliked artists, a time of day, and (Phase 10) a taste for a direction in a hidden sound space that no tag says. The listeners with \"messy tags\" have half their artists mis-tagged, as a real library's tags often are. The engine sees only their history. In 480 sessions after it, the listener picks a song and then autoplay picks 10 with the version 1 engine (it does not know what songs sound like), with the version 2 engine (the same engine given every song's sound, and the learned ranker when the listener's history earns one), or plain shuffle picks 10, all scored on the same random draws.\n\n\
         **Checks.** Version 1's early-skip rate is at most 60% of shuffle's on every listener. Version 2 beats version 1 overall: a lower mean early-skip rate across the listeners, lower on at least four of the six, and never more than two points worse on any.\n\n\
         **What this proves and what it does not.** The listeners and the songs' sounds are generated, and the generator gives sound a real part in taste, so this shows that the engine uses sound when it is there and that the wiring works, not how much it will help on a real library. Differences under about a point are noise (about 4,800 picks per listener). Where tags are perfect, sound adds little or costs a point; where they are messy it helps a good deal. The learned ranker never switches on here: the generated history is plain shuffle, so it holds none of the engine's own picks (what the ranker learns from in real use), and its gate (it must beat the hand-tuned score on the person's newest plays) refuses it. Its learning and its gate are tested on their own (`encore-recs` `learn` tests). The real measure is the person's own How suggestions are doing page after the library is analysed.\n\n\
         | Listener | Library | Plays | Early skips: v1 | Early skips: v2 | v2 without the ranker | Early skips: shuffle | v1 / shuffle | v2 / v1 | Finished: v1 | Finished: v2 | Finished: shuffle | Learned ranker |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for r in &results {
        let ratio = r.v1_early / r.shuffle_early.max(1e-9);
        if ratio > MAX_RATIO {
            failures.push(format!(
                "{}: version 1 {} vs shuffle {} early skips",
                r.name,
                pct(r.v1_early),
                pct(r.shuffle_early)
            ));
        }
        if r.v2_early > r.v1_early + MAX_WORSE {
            failures.push(format!(
                "{}: version 2 {} is more than two points worse than version 1 {} on early skips",
                r.name,
                pct(r.v2_early),
                pct(r.v1_early)
            ));
        }
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {:.2} | {:.2} | {} | {} | {} | {} |\n",
            r.name,
            r.library,
            r.plays,
            pct(r.v1_early),
            pct(r.v2_early),
            pct(r.v2s_early),
            pct(r.shuffle_early),
            ratio,
            r.v2_early / r.v1_early.max(1e-9),
            pct(r.v1_finished),
            pct(r.v2_finished),
            pct(r.shuffle_finished),
            match r.ranker {
                Some((n, learned, hand)) => format!("on: {n} plays, AUC {learned:.3} against the hand-tuned {hand:.3} on held-back plays"),
                None => format!("off ({} labelled plays: too few, or no better than the hand-tuned score)", r.examples),
            }
        ));
    }
    // Version 2 against version 1 overall.
    let count = results.len().max(1) as f64;
    let mean_v1 = results.iter().map(|r| r.v1_early).sum::<f64>() / count;
    let mean_v2 = results.iter().map(|r| r.v2_early).sum::<f64>() / count;
    let better = results.iter().filter(|r| r.v2_early < r.v1_early).count();
    md.push_str(&format!(
        "\nAcross the listeners: version 1 {}, version 2 {} (mean early-skip rate), version 2 lower on {} of {}.\n",
        pct(mean_v1),
        pct(mean_v2),
        better,
        results.len()
    ));
    if mean_v2 >= mean_v1 {
        failures.push(format!(
            "version 2 ({}) does not beat version 1 ({}) on the mean early-skip rate",
            pct(mean_v2),
            pct(mean_v1)
        ));
    }
    if results.len() >= 6 && better < 4 {
        failures.push(format!(
            "version 2 beats version 1 on only {better} of {} listeners",
            results.len()
        ));
    }
    let ok = |d: bool| if d { "within" } else { "OVER" };
    let within_build = b.build + b.mixes < Duration::from_secs(20);
    let within_memory = b.memory_mb < 150.0;
    let within_next = b.next_song < Duration::from_millis(50);
    let within_sound_build = b.sound_build < Duration::from_secs(20);
    let within_sound_memory = b.sound_memory_mb < 150.0;
    let within_sound_next = b.sound_next_song < Duration::from_millis(50);
    for (good, what) in [
        (within_build, "the mix build"),
        (within_memory, "the mix build's memory"),
        (within_next, "the next-song pick"),
        (within_sound_build, "the model build with sound"),
        (within_sound_memory, "the model build's memory with sound"),
        (within_sound_next, "the next-song pick with sound"),
    ] {
        if !good {
            failures.push(format!("{what} is over its budget"));
        }
    }
    md.push_str(&format!(
        "\n## Budgets (smartPicks.budgets; CI is not a phone)\n\n\
         On {} songs and {} plays, this run:\n\n\
         - model and five mixes: {:.2} s + {:.2} s, {} the 20 s budget;\n\
         - memory at the peak of the build: {:.1} MB, {} the 150 MB budget;\n\
         - the next-song pick (autoplay, 10 songs, median of 7): {:.1} ms, {} the 50 ms budget.\n\n\
         The same library with every song analysed (Phase 10):\n\n\
         - model build: {:.2} s, {} the 20 s budget;\n\
         - memory at the peak of the build: {:.1} MB, {} the 150 MB budget;\n\
         - the next-song pick: {:.1} ms, {} the 50 ms budget.\n",
        b.songs,
        b.plays,
        b.build.as_secs_f64(),
        b.mixes.as_secs_f64(),
        ok(within_build),
        b.memory_mb,
        ok(within_memory),
        b.next_song.as_secs_f64() * 1e3,
        ok(within_next),
        b.sound_build.as_secs_f64(),
        ok(within_sound_build),
        b.sound_memory_mb,
        ok(within_sound_memory),
        b.sound_next_song.as_secs_f64() * 1e3,
        ok(within_sound_next)
    ));
    print!("{md}");
    if let Some(path) = out
        && let Err(e) = std::fs::write(&path, &md)
    {
        eprintln!("couldn't write {path}: {e}");
        return std::process::ExitCode::FAILURE;
    }
    if check && !failures.is_empty() {
        for f in &failures {
            eprintln!("FAILED: {f}");
        }
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
