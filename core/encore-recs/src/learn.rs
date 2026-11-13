//! The learned ranker (Phase 10, docs/adr/core-0020): a small gradient-boosted tree model that
//! learns, from one person's own history, which of the engine's signals to trust for them.
//!
//! The hand-tuned score weighs liking, fit with the session, time of day, freshness and recent
//! skips the same for everyone. A person whose library has no genre tags, or whose taste is a
//! sound no tag names, is better served by weights that follow their own plays. The ranker takes
//! the same signals (plus how a song sounds against their taste, and how smooth the move from the
//! song before is) and predicts "will it be played past 30 seconds and finished".
//!
//! Three guards keep it from ever doing worse than the hand-tuned score (docs/adr/core-0020):
//! - the training examples are made the way picks are made: for each play, the signals come from a
//!   model built only from what happened *before* it, so nothing the play itself taught leaks in;
//! - it only switches on when it beats the hand-tuned score on the person's most recent plays,
//!   which it was not trained on;
//! - below a few hundred examples it doesn't train at all.
//!
//! Pure and deterministic like the rest of the engine: no randomness, ties broken by position,
//! floating point only in fixed order, so the server and a phone train the same model from the
//! same history.

use serde::{Deserialize, Serialize};

use crate::model::Model;
use crate::rank::{Context, features_of, hand_score};
use crate::{Catalog, History, Outcome, Params, bucket_of, weekend};

/// How many signals the ranker sees.
pub const FEATURES: usize = 9;

/// One song's signals in one moment: liking, fit with the session, time of day, freshness, recent
/// skip rate, how its sound fits the person's taste, whether that is known, how it sounds next to
/// the song before, and how big the jump in tempo and energy from it is.
pub type Features = [f32; FEATURES];

/// A new ranker is trained once a profile has this many labelled plays.
pub const MIN_EXAMPLES: usize = 300;
/// And only used when it beats the hand-tuned score by this much (area under the curve) on the
/// last quarter of the person's plays.
const MIN_GAIN: f64 = 0.01;
/// The share of examples, newest, held back to judge it.
const HOLDOUT: f64 = 0.25;

const ROUNDS: usize = 80;
const DEPTH: usize = 3;
const RATE: f64 = 0.12;
const MIN_LEAF: usize = 20;
const LAMBDA: f64 = 1.0;
const BINS: usize = 32;
/// The signals the trees learn from: the sound ones (taste fit, whether it is known, likeness to
/// the song before, the jump from it). The first five are the hand-tuned score's own.
const LEARNED_FEATURES: [usize; 4] = [5, 6, 7, 8];
/// The history is replayed from this many points in time to make examples without leaking.
const CHECKPOINTS: usize = 16;

/// A tree's node: a split on a feature, or a leaf (`feature` 255).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub feature: u8,
    pub threshold: f32,
    pub left: u16,
    pub right: u16,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

impl Tree {
    fn value(&self, x: &Features) -> f32 {
        let mut at = 0usize;
        loop {
            let n = &self.nodes[at];
            if n.feature == 255 {
                return n.value;
            }
            at = if x[usize::from(n.feature)] <= n.threshold {
                usize::from(n.left)
            } else {
                usize::from(n.right)
            };
        }
    }
}

/// The trained model, small enough to sync (about 60 KB), with how it did on the plays it was judged on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ranker {
    /// The model format; one this build does not know is ignored and the hand-tuned score runs.
    pub version: u32,
    /// Labelled plays it was trained on.
    pub examples: u32,
    /// The hand-tuned score as log-odds: `slope * score + intercept`, fitted to the examples.
    pub slope: f32,
    pub intercept: f32,
    pub trees: Vec<Tree>,
    /// Area under the ROC curve on the held-back recent plays: the ranker, and the hand-tuned score.
    pub holdout_auc: f32,
    pub hand_auc: f32,
}

pub const RANKER_VERSION: u32 = 1;

impl Ranker {
    /// The log-odds that an app pick is not skipped in its first 30 seconds: the hand-tuned score
    /// put on a log-odds scale (`slope` and `intercept`, fitted to this person), then the trees'
    /// corrections to it. Starting from the hand-tuned score keeps the order it gets right among
    /// good candidates; the trees only learn where this person differs from it.
    pub fn logit(&self, x: &Features) -> f64 {
        let mut z = f64::from(self.slope) * hand_score(x) + f64::from(self.intercept);
        for t in &self.trees {
            z += f64::from(t.value(x));
        }
        z
    }

