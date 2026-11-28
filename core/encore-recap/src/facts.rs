//! The candidate facts: one function per beat type. A fixed beat returns its card;
//! a rotating beat returns a candidate with how far it sits from this profile's own
//! baseline (`distance`) and how personal it is (`personal`), both 0 to 1000, or
//! nothing when the data is missing or too thin. Nothing is ever faked.
//!
//! Not built: the household deep cut (plan section 25, opt-in household learning).
//! It needs other profiles' plays and both profiles' consent, which this engine is
//! never given. TODO(phase 11 household): add it behind an explicit input; until
//! then it is never emitted.

use std::collections::BTreeMap;

use crate::input::{PastRecap, RecapInput};
use crate::output::{
    AlbumEntry, ArtistEntry, BeatData, Count, Drift, GenreEntry, Math, MonthTally, Role, Season,
    SongEntry, SoundPoint,
};
use crate::personality::{self, Metrics, SUPPORTING};
use crate::time::{MS_PER_DAY, MS_PER_HOUR, format_day};
use crate::words::{self, say, short_date, thousands};
use crate::year::{PART_LATE_NIGHT, PART_NAMES, Year, part_of_day, percent, permille};

/// `recap.data.streakMinDays`.
pub(crate) const STREAK_MIN_DAYS: u64 = 7;
/// `recap.data.discoveryMinPlays`.
pub(crate) const DISCOVERY_MIN_PLAYS: u64 = 50;
/// Comeback: plays before this year, a silence of this many days, and plays now.
pub(crate) const COMEBACK_MIN_PLAYS_BEFORE: u64 = 20;
pub(crate) const COMEBACK_MIN_GAP_DAYS: i64 = 180;
pub(crate) const COMEBACK_MIN_PLAYS: u64 = 20;
/// Obsession week: one artist's plays in one week, and their share of that week.
pub(crate) const OBSESSION_MIN_PLAYS: u64 = 25;
pub(crate) const OBSESSION_MIN_SHARE: u32 = 400;
/// Most replayed day: one song this many times in one day.
pub(crate) const REPLAY_DAY_MIN_PLAYS: u64 = 5;
/// Song of each season: seasons with this many plays, and how many such seasons.
pub(crate) const SEASON_MIN_PLAYS: u64 = 10;
pub(crate) const SEASONS_MIN: usize = 3;
/// Mood arc: half the plays fingerprinted, and six months with 10 such plays.
pub(crate) const MOOD_MIN_MONTH_PLAYS: u64 = 10;
pub(crate) const MOOD_MIN_MONTHS: usize = 6;
/// Mood arc: a change under this (permille of energy) reads as steady.
pub(crate) const MOOD_STEADY_UNDER: u32 = 50;
/// Most-sung line: jumps to one line.
pub(crate) const LYRIC_MIN_JUMPS: u64 = 3;
/// Your sound: a genre's rise (permille points) that counts as a drift.
pub(crate) const DRIFT_MIN_RISE: u32 = 80;
/// The lists on cards and on the detail page.
pub(crate) const CARD_TOP: usize = 5;
pub(crate) const DETAIL_TOP: usize = 50;
pub(crate) const PLAYLIST_SONGS: usize = 100;
/// Names longer than this are cut (with an ellipsis) to keep the document small.
pub(crate) const MAX_NAME_CHARS: usize = 100;
/// A personality supporting line and the minutes "peak" sentence need this share.
pub(crate) const PEAK_SENTENCE_MIN: u32 = 350;

/// What every beat function reads.
#[derive(Debug)]
pub(crate) struct Ctx<'a, 'y> {
    pub input: &'a RecapInput,
    pub y: &'y Year<'a>,
    pub seed: u64,
    pub baseline_known: bool,
    pub first_day: i64,
    pub last_day: i64,
    pub last_year: Option<&'a PastRecap>,
    pub so_far: bool,
    pub metrics: Metrics,
}

/// A beat before ordering and layout.
#[derive(Debug)]
pub(crate) struct Candidate {
    pub id: &'static str,
    pub role: Role,
    pub fixed: bool,
    pub list: bool,
    pub template: String,
    pub text: String,
    pub data: BeatData,
    pub math: Math,
    pub distance: i64,
    pub personal: i64,
    /// Set when a rotating beat is chosen.
    pub score: Option<i64>,
    pub themes: &'static [&'static str],
}

fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_NAME_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_NAME_CHARS - 1).collect();
    out.push('\u{2026}');
    out
}

fn count(label: &str, value: u64, unit: &str) -> Count {
    Count {
        label: label.into(),
        value: i64::try_from(value).unwrap_or(i64::MAX),
        unit: unit.into(),
    }
}

fn arg(name: &'static str, v: impl Into<String>) -> (&'static str, String) {
    (name, v.into())
}

