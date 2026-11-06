//! The play log's rules (plan sections 5, 8 and 21).
//!
//! - Every play is written the moment it starts and checkpointed while it plays, so
//!   history survives a crash; an unfinished play found at launch is closed as
//!   "ended by the system" with its last checkpoint.
//! - Private listening, Guest and "Learn from my listening" switched off write
//!   nothing at all.
//! - Each row carries every section 5 field: the stable song key, listened
//!   milliseconds, UTC start plus UTC offset, how it ended, what started it, who
//!   chose it, the device, and coarse context (local hour and weekday). Lyric-line
//!   jumps are their own rows. No location, ever.

use encore_store::{IN_PROGRESS, LyricJumpRecord, PlayEventRecord, Store};
use encore_sync::{Hlc, HlcClock};
use serde::{Deserialize, Serialize};

use crate::queue::{ChosenBy, StartedFrom};
use crate::signal::{Facts, Signal, SkipOrigin, classify};

/// How a play ended. Only `Skipped` can become a negative signal later, and only
/// when the smart-picks rules allow it (section 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndReason {
    Finished,
    Skipped,
    /// Paused and never resumed.
    PausedAbandoned,
    /// A headphone unplug, an output switch, a call or an assistant (section 21).
    Interrupted,
    /// The song failed to load or play.
    Error,
    /// A sleep timer, a closed app or a lost connection; also a play found
    /// unfinished after a crash.
    EndedBySystem,
}

impl EndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            EndReason::Finished => "finished",
            EndReason::Skipped => "skipped",
            EndReason::PausedAbandoned => "pausedAbandoned",
            EndReason::Interrupted => "interrupted",
            EndReason::Error => "error",
            EndReason::EndedBySystem => "endedBySystem",
        }
    }
}

/// Whether listening is recorded at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListeningMode {
    #[default]
    Normal,
    /// Private listening (a per-session switch on Now Playing).
    Private,
    /// A Guest session.
    Guest,
    /// "Learn from my listening" is switched off.
    LearningOff,
}

impl ListeningMode {
    pub fn records(self) -> bool {
        self == ListeningMode::Normal
    }
}

/// What the platform tells the core when a song starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayStart {
    pub song_key: String,
    pub source_kind: String,
    pub source_item_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: u64,
    pub started_from: StartedFrom,
    pub context_id: Option<String>,
    pub chosen_by: ChosenBy,
    /// Wall clock at the start, UTC milliseconds.
    pub started_utc_ms: i64,
    /// The device's UTC offset at that moment, in minutes (east positive).
    pub utc_offset_minutes: i32,
}

/// Local hour (0 to 23) and ISO weekday (1 Monday to 7 Sunday) of a UTC instant.
pub fn local_hour_and_weekday(utc_ms: i64, utc_offset_minutes: i32) -> (u8, u8) {
    let local_ms = utc_ms + i64::from(utc_offset_minutes) * 60_000;
    let hour = local_ms.div_euclid(3_600_000).rem_euclid(24) as u8;
    let days = local_ms.div_euclid(86_400_000);
    // 1 January 1970 was a Thursday (ISO 4).
    let weekday = ((days + 3).rem_euclid(7) + 1) as u8;
    (hour, weekday)
}

/// Writes the play log for one profile.
#[derive(Debug)]
pub struct PlayLog {
    device_id: String,
    checkpoint_ms: u64,
    clock: HlcClock,
    mode: ListeningMode,
    open: Option<OpenPlay>,
}

#[derive(Debug, Clone)]
struct OpenPlay {
    id: String,
    last_checkpoint_utc_ms: i64,
    started_utc_ms: i64,
    duration_ms: u64,
    chosen_by: ChosenBy,
    song_key: String,
}

/// What is known when a play closes, beyond how it ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Close {
    /// What moved on from it (None when nothing did, as when the app closes).
    pub origin: Option<SkipOrigin>,
    /// The last unplug, output switch, call or assistant, UTC milliseconds.
    pub last_interruption_utc_ms: Option<i64>,
}

