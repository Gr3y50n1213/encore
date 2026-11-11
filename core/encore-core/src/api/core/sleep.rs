//! The sleep timer (Phase 9; PLAN-UX, "Sleep timer"; board 31).
//!
//! The core owns the timer: its end, where a song or album timer stops in the
//! queue (Repeat One, shuffle and Autoplay included), the fade rule, and how the
//! play log reads the end: ended by the system, never a skip. The platform runs
//! the clock, the fade and the card.
//!
//! The timer is the device's, kept in the device registry (`device.sqlite`), not
//! in a profile: it carries on through a profile switch and survives the app being
//! closed. It ends when playback is handed to another device.
//!
//! Lock order: the registry is never held while the live, playback or store locks
//! are taken.

use super::{EncoreCore, Inner, lock, now_ms};
use crate::api::types::*;
use crate::playlog::EndReason as LogEnd;
use crate::queue::{Queue, RepeatMode as QueueRepeat};
use crate::tokens::tokens;
use encore_store::SleepTimerRecord;

/// The longest timer, and the furthest Add 15 min reaches: a day.
const MAX_MS: i64 = 24 * 60 * 60_000;
/// A timer of minutes whose end passed this long ago is cleared when it is read:
/// nothing here stopped the music then (the app was closed, say). Until then the
/// fade plan says to stop now.
const STALE_AFTER_MS: i64 = 60_000;

const KEEP_PLAYING: SleepFadePlan = SleepFadePlan {
    action: SleepFadeAction::KeepPlaying,
    fade_from_ms: 0,
    fade_ms: 0,
};

fn not_allowed(reason: &str) -> EncoreError {
    EncoreError::NotAllowed {
        reason: reason.into(),
    }
}

fn too_long() -> EncoreError {
    not_allowed("A sleep timer runs from 1 minute to 24 hours")
}

/// A timer of `minutes`, in milliseconds.
fn minutes_ms(minutes: u32) -> Result<i64, EncoreError> {
    let ms = i64::from(minutes) * 60_000;
    if (1..=MAX_MS).contains(&ms) {
        Ok(ms)
    } else {
        Err(too_long())
    }
}

/// How long the fade lasts (`motion.timing.sleepTimerFadeOutMs`).
fn fade_length() -> i64 {
    tokens().sleep_fade_ms
}

fn mode_name(mode: SleepTimerMode) -> &'static str {
    match mode {
        SleepTimerMode::Minutes => "minutes",
        SleepTimerMode::EndOfSong => "endOfSong",
        SleepTimerMode::EndOfAlbum => "endOfAlbum",
    }
}

fn mode_from(name: &str) -> Option<SleepTimerMode> {
    match name {
        "minutes" => Some(SleepTimerMode::Minutes),
        "endOfSong" => Some(SleepTimerMode::EndOfSong),
        "endOfAlbum" => Some(SleepTimerMode::EndOfAlbum),
        _ => None,
    }
}

/// The timer as the registry keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stored {
    mode: SleepTimerMode,
    set_utc_ms: i64,
    /// Set for Minutes; a song or album timer's end is worked out when read.
    ends_at_utc_ms: Option<i64>,
}

impl Stored {
    fn from_record(r: &SleepTimerRecord) -> Option<Stored> {
        Some(Stored {
            mode: mode_from(&r.mode)?,
            set_utc_ms: r.set_utc_ms,
            ends_at_utc_ms: r.ends_at_utc_ms,
        })
    }

    fn record(&self) -> SleepTimerRecord {
        SleepTimerRecord {
            mode: mode_name(self.mode).to_owned(),
            set_utc_ms: self.set_utc_ms,
            ends_at_utc_ms: self.ends_at_utc_ms,
        }
    }

    /// A timer of minutes whose end has come.
    fn ended(&self, now: i64) -> bool {
        self.mode == SleepTimerMode::Minutes && self.ends_at_utc_ms.is_none_or(|end| now >= end)
    }