/// A sentence made of two templates ("12% more than 2025." + "Most of it after
/// 10 pm.").
fn join(a: (String, String), b: Option<(String, String)>) -> (String, String) {
    match b {
        Some((bi, bt)) => (format!("{}+{}", a.0, bi), format!("{} {}", a.1, bt)),
        None => a,
    }
}

const RANK_RULE: &str = "Ranked by plays, then listened time. The same song on two albums counts once (MusicBrainz ID, else artist, title and length within 2 s).";

impl Ctx<'_, '_> {
    pub(crate) fn song(&self, s: usize) -> SongEntry {
        let a = &self.y.songs[s];
        SongEntry {
            rank: self.y.song_pos[s] as u32 + 1,
            key: a.key.to_string(),
            title: clip(a.title),
            artist: clip(a.artist),
            album: a.album.map(clip),
            plays: a.plays,
            listened_ms: a.ms,
        }
    }

    /// The artist at a place (0-based) in the year's ranking.
    fn artist_at(&self, rank0: usize) -> ArtistEntry {
        let a = &self.y.artists[self.y.artist_rank[rank0]];
        ArtistEntry {
            rank: rank0 as u32 + 1,
            key: a.key.to_string(),
            name: clip(a.name),
            plays: a.plays,
            listened_ms: a.ms,
        }
    }

    pub(crate) fn top_songs(&self, n: usize) -> Vec<SongEntry> {
        self.y
            .song_rank
            .iter()
            .take(n)
            .map(|&s| self.song(s))
            .collect()
    }

    pub(crate) fn top_artists(&self, n: usize) -> Vec<ArtistEntry> {
        (0..self.y.artist_rank.len().min(n))
            .map(|r| self.artist_at(r))
            .collect()
    }

    pub(crate) fn top_albums(&self, n: usize) -> Vec<AlbumEntry> {
        self.y
            .album_rank
            .iter()
            .take(n)
            .enumerate()
            .map(|(r, &i)| {
                let a = &self.y.albums[i];
                AlbumEntry {
                    rank: r as u32 + 1,
                    key: a.key.to_string(),
                    name: clip(a.name),
                    artist: clip(a.artist),
                    plays: a.plays,
                    listened_ms: a.ms,
                }
            })
            .collect()
    }

    pub(crate) fn months(&self) -> Vec<MonthTally> {
        (0..12)
            .map(|m| MonthTally {
                month: m as u32 + 1,
                plays: self.y.month_plays[m],
                minutes: self.y.month_ms[m] / 60_000,
            })
            .collect()
    }

    /// The part of the day with the most listening; for kids never late night.
    fn peak_part(&self) -> Option<usize> {
        let kid = self.input.kid_profile;
        let mut best: Option<usize> = None;
        for (p, &ms) in self.y.part_ms.iter().enumerate() {
            if kid && p == PART_LATE_NIGHT {
                continue;
            }
            if best.is_none_or(|b| ms > self.y.part_ms[b]) {
                best = Some(p);
            }
        }
        best.filter(|&p| self.y.part_ms[p] > 0)
    }

    fn peak_sentence(&self, part: usize) -> Option<(String, String)> {
        if permille(self.y.part_ms[part], self.y.total_ms) < PEAK_SENTENCE_MIN {
            return None;
        }
        let t = [
            &words::PEAK_MORNING,
            &words::PEAK_AFTERNOON,
            &words::PEAK_EVENING,
            &words::PEAK_LATE,
        ][part];
        Some(say(self.seed, t, &[]))
    }

    // ---- Fixed beats -------------------------------------------------------------

