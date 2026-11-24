//! What songs sound like, as the engine uses it (Phase 10, docs/adr/core-0019 and core-0020).
//!
//! `encore-analysis` makes each song's sound once, on a device. The engine receives it as a
//! [`Sound`]: a fixed 128-number projection of YAMNet's 1024 numbers (kept as integers, so
//! similarity is exact and the same on every platform and on the server) and a few plain measures
//! (tempo, energy, brightness, dynamics, key) for flow. A song with no sound is simply without
//! it: nothing is guessed, and every rule falls back to version 1's.

use serde::{Deserialize, Serialize};

/// The length of [`Sound::projection`].
pub const DIMS: usize = 128;

/// One song's sound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sound {
    /// A fixed ±1 random projection of the 1024-number embedding (`encore_analysis::project`),
    /// [`DIMS`] values. A random projection keeps distances, and 128 numbers keep the quality
    /// (docs/adr/core-0019).
    pub projection: Vec<i16>,
    /// Beats per minute in tenths, 0 when there is no steady beat.
    pub bpm_tenths: u16,
    /// 0 to 255: how intense (loudness, change from moment to moment, brightness).
    pub energy: u8,
    /// 0 to 255: warm and dark to bright.
    pub brightness: u8,
    /// 0 to 255: how much the loudness moves.
    pub dynamics: u8,
    /// 0 to 11 major (C = 0), 12 to 23 minor, 255 none.
    pub key: u8,
}

/// A song's projection with the library's mean taken out, and its squared length, for exact
/// cosine similarity in integers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Centred {
    pub v: Vec<i32>,
    pub norm2: i64,
    /// The length, the square root of `norm2` (correctly rounded, so the same everywhere), made once.
    pub norm: f64,
}

/// Centre every sound by the mean of all of them. Centring before comparing is a measured gain
/// (43.2% to 45.4% same-genre neighbours on the test clips): all embeddings share a large common
/// part that otherwise makes everything look alike. The projection is linear, so this equals
/// centring the original 1024 numbers.
pub(crate) fn centre(sounds: &[Option<&Sound>]) -> Vec<Option<Centred>> {
    let mut sum = vec![0i64; DIMS];
    let mut n = 0i64;
    for s in sounds.iter().flatten() {
        if s.projection.len() != DIMS {
            continue;
        }
        for (a, v) in sum.iter_mut().zip(&s.projection) {
            *a += i64::from(*v);
        }
        n += 1;
    }
    let mean: Vec<i64> = sum.iter().map(|a| if n > 0 { a / n } else { 0 }).collect();
    sounds
        .iter()
        .map(|s| {
            let s = s.filter(|s| s.projection.len() == DIMS)?;
            // The projection is at most 1020 in size; anything past 1024 is clamped, so the
            // centred values stay within 2048 and a dot product fits 32 bits (128 x 2048^2).
            let v: Vec<i32> = s
                .projection
                .iter()
                .zip(&mean)
                .map(|(p, m)| (i64::from((*p).clamp(-1024, 1024)) - m) as i32)
                .collect();
            let norm2: i64 = v.iter().map(|x| i64::from(*x) * i64::from(*x)).sum();
            Some(Centred {
                v,
                norm2,
                norm: (norm2 as f64).sqrt(),
            })
        })
        .collect()
}

/// Cosine similarity of two centred sounds, in -1..1. The dot product and norms are exact
/// integers; the division and square roots are correctly rounded, so every platform agrees.
pub(crate) fn cosine(a: &Centred, b: &Centred) -> f64 {
    if a.norm2 == 0 || b.norm2 == 0 {
        return 0.0;
    }
    // 32-bit products and sums: at most 128 x 2048 x 2048, well within range, and it vectorises.
    let dot: i32 = a.v.iter().zip(&b.v).map(|(x, y)| x * y).sum();
    f64::from(dot) / (a.norm * b.norm)
}

