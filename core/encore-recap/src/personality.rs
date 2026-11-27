//! The listening personality: about twelve archetypes, each a rule on one number,
//! so Show the math can say exactly why ("Deep Diver: your top six artists were
//! 34% of plays"). Every threshold is a starting value (ADR core-0025).

use crate::output::Metric;
use crate::year::{PART_LATE_NIGHT, Year, permille};

/// The numbers behind the personality and the year line. None: not known.
#[derive(Debug, Default, Clone)]
pub(crate) struct Metrics {
    pub top_six: Option<u32>,
    pub top_artist: Option<u32>,
    pub new_artists: Option<u32>,
    pub from_album: Option<u32>,
    pub radio: Option<u32>,
    pub early_skips: Option<u32>,
    pub late_night: Option<u32>,
    pub early_morning: Option<u32>,
    pub weekend: Option<u32>,
    /// A count: genres with at least 3% of the plays that have a genre.
    pub genres: Option<u32>,
    pub every_day: Option<u32>,
    pub repeat: Option<u32>,
    /// Average energy of fingerprinted plays (permille), when at least half the
    /// plays have a fingerprint.
    pub energy: Option<u32>,
    /// The top genre's share of plays with a genre.
    pub top_genre: Option<u32>,
}

/// A field must be known for at least half the plays before a share of it counts.
fn half_known(known: u64, total: u64) -> bool {
    total > 0 && known * 2 >= total
}

/// The metrics of a year. Kid profiles get no late-night number at all.
pub(crate) fn metrics(
    y: &Year<'_>,
    baseline_known: bool,
    days_in_range: i64,
    kid: bool,
) -> Metrics {
    let total = y.total_plays;
    let top_n = |n: usize| -> u64 {
        y.artist_rank
            .iter()
            .take(n)
            .map(|&a| y.artists[a].plays)
            .sum()
    };
    let songs_top10: u64 = y.song_rank.iter().take(10).map(|&s| y.songs[s].plays).sum();
    let new_plays: u64 = y
        .artists
        .iter()
        .filter(|a| !y.before.contains_key(a.key))
        .map(|a| a.plays)
        .sum();
    let genre_count = y
        .genres
        .iter()
        .filter(|g| permille(g.plays, y.genre_plays) >= 30)
        .count() as u32;
    Metrics {
        top_six: (total > 0).then(|| permille(top_n(6), total)),
        top_artist: (total > 0).then(|| permille(top_n(1), total)),
        new_artists: (baseline_known && total > 0).then(|| permille(new_plays, total)),
        from_album: half_known(y.started_known, total)
            .then(|| permille(y.from_album, y.started_known)),
        radio: half_known(y.started_known, total).then(|| permille(y.from_radio, y.started_known)),
        early_skips: half_known(y.ended_known, total)
            .then(|| permille(y.early_skips, y.ended_known)),
        late_night: (y.total_ms > 0 && !kid)
            .then(|| permille(y.part_ms[PART_LATE_NIGHT], y.total_ms)),
        early_morning: (y.total_ms > 0).then(|| permille(y.early_morning_ms, y.total_ms)),
        weekend: (y.total_ms > 0).then(|| permille(y.weekend_ms, y.total_ms)),
        genres: half_known(y.genre_plays, total).then_some(genre_count),
        every_day: (total > 0)
            .then(|| permille(y.active_days.len() as u64, days_in_range.max(1) as u64)),
        repeat: (total > 0).then(|| permille(songs_top10, total)),
        energy: half_known(y.fp_plays, total).then(|| (y.fp_energy_sum / y.fp_plays.max(1)) as u32),
        top_genre: y
            .genre_rank
            .first()
            .filter(|_| half_known(y.genre_plays, total))
            .map(|&g| permille(y.genres[g].plays, y.genre_plays)),
    }
}

/// One archetype: its metric, its threshold, and the words for the card and the
/// math.
#[derive(Debug)]
pub(crate) struct Archetype {
    pub id: &'static str,
    pub name: &'static str,
    pub metric: &'static str,
    pub threshold: u32,
    pub count: bool,
    /// The card's sentence; `{pct}` or `{n}` is the metric.
    pub line: &'static str,
    /// Show the math; `{pct}` or `{n}` is the metric, `{rule}` the threshold.
    pub rule: &'static str,
    pub kid_ok: bool,
}