    pub(crate) fn intro(&self, year_line: &str, since: Option<i64>) -> Candidate {
        let (template, text) = match since {
            Some(d) => say(
                self.seed,
                &words::INTRO_SINCE,
                &[arg("since", short_date(d))],
            ),
            None => say(self.seed, &words::INTRO, &[]),
        };
        Candidate {
            id: "intro",
            role: Role::Opener,
            fixed: true,
            list: false,
            template,
            text,
            data: BeatData::Intro {
                year: self.input.year,
                year_line: year_line.to_string(),
                album_keys: self.top_albums(3).into_iter().map(|a| a.key).collect(),
            },
            math: Math {
                counts: vec![
                    count("Plays counted", self.y.total_plays, "plays"),
                    count("Days with music", self.y.active_days.len() as u64, "days"),
                ],
                rule: "The year line is built from your strongest listening traits.".into(),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &[],
        }
    }

    pub(crate) fn minutes(&self, short: bool) -> Candidate {
        let minutes = self.y.total_ms / 60_000;
        let last_minutes = self
            .last_year
            .and_then(|p| p.listened_ms)
            .map(|ms| ms / 60_000)
            .filter(|&m| m > 0);
        let change = last_minutes.map(|last| {
            let num = (i128::from(minutes) - i128::from(last)) * 100;
            let den = i128::from(last);
            // Round half away from zero.
            let r = (2 * num + num.signum() * den) / (2 * den);
            r as i64
        });
        let peak = self.peak_part();
        let last_year = (self.input.year - 1).to_string();
        let (template, text) = if short {
            let title = self
                .y
                .song_rank
                .first()
                .map(|&s| clip(self.y.songs[s].title))
                .unwrap_or_default();
            let t = if self.so_far {
                &words::MINUTES_SHORT
            } else {
                &words::MINUTES_SHORT_FINAL
            };
            say(self.seed, t, &[arg("title", title)])
        } else {
            let main = match change {
                Some(c) if c > 2 => say(
                    self.seed,
                    &words::MINUTES_MORE,
                    &[arg("pct", thousands(c as u64)), arg("lastYear", last_year)],
                ),
                Some(c) if c < -2 => say(
                    self.seed,
                    &words::MINUTES_LESS,
                    &[
                        arg("pct", thousands(c.unsigned_abs())),
                        arg("lastYear", last_year),
                    ],
                ),
                Some(_) => say(
                    self.seed,
                    &words::MINUTES_SAME,
                    &[arg("lastYear", last_year)],
                ),
                None => say(
                    self.seed,
                    &words::MINUTES_PLAIN,
                    &[
                        arg("days", thousands(minutes / 1440)),
                        arg("hours", ((minutes % 1440) / 60).to_string()),
                    ],
                ),
            };
            join(main, peak.and_then(|p| self.peak_sentence(p)))
        };
        let mut counts = vec![
            count("Minutes listened", minutes, "minutes"),
            count("Plays", self.y.total_plays, "plays"),
        ];
        if let Some(l) = last_minutes {
            counts.push(count("Minutes last year", l, "minutes"));
        }
        Candidate {
            id: "minutes",
            role: Role::BigNumbers,
            fixed: true,
            list: false,
            template,
            text,
            data: BeatData::Minutes {
                minutes,
                days: minutes / 1440,
                hours: (minutes % 1440) / 60,
                months: self.months(),
                last_year_minutes: last_minutes,
                change_percent: change,
                peak_part: peak.map(|p| PART_NAMES[p].to_string()),
            },
            math: Math {
                counts,
                rule: "Listened time, not track length. Plays under 2 s are not counted, and each play counts on the calendar day of its own time zone.".into(),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &[],
        }
    }

    pub(crate) fn top_song(&self) -> Option<Candidate> {
        let &top = self.y.song_rank.first()?;
        let snippet = self.most_jumped_line(top).map(|(_, start, _)| start);
        let s = &self.y.songs[top];
        let (template, text) = say(
            self.seed,
            &words::TOP_SONG,
            &[arg("plays", thousands(s.plays))],
        );
        Some(Candidate {
            id: "topSong",
            role: Role::BigNumbers,
            fixed: true,
            list: true,
            template,
            text,
            data: BeatData::TopSong {
                top: self.top_songs(CARD_TOP),
                snippet_start_ms: snippet,
            },
            math: Math {
                counts: vec![
                    count("Plays", s.plays, "plays"),
                    count("Minutes listened", s.ms / 60_000, "minutes"),
                ],
                rule: RANK_RULE.into(),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &["song"],
        })
    }

    pub(crate) fn top_artist(&self) -> Option<Candidate> {
        let &top = self.y.artist_rank.first()?;
        let a = &self.y.artists[top];
        let (template, text) = say(
            self.seed,
            &words::TOP_ARTIST,
            &[
                arg("plays", thousands(a.plays)),
                arg("hours", thousands(a.ms / MS_PER_HOUR as u64)),
            ],
        );
        Some(Candidate {
            id: "topArtist",
            role: Role::BigNumbers,
            fixed: true,
            list: true,
            template,
            text,
            data: BeatData::TopArtist {
                top: self.top_artists(CARD_TOP),
            },
            math: Math {
                counts: vec![
                    count("Plays", a.plays, "plays"),
                    count("Minutes listened", a.ms / 60_000, "minutes"),
                ],
                rule: "Ranked by plays, then listened time.".into(),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &["artist"],
        })
    }

    pub(crate) fn top_album(&self) -> Option<Candidate> {
        let &top = self.y.album_rank.first()?;
        let al = &self.y.albums[top];
        let throughs = self
            .input
            .album_track_counts
            .get(al.key)
            .filter(|&&n| n > 0)
            .map(|_| self.y.play_throughs.get(&top).copied().unwrap_or(0));
        let (template, text) = match throughs {
            Some(n) if n > 0 => say(
                self.seed,
                &words::TOP_ALBUM_THROUGH,
                &[arg("n", thousands(n))],
            ),
            _ => say(
                self.seed,
                &words::TOP_ALBUM,
                &[arg("plays", thousands(al.plays))],
            ),
        };
        let mut counts = vec![count("Plays", al.plays, "plays")];
        if let Some(n) = throughs {
            counts.push(count("Played start to finish", n, "count"));
        }
        Some(Candidate {
            id: "topAlbum",
            role: Role::BigNumbers,
            fixed: true,
            list: false,
            template,
            text,
            data: BeatData::TopAlbum {
                top: self.top_albums(CARD_TOP),
                play_throughs: throughs,
            },
            math: Math {
                counts,
                rule: "Ranked by plays of its songs, then listened time. Start to finish: one unbroken run of the album's songs, none skipped, covering as many songs as it has.".into(),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &["album"],
        })
    }

    pub(crate) fn your_sound(&self) -> Option<Candidate> {
        let y = self.y;
        if y.genres.is_empty() || y.genre_plays * 2 < y.total_plays {
            return None;
        }
        let top: Vec<GenreEntry> = y
            .genre_rank
            .iter()
            .take(CARD_TOP)
            .enumerate()
            .map(|(r, &g)| GenreEntry {
                rank: r as u32 + 1,
                name: clip(y.genres[g].name),
                plays: y.genres[g].plays,
                permille: permille(y.genres[g].plays, y.genre_plays),
            })
            .collect();
        let tracked: Vec<usize> = y.genre_rank.iter().take(4).copied().collect();
        let share = |m: u32, g: usize| -> Option<u32> {
            let total = y.month_genre_total[(m - 1) as usize];
            (total > 0).then(|| permille(y.month_genre.get(&(m, g)).copied().unwrap_or(0), total))
        };
        let months: Vec<Vec<u32>> = (1..=12u32)
            .map(|m| tracked.iter().map(|&g| share(m, g).unwrap_or(0)).collect())
            .collect();
        // The drift: the tracked genre and split month with the biggest rise in
        // average monthly share; ties go to the higher genre, then the earlier month.
        let mut drift: Option<(usize, u32, u32)> = None;
        for (gi, &g) in tracked.iter().enumerate() {
            for split in 1..12u32 {
                let side = |r: std::ops::RangeInclusive<u32>| {
                    let v: Vec<u32> = r.filter_map(|m| share(m, g)).collect();
                    (v.len() >= 2).then(|| v.iter().sum::<u32>() / v.len() as u32)
                };
                let (Some(before), Some(after)) = (side(1..=split), side(split + 1..=12)) else {
                    continue;
                };
                if after < before + DRIFT_MIN_RISE {
                    continue;
                }
                let rise = after - before;
                if drift.is_none_or(|(_, _, r)| rise > r) {
                    drift = Some((gi, split, rise));
                }
            }
        }
        let drift = drift.map(|(gi, m, rise)| Drift {
            genre: clip(y.genres[tracked[gi]].name),
            after_month: m,
            rise_permille: rise,
        });
        let (template, text) = match &drift {
            Some(d) => say(
                self.seed,
                &words::SOUND_DRIFT,
                &[
                    arg("genre", d.genre.clone()),
                    arg("month", words::month_name(d.after_month)),
                    arg("nextMonth", words::month_name(d.after_month + 1)),
                ],
            ),
            None => say(
                self.seed,
                &words::SOUND_STEADY,
                &[arg("genre", top[0].name.clone())],
            ),
        };
        let mut counts = vec![
            count("Plays with a genre", y.genre_plays, "plays"),
            count("Genres", y.genres.len() as u64, "count"),
        ];
        if let Some(d) = &drift {
            counts.push(count(
                "Rise in share",
                u64::from(d.rise_permille),
                "permille",
            ));
        }
        Some(Candidate {
            id: "yourSound",
            role: Role::Turn,
            fixed: true,
            list: true,
            template,
            text,
            data: BeatData::YourSound { top, months, drift },
            math: Math {
                counts,
                rule: format!(
                    "Shares of the plays that have a genre. A drift is a genre whose average monthly share rose by {} points or more after one month.",
                    percent(DRIFT_MIN_RISE)
                ),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &["sound"],
        })
    }

    pub(crate) fn personality(&self) -> Candidate {
        let (a, value) = personality::choose(&self.metrics, self.input.kid_profile);
        let last = self
            .last_year
            .filter(|p| p.personality_metric.as_deref() == Some(a.metric))
            .and_then(|p| p.personality_value);
        let shown = |v: u32| {
            if a.count {
                v.to_string()
            } else {
                percent(v).to_string()
            }
        };
        let fill_args = [
            arg("pct", shown(value)),
            arg("n", shown(value)),
            arg("rule", shown(a.threshold)),
        ];
        let line = words::fill(a.line, &fill_args);
        let rule = words::fill(a.rule, &fill_args);
        let (template, text) = match last {
            Some(l) if !a.count => {
                let (t2, s2) = say(
                    self.seed,
                    &words::PERSONALITY_LAST_YEAR,
                    &[
                        arg("lastYear", (self.input.year - 1).to_string()),
                        arg("pct", percent(l).to_string()),
                    ],
                );
                (format!("personality.{}+{t2}", a.id), format!("{line} {s2}"))
            }
            _ => (format!("personality.{}", a.id), line),
        };
        let all = self.metrics.all();
        let supporting = all
            .iter()
            .filter(|m| SUPPORTING.contains(&m.id.as_str()) && m.id != a.metric)
            .cloned()
            .collect();
        let unit = if a.count { "count" } else { "permille" };
        Candidate {
            id: "personality",
            role: Role::Identity,
            fixed: true,
            list: false,
            template,
            text,
            data: BeatData::Personality {
                archetype: a.id.into(),
                name: a.name.into(),
                metric: a.metric.into(),
                value,
                threshold: a.threshold,
                unit: unit.into(),
                last_year_value: last,
                metrics: supporting,
            },
            math: Math {
                counts: all
                    .iter()
                    .map(|m| count(&m.label, u64::from(m.value), &m.unit))
                    .collect(),
                rule,
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &[],
        }
    }

    pub(crate) fn closer(&self) -> Candidate {
        let songs: Vec<usize> = self
            .y
            .song_rank
            .iter()
            .take(PLAYLIST_SONGS)
            .copied()
            .collect();
        let known: Vec<u64> = songs
            .iter()
            .filter_map(|&s| self.y.songs[s].duration_ms)
            .collect();
        let n = songs.len() as u64;
        let (template, text) = say(self.seed, &words::CLOSER, &[arg("count", thousands(n))]);
        Candidate {
            id: "closer",
            role: Role::Close,
            fixed: true,
            list: false,
            template,
            text,
            data: BeatData::Closer {
                title: format!("Your Top Songs {}", self.input.year),
                song_keys: songs
                    .iter()
                    .map(|&s| self.y.songs[s].key.to_string())
                    .collect(),
                count: n,
                duration_ms: known.iter().sum(),
                duration_known: known.len() as u64,
            },
            math: Math {
                counts: vec![count("Songs", n, "count")],
                rule: format!("Your top {PLAYLIST_SONGS} songs by plays."),
            },
            distance: 0,
            score: None,
            personal: 0,
            themes: &[],
        }
    }

    // ---- Rotating beats ----------------------------------------------------------

    // One constructor for every rotating beat; a struct would only rename the
    // arguments.
    #[allow(clippy::too_many_arguments)]
    fn rotating(
        &self,
        id: &'static str,
        role: Role,
        list: bool,
        said: (String, String),
        data: BeatData,
        math: Math,
        distance: i64,
        personal: i64,
        themes: &'static [&'static str],
    ) -> Candidate {
        Candidate {
            id,
            role,
            fixed: false,
            list,
            template: said.0,
            text: said.1,
            data,
            math,
            distance: distance.clamp(0, 1000),
            score: None,
            personal,
            themes,
        }
    }

    pub(crate) fn listening_clock(&self) -> Option<Candidate> {
        let y = self.y;
        if y.total_plays == 0 {
            return None;
        }
        let mut peak = 0usize;
        for (i, &n) in y.hour_week_plays.iter().enumerate() {
            if n > y.hour_week_plays[peak] {
                peak = i;
            }
        }
        let (wd, hour) = ((peak / 24) as u32, (peak % 24) as u32);
        let part = part_of_day(hour);
        // Kid profiles skip the late-night beats.
        if self.input.kid_profile && part == PART_LATE_NIGHT {
            return None;
        }
        let day_totals: Vec<u64> = (0..7)
            .map(|d| y.hour_week_plays[d * 24..d * 24 + 24].iter().sum())
            .collect();
        let mut top_wd = 0usize;
        for (d, &n) in day_totals.iter().enumerate() {
            if n > day_totals[top_wd] {
                top_wd = d;
            }
        }
        let peak_plays = y.hour_week_plays[peak];
        let ratio_x10 = (peak_plays * 168 * 10 / y.total_plays) as i64;
        let said = say(
            self.seed,
            &words::CLOCK,
            &[
                arg("weekday", words::WEEKDAYS[wd as usize]),
                arg("hour", words::hour_name(hour)),
            ],
        );
        Some(
            self.rotating(
                "listeningClock",
                Role::Surprise,
                false,
                said,
                BeatData::ListeningClock {
                    peak_weekday: wd,
                    peak_hour: hour,
                    grid: y.hour_week_plays.clone(),
                    peak_part: PART_NAMES[part].into(),
                    top_weekday: top_wd as u32,
                },
                Math {
                    counts: vec![
                        count("Plays in the peak hour", peak_plays, "plays"),
                        count("Plays", y.total_plays, "plays"),
                    ],
                    rule: "The hour of the week with the most plays, in each play's own time zone."
                        .into(),
                },
                (ratio_x10 - 10) * 20,
                600,
                &["time"],
            ),
        )
    }

    pub(crate) fn biggest_discovery(&self) -> Option<Candidate> {
        if !self.baseline_known {
            return None;
        }
        let y = self.y;
        let (r, &i) = y
            .artist_rank
            .iter()
            .enumerate()
            .find(|&(_, &i)| !y.before.contains_key(y.artists[i].key))?;
        let a = &y.artists[i];
        if a.plays < DISCOVERY_MIN_PLAYS {
            return None;
        }
        let (_, month, _) = crate::time::civil_from_days(a.first_day);
        let said = say(
            self.seed,
            &words::DISCOVERY,
            &[
                arg("month", words::month_name(month)),
                arg("plays", thousands(a.plays)),
            ],
        );
        let top = y.artists[y.artist_rank[0]].plays;
        Some(self.rotating(
            "biggestDiscovery",
            Role::Surprise,
            false,
            said,
            BeatData::BiggestDiscovery {
                artist: self.artist_at(r),
                first_day: format_day(a.first_day),
                first_month: month,
                months: a.months.to_vec(),
            },
            Math {
                counts: vec![
                    count("Plays this year", a.plays, "plays"),
                    count("Plays before this year", 0, "plays"),
                ],
                rule: format!(
                    "The artist you played most who was new to you this year; needs {DISCOVERY_MIN_PLAYS} plays."
                ),
            },
            i64::from(permille(a.plays, top)),
            800,
            &["artist", "new"],
        ))
    }

    pub(crate) fn comeback(&self) -> Option<Candidate> {
        let y = self.y;
        let top = y.artists[*y.artist_rank.first()?].plays;
        for (r, &i) in y.artist_rank.iter().enumerate() {
            let a = &y.artists[i];
            if a.plays < COMEBACK_MIN_PLAYS {
                break;
            }
            let Some(b) = y.before.get(a.key) else {
                continue;
            };
            let Some(last) = b.last_utc else { continue };
            if b.plays < COMEBACK_MIN_PLAYS_BEFORE {
                continue;
            }
            let gap = (a.first_utc - last).div_euclid(MS_PER_DAY);
            if gap < COMEBACK_MIN_GAP_DAYS {
                continue;
            }
            let said = say(
                self.seed,
                &words::COMEBACK,
                &[
                    arg("gap", thousands(gap as u64)),
                    arg("plays", thousands(a.plays)),
                ],
            );
            let distance = i64::from(permille(a.plays, top)) + gap.min(730) * 300 / 730;
            return Some(self.rotating(
                "comeback",
                Role::Surprise,
                false,
                said,
                BeatData::Comeback {
                    artist: self.artist_at(r),
                    plays_before: b.plays,
                    gap_days: gap as u64,
                },
                Math {
                    counts: vec![
                        count("Plays this year", a.plays, "plays"),
                        count("Plays before this year", b.plays, "plays"),
                        count("Days between", gap as u64, "days"),
                    ],
                    rule: format!(
                        "An artist with {COMEBACK_MIN_PLAYS_BEFORE} plays or more before, silent for {COMEBACK_MIN_GAP_DAYS} days or more, then {COMEBACK_MIN_PLAYS} plays or more this year."
                    ),
                },
                distance,
                850,
                &["artist"],
            ));
        }
        None
    }

    pub(crate) fn obsession_week(&self) -> Option<Candidate> {
        let y = self.y;
        let mut best: Option<(i64, usize, u64)> = None;
        for (&(week, artist), &n) in &y.week_artist {
            if n < OBSESSION_MIN_PLAYS || permille(n, y.week_total[&week]) < OBSESSION_MIN_SHARE {
                continue;
            }
            // Most plays; ties go to the earlier week, then the lower artist index
            // (the map's order).
            if best.is_none_or(|(_, _, b)| n > b) {
                best = Some((week, artist, n));
            }
        }
        let (week, artist, n) = best?;
        let a = &y.artists[artist];
        let days: Vec<u64> = (0..7)
            .map(|d| y.day_artist.get(&(week + d, artist)).copied().unwrap_or(0))
            .collect();
        let weeks = ((self.last_day - self.first_day + 1) / 7).max(1) as u64;
        let ratio_x10 = (n * weeks * 10 / a.plays.max(1)) as i64;
        let (_, m, d) = crate::time::civil_from_days(week);
        let date = format!("{} {d}", words::month_name(m));
        let said = say(
            self.seed,
            &words::OBSESSION,
            &[
                arg("date", date),
                arg("artist", clip(a.name)),
                arg("plays", thousands(n)),
            ],
        );
        let r = y.artist_rank.iter().position(|&x| x == artist).unwrap_or(0);
        Some(self.rotating(
            "obsessionWeek",
            Role::Surprise,
            false,
            said,
            BeatData::ObsessionWeek {
                artist: self.artist_at(r),
                week_start: format_day(week),
                days,
                week_plays: n,
                week_share_permille: permille(n, y.week_total[&week]),
            },
            Math {
                counts: vec![
                    count("Plays that week", n, "plays"),
                    count("All plays that week", y.week_total[&week], "plays"),
                    count("The artist's plays this year", a.plays, "plays"),
                ],
                rule: format!(
                    "The Monday-to-Sunday week with the most plays of one artist, at least {OBSESSION_MIN_PLAYS} plays and {}% of that week.",
                    percent(OBSESSION_MIN_SHARE)
                ),
            },
            (ratio_x10 - 10) * 25,
            800,
            &["artist", "time"],
        ))
    }

    pub(crate) fn longest_streak(&self) -> Option<Candidate> {
        let mut best = (0u64, 0i64, 0i64);
        let mut cur: Option<(i64, i64)> = None;
        for &d in &self.y.active_days {
            cur = match cur {
                Some((s, e)) if d == e + 1 => Some((s, d)),
                _ => Some((d, d)),
            };
            let (s, e) = cur.unwrap_or((d, d));
            let len = (e - s + 1) as u64;
            if len > best.0 {
                best = (len, s, e);
            }
        }
        let (len, s, e) = best;
        if len < STREAK_MIN_DAYS {
            return None;
        }
        let said = say(
            self.seed,
            &words::STREAK,
            &[
                arg("days", thousands(len)),
                arg("first", short_date(s)),
                arg("last", short_date(e)),
            ],
        );
        Some(self.rotating(
            "longestStreak",
            Role::Surprise,
            false,
            said,
            BeatData::LongestStreak {
                days: len,
                first_day: format_day(s),
                last_day: format_day(e),
            },
            Math {
                counts: vec![
                    count("Days in a row", len, "days"),
                    count("Days with music", self.y.active_days.len() as u64, "days"),
                ],
                rule: format!("The longest run of days in a row with at least one play; needs {STREAK_MIN_DAYS} days."),
            },
            (len * 1000 / 90) as i64,
            500,
            &["time", "habit"],
        ))
    }

    pub(crate) fn song_of_each_season(&self) -> Option<Candidate> {
        let y = self.y;
        const IDS: [&str; 4] = ["winter", "spring", "summer", "autumn"];
        let mut order: Vec<usize> = Vec::new();
        let mut months: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for m in 1..=12u32 {
            let s = crate::year::season_of(m, self.input.hemisphere);
            if !order.contains(&s) {
                order.push(s);
            }
            months.entry(s).or_default().push(m);
        }
        let mut seasons = Vec::new();
        for s in order {
            if y.season_plays[s] < SEASON_MIN_PLAYS {
                continue;
            }
            let Some((song, _)) = y.top_song_of_season(s) else {
                continue;
            };
            seasons.push(Season {
                id: IDS[s].into(),
                months: months[&s].clone(),
                song: self.song(song),
            });
        }
        if seasons.len() < SEASONS_MIN {
            return None;
        }
        let mut keys: Vec<&str> = seasons.iter().map(|s| s.song.key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        if keys.len() < 2 {
            return None;
        }
        let all_distinct = keys.len() == seasons.len();
        let said = if all_distinct {
            say(self.seed, &words::SEASONS, &[])
        } else {
            say(self.seed, &words::SEASONS_SHARED, &[])
        };
        let distance = match keys.len() {
            4 => 1000,
            3 => 750,
            _ => 450,
        };
        let counts = seasons
            .iter()
            .map(|s| {
                count(
                    &format!("Plays in {}", s.id),
                    y.season_plays[IDS.iter().position(|i| *i == s.id).unwrap_or(0)],
                    "plays",
                )
            })
            .collect();
        Some(self.rotating(
            "songOfEachSeason",
            Role::Surprise,
            true,
            said,
            BeatData::SongOfEachSeason { seasons },
            Math {
                counts,
                rule: format!(
                    "The most played song of each season with {SEASON_MIN_PLAYS} plays or more; needs {SEASONS_MIN} such seasons."
                ),
            },
            distance,
            650,
            &["song", "time"],
        ))
    }

    pub(crate) fn most_replayed_day(&self) -> Option<Candidate> {
        let y = self.y;
        // Most plays; ties go to the earlier day, then the lower song index (the
        // map's order).
        let mut best: Option<(i64, usize, u64)> = None;
        for (&(day, song), &n) in &y.day_song {
            if best.is_none_or(|(_, _, bn)| n > bn) {
                best = Some((day, song, n));
            }
        }
        let (day, song, n) = best?;
        if n < REPLAY_DAY_MIN_PLAYS {
            return None;
        }
        let entry = self.song(song);
        let said = say(
            self.seed,
            &words::REPLAY_DAY,
            &[
                arg("date", short_date(day)),
                arg("title", entry.title.clone()),
                arg("plays", thousands(n)),
            ],
        );
        Some(self.rotating(
            "mostReplayedDay",
            Role::Surprise,
            false,
            said,
            BeatData::MostReplayedDay {
                song: entry,
                day: format_day(day),
                plays: n,
            },
            Math {
                counts: vec![count("Plays that day", n, "plays")],
                rule: format!(
                    "The most plays of one song on one day; needs {REPLAY_DAY_MIN_PLAYS}."
                ),
            },
            (n as i64 - 4) * 120,
            700,
            &["song"],
        ))
    }

    pub(crate) fn mood_arc(&self) -> Option<Candidate> {
        let y = self.y;
        if y.fp_plays * 2 < y.total_plays || y.total_plays == 0 {
            return None;
        }
        let points: Vec<SoundPoint> = (0..12)
            .map(|m| {
                let (n, e, v) = y.fp_month[m];
                let ok = n >= MOOD_MIN_MONTH_PLAYS;
                SoundPoint {
                    month: m as u32 + 1,
                    energy: ok.then(|| (e / n.max(1)) as u32),
                    valence: ok.then(|| (v / n.max(1)) as u32),
                }
            })
            .collect();
        if points.iter().filter(|p| p.energy.is_some()).count() < MOOD_MIN_MONTHS {
            return None;
        }
        let half = |r: std::ops::Range<usize>| {
            let v: Vec<u32> = points[r].iter().filter_map(|p| p.energy).collect();
            (!v.is_empty()).then(|| v.iter().sum::<u32>() / v.len() as u32)
        };
        let (first, second) = (half(0..6)?, half(6..12)?);
        let diff = i64::from(second) - i64::from(first);
        let t = if diff.unsigned_abs() < u64::from(MOOD_STEADY_UNDER) {
            &words::MOOD_LEVEL
        } else if diff > 0 {
            &words::MOOD_UP
        } else {
            &words::MOOD_DOWN
        };
        let said = say(self.seed, t, &[]);
        Some(self.rotating(
            "moodArc",
            Role::Turn,
            false,
            said,
            BeatData::MoodArc {
                months: points,
                first_half_energy: first,
                second_half_energy: second,
            },
            Math {
                counts: vec![
                    count("Plays with a sound fingerprint", y.fp_plays, "plays"),
                    count("Energy, January to June", u64::from(first), "permille"),
                    count("Energy, July to December", u64::from(second), "permille"),
                ],
                rule: format!(
                    "The average energy of the songs you played each month, from their sound fingerprints; needs half the plays fingerprinted and {MOOD_MIN_MONTHS} months with {MOOD_MIN_MONTH_PLAYS} such plays. It describes the music, not you."
                ),
            },
            diff.abs() * 6,
            600,
            &["sound"],
        ))
    }

    /// The line of a song jumped to most: (line, start ms, jumps). Ties go to the
    /// lower line.
    fn most_jumped_line(&self, song: usize) -> Option<(u32, u64, u64)> {
        let mut best: Option<(u32, u64, u64)> = None;
        for (&(_, line), (n, starts)) in self.y.lyric_lines.range((song, 0)..=(song, u32::MAX)) {
            if best.is_none_or(|(_, _, b)| *n > b) {
                let start = most_start(starts);
                best = Some((line, start, *n));
            }
        }
        best
    }

    pub(crate) fn most_sung_line(&self) -> Option<Candidate> {
        let y = self.y;
        let mut best: Option<(usize, u32, u64)> = None;
        for (&(song, line), (n, _)) in &y.lyric_lines {
            let better = match best {
                None => true,
                Some((bs, _, bn)) => *n > bn || (*n == bn && y.song_pos[song] < y.song_pos[bs]),
            };
            if better {
                best = Some((song, line, *n));
            }
        }
        let (song, line, n) = best?;
        if n < LYRIC_MIN_JUMPS {
            return None;
        }
        let start = most_start(&y.lyric_lines[&(song, line)].1);
        let entry = self.song(song);
        let said = say(
            self.seed,
            &words::LYRIC,
            &[
                arg("jumps", thousands(n)),
                arg("title", entry.title.clone()),
            ],
        );
        Some(self.rotating(
            "mostSungLine",
            Role::Surprise,
            false,
            said,
            BeatData::MostSungLine {
                song: entry,
                line_index: line,
                line_start_ms: start,
                jumps: n,
            },
            Math {
                counts: vec![count("Jumps to this line", n, "count")],
                rule: format!("The lyric line you tapped most in the lyrics view; needs {LYRIC_MIN_JUMPS} taps."),
            },
            (n * 80) as i64,
            900,
            &["song", "voice"],
        ))
    }

    /// Every rotating candidate that has enough data.
    pub(crate) fn rotating_pool(&self) -> Vec<Candidate> {
        [
            self.listening_clock(),
            self.biggest_discovery(),
            self.comeback(),
            self.obsession_week(),
            self.longest_streak(),
            self.song_of_each_season(),
            self.most_replayed_day(),
            self.mood_arc(),
            self.most_sung_line(),
            // householdDeepCut: never emitted (see the module note).
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// The start time seen most for a line; ties go to the earliest.
fn most_start(starts: &BTreeMap<u64, u64>) -> u64 {
    let mut best: Option<(u64, u64)> = None;
    for (&s, &n) in starts {
        if best.is_none_or(|(_, b)| n > b) {
            best = Some((s, n));
        }
    }
    best.map_or(0, |(s, _)| s)
}