/// How far apart two tempos are as a share, 0 (the same, or half or double time) to 1; None when
/// either has no steady beat. Only exact arithmetic.
pub fn tempo_step(a_tenths: u16, b_tenths: u16) -> Option<f64> {
    if a_tenths == 0 || b_tenths == 0 {
        return None;
    }
    let (a, b) = (f64::from(a_tenths), f64::from(b_tenths));
    let steps = [
        (a - b).abs() / a.max(b),
        (a - 2.0 * b).abs() / a.max(2.0 * b),
        (2.0 * a - b).abs() / (2.0 * a).max(b),
    ];
    Some(steps.into_iter().fold(1.0, f64::min))
}

/// How much a move from one song to the next jumps in tempo and energy, 0 (smooth) to 1: the
/// plan's "tempo and energy move one step at a time". None when neither is known.
pub fn flow_cost(a: &Sound, b: &Sound) -> Option<f64> {
    let tempo = tempo_step(a.bpm_tenths, b.bpm_tenths).map(|t| (t * 3.0).min(1.0));
    let energy = Some((f64::from(a.energy) - f64::from(b.energy)).abs() / 255.0);
    match (tempo, energy) {
        (Some(t), Some(e)) => Some(0.5 * t + 0.5 * e),
        (None, Some(e)) => Some(e),
        (Some(t), None) => Some(t),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sound(projection: Vec<i16>) -> Sound {
        Sound {
            projection,
            bpm_tenths: 1200,
            energy: 128,
            brightness: 100,
            dynamics: 80,
            key: 0,
        }
    }

    fn vec_of(seed: i16) -> Vec<i16> {
        (0..DIMS as i16).map(|i| (i * seed) % 97 - 40).collect()
    }

    #[test]
    fn a_song_is_most_like_itself_and_the_mean_is_taken_out() {
        let sounds = [sound(vec_of(3)), sound(vec_of(5)), sound(vec_of(7))];
        let refs: Vec<Option<&Sound>> = sounds.iter().map(Some).collect();
        let c = centre(&refs);
        let c0 = c[0].as_ref().unwrap();
        assert!((cosine(c0, c0) - 1.0).abs() < 1e-12);
        // Centred by their own mean, the three vectors sum to zero in every dimension.
        let total: i64 = (0..DIMS)
            .map(|d| {
                c.iter()
                    .flatten()
                    .map(|x| i64::from(x.v[d]))
                    .sum::<i64>()
                    .abs()
            })
            .sum();
        assert!(total <= 3 * DIMS as i64, "{total}");
    }

    #[test]
    fn a_song_without_sound_or_of_the_wrong_length_is_left_out() {
        let good = sound(vec_of(3));
        let short = sound(vec![1, 2, 3]);
        let c = centre(&[Some(&good), None, Some(&short)]);
        assert!(c[0].is_some() && c[1].is_none() && c[2].is_none());
    }

    #[test]
    fn tempo_steps_forgive_half_and_double_time() {
        assert_eq!(tempo_step(1200, 1200), Some(0.0));
        assert_eq!(tempo_step(1200, 600), Some(0.0));
        assert_eq!(tempo_step(600, 1200), Some(0.0));
        assert!(tempo_step(1200, 1500).unwrap() > 0.15);
        assert_eq!(tempo_step(0, 1200), None);
    }

    #[test]
    fn a_smooth_move_costs_less_than_a_jump() {
        let a = sound(vec_of(3));
        let near = Sound {
            bpm_tenths: 1230,
            energy: 135,
            ..sound(vec_of(3))
        };
        let far = Sound {
            bpm_tenths: 1700,
            energy: 250,
            ..sound(vec_of(3))
        };
        assert!(flow_cost(&a, &near).unwrap() < flow_cost(&a, &far).unwrap());
        assert!(flow_cost(&a, &near).unwrap() < 0.1);
        let beatless = Sound {
            bpm_tenths: 0,
            ..near.clone()
        };
        assert!(
            flow_cost(&a, &beatless).is_some(),
            "energy still says something"
        );
    }
}