    /// A timer of minutes that ended long enough ago that nothing is left for it to do.
    fn stale(&self, now: i64) -> bool {
        self.mode == SleepTimerMode::Minutes
            && self
                .ends_at_utc_ms
                .is_none_or(|end| now >= end + STALE_AFTER_MS)
    }
}

/// The fade rule, one for every platform (PLAN-UX: "It fades out over the last 10
/// seconds (less if the song ends sooner), pauses"). `ends_at` is the timer's end,
/// `position_ms` and `duration_ms` the current song's (a duration of 0 or less:
/// unknown, so the clock alone decides), `fade` the fade's length.
fn fade_plan(
    ends_at: i64,
    now: i64,
    position_ms: i64,
    duration_ms: i64,
    fade: i64,
) -> SleepFadePlan {
    let position = position_ms.max(0);
    let left = ends_at - now;
    if left <= 0 {
        // The end has passed: stop now.
        return SleepFadePlan {
            action: SleepFadeAction::FadeNow,
            fade_from_ms: position,
            fade_ms: 0,
        };
    }
    if duration_ms > 0 {
        // The song ends inside the last `fade` before the end: it fades from where
        // the last `fade` begins (or from now, inside it) to its own end, and the
        // music stops with it.
        let song_left = (duration_ms - position).max(0);
        if (left - fade..=left).contains(&song_left) {
            let wait = (left - fade).max(0);
            return SleepFadePlan {
                action: SleepFadeAction::StopAtSongEnd,
                fade_from_ms: position + wait,
                fade_ms: song_left - wait,
            };
        }
    }
    if left <= fade {
        return SleepFadePlan {
            action: SleepFadeAction::FadeNow,
            fade_from_ms: position,
            fade_ms: left,
        };
    }
    KEEP_PLAYING
}

/// The entries that follow the current one as each ends by itself, in order, as
/// far as they can be named now (uid, song): Up Next, then Autoplay. Repeat One
/// has none (the song plays again). Repeat All stops at the end of Up Next: the
/// session then starts again with new entries. Songs that can't play offline are
/// passed over, as the queue passes over them.
fn upcoming(q: &Queue) -> Vec<(u64, i64)> {
    let v = q.view();
    if v.repeat == QueueRepeat::One {
        return Vec::new();
    }
    let mut out: Vec<(u64, i64)> = v
        .up_next
        .iter()
        .filter(|i| q.can_play(i.track_id))
        .map(|i| (i.uid, i.track_id))
        .collect();
    if v.repeat == QueueRepeat::Off && v.autoplay_enabled {
        out.extend(
            v.autoplay
                .iter()
                .filter(|i| q.can_play(i.track_id))
                .map(|i| (i.uid, i.track_id)),
        );
    }
    out
}

/// The entry after which a song or album timer pauses: End of song, the current
/// one; End of album, the last of the current album's songs that follow it in a
/// row (a song on no album, or Repeat One, ends with itself). None for Minutes, or
/// when nothing is queued. `album_of` gives a song's album.
fn stop_after(
    q: &Queue,
    mode: SleepTimerMode,
    album_of: impl Fn(i64) -> Option<i64>,
) -> Option<u64> {
    let current = q.current()?;
    match mode {
        SleepTimerMode::Minutes => None,
        SleepTimerMode::EndOfSong => Some(current.uid),
        SleepTimerMode::EndOfAlbum => {
            let Some(album) = album_of(current.track_id) else {
                return Some(current.uid);
            };
            let mut last = current.uid;
            for (uid, track_id) in upcoming(q) {
                if album_of(track_id) != Some(album) {
                    break;
                }
                last = uid;
            }
            Some(last)
        }
    }
}

