//! The replay on the owner's own history (Phase 5's "done when": "autoplay's
//! early-skip rate is clearly lower than shuffle on your own history"), from the
//! private file `fixture-recorder --phase5` wrote. Runs on the owner's PC only; the
//! numbers it prints are what goes into the Phase 5 review.
//!
//! Two checks, depending on what the server had (docs/adr/core-0014):
//! - With the Playback Reporting log (each play's duration): train on the first
//!   80% of plays, then in each later session seed autoplay with its first song.
//!   Of the songs autoplay would have queued that the owner actually played in
//!   that later period, how many were skipped in the first 30 s, against the same
//!   for shuffle's picks. And a ranking check: does the engine score the songs
//!   the owner finished above the songs they skipped early (0.5 is chance)?
//! - With Jellyfin's play counts only: hold out the most recently played fifth
//!   of the played songs; seeded with one of them, how many of autoplay's picks
//!   are among the others, against shuffle (a hit rate, weaker: no skips).

use encore_recs::det::Rng;
use encore_recs::{
    Catalog, Context, History, Model, Outcome, Params, Play, Song, autoplay, score_of,
};
use serde_json::Value;

/// "2026-10-01 20:00:00.0000000" as milliseconds (taken as UTC: the plugin writes
/// the server's local time, and only order and time of day matter here).
fn parse_time(s: &str) -> Option<i64> {
    let (date, time) = s.split_once(' ')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>());
    let (y, m, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let mut t = time.split(':');
    let (h, min) = (
        t.next()?.parse::<i64>().ok()?,
        t.next()?.parse::<i64>().ok()?,
    );
    let sec: f64 = t.next()?.parse().ok()?;
    // Days from 1970-01-01 (proleptic Gregorian).
    let (yy, mm) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * mm + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400_000 + h * 3_600_000 + min * 60_000 + (sec * 1000.0) as i64)
}

pub fn run(path: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let songs: Vec<Song> = serde_json::from_value(v["songs"].clone()).map_err(|e| e.to_string())?;
    let catalog = Catalog::new(songs);
    let params = Params::default();
    let mut out = format!("Library: {} songs.\n", catalog.len());
    let mut plays: Vec<Play> = v["pluginPlays"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let key = p["key"].as_str()?;
            let i = catalog.find(key)?;
            let at = parse_time(p["at"].as_str()?)?;
            let ms = (p["seconds"].as_f64()? * 1000.0) as u64;
            let dur = catalog.get(i).duration_ms.max(1);
            let outcome = if ms * 10 >= dur * 8 {
                Outcome::Finished
            } else if ms < params.early_skip_ms {
                Outcome::SkippedUserPick
            } else {
                Outcome::SkippedMid
            };
            Some(Play {
                song_key: key.to_owned(),
                started_utc_ms: at,
                utc_offset_minutes: 0,
                listened_ms: ms,
                outcome: Some(outcome),
                app_chosen: false,
                strong_intent: false,
            })
        })
        .collect();
    plays.sort_by_key(|p| p.started_utc_ms);
    if plays.len() >= 200 {
        out.push_str(&with_plays(&catalog, &params, &plays));
    } else {
        out.push_str(&format!(
            "Playback Reporting: {} plays, too few for the skip check (200 needed); using play counts.\n",
            plays.len()
        ));
        out.push_str(&with_counts(&catalog, &params));
    }
    Ok(out)
}

fn early(o: Option<Outcome>) -> bool {
    matches!(o, Some(Outcome::SkippedEarly | Outcome::SkippedUserPick))
}

