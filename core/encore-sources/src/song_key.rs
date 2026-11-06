//! The stable song key (plan section 5; docs/adr/core-0004-song-key.md).
//!
//! The key outlives every library: play events store it, the year recap counts by it,
//! and duplicates across servers merge by it. It must never change for a song, so
//! the rules here are frozen. A change needs a new key version and a migration.
//!
//! - Tagged with a MusicBrainz recording ID: `mb:<the ID, lower case>`.
//! - Otherwise: `t1:<hash>:<seconds>`, where `hash` is the first 32 hex digits of
//!   SHA-256 over the normalized artist, a 0x1F separator and the normalized title,
//!   and `seconds` is the duration rounded to whole seconds.
//!
//! Normalization: Unicode NFKC, lower case, whitespace trimmed and collapsed to one
//! space. Punctuation, accents and scripts are kept: "Song (Live)" and "Song" are
//! different recordings, and "Ölmez" is not "Olmez".
//!
//! Two `t1` keys are the same song when the hashes match and the durations differ by
//! no more than `recap.counting.sameSongDurationToleranceSeconds` (2 s).

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// The key of one song.
pub fn song_key(
    mbid_recording: Option<&str>,
    artist: &str,
    title: &str,
    duration_ms: u64,
) -> String {
    if let Some(mbid) = mbid_recording.map(str::trim).filter(|m| is_mbid(m)) {
        return format!("mb:{}", mbid.to_ascii_lowercase());
    }
    let mut hasher = Sha256::new();
    hasher.update(normalize(artist).as_bytes());
    hasher.update([0x1F]);
    hasher.update(normalize(title).as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("t1:{hex}:{}", (duration_ms + 500) / 1000)
}

/// Whether two keys name the same song, allowing `tolerance_seconds` between the
/// durations of `t1` keys.
pub fn same_song(a: &str, b: &str, tolerance_seconds: u64) -> bool {
    if a == b {
        return true;
    }
    match (split_t1(a), split_t1(b)) {
        (Some((ha, sa)), Some((hb, sb))) => ha == hb && sa.abs_diff(sb) <= tolerance_seconds,
        _ => false,
    }
}

fn split_t1(key: &str) -> Option<(&str, u64)> {
    let rest = key.strip_prefix("t1:")?;
    let (hash, secs) = rest.split_once(':')?;
    Some((hash, secs.parse().ok()?))
}

/// NFKC, lower case, trimmed, inner whitespace collapsed to one space.
pub fn normalize(s: &str) -> String {
    let nfkc: String = s.nfkc().collect();
    nfkc.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A MusicBrainz ID is a UUID: 8-4-4-4-12 hex digits.
fn is_mbid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MBID: &str = "5B11F4CE-A62D-471E-81FC-A69A8278C7DA";

    #[test]
    fn musicbrainz_id_wins_and_is_lower_case() {
        assert_eq!(
            song_key(Some(MBID), "Anyone", "Anything", 1),
            "mb:5b11f4ce-a62d-471e-81fc-a69a8278c7da"
        );
    }

    #[test]
    fn a_malformed_musicbrainz_id_falls_back_to_the_tags() {
        let fallback = song_key(None, "Artist", "Title", 200_000);
        assert_eq!(
            song_key(Some("not-an-id"), "Artist", "Title", 200_000),
            fallback
        );
        assert_eq!(song_key(Some(""), "Artist", "Title", 200_000), fallback);
    }

    #[test]
    fn fallback_key_is_frozen() {
        // If this value changes, every stored play event loses its song. Never update
        // it; add a new key version instead.
        assert_eq!(
            song_key(None, "Ember Cove", "Golden Hour Static", 222_400),
            song_key(None, "ember  cove ", "GOLDEN HOUR STATIC", 222_100),
        );
        assert!(song_key(None, "Ember Cove", "Golden Hour Static", 222_400).starts_with("t1:"));
        assert!(song_key(None, "Ember Cove", "Golden Hour Static", 222_400).ends_with(":222"));
        assert_eq!(
            song_key(None, "Ember Cove", "Golden Hour Static", 222_400),
            // Checked independently: Python hashlib.sha256(b"ember cove\x1fgolden hour static").
            "t1:5a0ab3d41d0b7ae62cddb08fa411c1f1:222"
        );
    }

    #[test]
    fn normalization_folds_case_width_and_spaces_but_keeps_meaning() {
        assert_eq!(normalize("  Hello\t World  "), "hello world");
        assert_eq!(normalize("ＡＢＣ"), "abc"); // full-width letters (NFKC)
        assert_eq!(normalize("ÉCOLE"), "école");
        assert_ne!(normalize("Ölmez"), normalize("Olmez"));
        assert_ne!(normalize("Song (Live)"), normalize("Song"));
        assert_eq!(normalize("東京"), "東京");
        assert_eq!(normalize("שלום  עולם"), "שלום עולם"); // right to left
    }

    #[test]
    fn composed_and_decomposed_accents_give_one_key() {
        let composed = "Beyonc\u{e9}";
        let decomposed = "Beyonce\u{301}";
        assert_eq!(
            song_key(None, composed, "Halo", 261_000),
            song_key(None, decomposed, "Halo", 261_000)
        );
    }

    #[test]
    fn artist_and_title_cannot_run_together() {
        assert_ne!(
            song_key(None, "ab", "c", 1000),
            song_key(None, "a", "bc", 1000)
        );
    }

    #[test]
    fn durations_within_the_tolerance_are_the_same_song() {
        let a = song_key(None, "A", "T", 200_000);
        let b = song_key(None, "A", "T", 202_000);
        let c = song_key(None, "A", "T", 203_000);
        assert!(same_song(&a, &b, 2));
        assert!(!same_song(&a, &c, 2));
        assert!(!same_song(&a, &song_key(None, "A", "U", 200_000), 2));
        let mb = song_key(Some(MBID), "A", "T", 200_000);
        assert!(same_song(&mb, &mb, 2));
        assert!(!same_song(&mb, &a, 2));
    }
}