impl Inner {
    /// The stored timer. One that ran out long ago (`STALE_AFTER_MS`) ends here.
    fn sleep_stored(&self, now: i64) -> Result<Option<Stored>, EncoreError> {
        let stored = lock(&self.registry)?
            .sleep_timer()?
            .as_ref()
            .and_then(Stored::from_record);
        match stored {
            Some(s) if s.stale(now) => {
                self.sleep_ran_out()?;
                Ok(None)
            }
            other => Ok(other),
        }
    }

    /// Write the timer, tell the app, and return it as the API shows it.
    fn sleep_save(&self, s: &Stored, now: i64) -> Result<SleepTimer, EncoreError> {
        lock(&self.registry)?.set_sleep_timer(&s.record())?;
        let timer = self.sleep_view(s, now);
        self.emit_sleep(Some(timer));
        Ok(timer)
    }

    fn emit_sleep(&self, timer: Option<SleepTimer>) {
        let mut e = CoreEvent::of(EventKind::SleepTimerChanged);
        e.sleep_timer = timer;
        self.emit(e);
    }

    /// Clear the timer, and tell the app when there was one.
    pub(super) fn sleep_clear(&self) -> Result<(), EncoreError> {
        let had = lock(&self.registry)?.clear_sleep_timer()?;
        if had {
            self.emit_sleep(None);
        }
        Ok(())
    }

    /// A timer of minutes ran out and nothing here said so (`sleep_timer_ended`):
    /// it is cleared, and a paused play is closed as ended by the system, so the
    /// next song can't make it a skip. A play that is still playing is left alone.
    fn sleep_ran_out(&self) -> Result<(), EncoreError> {
        if let Ok(p) = self.profile() {
            let playing = lock(&p.live)
                .map(|l| l.position_and_playing(now_ms()).1)
                .unwrap_or(true);
            if !playing {
                let mut pb = lock(&p.playback)?;
                let store = lock(&p.store)?;
                let listened = pb.last_listened_ms;
                pb.pending_origin = None;
                pb.log
                    .finish(&store, listened, LogEnd::EndedBySystem, now_ms())?;
            }
        }
        self.sleep_clear()
    }

    /// Before a song starts (`playback_started`): a timer of minutes whose end has
    /// come ends now, closing a paused play as ended by the system.
    pub(super) fn sleep_before_start(&self) {
        let now = now_ms();
        if let Ok(Some(s)) = self.sleep_stored(now)
            && s.ended(now)
        {
            let _ = self.sleep_ran_out();
        }
    }

    /// The queue changed: a song or album timer's stop point and end move with it.
    pub(super) fn sleep_queue_changed(&self) {
        let now = now_ms();
        if let Ok(Some(s)) = self.sleep_stored(now)
            && s.mode != SleepTimerMode::Minutes
        {
            let timer = self.sleep_view(&s, now);
            self.emit_sleep(Some(timer));
        }
    }

    /// The timer as the API shows it. A song or album timer is worked out from the
    /// open profile's queue; with nothing queued it ends now, with no stop point.
    fn sleep_view(&self, s: &Stored, now: i64) -> SleepTimer {
        let (ends_at_utc_ms, stop) = match (s.mode, s.ends_at_utc_ms) {
            (SleepTimerMode::Minutes, end) => (end.unwrap_or(now), None),
            (mode, _) => self
                .sleep_estimate(mode, now)
                .map(|(end, uid)| (end, Some(uid)))
                .unwrap_or((now, None)),
        };
        SleepTimer {
            mode: s.mode,
            set_utc_ms: s.set_utc_ms,
            ends_at_utc_ms,
            stop_after_uid: stop.map(|uid| uid as i64),
            fade_ms: fade_length(),
        }
    }