    /// 0 to 1.
    pub fn probability(&self, x: &Features) -> f64 {
        1.0 / (1.0 + crate::det::exp(-self.logit(x)))
    }

    /// Whether this build can run it.
    pub fn usable(&self) -> bool {
        self.version == RANKER_VERSION && !self.trees.is_empty()
    }
}

/// One labelled play: its signals and whether it was enjoyed.
#[derive(Debug, Clone)]
pub struct Example {
    pub x: Features,
    pub y: bool,
    /// The hand-tuned score, to compare against.
    pub hand: f64,
}

fn history_before(h: &History, t: i64) -> History {
    History {
        plays: h
            .plays
            .iter()
            .filter(|p| p.started_utc_ms < t)
            .cloned()
            .collect(),
        likes: h.likes.iter().filter(|(_, at)| *at < t).cloned().collect(),
        added: h.added.iter().filter(|(_, at)| *at < t).cloned().collect(),
        feedback: h
            .feedback
            .iter()
            .filter(|f| f.at_utc_ms < t)
            .cloned()
            .collect(),
        not_interested: h
            .not_interested
            .iter()
            .filter(|n| n.at_utc_ms < t)
            .cloned()
            .collect(),
        epoch_start_utc_ms: h.epoch_start_utc_ms,
    }
}

/// The labelled plays of a history, with each play's signals as the engine would have had them just
/// before it: the history is replayed from [`CHECKPOINTS`] points, and a play is described by the
/// model built from everything before its chunk (so it never sees its own outcome). Only plays the
/// app chose count: they are what the ranker will be asked to predict; a skip of a song the person
/// picked is browsing. Oldest first.
pub fn examples(
    history: &History,
    catalog: &Catalog,
    p: &Params,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Vec<Example> {
    let _ = now_utc_ms;
    let learned: Vec<usize> = history
        .plays
        .iter()
        .enumerate()
        .filter(|(_, pl)| pl.started_utc_ms >= history.epoch_start_utc_ms)
        .map(|(i, _)| i)
        .collect();
    if learned.len() < MIN_EXAMPLES {
        return Vec::new();
    }
    // The first fifth is only history for the first model; examples come from the rest.
    let warm = learned.len() / 5;
    let rest = &learned[warm..];
    let chunk = rest.len().div_ceil(CHECKPOINTS).max(1);
    let mut out = Vec::new();
    for part in rest.chunks(chunk) {
        let start = history.plays[part[0]].started_utc_ms;
        let model = Model::build(
            &history_before(history, start),
            catalog,
            p,
            start,
            utc_offset_minutes,
        );
        for &pi in part {
            let play = &history.plays[pi];
            let Some(outcome) = play.outcome else {
                continue;
            };
            if !play.app_chosen || outcome == Outcome::Abandoned {
                continue;
            }
            let Some(i) = catalog.find(&play.song_key) else {
                continue;
            };
            // What was playing just before, if it was the same session.
            let previous = pi.checked_sub(1).map(|j| &history.plays[j]).filter(|q| {
                play.started_utc_ms - q.started_utc_ms <= p.session_gap_minutes * 60_000
            });
            let ctx = Context {
                seeds: previous.map(|q| q.song_key.clone()).into_iter().collect(),
                ..Context::default()
            };
            let bucket = bucket_of(play.started_utc_ms, play.utc_offset_minutes);
            let wk = weekend(play.started_utc_ms, play.utc_offset_minutes);
            let (x, _, _) = features_of(&model, catalog, &ctx, i, bucket, wk);
            out.push(Example {
                x,
                // The success measure (plan section 7): an app pick should not be skipped in its
                // first 30 seconds. Everything else, even a later skip, counts as it having worked.
                y: outcome != Outcome::SkippedEarly,
                hand: hand_score(&x),
            });
        }
    }
    out
}

/// Area under the ROC curve of `score` against the labels: the chance a random enjoyed play is
/// scored above a random skipped one (ties count half). 0.5 is no better than chance.
pub fn auc(scored: &[(f64, bool)]) -> f64 {
    let (pos, neg) = (
        scored.iter().filter(|s| s.1).count() as f64,
        scored.iter().filter(|s| !s.1).count() as f64,
    );
    if pos == 0.0 || neg == 0.0 {
        return 0.5;
    }
    let mut v: Vec<(f64, bool)> = scored.to_vec();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Rank sum with ties averaged.
    let mut rank_sum = 0.0;
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1].0 == v[i].0 {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        rank_sum += v[i..=j].iter().filter(|s| s.1).count() as f64 * avg;
        i = j + 1;
    }
    (rank_sum - pos * (pos + 1.0) / 2.0) / (pos * neg)
}

