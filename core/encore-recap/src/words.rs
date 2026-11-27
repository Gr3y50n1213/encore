//! Every sentence the engine writes, in one place (English only for now).
//!
//! Sentences come from a bank of templates; the seed picks the variant. Numbers are
//! formatted here by hand (no locale): "41,208", "11 pm", "Mar 9". Copy is friendly,
//! never judgmental, and makes no mood or health claims (plan section 25).

use crate::look::pick;

/// A template id and its variants. `{name}` is replaced by the value of `name`.
pub(crate) struct Template {
    pub id: &'static str,
    pub variants: &'static [&'static str],
}

macro_rules! templates {
    ($($name:ident = $id:literal : [$($v:literal),+ $(,)?];)+) => {
        $(pub(crate) const $name: Template = Template { id: $id, variants: &[$($v),+] };)+
    };
}

templates! {
    INTRO = "intro": ["Your year in music.", "Here is how your year sounded."];
    INTRO_SINCE = "intro.since": ["Your year in music, since {since}.", "Since {since}, here is how it sounded."];
    MINUTES_MORE = "minutes.more": ["{pct}% more than {lastYear}.", "Up {pct}% on {lastYear}."];
    MINUTES_LESS = "minutes.less": ["A quieter year than {lastYear}, by {pct}%.", "{pct}% less than {lastYear}, and still plenty."];
    MINUTES_SAME = "minutes.same": ["About the same as {lastYear}.", "Right in step with {lastYear}."];
    MINUTES_PLAIN = "minutes.plain": ["That is {days} days and {hours} hours of music.", "Minute by minute, it added up."];
    MINUTES_SHORT = "minutes.short": ["A short year so far. Your top song was {title}.", "Just getting started. Your top song was {title}."];
    MINUTES_SHORT_FINAL = "minutes.shortFinal": ["A short year. Your top song was {title}.", "A small year, with one clear favourite: {title}."];
    PEAK_LATE = "peak.lateNight": ["Most of it after 10 pm.", "Most of it late at night."];
    PEAK_MORNING = "peak.morning": ["Most of it in the morning.", "Mornings did most of the work."];
    PEAK_AFTERNOON = "peak.afternoon": ["Most of it in the afternoon.", "Afternoons were your time."];
    PEAK_EVENING = "peak.evening": ["Most of it in the evening.", "Evenings were your time."];
    TOP_SONG = "topSong": ["You played it {plays} times.", "{plays} plays. The song of your year."];
    TOP_ARTIST = "topArtist": ["{plays} plays, {hours} hours.", "{hours} hours together this year."];
    TOP_ALBUM_THROUGH = "topAlbum.through": ["Played start to finish {n} times.", "Start to finish, {n} times over."];
    TOP_ALBUM = "topAlbum": ["{plays} plays from this one album.", "You came back to it {plays} times."];
    SOUND_DRIFT = "yourSound.drift": ["Your sound drifted toward {genre} after {month}.", "From {nextMonth} on, there was more {genre}."];
    SOUND_STEADY = "yourSound.steady": ["{genre} led the way all year.", "Mostly {genre}, with room for more."];
    CLOSER = "closer": ["{count} songs, ready to keep.", "Your year, in {count} songs."];
    CLOCK = "listeningClock": ["{weekday}s at {hour} were your time.", "Your busiest hour: {hour} on {weekday}s."];
    DISCOVERY = "biggestDiscovery": ["New to you in {month}. {plays} plays since.", "You found them in {month} and played them {plays} times."];
    COMEBACK = "comeback": ["Back after {gap} days: {plays} plays this year.", "An old favourite, back for {plays} plays."];
    OBSESSION = "obsessionWeek": ["The week of {date} belonged to {artist}.", "{plays} plays of {artist} in the week of {date}."];
    STREAK = "longestStreak": ["{days} days in a row with music, from {first} to {last}.", "Your longest run: {days} days in a row."];
    SEASONS = "songOfEachSeason": ["A different song for every season.", "Each season had its own song."];
    SEASONS_SHARED = "songOfEachSeason.shared": ["Some songs lasted more than one season.", "A few songs stayed for more than a season."];
    REPLAY_DAY = "mostReplayedDay": ["On {date} you played {title} {plays} times.", "{plays} times in one day: {title}, on {date}."];
    MOOD_UP = "moodArc.up": ["Your music got more energetic as the year went on.", "The second half of your year turned the energy up."];
    MOOD_DOWN = "moodArc.down": ["Your music got gentler as the year went on.", "The second half of your year was softer."];
    MOOD_LEVEL = "moodArc.level": ["Your music kept a steady energy all year.", "The energy held steady from start to end."];
    LYRIC = "mostSungLine": ["The line you jumped to most, {jumps} times, is in {title}.", "{jumps} times back to one line in {title}."];
    PERSONALITY_LAST_YEAR = "personality.lastYear": ["In {lastYear} it was {pct}%.", "It was {pct}% in {lastYear}."];
}