    /// A song or album timer's end and stop point: what is left of the current
    /// song (from the player's position once it has started it), plus the songs up
    /// to the stop point.
    fn sleep_estimate(&self, mode: SleepTimerMode, now: i64) -> Option<(i64, u64)> {
        let p = self.profile().ok()?;
        let (position, _) = lock(&p.live).ok()?.position_and_playing(now);
        let pb = lock(&p.playback).ok()?;
        let store = lock(&p.store).ok()?;
        let length = |track_id: i64| {
            store
                .track(track_id)
                .map(|t| t.duration_ms as i64)
                .unwrap_or(0)
        };
        let album_of = |track_id: i64| store.track(track_id).ok().and_then(|t| t.album_id);
        let current = pb.queue.current()?;
        let stop = stop_after(&pb.queue, mode, album_of)?;
        let position = if pb.playing_uid == Some(current.uid) {
            position.max(0)
        } else {
            0
        };
        let mut left = (length(current.track_id) - position).max(0);
        if stop != current.uid {
            for (uid, track_id) in upcoming(&pb.queue) {
                left += length(track_id);
                if uid == stop {
                    break;
                }
            }
        }
        Some((now + left, stop))
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Start the sleep timer, replacing any other (board 31). Minutes: it ends
    /// `spec.minutes` from now (1 to 1,440), fading out over the last `fade_ms`.
    /// EndOfSong and EndOfAlbum: the music plays to the end of the song or album,
    /// then pauses, with no fade; they need something in the queue (`NotAllowed`)
    /// and an open profile. `SleepTimerChanged` follows.
    pub fn set_sleep_timer(&self, spec: SleepTimerSpec) -> Result<SleepTimer, EncoreError> {
        let now = now_ms();
        let stored = match spec.mode {
            SleepTimerMode::Minutes => Stored {
                mode: SleepTimerMode::Minutes,
                set_utc_ms: now,
                ends_at_utc_ms: Some(now + minutes_ms(spec.minutes)?),
            },
            mode => {
                let p = self.inner.profile()?;
                if lock(&p.playback)?.queue.current().is_none() {
                    return Err(not_allowed("Play something first"));
                }
                Stored {
                    mode,
                    set_utc_ms: now,
                    ends_at_utc_ms: None,
                }
            }
        };
        self.inner.sleep_save(&stored, now)
    }

    /// Add 15 min (or any `minutes`): the end moves on that much. A song or album
    /// timer becomes a timer of minutes ending that long after the song or album
    /// would have ended. `NotFound` when no timer runs.
    pub fn extend_sleep_timer(&self, minutes: u32) -> Result<SleepTimer, EncoreError> {
        let add = minutes_ms(minutes)?;
        let now = now_ms();
        let s = self
            .inner
            .sleep_stored(now)?
            .ok_or_else(|| EncoreError::NotFound {
                reason: "No sleep timer is set".into(),
            })?;
        let ends_at = self.inner.sleep_view(&s, now).ends_at_utc_ms.max(now) + add;
        if ends_at - now > MAX_MS {
            return Err(too_long());
        }
        let longer = Stored {
            mode: SleepTimerMode::Minutes,
            set_utc_ms: s.set_utc_ms,
            ends_at_utc_ms: Some(ends_at),
        };
        self.inner.sleep_save(&longer, now)
    }

    /// Cancel timer (no confirmation): returns the timer it cancelled, for the
    /// "Timer cancelled" toast's Undo (`restore_sleep_timer`); None when none ran.
    pub fn cancel_sleep_timer(&self) -> Result<Option<SleepTimer>, EncoreError> {
        let now = now_ms();
        let Some(s) = self.inner.sleep_stored(now)? else {
            return Ok(None);
        };
        let timer = self.inner.sleep_view(&s, now);
        self.inner.sleep_clear()?;
        Ok(Some(timer))
    }

    /// Undo a cancel: the timer `cancel_sleep_timer` returned comes back as it
    /// was. The 5 s Undo window is the app's. A timer whose end has passed is
    /// refused (`NotAllowed`).
    pub fn restore_sleep_timer(&self, timer: SleepTimer) -> Result<SleepTimer, EncoreError> {
        let now = now_ms();
        if timer.ends_at_utc_ms <= now {
            return Err(not_allowed("That sleep timer has already ended"));
        }
        if timer.ends_at_utc_ms - now > MAX_MS {
            return Err(too_long());
        }
        let stored = Stored {
            mode: timer.mode,
            set_utc_ms: timer.set_utc_ms.min(now),
            ends_at_utc_ms: (timer.mode == SleepTimerMode::Minutes).then_some(timer.ends_at_utc_ms),
        };
        self.inner.sleep_save(&stored, now)
    }

    /// The running timer, or None. A timer of minutes that ended over a minute ago
    /// is cleared here (it ran out while the app was closed). No profile needed.
    pub fn sleep_timer(&self) -> Result<Option<SleepTimer>, EncoreError> {
        let now = now_ms();
        Ok(self
            .inner
            .sleep_stored(now)?
            .map(|s| self.inner.sleep_view(&s, now)))
    }

    /// The player faded out and paused at the timer's end (or a song or album
    /// timer's last song finished). The open play is closed as ended by the system
    /// with `listened_ms` (0 when it was already reported finished), never as a
    /// skip; the queue stays where it is; the timer is cleared. When the person
    /// plays again, report `playback_started` for the song (a new play), not
    /// `playback_resumed`.
    pub fn sleep_timer_ended(&self, listened_ms: i64) -> Result<(), EncoreError> {
        if let Ok(p) = self.inner.profile() {
            {
                let mut pb = lock(&p.playback)?;
                let store = lock(&p.store)?;
                pb.pending_origin = None;
                pb.log.finish(
                    &store,
                    listened_ms.max(0) as u64,
                    LogEnd::EndedBySystem,
                    now_ms(),
                )?;
                pb.last_listened_ms = 0;
            }
            self.inner.note_playback(Some(false), None);
        }
        self.inner.sleep_clear()
    }

    /// The fade rule, the same on every platform. Call it from the player's tick
    /// (about once a second) with the current song's position and length; it says
    /// whether to play on, fade now, or fade into the song's end and stop there.
    /// Song and album timers never fade (KeepPlaying).
    pub fn sleep_fade_plan(
        &self,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<SleepFadePlan, EncoreError> {
        let now = now_ms();
        Ok(match self.inner.sleep_stored(now)? {
            Some(Stored {
                mode: SleepTimerMode::Minutes,
                ends_at_utc_ms: Some(end),
                ..
            }) => fade_plan(end, now, position_ms, duration_ms, fade_length()),
            _ => KEEP_PLAYING,
        })
    }

    /// Testing only: a timer of `seconds` (Minutes mode), so emulator and device
    /// tests can run a 15 s timer. Apps never offer it.
    pub fn set_sleep_timer_seconds(&self, seconds: u32) -> Result<SleepTimer, EncoreError> {
        let length = i64::from(seconds) * 1000;
        if !(1..=MAX_MS).contains(&length) {
            return Err(too_long());
        }
        let now = now_ms();
        let quick = Stored {
            mode: SleepTimerMode::Minutes,
            set_utc_ms: now,
            ends_at_utc_ms: Some(now + length),
        };
        self.inner.sleep_save(&quick, now)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::queue::NewItem;

    const T: i64 = 1_762_400_000_000;
    const F: i64 = 10_000;

    fn plan(action: SleepFadeAction, fade_from_ms: i64, fade_ms: i64) -> SleepFadePlan {
        SleepFadePlan {
            action,
            fade_from_ms,
            fade_ms,
        }
    }

    #[test]
    fn the_fade_rule_is_one_table_for_every_platform() {
        use crate::api::types::SleepFadeAction::{FadeNow, KeepPlaying, StopAtSongEnd};
        // (time left on the timer, position, song length) and what the player does.
        let cases = [
            // Ten minutes left: play on.
            (600_000, 0, 200_000, plan(KeepPlaying, 0, 0)),
            // The song ends 20 s before the timer, before its last 10 s: play on;
            // the next song fades.
            (40_000, 180_000, 200_000, plan(KeepPlaying, 0, 0)),
            // It ends exactly as the last 10 s begin: stop with it, nothing to fade.
            (40_000, 170_000, 200_000, plan(StopAtSongEnd, 200_000, 0)),
            // It ends 5 s into the last 10 s: fade from 15 s before the end for 5 s.
            (
                25_000,
                180_000,
                200_000,
                plan(StopAtSongEnd, 195_000, 5_000),
            ),
            // It ends with the timer: the whole fade, then its own end.
            (
                20_000,
                180_000,
                200_000,
                plan(StopAtSongEnd, 190_000, 10_000),
            ),
            // Inside the last 10 s and the song ends sooner: over what's left of it.
            (8_000, 196_000, 200_000, plan(StopAtSongEnd, 196_000, 4_000)),
            // The last 10 s begin mid-song: fade now, over all of them.
            (10_000, 30_000, 200_000, plan(FadeNow, 30_000, 10_000)),
            // Inside them (the tick came late): over what is left.
            (8_000, 30_000, 200_000, plan(FadeNow, 30_000, 8_000)),
            // The end has passed: stop now.
            (0, 50_000, 200_000, plan(FadeNow, 50_000, 0)),
            (-5_000, 50_000, 200_000, plan(FadeNow, 50_000, 0)),
            // The song's length unknown: the clock alone.
            (5_000, 50_000, 0, plan(FadeNow, 50_000, 5_000)),
            (50_000, 50_000, 0, plan(KeepPlaying, 0, 0)),
        ];
        for (left, position, duration, want) in cases {
            assert_eq!(
                fade_plan(T + left, T, position, duration, F),
                want,
                "{left} ms left, at {position} of {duration}"
            );
        }
    }

    #[test]
    fn the_fade_comes_from_the_tokens_and_the_end_is_kept_a_minute() {
        assert_eq!(fade_length(), 10_000);
        for m in [
            SleepTimerMode::Minutes,
            SleepTimerMode::EndOfSong,
            SleepTimerMode::EndOfAlbum,
        ] {
            assert_eq!(mode_from(mode_name(m)), Some(m));
        }
        assert_eq!(mode_from("later"), None, "a newer app's mode is no timer");
        let s = Stored {
            mode: SleepTimerMode::Minutes,
            set_utc_ms: T,
            ends_at_utc_ms: Some(T + 60_000),
        };
        assert!(!s.ended(T + 59_999) && s.ended(T + 60_000));
        assert!(!s.stale(T + 60_000 + STALE_AFTER_MS - 1));
        assert!(s.stale(T + 60_000 + STALE_AFTER_MS));
        let song = Stored {
            mode: SleepTimerMode::EndOfSong,
            set_utc_ms: T,
            ends_at_utc_ms: None,
        };
        assert!(
            !song.ended(T + MAX_MS) && !song.stale(T + MAX_MS),
            "a song timer ends with its song"
        );
        assert_eq!(Stored::from_record(&s.record()), Some(s));
    }

    /// Songs 1 to 4 are on album 10, 5 and 6 on album 20; 7 is on none.
    fn album_of(track_id: i64) -> Option<i64> {
        match track_id {
            1..=4 => Some(10),
            5 | 6 => Some(20),
            _ => None,
        }
    }

    fn item(track_id: i64) -> NewItem {
        NewItem {
            track_id,
            chosen_by: crate::queue::ChosenBy::User,
            started_from: crate::queue::StartedFrom::Album,
            context_id: None,
        }
    }

    fn queue_of(tracks: &[i64]) -> Queue {
        let mut q = Queue::new(7);
        q.play(tracks.iter().map(|&t| item(t)).collect(), 0);
        q
    }

    /// The uid of a song's first entry (current, Up Next, then Autoplay).
    fn uid(q: &Queue, track_id: i64) -> u64 {
        let v = q.view();
        v.current
            .into_iter()
            .chain(v.up_next)
            .chain(v.autoplay)
            .find(|i| i.track_id == track_id)
            .map(|i| i.uid)
            .expect("in the queue")
    }

    fn stop(q: &Queue, mode: SleepTimerMode) -> Option<u64> {
        stop_after(q, mode, album_of)
    }

    #[test]
    fn end_of_song_and_album_follow_repeat_and_autoplay() {
        use crate::api::types::SleepTimerMode::{EndOfAlbum, EndOfSong, Minutes};
        // Album 10, then album 20.
        let mut q = queue_of(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(stop(&q, Minutes), None);
        assert_eq!(stop(&q, EndOfSong), Some(uid(&q, 1)));
        assert_eq!(stop(&q, EndOfAlbum), Some(uid(&q, 4)));
        // Repeat One plays the song again: the album ends with it too.
        q.set_repeat(QueueRepeat::One);
        assert_eq!(stop(&q, EndOfSong), Some(uid(&q, 1)));
        assert_eq!(stop(&q, EndOfAlbum), Some(uid(&q, 1)));

        // Repeat All: the album ends with Up Next (the session then starts again),
        // and Autoplay isn't reached.
        let mut q = queue_of(&[1, 2, 3]);
        q.set_repeat(QueueRepeat::All);
        q.set_autoplay(vec![item(4)]);
        assert_eq!(stop(&q, EndOfAlbum), Some(uid(&q, 3)));

        // Autoplay: the album carries on into it when Up Next runs out...
        let mut q = queue_of(&[3, 4]);
        q.set_autoplay(vec![item(1), item(2), item(5)]);
        let first_fill = uid(&q, 2);
        assert_eq!(stop(&q, EndOfAlbum), Some(first_fill));
        // ...a refill brings new entries, and the stop point moves to them...
        q.set_autoplay(vec![item(2), item(6)]);
        let refilled = stop(&q, EndOfAlbum).unwrap();
        assert_eq!(refilled, uid(&q, 2));
        assert_ne!(refilled, first_fill);
        // ...and with Autoplay off the album ends with Up Next.
        q.set_autoplay_enabled(false);
        assert_eq!(stop(&q, EndOfAlbum), Some(uid(&q, 4)));

        // Offline, a song that can't play is passed over, not the album's end.
        let mut q = queue_of(&[1, 2, 3, 4, 5]);
        q.set_playable(Some(HashSet::from([1, 2, 4, 5])));
        assert_eq!(stop(&q, EndOfAlbum), Some(uid(&q, 4)));

        // A song on no album ends with itself; nothing queued, nothing to stop.
        let q = queue_of(&[7, 1]);
        assert_eq!(stop(&q, EndOfAlbum), Some(uid(&q, 7)));
        assert_eq!(stop(&Queue::new(1), EndOfSong), None);
    }

    #[test]
    fn with_shuffle_the_album_ends_where_its_run_ends_in_the_shuffled_order() {
        for seed in 0..20 {
            let mut q = Queue::new(seed);
            q.play([1, 5, 2, 6, 3, 4].map(item).to_vec(), 0);
            q.set_shuffle(true);
            // The order the songs will play in now.
            let ahead: Vec<(u64, i64)> = q
                .view()
                .up_next
                .iter()
                .map(|i| (i.uid, i.track_id))
                .collect();
            let run = ahead
                .iter()
                .take_while(|(_, t)| album_of(*t) == Some(10))
                .count();
            let want = if run == 0 {
                uid(&q, 1)
            } else {
                ahead[run - 1].0
            };
            assert_eq!(
                stop(&q, SleepTimerMode::EndOfAlbum),
                Some(want),
                "seed {seed}"
            );
            // Unshuffled, song 5 (album 20) is next: the album ends with song 1.
            q.set_shuffle(false);
            assert_eq!(stop(&q, SleepTimerMode::EndOfAlbum), Some(uid(&q, 1)));
        }
    }
}
