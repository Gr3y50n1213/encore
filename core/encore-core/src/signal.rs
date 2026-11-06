//! The skip signal (plan section 21, board 42): how one play reads, decided once
//! here so every surface (Next, a swipe, a headset, a car, the lock screen) teaches
//! the same lesson. Phase 5 weighs these; Phase 3 decides and stores them.
//!
//! "A wrong lesson is worse than no lesson": interruptions, errors, accidents,
//! system ends and plays too soon after the last never become a negative signal.
//! The raw end reason and the time listened stay stored beside the signal, so it
//! can be worked out again if these rules change.

use serde::{Deserialize, Serialize};

use crate::playlog::EndReason;
use crate::queue::ChosenBy;

/// How a play reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Signal {
    /// Played to the end.
    Finished,
    /// Skipped after most of it (past `lateSkipPercent`): mildly positive.
    SkippedLate,
    /// Skipped between the early-skip boundary and `lateSkipPercent`.
    SkippedMid,
    /// An app-chosen song skipped before the early-skip boundary: the key negative.
    SkippedEarly,
    /// A song the listener picked, skipped early: barely counts (often browsing).
    SkippedUserPick,
    /// Ended by, or skipped just after, an unplug, an output switch, a call or an
    /// assistant: never negative.
    Interruption,
    /// Failed to load or play, or the app moved on by itself: never negative.
    Error,
    /// Stopped within `accidentSeconds` of starting: never negative.
    Accident,
    /// A negative within `tooSoonHours` of hearing it: "too soon", not dislike.
    TooSoon,
    /// A sleep timer, a closed app, a lost connection, a crash: never negative.
    System,
    /// Paused and never resumed: counts for nothing.
    PausedAbandoned,
}

impl Signal {
    pub fn as_str(self) -> &'static str {
        match self {
            Signal::Finished => "finished",
            Signal::SkippedLate => "skippedLate",
            Signal::SkippedMid => "skippedMid",
            Signal::SkippedEarly => "skippedEarly",
            Signal::SkippedUserPick => "skippedUserPick",
            Signal::Interruption => "interruption",
            Signal::Error => "error",
            Signal::Accident => "accident",
            Signal::TooSoon => "tooSoon",
            Signal::System => "system",
            Signal::PausedAbandoned => "pausedAbandoned",
        }
    }

    /// Whether this would teach "less like this".
    pub fn is_negative(self) -> bool {
        matches!(
            self,
            Signal::SkippedMid | Signal::SkippedEarly | Signal::SkippedUserPick
        )
    }
}

/// What moved on from a song.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipOrigin {
    Button,
    Swipe,
    Headset,
    Car,
    LockScreen,
    Keyboard,
    /// The song ended, failed, or the app moved on by itself.
    Auto,
}

impl SkipOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            SkipOrigin::Button => "button",
            SkipOrigin::Swipe => "swipe",
            SkipOrigin::Headset => "headset",
            SkipOrigin::Car => "car",
            SkipOrigin::LockScreen => "lockScreen",
            SkipOrigin::Keyboard => "keyboard",
            SkipOrigin::Auto => "auto",
        }
    }
}

/// Starting values from `design/tokens.json` (`smartPicks`, `gestures.playerSwipe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    pub early_skip_ms: u64,
    pub late_skip_percent: u64,
    pub accident_ms: u64,
    pub interruption_window_ms: i64,
    pub too_soon_ms: i64,
    /// A song reached by a skip and skipped again this soon was only passed over.
    pub coalesce_ms: i64,
}

/// What is known about a play when it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub end: EndReason,
    pub listened_ms: u64,
    pub duration_ms: u64,
    pub chosen_by: ChosenBy,
    /// Since the last unplug, output switch, call or assistant; None if none.
    pub ms_since_interruption: Option<i64>,
    /// Heard (past the early-skip boundary) within `too_soon_ms` before this play.
    pub heard_recently: bool,
}

