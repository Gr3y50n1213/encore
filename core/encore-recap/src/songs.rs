//! The same song on two albums counts once.
//!
//! The song key rules live in `encore-sources/src/song_key.rs` (ADR core-0004). That
//! crate pulls in HTTP and file-format readers, far too heavy for this pure engine,
//! so the two tiny pieces needed here (splitting a `t1` key and the 2 s tolerance)
//! are repeated. The test below pins the same keys that crate's frozen-key test
//! pins, so a change on either side shows up.

use std::collections::BTreeMap;

/// `recap.counting.sameSongDurationToleranceSeconds`; the same value as
/// `encore_sources::song_key::DURATION_TOLERANCE_SECONDS`.
pub(crate) const DURATION_TOLERANCE_SECONDS: u64 = 2;

/// The hash and seconds of a `t1:<hash>:<seconds>` key, as in
/// `encore_sources::song_key::split_t1`.
pub(crate) fn split_t1(key: &str) -> Option<(&str, u64)> {
    let rest = key.strip_prefix("t1:")?;
    let (hash, secs) = rest.split_once(':')?;
    Some((hash, secs.parse().ok()?))
}

/// Whether two keys name the same song, as `encore_sources::song_key::same_song`.
#[cfg(test)]
pub(crate) fn same_song(a: &str, b: &str, tolerance_seconds: u64) -> bool {
    if a == b {
        return true;
    }
    match (split_t1(a), split_t1(b)) {
        (Some((ha, sa)), Some((hb, sb))) => ha == hb && sa.abs_diff(sb) <= tolerance_seconds,
        _ => false,
    }
}

/// Groups raw song keys into songs: key -> the group's anchor key.
///
/// `t1` keys with the same hash are walked in order of seconds; a group starts at
/// its shortest key and takes every key within the tolerance of that anchor. So
/// every pair inside a group is the same song by `same_song`, and the grouping does
/// not depend on the order keys arrive in. Other keys (`mb:`) stand alone.
pub(crate) fn group_keys<'a>(keys: impl Iterator<Item = &'a str>) -> BTreeMap<&'a str, &'a str> {
    let mut by_hash: BTreeMap<&str, Vec<(u64, &str)>> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for k in keys {
        match split_t1(k) {
            Some((hash, secs)) => by_hash.entry(hash).or_default().push((secs, k)),
            None => {
                out.insert(k, k);
            }
        }
    }
    for (_, mut list) in by_hash {
        list.sort_unstable();
        list.dedup();
        let mut anchor: Option<(u64, &str)> = None;
        for (secs, k) in list {
            match anchor {
                Some((a, ak)) if secs - a <= DURATION_TOLERANCE_SECONDS => {
                    out.insert(k, ak);
                }
                _ => {
                    anchor = Some((secs, k));
                    out.insert(k, k);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The frozen key from encore-sources' `fallback_key_is_frozen` test (Python
    // hashlib.sha256(b"ember cove\x1fgolden hour static")). If that test changes,
    // change this one with it.
    const FROZEN: &str = "t1:5a0ab3d41d0b7ae62cddb08fa411c1f1:222";

    #[test]
    fn t1_keys_split_like_encore_sources() {
        assert_eq!(
            split_t1(FROZEN),
            Some(("5a0ab3d41d0b7ae62cddb08fa411c1f1", 222))
        );
        assert_eq!(split_t1("mb:5b11f4ce-a62d-471e-81fc-a69a8278c7da"), None);
        assert_eq!(split_t1("t1:abc:x"), None);
        assert_eq!(DURATION_TOLERANCE_SECONDS, 2);
        // The same cases as encore-sources' `durations_within_the_tolerance_are_the_same_song`.
        let h = "t1:5a0ab3d41d0b7ae62cddb08fa411c1f1:";
        assert!(same_song(&format!("{h}200"), &format!("{h}202"), 2));
        assert!(!same_song(&format!("{h}200"), &format!("{h}203"), 2));
        assert!(!same_song(&format!("{h}200"), "t1:ffff:200", 2));
    }

    #[test]
    fn groups_are_order_free_and_every_pair_matches() {
        let keys = [
            "t1:aa:204",
            "t1:aa:200",
            "t1:aa:202",
            "t1:bb:200",
            "mb:x",
            "t1:aa:207",
        ];
        let g = group_keys(keys.iter().copied());
        assert_eq!(g["t1:aa:200"], "t1:aa:200");
        assert_eq!(g["t1:aa:202"], "t1:aa:200");
        assert_eq!(g["t1:aa:204"], "t1:aa:204");
        assert_eq!(g["t1:aa:207"], "t1:aa:207");
        assert_eq!(g["t1:bb:200"], "t1:bb:200");
        assert_eq!(g["mb:x"], "mb:x");
        let mut rev = keys;
        rev.reverse();
        assert_eq!(group_keys(rev.iter().copied()), g);
        for (k, a) in &g {
            assert!(same_song(k, a, DURATION_TOLERANCE_SECONDS));
        }
    }
}
