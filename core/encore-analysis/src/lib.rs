//! Sound analysis (plan sections 7 and 21, docs/adr/core-0019): what a song sounds like, made
//! once per song from about 30 seconds of its audio, by signal processing alone.
//!
//! `analyse` takes mono samples and returns a [`SoundProfile`]: tempo, key, loudness, energy,
//! brightness, dynamics and a 48-number texture. Nothing here reads a file or the network;
//! the decoder (feature `decode`) and the ranged reader sit around it. The profile is about
//! the song, never the listener, and is computed once, so tiny float differences between
//! platforms don't matter: the stored bytes are what everything else compares.

#![forbid(unsafe_code)]

mod dsp;
mod profile;
mod tempo;

#[cfg(feature = "decode")]
mod decode;
#[cfg(feature = "decode")]
pub use decode::{WINDOW_SECONDS, Window, analyse_source, decode_window};

pub use profile::{
    AnalysisError, ENCODED_LEN, Key, SoundProfile, TEXTURE_LEN, VERSION, texture_cosine,
};

use dsp::{Frames, MELS};

/// Shorter clips are refused: tempo and key need a few bars.
pub const MIN_SECONDS: f32 = 8.0;
/// Longer clips are cut to this much, so a mistake can't cost minutes.
pub const MAX_SECONDS: f32 = 90.0;
/// The slowest sample rate accepted.
const MIN_RATE: u32 = 8_000;
/// Quieter than this is silence.
const SILENCE_LUFS: f32 = -60.0;

/// Analyse mono `samples` (-1..1) recorded at `rate` Hz.
pub fn analyse(samples: &[f32], rate: u32) -> Result<SoundProfile, AnalysisError> {
    if rate < MIN_RATE {
        return Err(AnalysisError::Unsupported(format!(
            "a sample rate of {rate} Hz"
        )));
    }
    let max = (MAX_SECONDS * rate as f32) as usize;
    let samples = &samples[..samples.len().min(max)];
    if (samples.len() as f32) < MIN_SECONDS * rate as f32 {
        return Err(AnalysisError::TooShort);
    }
    let lufs = loudness(samples, rate);
    if lufs < SILENCE_LUFS {
        return Err(AnalysisError::Silent);
    }
    let frames = dsp::frames(samples, rate);
    let onset = onset_strength(&frames);
    let (bpm, beat_strength) = tempo::tempo(&onset, frames.frame_rate);
    let mean_chroma = mean_chroma(&frames);
    let key = tempo::key(&mean_chroma);
    let brightness = brightness(&frames);
    let dynamics = dynamics(&frames);
    let onset_mean = onset.iter().sum::<f32>() / onset.len().max(1) as f32;
    let energy = (0.5 * ((lufs + 35.0) / 25.0).clamp(0.0, 1.0)
        + 0.3 * (onset_mean / 8.0).clamp(0.0, 1.0)
        + 0.2 * brightness)
        .clamp(0.0, 1.0);
    let texture = texture(&frames, &mean_chroma, onset_mean);
    Ok(SoundProfile {
        bpm,
        beat_strength,
        key,
        lufs,
        energy,
        brightness,
        dynamics,
        texture,
    })
}

/// Integrated loudness, LUFS (ITU-R BS.1770).
fn loudness(samples: &[f32], rate: u32) -> f32 {
    let Ok(mut meter) = ebur128::EbuR128::new(1, rate, ebur128::Mode::I) else {
        return f32::NEG_INFINITY;
    };
    if meter.add_frames_f32(samples).is_err() {
        return f32::NEG_INFINITY;
    }
    meter
        .loudness_global()
        .map_or(f32::NEG_INFINITY, |l| l as f32)
}

/// How much louder each frame is than the one before, summed over the mel bands that rose and
/// averaged over the bands: a beat or a note starting shows as a spike.
fn onset_strength(f: &Frames) -> Vec<f32> {
    let mut out = vec![0f32; f.mel_db.len()];
    for (slot, pair) in out.iter_mut().skip(1).zip(f.mel_db.windows(2)) {
        let rise: f32 = pair[1]
            .iter()
            .zip(&pair[0])
            .map(|(now, before)| (now - before).max(0.0))
            .sum();
        *slot = rise / MELS as f32;
    }
    out
}