/// The archetypes in tie-break order.
pub(crate) const ARCHETYPES: &[Archetype] = &[
    Archetype {
        id: "deepDiver",
        name: "The Deep Diver",
        metric: "topSix",
        threshold: 300,
        count: false,
        line: "{pct}% of your year went to six artists.",
        rule: "Deep Diver: your top six artists were {pct}% of plays (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "explorer",
        name: "The Explorer",
        metric: "newArtists",
        threshold: 250,
        count: false,
        line: "{pct}% of your plays were artists new to you.",
        rule: "Explorer: artists new this year were {pct}% of plays (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "albumListener",
        name: "The Album Listener",
        metric: "fromAlbum",
        threshold: 600,
        count: false,
        line: "{pct}% of your plays started from an album.",
        rule: "Album Listener: {pct}% of plays started from an album (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "loyalist",
        name: "The Loyalist",
        metric: "topArtist",
        threshold: 150,
        count: false,
        line: "{pct}% of your plays went to one artist.",
        rule: "Loyalist: your top artist was {pct}% of plays (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "replayer",
        name: "The Replayer",
        metric: "repeat",
        threshold: 200,
        count: false,
        line: "Your top ten songs were {pct}% of your plays.",
        rule: "Replayer: your top ten songs were {pct}% of plays (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "nightOwl",
        name: "The Night Owl",
        metric: "lateNight",
        threshold: 350,
        count: false,
        line: "{pct}% of your listening was after 10 pm.",
        rule: "Night Owl: {pct}% of listened time was between 10 pm and 5 am (the rule: {rule}% or more).",
        kid_ok: false,
    },
    Archetype {
        id: "earlyRiser",
        name: "The Early Riser",
        metric: "earlyMorning",
        threshold: 300,
        count: false,
        line: "{pct}% of your listening was between 5 and 10 am.",
        rule: "Early Riser: {pct}% of listened time was between 5 and 10 am (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "weekender",
        name: "The Weekender",
        metric: "weekend",
        threshold: 450,
        count: false,
        line: "{pct}% of your listening was at the weekend.",
        rule: "Weekender: {pct}% of listened time was on Saturdays and Sundays (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "genreHopper",
        name: "The Genre Hopper",
        metric: "genres",
        threshold: 8,
        count: true,
        line: "{n} genres each had at least 3% of your plays.",
        rule: "Genre Hopper: {n} genres had 3% of plays or more (the rule: {rule} or more).",
        kid_ok: true,
    },
    Archetype {
        id: "everyDay",
        name: "The Everyday Listener",
        metric: "everyDay",
        threshold: 800,
        count: false,
        line: "You listened on {pct}% of the days.",
        rule: "Everyday Listener: music on {pct}% of the days counted (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "radioRider",
        name: "The Radio Rider",
        metric: "radio",
        threshold: 400,
        count: false,
        line: "{pct}% of your plays started from radio or a mix.",
        rule: "Radio Rider: {pct}% of plays started from radio or a mix (the rule: {rule}% or more).",
        kid_ok: true,
    },
    Archetype {
        id: "curator",
        name: "The Curator",
        metric: "earlySkips",
        threshold: 250,
        count: false,
        line: "You knew what you wanted: {pct}% of songs moved on in the first 30 s.",
        rule: "Curator: {pct}% of plays were skipped in the first 30 s (the rule: {rule}% or more).",
        kid_ok: true,
    },
];

/// Nothing stood out: the all-rounder, shown with the top-six share.
pub(crate) const ALL_ROUNDER: Archetype = Archetype {
    id: "allRounder",
    name: "The All-Rounder",
    metric: "topSix",
    threshold: 0,
    count: false,
    line: "No single habit ran your year: your top six artists were {pct}% of it.",
    rule: "All-Rounder: no other archetype's rule was met. Your top six artists were {pct}% of plays.",
    kid_ok: true,
};

impl Metrics {
    pub(crate) fn get(&self, id: &str) -> Option<u32> {
        match id {
            "topSix" => self.top_six,
            "topArtist" => self.top_artist,
            "newArtists" => self.new_artists,
            "fromAlbum" => self.from_album,
            "radio" => self.radio,
            "earlySkips" => self.early_skips,
            "lateNight" => self.late_night,
            "earlyMorning" => self.early_morning,
            "weekend" => self.weekend,
            "genres" => self.genres,
            "everyDay" => self.every_day,
            "repeat" => self.repeat,
            _ => None,
        }
    }

    /// Every known metric, for the detail page.
    pub(crate) fn all(&self) -> Vec<Metric> {
        METRIC_LABELS
            .iter()
            .filter_map(|(id, label)| {
                self.get(id).map(|v| Metric {
                    id: (*id).into(),
                    label: (*label).into(),
                    value: v,
                    unit: if *id == "genres" { "count" } else { "permille" }.into(),
                })
            })
            .collect()
    }
}

pub(crate) const METRIC_LABELS: &[(&str, &str)] = &[
    ("topSix", "Top six artists"),
    ("topArtist", "Top artist"),
    ("newArtists", "New artists"),
    ("fromAlbum", "Plays started from an album"),
    ("earlySkips", "Skipped in the first 30 s"),
    ("radio", "Started from radio or a mix"),
    ("lateNight", "Listening after 10 pm"),
    ("earlyMorning", "Listening from 5 to 10 am"),
    ("weekend", "Listening at the weekend"),
    ("genres", "Genres with 3% or more"),
    ("everyDay", "Days with music"),
    ("repeat", "Top ten songs"),
];

/// The supporting numbers shown on the card (board 39), without the chosen one.
pub(crate) const SUPPORTING: [&str; 3] = ["newArtists", "fromAlbum", "earlySkips"];

/// The archetype whose metric clears its threshold by the widest margin (relative
/// to the threshold); ties go to the earlier archetype.
pub(crate) fn choose(m: &Metrics, kid: bool) -> (&'static Archetype, u32) {
    let mut best: Option<(&Archetype, u32, i64)> = None;
    for a in ARCHETYPES {
        if kid && !a.kid_ok {
            continue;
        }
        let Some(v) = m.get(a.metric) else { continue };
        if v < a.threshold {
            continue;
        }
        let margin = (i64::from(v) - i64::from(a.threshold)) * 1000 / i64::from(a.threshold.max(1));
        if best.is_none_or(|(_, _, b)| margin > b) {
            best = Some((a, v, margin));
        }
    }
    match best {
        Some((a, v, _)) => (a, v),
        None => (&ALL_ROUNDER, m.top_six.unwrap_or(0)),
    }
}