fn with_plays(catalog: &Catalog, params: &Params, plays: &[Play]) -> String {
    let split = plays.len() * 8 / 10;
    let (train, test) = plays.split_at(split);
    let history = History {
        plays: train.to_vec(),
        ..History::default()
    };
    // Each song's outcomes in the test period.
    let mut known: std::collections::HashMap<&str, (u32, u32)> = Default::default();
    for p in test {
        let e = known.entry(p.song_key.as_str()).or_default();
        e.0 += 1;
        if early(p.outcome) {
            e.1 += 1;
        }
    }
    let mut sessions: Vec<Vec<&Play>> = Vec::new();
    for p in test {
        match sessions.last_mut() {
            Some(s)
                if p.started_utc_ms - s.last().map_or(0, |l| l.started_utc_ms) <= 30 * 60_000 =>
            {
                s.push(p)
            }
            _ => sessions.push(vec![p]),
        }
    }
    let mut rng = Rng::new(5);
    let (mut a_known, mut a_early, mut s_known, mut s_early) = (0u32, 0u32, 0u32, 0u32);
    let (mut fin, mut skip) = (Vec::new(), Vec::new());
    for (x, s) in sessions.iter().enumerate().filter(|(_, s)| s.len() >= 2) {
        let t = s[0].started_utc_ms;
        let model = Model::build(&history, catalog, params, t, 0);
        let ctx = Context {
            seeds: vec![s[0].song_key.clone()],
            exploration_percent: 10,
            seed: x as u64,
            ..Context::default()
        };
        for pick in autoplay(&model, catalog, &ctx, 10, params) {
            if let Some((n, e)) = known.get(pick.song_key.as_str()) {
                a_known += n;
                a_early += e;
            }
        }
        for _ in 0..10 {
            let k = &catalog.get(rng.below(catalog.len())).key;
            if let Some((n, e)) = known.get(k.as_str()) {
                s_known += n;
                s_early += e;
            }
        }
        for w in s.windows(2) {
            let c = Context {
                seeds: vec![w[0].song_key.clone()],
                ..ctx.clone()
            };
            if let Some(score) = score_of(&model, catalog, &c, &w[1].song_key) {
                match w[1].outcome {
                    Some(o) if o.enjoyed() => fin.push(score),
                    o if early(o) => skip.push(score),
                    _ => {}
                }
            }
        }
    }
    let rate = |e: u32, n: u32| {
        if n == 0 {
            f64::NAN
        } else {
            f64::from(e) / f64::from(n)
        }
    };
    // AUC: the chance a finished song outscores an early-skipped one.
    let mut wins = 0.0;
    for f in &fin {
        for s in &skip {
            wins += if f > s {
                1.0
            } else if f == s {
                0.5
            } else {
                0.0
            };
        }
    }
    let auc = wins / (fin.len() * skip.len()).max(1) as f64;
    format!(
        "Your history (Playback Reporting): {} plays; trained on the first {split}, tested on {} later sessions.\n\
         Early skips among songs autoplay would have queued and you played: {:.1}% (of {a_known} plays).\n\
         Early skips among shuffle's picks you played: {:.1}% (of {s_known} plays).\n\
         Ranking check: finished songs scored above early skips {:.2} of the time ({} finished, {} skipped; 0.50 is chance).\n",
        plays.len(),
        sessions.iter().filter(|s| s.len() >= 2).count(),
        rate(a_early, a_known) * 100.0,
        rate(s_early, s_known) * 100.0,
        auc,
        fin.len(),
        skip.len()
    )
}