impl PlayLog {
    /// `checkpoint_seconds` is `playLog.checkpointSeconds` in tokens.
    pub fn new(
        store: &Store,
        device_id: &str,
        checkpoint_seconds: u64,
    ) -> encore_store::Result<PlayLog> {
        let last = store
            .newest_hlc()?
            .and_then(|s| s.parse::<Hlc>().ok())
            .unwrap_or_default();
        Ok(PlayLog {
            device_id: device_id.to_owned(),
            checkpoint_ms: checkpoint_seconds * 1000,
            clock: HlcClock::resume(last),
            mode: ListeningMode::Normal,
            open: None,
        })
    }

    pub fn mode(&self) -> ListeningMode {
        self.mode
    }

    /// Change the mode. Leaving Normal closes the current play first, so nothing
    /// that happens afterwards is written.
    pub fn set_mode(
        &mut self,
        store: &Store,
        mode: ListeningMode,
        listened_ms: u64,
        now_utc_ms: i64,
    ) -> encore_store::Result<()> {
        if !mode.records() {
            self.finish(store, listened_ms, EndReason::EndedBySystem, now_utc_ms)?;
        }
        self.mode = mode;
        Ok(())
    }

    /// Close plays left open by a crash or a killed app. Call once at launch.
    pub fn recover(&mut self, store: &Store, now_utc_ms: i64) -> encore_store::Result<usize> {
        let open = store.open_play_events()?;
        for e in &open {
            store.finish_play_event(
                &e.id,
                e.listened_ms,
                EndReason::EndedBySystem.as_str(),
                Some(Signal::System.as_str()),
                None,
                now_utc_ms,
            )?;
        }
        self.open = None;
        Ok(open.len())
    }

    /// A song started. Returns the new play's ID, or None when nothing is recorded.
    /// A play still open is closed as skipped first, with `previous_listened_ms`
    /// and what moved on from it (`previous`).
    pub fn start(
        &mut self,
        store: &Store,
        s: &PlayStart,
        previous_listened_ms: u64,
        previous: &Close,
    ) -> encore_store::Result<Option<String>> {
        if self.open.is_some() {
            self.close(
                store,
                previous_listened_ms,
                EndReason::Skipped,
                s.started_utc_ms,
                previous,
            )?;
        }
        if !self.mode.records() {
            return Ok(None);
        }
        let hlc = self.clock.tick(s.started_utc_ms);
        let (local_hour, local_weekday) =
            local_hour_and_weekday(s.started_utc_ms, s.utc_offset_minutes);
        let id = new_id();
        store.insert_play_event(&PlayEventRecord {
            id: id.clone(),
            device_id: self.device_id.clone(),
            hlc: hlc.to_string(),
            song_key: s.song_key.clone(),
            source_kind: s.source_kind.clone(),
            source_item_id: s.source_item_id.clone(),
            title: s.title.clone(),
            artist: s.artist.clone(),
            album: s.album.clone(),
            duration_ms: s.duration_ms,
            started_utc_ms: s.started_utc_ms,
            utc_offset_minutes: s.utc_offset_minutes,
            local_hour,
            local_weekday,
            listened_ms: 0,
            end_reason: IN_PROGRESS.to_owned(),
            started_from: s.started_from.as_str().to_owned(),
            context_id: s.context_id.clone(),
            chosen_by: s.chosen_by.as_str().to_owned(),
            updated_utc_ms: s.started_utc_ms,
            signal: None,
            origin: None,
            seeks: 0,
        })?;
        self.open = Some(OpenPlay {
            id: id.clone(),
            last_checkpoint_utc_ms: s.started_utc_ms,
            started_utc_ms: s.started_utc_ms,
            duration_ms: s.duration_ms,
            chosen_by: s.chosen_by,
            song_key: s.song_key.clone(),
        });
        Ok(Some(id))
    }

    /// The listener dragged the progress bar: counted on the play, never a skip.
    pub fn seek(&mut self, store: &Store) -> encore_store::Result<()> {
        if let Some(open) = &self.open {
            store.add_seek(&open.id)?;
        }
        Ok(())
    }

