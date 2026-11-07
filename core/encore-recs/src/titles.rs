//! Mix titles and subtitles (plan section 7.1, "Titles that fit but do not
//! shout"): 2 to 4 plain words in Title Case, the mood or moment first, no
//! emoji, no hype words, no puns, no artist names, no genre unless it dominates,
//! no two titles in a row sharing a main word, no title reused within 14 days
//! unless pinned, and never a fact the app doesn't know (no weather, no place, no
//! commute). Version 1 uses the fixed vocabulary only; model-written titles come
//! with the audio analysis (Phase 9) and must pass the same rules.

/// Mood words, by the family of the songs' genres (no audio features yet).
const MOODS: &[(&str, &[&str])] = &[
    (
        "calm",
        &["Quiet", "Calm", "Gentle", "Soft", "Slow", "Still", "Hushed"],
    ),
    (
        "warm",
        &["Warm", "Mellow", "Easy", "Golden", "Tender", "Cozy"],
    ),
    (
        "bright",
        &["Upbeat", "Bright", "Light", "Breezy", "Lively", "Easygoing"],
    ),
    (
        "loud",
        &["Loud", "Driving", "Heavy", "Raw", "Restless", "Electric"],
    ),
    (
        "groove",
        &["Smooth", "Steady", "Rolling", "Loose", "Laid-Back"],
    ),
    ("dark", &["Dark", "Moody", "Deep", "Hazy", "Blue"]),
    (
        "any",
        &["Familiar", "Favorite", "Trusted", "Everyday", "Old", "New"],
    ),
];

/// Moment words: (bucket, weekend) -> phrase. Buckets: 0 night .. 3 evening.
const MOMENTS: &[((u8, bool), &[&str])] = &[
    ((0, false), &["Late Nights", "Small Hours", "Midnight"]),
    ((0, true), &["Weekend Nights", "Late Nights"]),
    ((1, false), &["Mornings", "Early Hours", "Weekday Mornings"]),
    (
        (1, true),
        &["Weekend Mornings", "Slow Mornings", "Sunday Mornings"],
    ),
    ((2, false), &["Afternoons", "Weekday Afternoons", "Daytime"]),
    ((2, true), &["Weekend Afternoons", "Lazy Afternoons"]),
    ((3, false), &["Evenings", "Weeknight Evenings", "Dusk"]),
    ((3, true), &["Weekend Evenings", "Saturday Evenings"]),
];

/// For a mix not tied to a time of day.
const NOUNS: &[&str] = &[
    "Picks",
    "Favorites",
    "Comfort",
    "Rotation",
    "Selection",
    "Mix",
    "Company",
    "Songs",
    "Standards",
    "Returns",
    "Corners",
    "Finds",
];

const BANNED: &[&str] = &[
    "ultimate",
    "epic",
    "vibes",
    "vibe",
    "bangers",
    "banger",
    "best",
    "greatest",
    "hits",
    "insane",
    "fire",
    "lit",
    "killer",
    "legendary",
    "perfect",
    "essential",
    "must",
    "top",
    "weather",
    "rain",
    "rainy",
    "sunny day",
    "commute",
    "drive",
    "gym",
    "workout",
    "party",
];

/// Words that don't count as a title's main word.
const MINOR: &[&str] = &["and", "of", "the", "for", "in"];

fn family_of(genre: &str) -> &'static str {
    let g = genre.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| g.contains(w));
    if has(&[
        "ambient",
        "classical",
        "piano",
        "new age",
        "minimal",
        "soundtrack",
        "score",
        "drone",
    ]) {
        "calm"
    } else if has(&[
        "folk", "acoustic", "singer", "country", "jazz", "bossa", "lounge", "chill", "lo-fi",
        "lofi",
    ]) {
        "warm"
    } else if has(&[
        "metal",
        "punk",
        "hard",
        "grunge",
        "rock",
        "industrial",
        "hardcore",
    ]) {
        "loud"
    } else if has(&[
        "hip hop", "hip-hop", "rap", "r&b", "soul", "funk", "reggae", "dub", "trip",
    ]) {
        "groove"
    } else if has(&[
        "pop", "dance", "house", "disco", "electro", "edm", "synth", "indie",
    ]) {
        "bright"
    } else if has(&["goth", "dark", "post-punk", "shoegaze", "doom", "blues"]) {
        "dark"
    } else {
        "any"
    }
}

