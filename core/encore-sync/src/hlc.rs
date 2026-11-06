//! Hybrid logical clock (Kulkarni et al., 2014), as plan section 5 asks.
//!
//! A timestamp is wall-clock milliseconds plus a counter that breaks ties, so events
//! from one device always order the way they happened, even when the wall clock
//! stands still or steps back, and events from several devices order close to real
//! time. Ties between devices are broken by the device ID, which the event stores.
//!
//! The text form is fixed width (`<13 digit ms>-<5 digit counter>`), so it sorts as a
//! string the same way it sorts as a number.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hlc {
    /// Wall-clock milliseconds since 1970 (UTC).
    pub physical_ms: i64,
    /// Ticks within the same millisecond.
    pub logical: u32,
}

impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:013}-{:05}", self.physical_ms, self.logical)
    }
}

impl FromStr for Hlc {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ms, logical) = s
            .split_once('-')
            .ok_or_else(|| format!("not a clock value: {s}"))?;
        Ok(Hlc {
            physical_ms: ms.parse().map_err(|_| format!("not a clock value: {s}"))?,
            logical: logical
                .parse()
                .map_err(|_| format!("not a clock value: {s}"))?,
        })
    }
}

/// One device's clock. Feed it the wall clock; it never goes backwards.
#[derive(Debug, Clone, Default)]
pub struct HlcClock {
    last: Hlc,
}

impl HlcClock {
    /// A clock that continues after `last` (the newest value stored on this device).
    pub fn resume(last: Hlc) -> Self {
        HlcClock { last }
    }

    /// A timestamp for a local event happening at `wall_ms`.
    pub fn tick(&mut self, wall_ms: i64) -> Hlc {
        self.last = if wall_ms > self.last.physical_ms {
            Hlc {
                physical_ms: wall_ms,
                logical: 0,
            }
        } else {
            Hlc {
                physical_ms: self.last.physical_ms,
                logical: self.last.logical + 1,
            }
        };
        self.last
    }

    /// Merge a timestamp received from another device (Phase 4 sync), so the next
    /// local event orders after it.
    pub fn observe(&mut self, remote: Hlc, wall_ms: i64) -> Hlc {
        let physical = wall_ms.max(self.last.physical_ms).max(remote.physical_ms);
        let logical = if physical == self.last.physical_ms && physical == remote.physical_ms {
            self.last.logical.max(remote.logical) + 1
        } else if physical == self.last.physical_ms {
            self.last.logical + 1
        } else if physical == remote.physical_ms {
            remote.logical + 1
        } else {
            0
        };
        self.last = Hlc {
            physical_ms: physical,
            logical,
        };
        self.last
    }

    pub fn last(&self) -> Hlc {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_wall_clock_when_it_moves_forward() {
        let mut c = HlcClock::default();
        assert_eq!(
            c.tick(1_000),
            Hlc {
                physical_ms: 1_000,
                logical: 0
            }
        );
        assert_eq!(
            c.tick(2_000),
            Hlc {
                physical_ms: 2_000,
                logical: 0
            }
        );
    }

    #[test]
    fn never_goes_backwards_when_the_wall_clock_stalls_or_steps_back() {
        let mut c = HlcClock::default();
        let a = c.tick(5_000);
        let b = c.tick(5_000);
        let d = c.tick(4_000); // the user set the clock back
        assert!(a < b && b < d);
        assert_eq!(
            d,
            Hlc {
                physical_ms: 5_000,
                logical: 2
            }
        );
    }

    #[test]
    fn resumes_after_the_newest_stored_value() {
        let mut c = HlcClock::resume(Hlc {
            physical_ms: 9_000,
            logical: 3,
        });
        assert_eq!(
            c.tick(8_000),
            Hlc {
                physical_ms: 9_000,
                logical: 4
            }
        );
    }

    #[test]
    fn orders_after_a_remote_event() {
        let mut c = HlcClock::default();
        c.tick(1_000);
        let merged = c.observe(
            Hlc {
                physical_ms: 3_000,
                logical: 7,
            },
            2_000,
        );
        assert_eq!(
            merged,
            Hlc {
                physical_ms: 3_000,
                logical: 8
            }
        );
        assert!(c.tick(2_500) > merged);
    }

    #[test]
    fn text_form_round_trips_and_sorts_like_the_value() {
        let a = Hlc {
            physical_ms: 1_762_400_000_000,
            logical: 2,
        };
        let b = Hlc {
            physical_ms: 1_762_400_000_000,
            logical: 10,
        };
        assert_eq!(a.to_string(), "1762400000000-00002");
        assert_eq!(a.to_string().parse::<Hlc>().unwrap(), a);
        assert!(a.to_string() < b.to_string());
        assert!("x".parse::<Hlc>().is_err());
    }
}