    /// The player reports progress. A checkpoint is written at most every
    /// `checkpoint_seconds`. Returns true when one was written.
    pub fn progress(
        &mut self,
        store: &Store,
        listened_ms: u64,
        now_utc_ms: i64,
    ) -> encore_store::Result<bool> {
        let Some(open) = self.open.as_mut() else {
            return Ok(false);
        };
        if now_utc_ms - open.last_checkpoint_utc_ms < self.checkpoint_ms as i64 {
            return Ok(false);
        }
        open.last_checkpoint_utc_ms = now_utc_ms;
        store.checkpoint_play_event(&open.id, listened_ms, now_utc_ms)
    }

    /// The current play ended, with nothing known about what moved on from it.
    pub fn finish(
        &mut self,
        store: &Store,
        listened_ms: u64,
        reason: EndReason,
        now_utc_ms: i64,
    ) -> encore_store::Result<()> {
        self.close(store, listened_ms, reason, now_utc_ms, &Close::default())
            .map(|_| ())
    }

    /// The current play ended: write how it ended and how it reads (section 21).
    /// A play reached by a skip and skipped again within `coalesceMs` was only
    /// passed over: it is removed, with no signal (board 42). Returns the signal,
    /// or None when nothing was open or the play was removed.
    pub fn close(
        &mut self,
        store: &Store,
        listened_ms: u64,
        reason: EndReason,
        now_utc_ms: i64,
        c: &Close,
    ) -> encore_store::Result<Option<Signal>> {
        let Some(open) = self.open.take() else {
            return Ok(None);
        };
        let rules = crate::tokens::tokens().signal;
        if reason == EndReason::Skipped && now_utc_ms - open.started_utc_ms < rules.coalesce_ms {
            store.discard_play_event(&open.id)?;
            return Ok(None);
        }
        let heard_recently = store.heard_since(
            &open.song_key,
            open.started_utc_ms - rules.too_soon_ms,
            rules.early_skip_ms,
            &open.id,
        )?;
        let signal = classify(
            &Facts {
                end: reason,
                listened_ms,
                duration_ms: open.duration_ms,
                chosen_by: open.chosen_by,
                ms_since_interruption: c.last_interruption_utc_ms.map(|t| now_utc_ms - t),
                heard_recently,
            },
            &rules,
        );
        store.finish_play_event(
            &open.id,
            listened_ms,
            reason.as_str(),
            Some(signal.as_str()),
            c.origin.map(SkipOrigin::as_str),
            now_utc_ms,
        )?;
        Ok(Some(signal))
    }

    /// A lyric line was tapped during the current play.
    pub fn lyric_jump(
        &mut self,
        store: &Store,
        line_index: u32,
        line_start_ms: u64,
        now_utc_ms: i64,
    ) -> encore_store::Result<bool> {
        if !self.mode.records() {
            return Ok(false);
        }
        let Some(open) = self.open.as_ref() else {
            return Ok(false);
        };
        let hlc = self.clock.tick(now_utc_ms);
        store.insert_lyric_jump(&LyricJumpRecord {
            id: new_id(),
            play_event_id: open.id.clone(),
            device_id: self.device_id.clone(),
            hlc: hlc.to_string(),
            line_index,
            line_start_ms,
            at_utc_ms: now_utc_ms,
        })?;
        Ok(true)
    }

    /// The ID of the play in progress, if any.
    pub fn current_play_id(&self) -> Option<&str> {
        self.open.as_ref().map(|o| o.id.as_str())
    }
}