fn with_counts(catalog: &Catalog, params: &Params) -> String {
    let mut played: Vec<(i64, usize)> = (0..catalog.len())
        .filter(|&i| catalog.get(i).server_play_count > 0)
        .map(|i| (catalog.get(i).server_last_played_utc_ms.unwrap_or(0), i))
        .collect();
    if played.len() < 50 {
        return format!(
            "Play counts: {} songs played in other apps, too few for a check (50 needed).\n",
            played.len()
        );
    }
    played.sort();
    let held: Vec<usize> = played
        .iter()
        .rev()
        .take(played.len() / 5)
        .map(|x| x.1)
        .collect();
    let held_keys: std::collections::HashSet<String> =
        held.iter().map(|&i| catalog.get(i).key.clone()).collect();
    // Train without the held-out songs' counts.
    let songs: Vec<Song> = catalog
        .songs()
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if held_keys.contains(&s.key) {
                s.server_play_count = 0;
                s.server_last_played_utc_ms = None;
            }
            s
        })
        .collect();
    let train = Catalog::new(songs);
    let now = played.last().map_or(0, |x| x.0) + 86_400_000;
    let model = Model::build(&History::default(), &train, params, now, 0);
    let (mut hits, mut picks) = (0usize, 0usize);
    // The fair half: among picks the model sees as never played (the held-out
    // songs and the ones truly never played), how many are held out. Counts
    // can't help here; only the seed's artist, genre and era can.
    let (mut cold_hits, mut cold_picks) = (0usize, 0usize);
    for (x, &h) in held.iter().take(100).enumerate() {
        let seed = &catalog.get(h).key;
        let ctx = Context {
            seeds: vec![seed.clone()],
            exploration_percent: 10,
            seed: x as u64,
            ..Context::default()
        };
        for p in autoplay(&model, &train, &ctx, 10, params) {
            picks += 1;
            let is_held = held_keys.contains(&p.song_key) && &p.song_key != seed;
            if is_held {
                hits += 1;
            }
            if train
                .find(&p.song_key)
                .is_some_and(|i| train.get(i).server_play_count == 0)
            {
                cold_picks += 1;
                if is_held {
                    cold_hits += 1;
                }
            }
        }
    }
    let base = (held.len().saturating_sub(1)) as f64 / catalog.len() as f64;
    let hit = hits as f64 / picks.max(1) as f64;
    let cold_pool = (0..train.len())
        .filter(|&i| train.get(i).server_play_count == 0)
        .count();
    let cold_base = held.len().saturating_sub(1) as f64 / cold_pool.saturating_sub(1).max(1) as f64;
    let cold_hit = cold_hits as f64 / cold_picks.max(1) as f64;
    format!(
        "Your history (play counts only): {} songs played elsewhere; held out the {} most recent.\n\
         Autoplay's picks that were among them: {:.1}%; shuffle would pick them {:.1}% of the time ({:.1}x).\n\
         (Biased against the engine: hiding their counts makes them look unplayed, and it prefers songs it sees played.)\n\
         Fair half: of autoplay's {cold_picks} picks among songs it sees as unplayed ({cold_pool}), {:.1}% were held out; shuffle among them {:.1}% ({:.1}x).\n\
         This is a hit rate, not a skip rate: Jellyfin keeps counts, not skips.\n",
        played.len(),
        held.len(),
        hit * 100.0,
        base * 100.0,
        hit / base.max(1e-9),
        cold_hit * 100.0,
        cold_base * 100.0,
        cold_hit / cold_base.max(1e-9)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The skip check, end to end, on a generated listener written in the private
    /// file's shape (the owner's real file stays on the owner's PC).
    #[test]
    fn the_skip_check_runs_on_a_plugin_shaped_history() {
        let p = &crate::sim::PROFILES[2];
        let world = crate::sim::World::new(p);
        let (history, _) = world.history(p);
        let songs = serde_json::to_value(world.catalog.songs()).unwrap();
        let plays: Vec<Value> = history
            .plays
            .iter()
            .map(|x| {
                let secs = x.listened_ms as f64 / 1000.0;
                let ms = x.started_utc_ms;
                let day = ms.div_euclid(86_400_000);
                let rem = ms.rem_euclid(86_400_000);
                // Back to "YYYY-MM-DD HH:MM:SS" (civil from days).
                let z = day + 719_468;
                let era = z.div_euclid(146_097);
                let doe = z - era * 146_097;
                let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
                let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
                let mp = (5 * doy + 2) / 153;
                let d = doy - (153 * mp + 2) / 5 + 1;
                let m = if mp < 10 { mp + 3 } else { mp - 9 };
                let y = yoe + era * 400 + i64::from(m <= 2);
                let at = format!(
                    "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.0000000",
                    rem / 3_600_000,
                    rem / 60_000 % 60,
                    rem / 1000 % 60
                );
                serde_json::json!({ "key": x.song_key, "at": at, "seconds": secs })
            })
            .collect();
        let dir = std::env::temp_dir().join(format!("recs-private-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("history.json");
        std::fs::write(
            &file,
            serde_json::json!({ "songs": songs, "pluginPlays": plays, "hasPlugin": true })
                .to_string(),
        )
        .unwrap();
        let report = run(file.to_str().unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            report.contains("Early skips among songs autoplay would have queued"),
            "{report}"
        );
        assert!(report.contains("Ranking check"), "{report}");
        println!("{report}");
    }

    #[test]
    fn the_plugins_times_parse() {
        assert_eq!(parse_time("1970-01-01 00:00:00.0000000"), Some(0));
        assert_eq!(
            parse_time("2026-11-07 06:30:15.5000000"),
            Some(1_794_033_015_500)
        );
        assert_eq!(parse_time("nonsense"), None);
    }
}
