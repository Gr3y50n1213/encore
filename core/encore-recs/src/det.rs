//! Deterministic numbers: the same result on every platform and build.
//!
//! IEEE 754 makes `+ - * /` and `sqrt` exact to the bit everywhere; the maths
//! library's `exp` is not (it may differ by one unit in the last place between
//! systems). One bit can reorder two songs with nearly equal scores, so the
//! engine uses this `exp`, made only of exact operations, and its own PRNG.

/// e^x from exact operations: x = k ln2 + r with |r| <= ln2/2, then a degree-12
/// Taylor polynomial for e^r (error below 1e-15 there), then a power of two.
pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 700.0 {
        return f64::INFINITY;
    }
    if x < -700.0 {
        return 0.0;
    }
    const LN2: f64 = std::f64::consts::LN_2;
    let k = (x / LN2 + if x >= 0.0 { 0.5 } else { -0.5 }) as i32;
    let r = x - f64::from(k) * LN2;
    let mut term = 1.0;
    let mut sum = 1.0;
    for i in 1..=12 {
        term = term * r / f64::from(i);
        sum += term;
    }
    // 2^k built from its bits, exact.
    let bits = ((i64::from(k) + 1023) as u64) << 52;
    sum * f64::from_bits(bits)
}

/// How much of a weight is left after `age_days`, halving every `half_life_days`.
pub fn decay(age_days: f64, half_life_days: f64) -> f64 {
    if age_days <= 0.0 {
        1.0
    } else {
        exp(-age_days * std::f64::consts::LN_2 / half_life_days)
    }
}

/// A smooth squash to (-1, 1): x / (1 + |x|), exact operations only.
pub fn squash(x: f64) -> f64 {
    x / (1.0 + x.abs())
}

/// SplitMix64: small, fast, and the same sequence everywhere.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// A seed from text (a mix's ID, a day): FNV-1a.
    pub fn seed_of(text: &str) -> u64 {
        text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1), 53 bits.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// 0..n.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// A Beta(a, b) draw for Thompson sampling, a and b whole counts from 1 (1 + the
    /// successes, 1 + the failures): the a-th smallest of a + b - 1 uniforms has
    /// exactly that distribution. Counts are capped, which keeps it cheap.
    pub fn beta(&mut self, a: u32, b: u32) -> f64 {
        let a = a.clamp(1, 40) as usize;
        let b = b.clamp(1, 40) as usize;
        let mut u: Vec<f64> = (0..a + b - 1).map(|_| self.unit()).collect();
        u.sort_by(|x, y| x.total_cmp(y));
        u[a - 1]
    }

    /// Shuffle in place (Fisher-Yates).
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_matches_the_maths_library_closely_and_is_its_own_everywhere() {
        let mut worst = 0.0f64;
        let mut x = -30.0;
        while x < 30.0 {
            let rel = ((exp(x) - x.exp()) / x.exp()).abs();
            worst = worst.max(rel);
            x += 0.0137;
        }
        assert!(worst < 1e-14, "relative error {worst}");
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(exp(-1000.0), 0.0);
    }

    #[test]
    fn decay_halves_every_half_life() {
        assert_eq!(decay(0.0, 30.0), 1.0);
        assert!((decay(30.0, 30.0) - 0.5).abs() < 1e-12);
        assert!((decay(60.0, 30.0) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn the_prng_is_the_same_sequence_and_beta_has_the_right_mean() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        let xs: Vec<u64> = (0..5).map(|_| a.next_u64()).collect();
        let ys: Vec<u64> = (0..5).map(|_| b.next_u64()).collect();
        assert_eq!(xs, ys);
        assert_ne!(xs[0], xs[1]);
        let mut r = Rng::new(1);
        let n = 4000;
        let mean: f64 = (0..n).map(|_| r.beta(3, 1)).sum::<f64>() / f64::from(n);
        assert!((mean - 0.75).abs() < 0.02, "Beta(3,1) mean {mean}");
        assert_eq!(Rng::seed_of("a"), Rng::seed_of("a"));
        assert_ne!(Rng::seed_of("a"), Rng::seed_of("b"));
    }
}