/// The clip's average pitch content: each frame's pitch classes as shares of that frame, over the
/// frames that aren't far quieter than the loudest.
fn mean_chroma(f: &Frames) -> [f32; 12] {
    let loudest = f.total_db.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = [0f32; 12];
    for (chroma, db) in f.chroma.iter().zip(&f.total_db) {
        if *db < loudest - 30.0 {
            continue;
        }
        let total: f32 = chroma.iter().sum();
        if total <= 1e-12 {
            continue;
        }
        for (s, c) in sum.iter_mut().zip(chroma) {
            *s += c / total;
        }
    }
    sum
}

/// 0 (centroid around 200 Hz) to 1 (around 10 kHz), on a log scale, averaged over the clip.
fn brightness(f: &Frames) -> f32 {
    let mean = f
        .centroid_hz
        .iter()
        .map(|c| (c.max(200.0) / 200.0).log2())
        .sum::<f32>()
        / f.centroid_hz.len().max(1) as f32;
    (mean / (10_000f32 / 200.0).log2()).clamp(0.0, 1.0)
}

/// The spread of frame loudness (95th minus 10th percentile, in dB) over 30 dB.
fn dynamics(f: &Frames) -> f32 {
    let mut db = f.total_db.clone();
    db.sort_by(|a, b| a.total_cmp(b));
    let at = |p: f32| db[(((db.len() - 1) as f32) * p).round() as usize];
    ((at(0.95) - at(0.10)) / 30.0).clamp(0.0, 1.0)
}

fn mean(v: impl Iterator<Item = f32>, n: usize) -> f32 {
    v.sum::<f32>() / n.max(1) as f32
}

fn unit_to_i8(v: f32) -> i8 {
    (v.clamp(-1.0, 1.0) * 127.0).round() as i8
}

/// The texture: sixteen band groups' average level (shape of the spectrum, with the overall level
/// taken out), sixteen groups' variation over time, the twelve pitch classes' shares, and four
/// summaries (onset rate, how much the brightness moves, zero crossings, noisiness). Fixed scales
/// map each to -1..1; they are starting values to be tuned on real music (core-0019).
fn texture(f: &Frames, chroma: &[f32; 12], onset_mean: f32) -> [i8; TEXTURE_LEN] {
    let n = f.mel_db.len();
    let mut out = [0i8; TEXTURE_LEN];
    let group = |t: usize, g: usize| (f.mel_db[t][2 * g] + f.mel_db[t][2 * g + 1]) / 2.0;
    let means: Vec<f32> = (0..16)
        .map(|g| mean((0..n).map(|t| group(t, g)), n))
        .collect();
    let overall = means.iter().sum::<f32>() / 16.0;
    for g in 0..16 {
        out[g] = unit_to_i8((means[g] - overall) / 20.0);
        let var = mean((0..n).map(|t| (group(t, g) - means[g]).powi(2)), n);
        out[16 + g] = unit_to_i8((var.sqrt() - 6.0) / 8.0);
    }
    let total: f32 = chroma.iter().sum();
    for (i, c) in chroma.iter().enumerate() {
        let share = if total > 1e-12 { c / total } else { 1.0 / 12.0 };
        out[32 + i] = unit_to_i8((share * 12.0 - 1.0) / 2.0);
    }
    let logs: Vec<f32> = f.centroid_hz.iter().map(|c| c.max(100.0).log2()).collect();
    let log_mean = mean(logs.iter().copied(), n);
    let log_std = mean(logs.iter().map(|l| (l - log_mean).powi(2)), n).sqrt();
    out[44] = unit_to_i8(onset_mean / 4.0 - 1.0);
    out[45] = unit_to_i8(log_std * 3.0 - 1.0);
    out[46] = unit_to_i8(mean(f.zcr.iter().copied(), n) * 10.0 - 1.0);
    out[47] = unit_to_i8(mean(f.flatness.iter().copied(), n) * 6.0 - 1.0);
    out
}

#[cfg(test)]
mod tests;
