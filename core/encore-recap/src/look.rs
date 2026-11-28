//! Looking different every year (plan section 25, board 40): the pack, the seed,
//! the palette and each beat's layout. Everything comes from SHA-256 and integer
//! arithmetic, so it is the same on every platform.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::input::{PackInfo, PastRecap};
use crate::output::{PackRef, Palette};

/// `recap.variation.neverEitherOfLastTwoWhenPacksAtLeast`.
pub(crate) const AVOID_LAST_TWO_FROM_PACKS: usize = 6;
/// `recap.variation.leadHueMinDegreesFromLastYear`.
pub(crate) const LEAD_HUE_MIN_GAP: u16 = 60;
/// `recap.variation.palettePicksFromTopAlbums`.
pub(crate) const PALETTE_ALBUMS: usize = 3;
/// At most this many colours in the palette.
pub(crate) const PALETTE_MAX: usize = 6;
/// A colour whose max-min channel spread is below this is a grey: it has no
/// trustworthy hue and is never the lead.
pub(crate) const MIN_CHROMA: i32 = 40;

/// SHA-256 over parts joined by 0x1F.
pub(crate) fn sha(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            h.update([0x1F]);
        }
        h.update(p);
    }
    h.finalize().into()
}

fn u64_of(b: &[u8; 32]) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[..8]);
    u64::from_be_bytes(x)
}

/// A number in 0..n from the seed and a label ("layout/topSong").
pub(crate) fn pick(seed: u64, label: &str, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    (u64_of(&sha(&[&seed.to_be_bytes(), label.as_bytes()])) % n as u64) as usize
}

/// A u32 from the seed and a label.
pub(crate) fn pick_u32(seed: u64, label: &str) -> u32 {
    (u64_of(&sha(&[&seed.to_be_bytes(), label.as_bytes()])) >> 32) as u32
}

/// The pack: of the packs offered, never last year's (nor the year before's once
/// six or more are offered), the one whose hash with this profile and year is
/// lowest (rendezvous hashing: independent of the list's order, and stable when a
/// new pack is added unless the new one wins). If the rule leaves nothing, it is
/// relaxed: first the year-before rule, then last year's.
pub(crate) fn choose_pack(
    profile_id: &str,
    year: i32,
    packs: &[PackInfo],
    past: &[PastRecap],
) -> Option<PackRef> {
    // One entry per id, the highest version.
    let mut offered: BTreeMap<&str, u32> = BTreeMap::new();
    for p in packs {
        let v = offered.entry(p.id.as_str()).or_insert(p.version);
        *v = (*v).max(p.version);
    }
    if offered.is_empty() {
        return None;
    }
    let pack_of = |y: i32| -> Option<&str> {
        past.iter()
            .filter(|p| p.year == y)
            .filter_map(|p| p.pack.as_deref())
            .min()
    };
    let last = pack_of(year - 1);
    let before = pack_of(year - 2);
    let mut avoid: Vec<&str> = last.into_iter().collect();
    if offered.len() >= AVOID_LAST_TWO_FROM_PACKS {
        avoid.extend(before);
    }
    let rank = |id: &str| {
        sha(&[
            b"encore-recap/pack/v1",
            profile_id.as_bytes(),
            &year.to_be_bytes(),
            id.as_bytes(),
        ])
    };
    let choose = |avoid: &[&str]| {
        offered
            .iter()
            .filter(|(id, _)| !avoid.contains(id))
            .min_by_key(|(id, _)| (rank(id), **id))
            .map(|(id, v)| PackRef {
                id: (*id).to_string(),
                version: *v,
            })
    };
    choose(&avoid)
        .or_else(|| choose(&last.into_iter().collect::<Vec<_>>()))
        .or_else(|| choose(&[]))
}

/// The seed: profile ID, year and the chosen pack and its version (plan section
/// 10), so a pack that comes round again with a new version gets new layouts.
pub(crate) fn seed_of(profile_id: &str, year: i32, pack: Option<&PackRef>) -> u64 {
    let (id, ver) = pack.map_or(("", 0), |p| (p.id.as_str(), p.version));
    u64_of(&sha(&[
        b"encore-recap/seed/v1",
        profile_id.as_bytes(),
        &year.to_be_bytes(),
        id.as_bytes(),
        &ver.to_be_bytes(),
    ]))
}