/// The mood family most of these genres belong to ("any" when none dominates).
pub fn dominant_family(genres: &[&str]) -> &'static str {
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for g in genres {
        let f = family_of(g);
        match counts.iter_mut().find(|(k, _)| *k == f) {
            Some(c) => c.1 += 1,
            None => counts.push((f, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    match counts.first() {
        Some((f, c)) if *f != "any" && *c * 2 >= genres.len() => f,
        _ => "any",
    }
}

fn main_words(title: &str) -> Vec<String> {
    title
        .split(' ')
        .map(str::to_lowercase)
        .filter(|w| !MINOR.contains(&w.as_str()))
        .collect()
}

/// Whether a title follows every rule, given the other titles in the row and the
/// titles shown in the last 14 days.
pub fn check(title: &str, row: &[String], recent: &[String]) -> Result<(), &'static str> {
    let words: Vec<&str> = title.split(' ').collect();
    if !(2..=4).contains(&words.len()) {
        return Err("2 to 4 words");
    }
    for w in &words {
        let lower = w.to_lowercase();
        if MINOR.contains(&lower.as_str()) {
            continue;
        }
        let first = w.chars().next().unwrap_or(' ');
        if !first.is_uppercase() {
            return Err("Title Case");
        }
        if !w
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
        {
            return Err("letters only (no emoji, digits or symbols)");
        }
    }
    let lower = title.to_lowercase();
    if BANNED
        .iter()
        .any(|b| lower.split(' ').any(|w| w == *b) || (b.contains(' ') && lower.contains(b)))
    {
        return Err("a banned word");
    }
    let mine = main_words(title);
    if row
        .iter()
        .any(|t| main_words(t).iter().any(|w| mine.contains(w)))
    {
        return Err("shares a main word with another title in the row");
    }
    if recent.iter().any(|t| t.eq_ignore_ascii_case(title)) {
        return Err("used in the last 14 days");
    }
    Ok(())
}

/// A title for a mix: its mood family, and its time of day when it has one.
/// Tries the vocabulary in a fixed order until one passes the rules.
pub fn make(
    family: &str,
    moment: Option<(u8, bool)>,
    row: &[String],
    recent: &[String],
    salt: u64,
) -> String {
    let moods: &[&str] = MOODS
        .iter()
        .find(|(f, _)| *f == family)
        .map_or(MOODS[MOODS.len() - 1].1, |(_, w)| w);
    let any: &[&str] = MOODS[MOODS.len() - 1].1;
    let tails: Vec<&str> = match moment {
        Some(m) => MOMENTS
            .iter()
            .filter(|(k, _)| *k == m || (k.0 == m.0 && k.1 != m.1))
            .flat_map(|(_, w)| w.iter().copied())
            .collect(),
        None => NOUNS.to_vec(),
    };
    let start = (salt % moods.len().max(1) as u64) as usize;
    for mood in moods
        .iter()
        .cycle()
        .skip(start)
        .take(moods.len())
        .chain(any.iter())
    {
        for tail in tails.iter().chain(NOUNS.iter()) {
            let t = format!("{mood} {tail}");
            if check(&t, row, recent).is_ok() {
                return t;
            }
        }
    }
    // Every combination is taken (a very long row): still a valid shape.
    format!(
        "Your Mix {}",
        ["One", "Two", "Three", "Four", "Five", "Six"][row.len().min(5)]
    )
}

/// "Mostly A, B and 12 more": the two artists with the most songs, then a count of
/// the rest (artists, never claims).
pub fn subtitle(artists: &[&str]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for a in artists {
        match counts.iter_mut().find(|(k, _)| k == a) {
            Some(c) => c.1 += 1,
            None => counts.push((a, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    match counts.len() {
        0 => String::new(),
        1 => format!("Mostly {}", counts[0].0),
        2 => format!("Mostly {} and {}", counts[0].0, counts[1].0),
        n => format!("Mostly {}, {} and {} more", counts[0].0, counts[1].0, n - 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rules_hold_for_every_title_the_vocabulary_makes() {
        let mut made = 0;
        for (family, _) in MOODS {
            for moment in [
                None,
                Some((0, false)),
                Some((1, true)),
                Some((2, false)),
                Some((3, true)),
            ] {
                let mut row: Vec<String> = Vec::new();
                for salt in 0..5u64 {
                    let t = make(family, moment, &row, &[], salt);
                    assert!(check(&t, &row, &[]).is_ok(), "{t:?} breaks a rule");
                    row.push(t);
                    made += 1;
                }
            }
        }
        assert!(made >= 150, "a few hundred titles checked");
    }

    #[test]
    fn rules_refuse_what_they_should() {
        assert!(check("Quiet", &[], &[]).is_err());
        assert!(check("One Two Three Four Five", &[], &[]).is_err());
        assert!(check("quiet Evenings", &[], &[]).is_err());
        assert!(check("Epic Evenings", &[], &[]).is_err());
        assert!(check("Rainy Commute", &[], &[]).is_err());
        assert!(check("Quiet 🌙 Nights", &[], &[]).is_err());
        assert!(check("Quiet Evenings", &["Quiet Mornings".into()], &[]).is_err());
        assert!(check("Quiet Evenings", &[], &["quiet evenings".into()]).is_err());
        assert!(check("Slow Sunday Mornings", &[], &[]).is_ok());
        assert!(check("Warm and Mellow", &[], &[]).is_ok());
    }

    #[test]
    fn families_and_subtitles() {
        assert_eq!(dominant_family(&["Ambient", "Classical", "Rock"]), "calm");
        assert_eq!(dominant_family(&["Rock", "Rock", "Pop"]), "loud");
        assert_eq!(
            dominant_family(&["Rock", "Pop"]),
            "bright",
            "a tie goes alphabetically"
        );
        assert_eq!(dominant_family(&["Polka"]), "any");
        assert_eq!(
            subtitle(&["A", "B", "A", "C", "D"]),
            "Mostly A, B and 2 more"
        );
        assert_eq!(subtitle(&["A"]), "Mostly A");
        assert_eq!(subtitle(&[]), "");
    }
}