/// Train on labelled examples (no gate: see [`train`]).
pub fn fit(examples: &[Example]) -> Ranker {
    let n = examples.len();
    let y: Vec<f64> = examples.iter().map(|e| f64::from(u8::from(e.y))).collect();
    let share = (y.iter().sum::<f64>() / n.max(1) as f64).clamp(0.02, 0.98);
    let base = (share / (1.0 - share)).ln();
    // Each feature cut into at most BINS pieces by its own values' quantiles.
    let mut cuts: Vec<Vec<f32>> = Vec::with_capacity(FEATURES);
    for f in 0..FEATURES {
        let mut vals: Vec<f32> = examples.iter().map(|e| e.x[f]).collect();
        vals.sort_by(f32::total_cmp);
        vals.dedup();
        let mut c: Vec<f32> = if vals.len() <= BINS {
            vals.windows(2).map(|w| (w[0] + w[1]) / 2.0).collect()
        } else {
            (1..BINS).map(|k| vals[k * vals.len() / BINS]).collect()
        };
        c.dedup();
        cuts.push(c);
    }
    let bin_of = |f: usize, v: f32| cuts[f].partition_point(|c| *c < v);
    let binned: Vec<[u8; FEATURES]> = examples
        .iter()
        .map(|e| {
            let mut b = [0u8; FEATURES];
            for f in 0..FEATURES {
                b[f] = bin_of(f, e.x[f]) as u8;
            }
            b
        })
        .collect();
    // Start from the hand-tuned score put on a log-odds scale, so the trees only learn corrections.
    let (slope, intercept) = calibrate(examples, &y, base);
    let mut f_sum: Vec<f64> = examples
        .iter()
        .map(|e| slope * e.hand + intercept)
        .collect();
    let mut trees = Vec::with_capacity(ROUNDS);
    for _ in 0..ROUNDS {
        let (mut g, mut h) = (vec![0f64; n], vec![0f64; n]);
        for i in 0..n {
            let p = 1.0 / (1.0 + crate::det::exp(-f_sum[i]));
            g[i] = y[i] - p;
            h[i] = p * (1.0 - p);
        }
        let mut nodes: Vec<Node> = Vec::new();
        let all: Vec<usize> = (0..n).collect();
        grow(&all, 0, &binned, &cuts, &g, &h, &mut nodes);
        let tree = Tree { nodes };
        for (i, e) in examples.iter().enumerate() {
            f_sum[i] += f64::from(tree.value(&e.x));
        }
        trees.push(tree);
    }
    Ranker {
        version: RANKER_VERSION,
        examples: n as u32,
        slope: slope as f32,
        intercept: intercept as f32,
        trees,
        holdout_auc: 0.5,
        hand_auc: 0.5,
    }
}

/// The slope and intercept that put the hand-tuned score on a log-odds scale: a one-variable
/// logistic regression by Newton's method (a fixed number of steps, so it is deterministic).
fn calibrate(examples: &[Example], y: &[f64], base: f64) -> (f64, f64) {
    let (mut a, mut b) = (0.0f64, base);
    for _ in 0..25 {
        let (mut ga, mut gb, mut haa, mut hab, mut hbb) = (0.0, 0.0, 1e-6, 0.0, 1e-6);
        for (e, yi) in examples.iter().zip(y) {
            let p = 1.0 / (1.0 + crate::det::exp(-(a * e.hand + b)));
            let w = p * (1.0 - p);
            ga += (yi - p) * e.hand;
            gb += yi - p;
            haa += w * e.hand * e.hand;
            hab += w * e.hand;
            hbb += w;
        }
        let det = haa * hbb - hab * hab;
        if det.abs() < 1e-12 {
            break;
        }
        a += (hbb * ga - hab * gb) / det;
        b += (haa * gb - hab * ga) / det;
    }
    (a, b)
}