pub fn classify(f: &Facts, r: &Rules) -> Signal {
    let base = match f.end {
        EndReason::Finished => return Signal::Finished,
        EndReason::Error => return Signal::Error,
        EndReason::Interrupted => return Signal::Interruption,
        EndReason::EndedBySystem => return Signal::System,
        EndReason::PausedAbandoned => return Signal::PausedAbandoned,
        EndReason::Skipped => {
            if f.ms_since_interruption
                .is_some_and(|ms| (0..=r.interruption_window_ms).contains(&ms))
            {
                return Signal::Interruption;
            }
            if f.listened_ms < r.accident_ms {
                return Signal::Accident;
            }
            let late = f.duration_ms > 0
                && f.listened_ms.saturating_mul(100)
                    >= f.duration_ms.saturating_mul(r.late_skip_percent);
            if late {
                Signal::SkippedLate
            } else if f.chosen_by == ChosenBy::User {
                Signal::SkippedUserPick
            } else if f.listened_ms < r.early_skip_ms {
                Signal::SkippedEarly
            } else {
                Signal::SkippedMid
            }
        }
    };
    if base.is_negative() && f.heard_recently {
        Signal::TooSoon
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules = Rules {
        early_skip_ms: 30_000,
        late_skip_percent: 80,
        accident_ms: 2_000,
        interruption_window_ms: 3_000,
        too_soon_ms: 7_200_000,
        coalesce_ms: 600,
    };

    fn facts(end: EndReason, listened_ms: u64, chosen_by: ChosenBy) -> Facts {
        Facts {
            end,
            listened_ms,
            duration_ms: 200_000,
            chosen_by,
            ms_since_interruption: None,
            heard_recently: false,
        }
    }

    /// Every rule of plan section 21, one row each.
    #[test]
    fn every_section_21_rule() {
        use ChosenBy::{App, User};
        use EndReason::*;
        let cases: [(Facts, Signal, &str); 16] = [
            (
                facts(Finished, 200_000, App),
                Signal::Finished,
                "played to the end",
            ),
            (
                facts(Finished, 1_000, App),
                Signal::Finished,
                "finished after a seek to the end",
            ),
            (
                facts(Skipped, 10_000, App),
                Signal::SkippedEarly,
                "app-chosen, skipped in the first 30 s",
            ),
            (
                facts(Skipped, 29_999, App),
                Signal::SkippedEarly,
                "just inside the boundary",
            ),
            (
                facts(Skipped, 30_000, App),
                Signal::SkippedMid,
                "the boundary itself is not early",
            ),
            (
                facts(Skipped, 120_000, App),
                Signal::SkippedMid,
                "between 30 s and 80%",
            ),
            (
                facts(Skipped, 160_000, App),
                Signal::SkippedLate,
                "after 80%",
            ),
            (
                facts(Skipped, 10_000, User),
                Signal::SkippedUserPick,
                "a song you picked barely counts",
            ),
            (
                facts(Skipped, 170_000, User),
                Signal::SkippedLate,
                "late is late, whoever picked it",
            ),
            (
                facts(Skipped, 1_999, App),
                Signal::Accident,
                "stopped under 2 s",
            ),
            (facts(Skipped, 0, User), Signal::Accident, "nothing heard"),
            (facts(Error, 50_000, App), Signal::Error, "failed to load"),
            (
                facts(Interrupted, 40_000, App),
                Signal::Interruption,
                "ended by an unplug or a call",
            ),
            (
                facts(EndedBySystem, 90_000, App),
                Signal::System,
                "sleep timer, closed app, crash",
            ),
            (
                facts(PausedAbandoned, 90_000, App),
                Signal::PausedAbandoned,
                "paused, never resumed",
            ),
            (
                Facts {
                    duration_ms: 0,
                    ..facts(Skipped, 10_000, App)
                },
                Signal::SkippedEarly,
                "unknown length: never 'late'",
            ),
        ];
        for (f, want, why) in cases {
            assert_eq!(classify(&f, &RULES), want, "{why}");
        }
    }

    #[test]
    fn a_skip_right_after_an_interruption_is_not_a_skip() {
        let mut f = facts(EndReason::Skipped, 10_000, ChosenBy::App);
        f.ms_since_interruption = Some(2_999);
        assert_eq!(classify(&f, &RULES), Signal::Interruption);
        f.ms_since_interruption = Some(3_000);
        assert_eq!(
            classify(&f, &RULES),
            Signal::Interruption,
            "the window includes 3 s"
        );
        f.ms_since_interruption = Some(3_001);
        assert_eq!(classify(&f, &RULES), Signal::SkippedEarly);
        f.ms_since_interruption = Some(-5);
        assert_eq!(
            classify(&f, &RULES),
            Signal::SkippedEarly,
            "a clock step back is not an interruption"
        );
    }

    #[test]
    fn negatives_too_soon_after_hearing_it_are_too_soon() {
        for (listened, chosen, want) in [
            (10_000, ChosenBy::App, Signal::TooSoon),
            (100_000, ChosenBy::App, Signal::TooSoon),
            (10_000, ChosenBy::User, Signal::TooSoon),
            // Not negatives: unchanged.
            (180_000, ChosenBy::App, Signal::SkippedLate),
            (1_000, ChosenBy::App, Signal::Accident),
        ] {
            let mut f = facts(EndReason::Skipped, listened, chosen);
            f.heard_recently = true;
            assert_eq!(classify(&f, &RULES), want, "{listened} {chosen:?}");
        }
        let mut f = facts(EndReason::Finished, 200_000, ChosenBy::App);
        f.heard_recently = true;
        assert_eq!(classify(&f, &RULES), Signal::Finished);
    }

    #[test]
    fn huge_numbers_do_not_overflow() {
        let f = Facts {
            listened_ms: u64::MAX,
            duration_ms: u64::MAX,
            ..facts(EndReason::Skipped, 0, ChosenBy::App)
        };
        let _ = classify(&f, &RULES);
    }
}
