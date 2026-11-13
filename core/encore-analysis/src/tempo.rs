//! Tempo from the onset envelope's autocorrelation, and the key from the pitch content.

use crate::profile::Key;

/// The tempo range searched, beats per minute.
const BPM_LOW: f32 = 60.0;
const BPM_HIGH: f32 = 200.0;
/// The most likely tempo and how wide the preference is, in octaves.
const PRIOR_BPM: f32 = 120.0;
const PRIOR_OCTAVES: f32 = 0.8;
/// Below this steadiness the clip is said to have no steady beat.
const MIN_STRENGTH: f32 = 0.12;
/// The onset envelope's variance (dB squared) below which there is no beat to find.
const MIN_ONSET_VARIANCE: f32 = 0.25;

/// Beats per minute and how steady the beat is (0 to 1), from the onset strength of each
/// frame. (0.0, strength) when nothing steady was found.
pub(crate) fn tempo(onset: &[f32], frame_rate: f32) -> (f32, f32) {
    let n = onset.len();
    let lag_min = ((frame_rate * 60.0 / BPM_HIGH).ceil() as usize).max(2);
    let lag_max = (frame_rate * 60.0 / BPM_LOW).floor() as usize;
    if n < lag_max * 3 || lag_max <= lag_min + 2 {
        return (0.0, 0.0);
    }
    let mean = onset.iter().sum::<f32>() / n as f32;
    let x: Vec<f32> = onset.iter().map(|v| v - mean).collect();
    let zero: f32 = x.iter().map(|v| v * v).sum::<f32>() / n as f32;
    // A flat envelope (a held note, a drone) has nothing to be steady about, and dividing by its
    // tiny variance would turn rounding noise into a tempo: the envelope must move by at least
    // about half a dB between frames.
    if zero < MIN_ONSET_VARIANCE {
        return (0.0, 0.0);
    }
    // Normalised autocorrelation at each lag, and the same weighted by the tempo prior.
    let mut acf = vec![0f32; lag_max + 2];
    for (lag, slot) in acf.iter_mut().enumerate().skip(lag_min - 1) {
        let m = n - lag;
        let s: f32 = (0..m).map(|t| x[t] * x[t + lag]).sum();
        *slot = s / m as f32 / zero;
    }
    let weight = |lag: usize| {
        let bpm = 60.0 * frame_rate / lag as f32;
        let octaves = (bpm / PRIOR_BPM).log2() / PRIOR_OCTAVES;
        (-0.5 * octaves * octaves).exp()
    };
    let mut best = lag_min;
    for lag in lag_min..=lag_max {
        if acf[lag] * weight(lag) > acf[best] * weight(best) {
            best = lag;
        }
    }
    let strength = acf[best].clamp(0.0, 1.0);
    if strength < MIN_STRENGTH {
        return (0.0, strength);
    }
    // A parabola through the peak and its neighbours places it between whole frames.
    let (a, b, c) = (
        acf[best - 1] * weight(best - 1),
        acf[best] * weight(best),
        acf[best + 1] * weight(best + 1),
    );
    let denom = a - 2.0 * b + c;
    let shift = if denom.abs() > 1e-9 {
        (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    (60.0 * frame_rate / (best as f32 + shift), strength)
}

// Krumhansl and Kessler's key profiles (tonic first), the usual starting point for key finding.
const MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// Below this correlation no key stands out.
const MIN_KEY_CORRELATION: f32 = 0.45;

fn correlation(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (mut num, mut da, mut db) = (0f32, 0f32, 0f32);
    for i in 0..12 {
        let (x, y) = (a[i] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da < 1e-12 || db < 1e-12 {
        0.0
    } else {
        num / (da.sqrt() * db.sqrt())
    }
}

/// The key whose profile best matches the clip's average pitch content, when one stands out.
pub(crate) fn key(chroma: &[f32; 12]) -> Option<Key> {
    let mut best: Option<(f32, Key)> = None;
    for pitch_class in 0..12u8 {
        for (profile, major) in [(&MAJOR, true), (&MINOR, false)] {
            // The profile turned so its tonic sits on `pitch_class`.
            let mut turned = [0f32; 12];
            for (i, v) in profile.iter().enumerate() {
                turned[(i + usize::from(pitch_class)) % 12] = *v;
            }
            let r = correlation(chroma, &turned);
            if best.is_none_or(|(b, _)| r > b) {
                best = Some((r, Key { pitch_class, major }));
            }
        }
    }
    best.filter(|(r, _)| *r >= MIN_KEY_CORRELATION)
        .map(|(_, k)| k)
}