/// Grow one node (and below) on `rows`; returns its index in `nodes`.
fn grow(
    rows: &[usize],
    depth: usize,
    binned: &[[u8; FEATURES]],
    cuts: &[Vec<f32>],
    g: &[f64],
    h: &[f64],
    nodes: &mut Vec<Node>,
) -> u16 {
    let (gs, hs): (f64, f64) = rows
        .iter()
        .fold((0.0, 0.0), |a, &i| (a.0 + g[i], a.1 + h[i]));
    let leaf = Node {
        feature: 255,
        threshold: 0.0,
        left: 0,
        right: 0,
        value: (RATE * gs / (hs + LAMBDA)) as f32,
    };
    let at = nodes.len() as u16;
    if depth >= DEPTH || rows.len() < 2 * MIN_LEAF {
        nodes.push(leaf);
        return at;
    }
    // The best split by gain, from a histogram of each feature; ties by feature then cut. Only the
    // signals the hand-tuned score cannot see are split on (`LEARNED_FEATURES`): the hand-tuned
    // part is already in the starting score, and splitting on it would let the trees bend its
    // order in the region where picks live, which the plays they are trained on seldom reach.
    let parent = gs * gs / (hs + LAMBDA);
    let mut best: Option<(f64, usize, usize)> = None;
    for f in LEARNED_FEATURES {
        let bins = cuts[f].len() + 1;
        let (mut hg, mut hh, mut hc) = (vec![0f64; bins], vec![0f64; bins], vec![0usize; bins]);
        for &i in rows {
            let b = usize::from(binned[i][f]);
            hg[b] += g[i];
            hh[b] += h[i];
            hc[b] += 1;
        }
        let (mut lg, mut lh, mut lc) = (0.0, 0.0, 0usize);
        for b in 0..bins - 1 {
            lg += hg[b];
            lh += hh[b];
            lc += hc[b];
            let rc = rows.len() - lc;
            if lc < MIN_LEAF || rc < MIN_LEAF {
                continue;
            }
            let (rg, rh) = (gs - lg, hs - lh);
            let gain = lg * lg / (lh + LAMBDA) + rg * rg / (rh + LAMBDA) - parent;
            if gain > best.map_or(1e-9, |x| x.0) {
                best = Some((gain, f, b));
            }
        }
    }
    let Some((_, f, b)) = best else {
        nodes.push(leaf);
        return at;
    };
    nodes.push(leaf.clone()); // a placeholder, filled in below
    let (left, right): (Vec<usize>, Vec<usize>) =
        rows.iter().partition(|&&i| usize::from(binned[i][f]) <= b);
    let l = grow(&left, depth + 1, binned, cuts, g, h, nodes);
    let r = grow(&right, depth + 1, binned, cuts, g, h, nodes);
    nodes[usize::from(at)] = Node {
        feature: f as u8,
        threshold: cuts[f][b],
        left: l,
        right: r,
        value: 0.0,
    };
    at
}