/// The year-line phrases for each trait (first letter capitalized when it leads).
pub(crate) const TRAIT_PHRASES: &[(&str, &[&str])] = &[
    ("lateNights", &["late nights", "after-dark hours"]),
    ("earlyMornings", &["early mornings", "first light"]),
    ("loudChoruses", &["loud choruses", "big choruses"]),
    ("softEdges", &["quiet songs", "soft edges"]),
    ("wholeAlbums", &["whole albums", "albums start to finish"]),
    ("newNames", &["new names", "fresh finds"]),
    ("onRepeat", &["songs on repeat", "the repeat button"]),
    ("everyDay", &["music every day", "a song a day"]),
    ("weekends", &["long weekends", "weekend soundtracks"]),
    ("topGenre", &["plenty of {genre}", "a lot of {genre}"]),
    ("fewFavourites", &["a few favourites", "familiar voices"]),
];
pub(crate) const YEAR_LINE_ONE: Template = Template {
    id: "yearLine.one",
    variants: &["A year of {a}.", "{A}, all year."],
};
pub(crate) const YEAR_LINE_TWO: Template = Template {
    id: "yearLine.two",
    variants: &["{A}, {b}."],
};
pub(crate) const YEAR_LINE_NONE: Template = Template {
    id: "yearLine.none",
    variants: &["Your year in music.", "A year, in songs."],
};

/// Fills a template variant chosen by the seed. Returns (template id, sentence).
pub(crate) fn say(seed: u64, t: &Template, args: &[(&str, String)]) -> (String, String) {
    let i = pick(seed, &format!("words/{}", t.id), t.variants.len());
    (format!("{}#{}", t.id, i), fill(t.variants[i], args))
}

pub(crate) fn fill(text: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(n, _)| *n == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The first ASCII letter in upper case (phrases are ours, and start in ASCII).
pub(crate) fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

/// 41208 -> "41,208".
pub(crate) fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub(crate) const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub(crate) const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// A month number (1-12) as its name.
pub(crate) fn month_name(m: u32) -> String {
    MONTHS[((m.clamp(1, 12)) - 1) as usize].to_string()
}

/// A day number as "Mar 9".
pub(crate) fn short_date(day: i64) -> String {
    let (_, m, d) = crate::time::civil_from_days(day);
    format!("{} {d}", &MONTHS[(m - 1) as usize][..3])
}

/// 23 -> "11 pm", 0 -> "12 am".
pub(crate) fn hour_name(h: u32) -> String {
    let h12 = match h % 12 {
        0 => 12,
        x => x,
    };
    format!("{h12} {}", if h < 12 { "am" } else { "pm" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_dates_are_formatted_without_a_locale() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(41_208), "41,208");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(hour_name(0), "12 am");
        assert_eq!(hour_name(12), "12 pm");
        assert_eq!(hour_name(23), "11 pm");
        assert_eq!(
            short_date(crate::time::days_from_civil(2026, 3, 9)),
            "Mar 9"
        );
        assert_eq!(
            fill("{a} and {b} {c}", &[("a", "x".into()), ("b", "y".into())]),
            "x and y {c}"
        );
        assert_eq!(capitalize("late nights"), "Late nights");
    }

    #[test]
    fn every_trait_has_phrases_and_no_template_is_empty() {
        for (_, p) in TRAIT_PHRASES {
            assert!(!p.is_empty());
        }
        for t in [INTRO, MINUTES_MORE, CLOCK, LYRIC, YEAR_LINE_TWO] {
            assert!(t.variants.iter().all(|v| !v.is_empty()));
        }
    }
}