/// A new event ID: a ULID (sortable by time, unique across devices).
fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start_at(ms: i64) -> PlayStart {
        PlayStart {
            song_key: "t1:5a0ab3d41d0b7ae62cddb08fa411c1f1:222".into(),
            source_kind: "jellyfin".into(),
            source_item_id: "t1".into(),
            title: "Slow Tide".into(),
            artist: "Ember Cove".into(),
            album: Some("Golden Hour Static".into()),
            duration_ms: 222_400,
            started_from: StartedFrom::Album,
            context_id: Some("al1".into()),
            chosen_by: ChosenBy::User,
            started_utc_ms: ms,
            utc_offset_minutes: -300,
        }
    }

    const T0: i64 = 1_762_400_000_000; // 2025-11-06 03:33:20 UTC, a Thursday

    #[test]
    fn local_time_uses_the_offset_at_the_start() {
        // 03:33 UTC on a Thursday is 22:33 on Wednesday at UTC-5.
        assert_eq!(local_hour_and_weekday(T0, -300), (22, 3));
        assert_eq!(local_hour_and_weekday(T0, 0), (3, 4));
        assert_eq!(local_hour_and_weekday(T0, 600), (13, 4));
        assert_eq!(local_hour_and_weekday(0, 0), (0, 4)); // 1 January 1970, Thursday
        assert_eq!(local_hour_and_weekday(-1, 0), (23, 3)); // a moment before
    }

    #[test]
    fn a_play_stores_every_section_5_field() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "device-A", 10).unwrap();
        let id = log
            .start(&store, &start_at(T0), 0, &Close::default())
            .unwrap()
            .unwrap();
        log.finish(&store, 210_000, EndReason::Finished, T0 + 211_000)
            .unwrap();
        let e = store.play_event(&id).unwrap().unwrap();
        assert_eq!(e.song_key, "t1:5a0ab3d41d0b7ae62cddb08fa411c1f1:222");
        assert_eq!(e.listened_ms, 210_000);
        assert_eq!((e.started_utc_ms, e.utc_offset_minutes), (T0, -300));
        assert_eq!(e.end_reason, "finished");
        assert_eq!(e.started_from, "album");
        assert_eq!(e.context_id.as_deref(), Some("al1"));
        assert_eq!(e.chosen_by, "user");
        assert_eq!(e.device_id, "device-A");
        assert_eq!((e.local_hour, e.local_weekday), (22, 3));
        assert_eq!(
            (e.title.as_str(), e.artist.as_str()),
            ("Slow Tide", "Ember Cove")
        );
        assert!(e.hlc.parse::<Hlc>().is_ok());
        assert_eq!(e.id.len(), 26); // a ULID
    }

    #[test]
    fn checkpoints_are_written_every_ten_seconds() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        let id = log
            .start(&store, &start_at(T0), 0, &Close::default())
            .unwrap()
            .unwrap();
        assert!(!log.progress(&store, 5_000, T0 + 5_000).unwrap());
        assert!(log.progress(&store, 10_000, T0 + 10_000).unwrap());
        assert!(!log.progress(&store, 15_000, T0 + 15_000).unwrap());
        assert!(log.progress(&store, 20_500, T0 + 20_500).unwrap());
        assert_eq!(store.play_event(&id).unwrap().unwrap().listened_ms, 20_500);
    }

    #[test]
    fn a_crash_leaves_history_that_is_closed_at_the_next_launch() {
        let store = Store::open_in_memory().unwrap();
        let id = {
            let mut log = PlayLog::new(&store, "d", 10).unwrap();
            let id = log
                .start(&store, &start_at(T0), 0, &Close::default())
                .unwrap()
                .unwrap();
            log.progress(&store, 30_000, T0 + 30_000).unwrap();
            id
            // The app dies here: no finish.
        };
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        assert_eq!(log.recover(&store, T0 + 99_000).unwrap(), 1);
        let e = store.play_event(&id).unwrap().unwrap();
        assert_eq!(
            (e.end_reason.as_str(), e.listened_ms),
            ("endedBySystem", 30_000)
        );
        assert_eq!(log.recover(&store, T0 + 100_000).unwrap(), 0);
    }

    #[test]
    fn starting_a_new_song_closes_the_old_one_as_skipped() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        let first = log
            .start(&store, &start_at(T0), 0, &Close::default())
            .unwrap()
            .unwrap();
        let second = log
            .start(&store, &start_at(T0 + 12_000), 12_000, &Close::default())
            .unwrap()
            .unwrap();
        let e = store.play_event(&first).unwrap().unwrap();
        assert_eq!((e.end_reason.as_str(), e.listened_ms), ("skipped", 12_000));
        assert_eq!(log.current_play_id(), Some(second.as_str()));
        // Clock values order the plays even within the same millisecond.
        let a = store.play_event(&first).unwrap().unwrap().hlc;
        let b = store.play_event(&second).unwrap().unwrap().hlc;
        assert!(a < b);
    }

    fn app_pick(ms: i64, key: &str) -> PlayStart {
        PlayStart {
            song_key: key.into(),
            chosen_by: ChosenBy::App,
            started_from: StartedFrom::Queue,
            ..start_at(ms)
        }
    }

    fn swipe() -> Close {
        Close {
            origin: Some(SkipOrigin::Swipe),
            last_interruption_utc_ms: None,
        }
    }

    /// Board 42: swiping past songs writes nothing for them; only the song you
    /// land on, and the one you left, count.
    #[test]
    fn songs_passed_over_by_quick_swipes_leave_no_trace() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        let left = log
            .start(&store, &app_pick(T0, "a"), 0, &swipe())
            .unwrap()
            .unwrap();
        // 20 s in, swipe: song b starts, and is swiped away 300 ms later.
        let passed = log
            .start(&store, &app_pick(T0 + 20_000, "b"), 20_000, &swipe())
            .unwrap()
            .unwrap();
        let landed = log
            .start(&store, &app_pick(T0 + 20_300, "c"), 300, &swipe())
            .unwrap()
            .unwrap();
        assert!(
            store.play_event(&passed).unwrap().is_none(),
            "b was only passed over"
        );
        let a = store.play_event(&left).unwrap().unwrap();
        assert_eq!(
            (
                a.end_reason.as_str(),
                a.signal.as_deref(),
                a.origin.as_deref()
            ),
            ("skipped", Some("skippedEarly"), Some("swipe"))
        );
        assert_eq!(log.current_play_id(), Some(landed.as_str()));
        // Exactly at the window it is a real (if short) play: an accident.
        let next = log
            .start(&store, &app_pick(T0 + 20_900, "d"), 600, &swipe())
            .unwrap()
            .unwrap();
        let c = store.play_event(&landed).unwrap().unwrap();
        assert_eq!(c.signal.as_deref(), Some("accident"));
        assert_ne!(next, landed);
    }

    #[test]
    fn signals_too_soon_interruptions_and_seeks() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        // Heard song "x" fully, then 30 minutes later it comes back and is skipped
        // early: "too soon", not dislike.
        log.start(&store, &app_pick(T0, "x"), 0, &Close::default())
            .unwrap();
        log.close(
            &store,
            222_400,
            EndReason::Finished,
            T0 + 222_400,
            &Close::default(),
        )
        .unwrap();
        let again = log
            .start(&store, &app_pick(T0 + 1_800_000, "x"), 0, &Close::default())
            .unwrap()
            .unwrap();
        log.seek(&store).unwrap();
        let signal = log
            .close(&store, 8_000, EndReason::Skipped, T0 + 1_808_000, &swipe())
            .unwrap();
        assert_eq!(signal, Some(Signal::TooSoon));
        let e = store.play_event(&again).unwrap().unwrap();
        assert_eq!((e.signal.as_deref(), e.seeks), (Some("tooSoon"), 1));

        // Three hours later the same skip is a real early skip.
        log.start(
            &store,
            &app_pick(T0 + 12_600_000, "x"),
            0,
            &Close::default(),
        )
        .unwrap();
        assert_eq!(
            log.close(&store, 8_000, EndReason::Skipped, T0 + 12_608_000, &swipe())
                .unwrap(),
            Some(Signal::SkippedEarly)
        );

        // A headset skip 2 s after the headphones came out: an interruption.
        log.start(
            &store,
            &app_pick(T0 + 13_000_000, "y"),
            0,
            &Close::default(),
        )
        .unwrap();
        let unplugged = Close {
            origin: Some(SkipOrigin::Headset),
            last_interruption_utc_ms: Some(T0 + 13_010_000),
        };
        assert_eq!(
            log.close(
                &store,
                12_000,
                EndReason::Skipped,
                T0 + 13_012_000,
                &unplugged
            )
            .unwrap(),
            Some(Signal::Interruption)
        );
        // After a crash, plays read as system ends.
        let id = log
            .start(
                &store,
                &app_pick(T0 + 14_000_000, "z"),
                0,
                &Close::default(),
            )
            .unwrap()
            .unwrap();
        let mut relaunched = PlayLog::new(&store, "d", 10).unwrap();
        relaunched.recover(&store, T0 + 14_100_000).unwrap();
        assert_eq!(
            store.play_event(&id).unwrap().unwrap().signal.as_deref(),
            Some("system")
        );
    }

    #[test]
    fn private_guest_and_learning_off_write_nothing_at_the_database_level() {
        for mode in [
            ListeningMode::Private,
            ListeningMode::Guest,
            ListeningMode::LearningOff,
        ] {
            let store = Store::open_in_memory().unwrap();
            let mut log = PlayLog::new(&store, "d", 10).unwrap();
            log.set_mode(&store, mode, 0, T0).unwrap();
            assert_eq!(
                log.start(&store, &start_at(T0), 0, &Close::default())
                    .unwrap(),
                None
            );
            assert!(!log.progress(&store, 20_000, T0 + 20_000).unwrap());
            assert!(!log.lyric_jump(&store, 3, 1_000, T0 + 21_000).unwrap());
            log.finish(&store, 30_000, EndReason::Finished, T0 + 30_000)
                .unwrap();
            assert_eq!(store.listening_row_counts().unwrap(), (0, 0), "{mode:?}");
        }
    }

    #[test]
    fn switching_to_private_mid_song_closes_the_play_and_stops_recording() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        let id = log
            .start(&store, &start_at(T0), 0, &Close::default())
            .unwrap()
            .unwrap();
        log.set_mode(&store, ListeningMode::Private, 40_000, T0 + 40_000)
            .unwrap();
        let e = store.play_event(&id).unwrap().unwrap();
        assert_eq!(
            (e.end_reason.as_str(), e.listened_ms),
            ("endedBySystem", 40_000)
        );
        assert_eq!(
            log.start(&store, &start_at(T0 + 50_000), 0, &Close::default())
                .unwrap(),
            None
        );
        assert_eq!(store.listening_row_counts().unwrap(), (1, 0));
    }

    #[test]
    fn lyric_jumps_are_logged_against_the_current_play() {
        let store = Store::open_in_memory().unwrap();
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        assert!(!log.lyric_jump(&store, 1, 0, T0).unwrap()); // nothing playing
        let id = log
            .start(&store, &start_at(T0), 0, &Close::default())
            .unwrap()
            .unwrap();
        assert!(log.lyric_jump(&store, 12, 61_500, T0 + 30_000).unwrap());
        let jumps = store.lyric_jumps(&id).unwrap();
        assert_eq!((jumps[0].line_index, jumps[0].line_start_ms), (12, 61_500));
    }

    #[test]
    fn the_clock_resumes_after_what_is_stored() {
        let store = Store::open_in_memory().unwrap();
        let first = {
            let mut log = PlayLog::new(&store, "d", 10).unwrap();
            let id = log
                .start(&store, &start_at(T0), 0, &Close::default())
                .unwrap()
                .unwrap();
            log.finish(&store, 1, EndReason::Finished, T0).unwrap();
            id
        };
        // The device clock is now behind the stored history.
        let mut log = PlayLog::new(&store, "d", 10).unwrap();
        let second = log
            .start(&store, &start_at(T0 - 60_000), 0, &Close::default())
            .unwrap()
            .unwrap();
        let a = store.play_event(&first).unwrap().unwrap().hlc;
        let b = store.play_event(&second).unwrap().unwrap().hlc;
        assert!(b > a);
    }
}