/// Train a ranker for this history, or None when there is not enough to learn from or it does not
/// beat the hand-tuned score on the newest quarter of the plays (which it was not trained on).
pub fn train(
    history: &History,
    catalog: &Catalog,
    p: &Params,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Option<Ranker> {
    let all = examples(history, catalog, p, now_utc_ms, utc_offset_minutes);
    judge(&all)
}

/// The gate: train on the older part, judge on the newest; if it passes, train on everything.
pub fn judge(all: &[Example]) -> Option<Ranker> {
    if all.len() < MIN_EXAMPLES {
        return None;
    }
    let cut = (all.len() as f64 * (1.0 - HOLDOUT)) as usize;
    let (older, newest) = all.split_at(cut);
    let trial = fit(older);
    // Judged where picks live: the half of the newest plays whose song the person likes most.
    // Telling a loved song from a hated one is easy and says nothing about ordering good
    // candidates, which is the job.
    let mut likings: Vec<f32> = newest.iter().map(|e| e.x[0]).collect();
    likings.sort_by(f32::total_cmp);
    let median = likings[likings.len() / 2];
    let region: Vec<&Example> = newest.iter().filter(|e| e.x[0] >= median).collect();
    let judged: Vec<&Example> = if region.len() >= 60 {
        region
    } else {
        newest.iter().collect()
    };
    let learned: Vec<(f64, bool)> = judged.iter().map(|e| (trial.logit(&e.x), e.y)).collect();
    let hand: Vec<(f64, bool)> = judged.iter().map(|e| (e.hand, e.y)).collect();
    let (a_learned, a_hand) = (auc(&learned), auc(&hand));
    if a_learned < a_hand + MIN_GAIN {
        return None;
    }
    let mut ranker = fit(all);
    ranker.holdout_auc = a_learned as f32;
    ranker.hand_auc = a_hand as f32;
    Some(ranker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::det::Rng;

    /// Examples where only feature 5 (sound fit) matters and the hand score ignores it.
    fn sound_matters(n: usize, seed: u64) -> Vec<Example> {
        let mut rng = Rng::new(seed);
        (0..n)
            .map(|_| {
                let mut x = [0f32; FEATURES];
                for v in &mut x {
                    *v = rng.unit() as f32;
                }
                x[6] = 1.0;
                let p = 0.1 + 0.8 * f64::from(x[5]);
                Example {
                    y: rng.unit() < p,
                    hand: hand_score(&x),
                    x,
                }
            })
            .collect()
    }

    #[test]
    fn it_learns_the_signal_the_hand_tuned_score_ignores() {
        let all = sound_matters(1200, 3);
        let r = judge(&all).expect("a clear signal passes the gate");
        assert!(
            r.holdout_auc > r.hand_auc + 0.1,
            "{} vs {}",
            r.holdout_auc,
            r.hand_auc
        );
        // It ranks a high sound fit above a low one, holding everything else equal.
        let (mut lo, mut hi) = ([0.5f32; FEATURES], [0.5f32; FEATURES]);
        lo[5] = 0.05;
        hi[5] = 0.95;
        assert!(r.probability(&hi) > r.probability(&lo) + 0.3);
    }

    #[test]
    fn it_does_not_switch_on_when_it_cannot_beat_the_hand_tuned_score() {
        // Labels that follow the hand-tuned score exactly (plus noise): nothing to gain.
        let mut rng = Rng::new(9);
        let all: Vec<Example> = (0..1200)
            .map(|_| {
                let mut x = [0f32; FEATURES];
                for v in &mut x {
                    *v = rng.unit() as f32;
                }
                let hand = hand_score(&x);
                let p = (0.15 + 0.7 * hand).clamp(0.02, 0.98);
                Example {
                    y: rng.unit() < p,
                    hand,
                    x,
                }
            })
            .collect();
        let r = judge(&all);
        assert!(
            r.is_none()
                || r.as_ref()
                    .is_some_and(|r| r.holdout_auc >= r.hand_auc + 0.01),
            "{r:?}"
        );
    }

    #[test]
    fn too_few_examples_train_nothing() {
        assert!(judge(&sound_matters(MIN_EXAMPLES - 1, 1)).is_none());
    }

    #[test]
    fn training_is_deterministic_and_the_model_round_trips() {
        let all = sound_matters(500, 5);
        let (a, b) = (fit(&all), fit(&all));
        assert_eq!(a, b);
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.len() < 120_000, "{} bytes", json.len());
        let back: Ranker = serde_json::from_str(&json).unwrap();
        assert_eq!(back.logit(&all[0].x), a.logit(&all[0].x));
        assert!(back.usable());
    }

    #[test]
    fn auc_is_the_chance_of_ranking_an_enjoyed_play_first() {
        let perfect = [(0.9, true), (0.8, true), (0.2, false), (0.1, false)];
        assert_eq!(auc(&perfect), 1.0);
        let worst = [(0.1, true), (0.2, true), (0.8, false), (0.9, false)];
        assert_eq!(auc(&worst), 0.0);
        let ties = [(0.5, true), (0.5, false)];
        assert_eq!(auc(&ties), 0.5);
        assert_eq!(auc(&[(0.5, true)]), 0.5, "no negatives: nothing to say");
    }
}
