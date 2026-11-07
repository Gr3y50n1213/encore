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

/// Autoplay must have at most this share of shuffle's early-skip rate.
const MAX_RATIO: f64 = 0.6;

struct Result {
    name: &'static str,
    plays: usize,
    library: usize,
    auto_early: f64,
    shuffle_early: f64,
    auto_finished: f64,
    shuffle_finished: f64,
}

fn replay(p: &sim::Profile) -> Result {
    let world = sim::World::new(p);
    let (history, end) = world.history(p);
    let params = Params::default();
    let cat = &world.catalog;
    let mut rng = Rng::new(p.seed ^ 0x5EED);
    let (mut ae, mut se, mut af, mut sf, mut n) = (0, 0, 0, 0, 0);
    for s in 0..120usize {
        let t = end + (s as i64 % 14) * DAY_MS + [8i64, 13, 19, 23][s % 4] * 3_600_000;
        let bucket = bucket_of(t, 0);
        let model = Model::build(&history, cat, &params, t, 0);
        let seed = world.user_pick(&mut rng, bucket);
        let ctx = Context {
            seeds: vec![cat.get(seed).key.clone()],
            exploration_percent: 10,
            seed: s as u64,
            ..Context::default()
        };
        let picks = autoplay(&model, cat, &ctx, 10, &params);
        let mut shuffled: Vec<usize> = (0..cat.len()).filter(|&i| i != seed).collect();
        rng.shuffle(&mut shuffled);
        for (k, pick) in picks.iter().enumerate() {
            let (u1, u2) = (rng.unit(), rng.unit());
            let a = cat.find(&pick.song_key).unwrap_or(0);
            let oa = world.outcome(a, bucket, true, u1, u2);
            let os = world.outcome(shuffled[k], bucket, true, u1, u2);
            ae += usize::from(oa == Outcome::SkippedEarly);
            se += usize::from(os == Outcome::SkippedEarly);
            af += usize::from(oa.enjoyed());
            sf += usize::from(os.enjoyed());
            n += 1;
        }
    }
    let rate = |x: usize| x as f64 / n.max(1) as f64;
    Result {
        name: p.name,
        plays: history.plays.len(),
        library: cat.len(),
        auto_early: rate(ae),
        shuffle_early: rate(se),
        auto_finished: rate(af),
        shuffle_finished: rate(sf),
    }
}

struct Budgets {
    songs: usize,
    plays: usize,
    build: Duration,
    mixes: Duration,
    memory_mb: f64,
    next_song: Duration,
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
    Budgets {
        songs: world.catalog.len(),
        plays: history.plays.len(),
        build,
        mixes,
        memory_mb,
        next_song: times[3],
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
         Made by `tools/recs-replay` (docs/adr/core-0014). Each listener is generated with hidden tastes; the engine sees only their history. In 120 sessions after it, the listener picks a song and then autoplay picks 10, or plain shuffle picks 10, scored on the same random draws. The check: autoplay's early-skip rate is at most 60% of shuffle's on every listener.\n\n\
         | Listener | Library | Plays | Early skips: autoplay | Early skips: shuffle | Autoplay / shuffle | Finished: autoplay | Finished: shuffle |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for r in &results {
        let ratio = r.auto_early / r.shuffle_early.max(1e-9);
        if ratio > MAX_RATIO {
            failures.push(format!(
                "{}: autoplay {} vs shuffle {} early skips",
                r.name,
                pct(r.auto_early),
                pct(r.shuffle_early)
            ));
        }
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.2} | {} | {} |\n",
            r.name,
            r.library,
            r.plays,
            pct(r.auto_early),
            pct(r.shuffle_early),
            ratio,
            pct(r.auto_finished),
            pct(r.shuffle_finished)
        ));
    }
    let ok = |d: bool| if d { "within" } else { "OVER" };
    let within_build = b.build + b.mixes < Duration::from_secs(20);
    let within_memory = b.memory_mb < 150.0;
    let within_next = b.next_song < Duration::from_millis(50);
    for (good, what) in [
        (within_build, "the mix build"),
        (within_memory, "the mix build's memory"),
        (within_next, "the next-song pick"),
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
         - the next-song pick (autoplay, 10 songs, median of 7): {:.1} ms, {} the 50 ms budget.\n",
        b.songs,
        b.plays,
        b.build.as_secs_f64(),
        b.mixes.as_secs_f64(),
        ok(within_build),
        b.memory_mb,
        ok(within_memory),
        b.next_song.as_secs_f64() * 1e3,
        ok(within_next)
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