/// "#rrggbb" (either case) to its channels.
pub(crate) fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((ch(0)?, ch(2)?, ch(4)?))
}

pub(crate) fn hex(c: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// Hue in whole degrees (0-359) and chroma (max - min channel), integers only.
pub(crate) fn hue_chroma(c: (u8, u8, u8)) -> (u16, i32) {
    let (r, g, b) = (i32::from(c.0), i32::from(c.1), i32::from(c.2));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    if d == 0 {
        return (0, 0);
    }
    // 60 * x / d rounded to nearest: floor((120x + d) / 2d).
    let sixty = |x: i32| (120 * x + d).div_euclid(2 * d);
    let h = if max == r {
        sixty(g - b)
    } else if max == g {
        sixty(b - r) + 120
    } else {
        sixty(r - g) + 240
    };
    (h.rem_euclid(360) as u16, d)
}

/// Degrees between two hues on the circle (0-180).
pub(crate) fn hue_gap(a: u16, b: u16) -> u16 {
    let d = a.abs_diff(b) % 360;
    d.min(360 - d)
}

/// A colour from hue, with fixed saturation and value (HSV), integers only.
pub(crate) fn from_hue(h: u16) -> (u8, u8, u8) {
    let (v, s) = (224u32, 176u32);
    let h = u32::from(h % 360);
    let region = h / 60;
    let rem = (h % 60) * 255 / 60;
    let p = v * (255 - s) / 255;
    let q = v * (255 - s * rem / 255) / 255;
    let t = v * (255 - s * (255 - rem) / 255) / 255;
    let (r, g, b) = match region {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (r as u8, g as u8, b as u8)
}

/// The palette from the top albums' colours, lead hue at least 60 degrees from
/// last year's, else the colour furthest from it. With no album colours at all, one
/// is made from the seed (always 60 to 180 degrees from last year's).
pub(crate) fn palette(album_colours: &[&[String]], last_hue: Option<u16>, seed: u64) -> Palette {
    let mut colours: Vec<(u8, u8, u8)> = Vec::new();
    for list in album_colours.iter().take(PALETTE_ALBUMS) {
        for c in list.iter().filter_map(|s| parse_hex(s)) {
            if !colours.contains(&c) {
                colours.push(c);
            }
        }
    }
    if colours.is_empty() {
        let lead_hue = match last_hue {
            // 62 to 298 degrees on: at least 62 away, so the rounding of the
            // made colour (at most 2 degrees) keeps it 60 or more.
            Some(h) => (h + LEAD_HUE_MIN_GAP + 2 + pick(seed, "palette/hue", 237) as u16) % 360,
            None => pick(seed, "palette/hue", 360) as u16,
        };
        let out: Vec<(u8, u8, u8)> = (0..4u16)
            .map(|i| from_hue((lead_hue + i * 90) % 360))
            .collect();
        return Palette {
            source: "seed".into(),
            colours: out.iter().map(|c| hex(*c)).collect(),
            lead: hex(out[0]),
            lead_hue: hue_chroma(out[0]).0,
            hue_gap_from_last_year: last_hue.map(|h| hue_gap(h, hue_chroma(out[0]).0)),
        };
    }
    let chromatic: Vec<usize> = (0..colours.len())
        .filter(|&i| hue_chroma(colours[i]).1 >= MIN_CHROMA)
        .collect();
    let lead_i = match (last_hue, chromatic.first()) {
        (_, None) => 0,
        (None, Some(&first)) => first,
        (Some(last), Some(&first)) => chromatic
            .iter()
            .copied()
            .find(|&i| hue_gap(hue_chroma(colours[i]).0, last) >= LEAD_HUE_MIN_GAP)
            .unwrap_or_else(|| {
                // The next best: the furthest; ties go to the earlier colour.
                let mut best = first;
                for &i in &chromatic {
                    if hue_gap(hue_chroma(colours[i]).0, last)
                        > hue_gap(hue_chroma(colours[best]).0, last)
                    {
                        best = i;
                    }
                }
                best
            }),
    };
    let lead = colours.remove(lead_i);
    colours.insert(0, lead);
    colours.truncate(PALETTE_MAX);
    let lead_hue = hue_chroma(lead).0;
    Palette {
        source: "albums".into(),
        colours: colours.iter().map(|c| hex(*c)).collect(),
        lead: hex(lead),
        lead_hue,
        hue_gap_from_last_year: last_hue.map(|h| hue_gap(h, lead_hue)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(id: &str) -> PackInfo {
        PackInfo {
            id: id.into(),
            version: 1,
        }
    }

    fn past(year: i32, pack: &str) -> PastRecap {
        PastRecap {
            year,
            pack: Some(pack.into()),
            lead_hue: None,
            listened_ms: None,
            personality_metric: None,
            personality_value: None,
        }
    }

    #[test]
    fn hues_are_integer_and_exact() {
        assert_eq!(hue_chroma((255, 0, 0)), (0, 255));
        assert_eq!(hue_chroma((0, 255, 0)).0, 120);
        assert_eq!(hue_chroma((0, 0, 255)).0, 240);
        assert_eq!(hue_chroma((255, 255, 0)).0, 60);
        assert_eq!(hue_chroma((255, 0, 128)).0, 330);
        assert_eq!(hue_chroma((90, 90, 90)), (0, 0));
        assert_eq!(hue_gap(350, 10), 20);
        assert_eq!(hue_gap(0, 180), 180);
        for h in (0..360).step_by(7) {
            let back = hue_chroma(from_hue(h)).0;
            assert!(hue_gap(back, h) <= 2, "{h} -> {back}");
        }
    }

    #[test]
    fn the_pack_never_repeats_last_year_and_ignores_list_order() {
        let packs = vec![pk("editorial"), pk("tide"), pk("bloom"), pk("glass")];
        let mut rev = packs.clone();
        rev.reverse();
        for y in 2020..2060 {
            let a = choose_pack("p-1", y, &packs, &[]).unwrap();
            assert_eq!(Some(a.clone()), choose_pack("p-1", y, &rev, &[]));
            let b = choose_pack("p-1", y, &packs, &[past(y - 1, &a.id)]).unwrap();
            assert_ne!(a.id, b.id);
        }
        // Six or more: neither of the last two.
        let six: Vec<PackInfo> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|s| pk(s))
            .collect();
        for y in 2020..2060 {
            let first = choose_pack("p-2", y, &six, &[]).unwrap().id;
            let second = choose_pack("p-2", y, &six, &[past(y - 1, &first)])
                .unwrap()
                .id;
            let c = choose_pack("p-2", y, &six, &[past(y - 1, &first), past(y - 2, &second)])
                .unwrap()
                .id;
            assert_ne!(c, first);
            assert_ne!(c, second);
        }
        // One pack only: it is used anyway.
        assert_eq!(
            choose_pack("p", 2026, &[pk("tide")], &[past(2025, "tide")])
                .unwrap()
                .id,
            "tide"
        );
        assert_eq!(choose_pack("p", 2026, &[], &[]), None);
    }

    #[test]
    fn the_lead_hue_keeps_its_distance() {
        let cols = vec!["#d04020".to_string(), "#c06030".into(), "#2060d0".into()];
        let p = palette(&[&cols], Some(10), 7);
        assert!(p.hue_gap_from_last_year.unwrap() >= 60);
        assert_eq!(p.lead, "#2060d0");
        // Nothing far enough: the furthest is used.
        let near = vec!["#d04020".to_string(), "#d06020".into()];
        let p = palette(&[&near], Some(10), 7);
        assert_eq!(p.lead, "#d06020");
        // No colours: from the seed, always far enough.
        for s in 0..200 {
            let p = palette(&[], Some(200), s);
            assert!(p.hue_gap_from_last_year.unwrap() >= 60, "{p:?}");
        }
    }
}
