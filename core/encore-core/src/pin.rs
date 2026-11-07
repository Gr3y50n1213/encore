//! Profile PINs (board 14; plan section 6). A PIN is a screen lock, not
//! encryption: it keeps a child out of a parent's profile on a shared iPad. It is
//! kept as a salted SHA-256 hash, iterated so a copied registry isn't read in an
//! instant; with 10^6 possible PINs nothing makes a stolen hash safe, which is
//! why the lock-out, not the hash, is the guard.
//!
//! The lock-out (numbers in `profiles` tokens): after five wrong tries the PIN
//! can't be tried for 30 s, and each lock-out after that doubles the wait, up to
//! an hour. The right PIN resets it.

use sha2::{Digest, Sha256};

/// Hash rounds. A starting value: about 50 ms on a phone, measured in Phase 6.
pub const ITERATIONS: u32 = 100_000;

/// Whether `pin` is `min..=max` ASCII digits.
pub fn valid(pin: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

/// A new random salt (two ULIDs' random parts are 160 bits).
pub fn new_salt() -> String {
    format!("{}{}", ulid::Ulid::generate(), ulid::Ulid::generate())
}

/// The stored form of a PIN.
pub fn hash(salt: &str, pin: &str) -> String {
    let mut h: [u8; 32] = Sha256::new()
        .chain_update(salt.as_bytes())
        .chain_update([0u8])
        .chain_update(pin.as_bytes())
        .finalize()
        .into();
    for _ in 1..ITERATIONS {
        h = Sha256::new()
            .chain_update(h)
            .chain_update(salt.as_bytes())
            .finalize()
            .into();
    }
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// Equal without stopping at the first difference.
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The lock-out rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    pub wrong_tries: u32,
    pub first_lock_ms: i64,
    pub max_lock_ms: i64,
}

/// A PIN's try state: wrong tries since the last lock-out, until when it is
/// locked, and how many lock-outs in a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TryState {
    pub failed: u32,
    pub locked_until_utc_ms: i64,
    pub lockouts: u32,
}

impl Rules {
    /// The state after one more wrong try at `now`.
    pub fn after_wrong(&self, s: TryState, now: i64) -> TryState {
        let failed = s.failed + 1;
        if failed < self.wrong_tries {
            return TryState { failed, ..s };
        }
        let wait = self
            .first_lock_ms
            .saturating_mul(1i64 << s.lockouts.min(20))
            .min(self.max_lock_ms);
        TryState {
            failed: 0,
            locked_until_utc_ms: now + wait,
            lockouts: s.lockouts + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules = Rules {
        wrong_tries: 5,
        first_lock_ms: 30_000,
        max_lock_ms: 3_600_000,
    };

    #[test]
    fn only_four_to_six_digits() {
        assert!(valid("1234", 4, 6));
        assert!(valid("123456", 4, 6));
        assert!(!valid("123", 4, 6));
        assert!(!valid("1234567", 4, 6));
        assert!(!valid("12a4", 4, 6));
        assert!(!valid("١٢٣٤", 4, 6), "digits are ASCII digits");
    }

    #[test]
    fn the_hash_is_salted_and_repeatable() {
        let s1 = new_salt();
        let s2 = new_salt();
        assert_ne!(s1, s2);
        assert_eq!(s1.len(), 52);
        let h = hash(&s1, "4321");
        assert_eq!(h.len(), 64);
        assert_eq!(h, hash(&s1, "4321"));
        assert_ne!(h, hash(&s2, "4321"), "another salt");
        assert_ne!(h, hash(&s1, "4322"));
        assert!(same(&h, &hash(&s1, "4321")));
        assert!(!same(&h, &hash(&s1, "1111")));
        assert!(!same("ab", "abc"));
    }

    #[test]
    fn five_wrong_tries_lock_for_30_s_then_longer() {
        let mut s = TryState::default();
        for i in 1..5 {
            s = RULES.after_wrong(s, 0);
            assert_eq!((s.failed, s.locked_until_utc_ms), (i, 0));
        }
        s = RULES.after_wrong(s, 1_000);
        assert_eq!(
            s,
            TryState {
                failed: 0,
                locked_until_utc_ms: 31_000,
                lockouts: 1
            }
        );
        for _ in 0..5 {
            s = RULES.after_wrong(s, 100_000);
        }
        assert_eq!(s.locked_until_utc_ms, 160_000, "then 60 s");
        for _ in 0..50 {
            s = RULES.after_wrong(s, 0);
        }
        assert_eq!(s.locked_until_utc_ms, 3_600_000, "never more than an hour");
    }
}
