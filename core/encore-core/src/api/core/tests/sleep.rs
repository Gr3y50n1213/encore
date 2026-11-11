//! The sleep timer (Phase 9) through the public API, on the Jellyfin recording:
//! the presets, Add 15 min, Cancel and its Undo, a restart, a profile switch, End
//! of song and End of album as the queue changes, and the play log at the end.

use encore_store::SleepTimerRecord;

use super::*;

const MINUTE: i64 = 60_000;

fn minutes(n: u32) -> SleepTimerSpec {
    SleepTimerSpec {
        mode: SleepTimerMode::Minutes,
        minutes: n,
    }
}

fn until(mode: SleepTimerMode) -> SleepTimerSpec {
    SleepTimerSpec { mode, minutes: 0 }
}

/// What the `SleepTimerChanged` events since the last call carried, oldest first.
fn sleep_events(events: &Events) -> Vec<Option<SleepTimer>> {
    events
        .take()
        .into_iter()
        .filter(|e| e.kind == EventKind::SleepTimerChanged)
        .map(|e| e.sleep_timer)
        .collect()
}

/// The timer the newest `SleepTimerChanged` event carried.
fn last_sleep_event(events: &Events) -> Option<SleepTimer> {
    sleep_events(events).pop().flatten()
}

#[test]
fn presets_add_time_cancel_and_undo() {
    let dir = temp_dir("sleep-presets");
    let (core, events) = core_on_recording(&dir);
    assert_eq!(core.sleep_timer().unwrap(), None);
    // Every preset ends that many minutes from now. No profile is needed: the
    // timer is the device's.
    for n in [5, 10, 15, 30, 45, 60] {
        let before = now_ms();
        let t = core.set_sleep_timer(minutes(n)).unwrap();
        let after = now_ms();
        let length = i64::from(n) * MINUTE;
        assert!(
            (before + length..=after + length).contains(&t.ends_at_utc_ms),
            "{n} min"
        );
        assert!((before..=after).contains(&t.set_utc_ms));
        assert_eq!(
            (t.mode, t.stop_after_uid, t.fade_ms),
            (SleepTimerMode::Minutes, None, 10_000)
        );
        assert_eq!(core.sleep_timer().unwrap(), Some(t));
    }
    assert_eq!(sleep_events(&events).len(), 6, "one event per timer set");
    let set = core.sleep_timer().unwrap().unwrap();

    // Add 15 min: the end moves on, the start stays (the sheet's bar).
    let longer = core.extend_sleep_timer(15).unwrap();
    assert_eq!(longer.ends_at_utc_ms, set.ends_at_utc_ms + 15 * MINUTE);
    assert_eq!(longer.set_utc_ms, set.set_utc_ms);
    // Cancel hands the timer back for the toast's Undo, which puts it back.
    assert_eq!(core.cancel_sleep_timer().unwrap(), Some(longer));
    assert_eq!(core.sleep_timer().unwrap(), None);
    assert_eq!(
        core.cancel_sleep_timer().unwrap(),
        None,
        "nothing left to cancel"
    );
    assert_eq!(core.restore_sleep_timer(longer).unwrap(), longer);
    assert_eq!(core.sleep_timer().unwrap(), Some(longer));
    assert_eq!(sleep_events(&events), [Some(longer), None, Some(longer)]);

    // A timer whose end has passed can't come back.
    let ended = SleepTimer {
        ends_at_utc_ms: now_ms() - 1,
        ..longer
    };
    assert!(matches!(
        core.restore_sleep_timer(ended),
        Err(EncoreError::NotAllowed { .. })
    ));
    assert_eq!(
        core.sleep_timer().unwrap(),
        Some(longer),
        "the running one stays"
    );
    // 1 minute to 24 hours.
    for n in [0, 24 * 60 + 1] {
        assert!(matches!(
            core.set_sleep_timer(minutes(n)),
            Err(EncoreError::NotAllowed { .. })
        ));
    }
    // Song and album timers play from the open profile's queue.
    assert!(matches!(
        core.set_sleep_timer(until(SleepTimerMode::EndOfSong)),
        Err(EncoreError::NoProfile { .. })
    ));

    // Testing only: a timer in seconds, for the emulators' 15 s timer.
    let before = now_ms();
    let quick = core.set_sleep_timer_seconds(15).unwrap();
    assert!((before + 15_000..=now_ms() + 15_000).contains(&quick.ends_at_utc_ms));
    assert_eq!(quick.mode, SleepTimerMode::Minutes);
    assert!(core.set_sleep_timer_seconds(0).is_err());

    // Nothing to add time to once it's cancelled.
    core.cancel_sleep_timer().unwrap();
    assert!(matches!(
        core.extend_sleep_timer(15),
        Err(EncoreError::NotFound { .. })
    ));
}

/// The end is saved: a new core on the same data folder (the app restarted) has
/// it. One that ran out while the app was closed is gone at the first read.
#[test]
fn the_timer_survives_a_restart_and_goes_once_it_has_run_out() {
    let dir = temp_dir("sleep-restart");
    let set = {
        let (core, _) = core_on_recording(&dir);
        core.set_sleep_timer(minutes(45)).unwrap()
    };
    let (core, _) = core_on_recording(&dir);
    assert_eq!(core.sleep_timer().unwrap(), Some(set));
    assert_eq!(
        core.sleep_fade_plan(0, 200_000).unwrap().action,
        SleepFadeAction::KeepPlaying
    );

    // It ended a second ago (the player is pausing): still there, and the fade
    // plan says to stop now.
    let just_ended = SleepTimerRecord {
        mode: "minutes".into(),
        set_utc_ms: set.set_utc_ms,
        ends_at_utc_ms: Some(now_ms() - 1_000),
    };
    lock(&core.inner.registry)
        .unwrap()
        .set_sleep_timer(&just_ended)
        .unwrap();
    let plan = core.sleep_fade_plan(50_000, 200_000).unwrap();
    assert_eq!((plan.action, plan.fade_ms), (SleepFadeAction::FadeNow, 0));
    assert!(core.sleep_timer().unwrap().is_some());

    // It ran out two minutes ago, while the app was closed: gone at the first
    // read, from the device registry too.
    let ran_out = SleepTimerRecord {
        ends_at_utc_ms: Some(now_ms() - 2 * MINUTE),
        ..just_ended
    };
    lock(&core.inner.registry)
        .unwrap()
        .set_sleep_timer(&ran_out)
        .unwrap();
    assert_eq!(core.sleep_timer().unwrap(), None);
    assert_eq!(
        lock(&core.inner.registry).unwrap().sleep_timer().unwrap(),
        None
    );
}

/// Board 31, item 7: the timer names no profile, so a profile switch changes
/// nothing; it carries on with no profile open too.
#[test]
fn the_timer_carries_on_through_a_profile_switch() {
    let dir = temp_dir("sleep-switch");
    let swap = Arc::new(Swap(Mutex::new(
        Arc::new(FixtureTransport::from_json(RECORDED).unwrap()) as Arc<dyn Transport>,
    )));
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "ipad".into(),
        device_name: "iPad".into(),
        platform: "test".into(),
    };
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(swap.clone())).unwrap();
    let sam = signed_in(&core).profile_id;
    let set = core.set_sleep_timer(minutes(30)).unwrap();

    // Alex signs in on the same device: Alex's profile opens.
    *swap.0.lock().unwrap() = second_user();
    let alex = signed_in(&core).profile_id;
    assert_ne!(alex, sam);
    assert_eq!(core.sleep_timer().unwrap(), Some(set));
    // Back to Sam with the switcher.
    *swap.0.lock().unwrap() = Arc::new(FixtureTransport::from_json(RECORDED).unwrap());
    block(core.switch_profile(sam.clone(), tokens_of(&sam), false)).unwrap();
    assert_eq!(core.sleep_timer().unwrap(), Some(set));
    core.close_profile().unwrap();
    assert_eq!(
        core.sleep_timer().unwrap(),
        Some(set),
        "and with no profile open"
    );
}

/// End of song and End of album stop after an entry of the queue, worked out
/// again whenever the queue changes; their end is estimated from the position.
#[test]
fn end_of_song_and_album_follow_the_queue() {
    let dir = temp_dir("sleep-album");
    let (core, events) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    assert!(
        matches!(
            core.set_sleep_timer(until(SleepTimerMode::EndOfSong)),
            Err(EncoreError::NotAllowed { .. })
        ),
        "nothing in the queue"
    );
    let album = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.song_count >= 3)
        .unwrap();
    let songs = core.album(album.id).unwrap().songs;
    // Autoplay off: after the album, nothing.
    core.set_autoplay(false).unwrap();
    let state = core
        .play(
            songs.iter().map(|s| s.id).collect(),
            0,
            StartedFrom::Album,
            Some("al".into()),
        )
        .unwrap();
    let first = state.current.clone().unwrap();
    let last = state.up_next.last().unwrap().uid;
    let length = first.song.duration_ms;

    // End of song: before the song starts, all of it is left.
    let before = now_ms();
    let t = core
        .set_sleep_timer(until(SleepTimerMode::EndOfSong))
        .unwrap();
    assert_eq!(
        (t.mode, t.stop_after_uid),
        (SleepTimerMode::EndOfSong, Some(first.uid))
    );
    assert!((before + length..=now_ms() + length).contains(&t.ends_at_utc_ms));
    // Halfway through: half of it.
    core.playback_started(first.uid, 0).unwrap();
    let at = length / 2;
    let before = now_ms();
    core.playback_progress(at, at).unwrap();
    let t = core.sleep_timer().unwrap().unwrap();
    let after = now_ms();
    let left = length - at;
    assert!((before + left..=after + left).contains(&t.ends_at_utc_ms));

    // End of album: its last song; the songs between count too.
    let t = core
        .set_sleep_timer(until(SleepTimerMode::EndOfAlbum))
        .unwrap();
    let after = now_ms();
    assert_eq!(t.stop_after_uid, Some(last));
    let rest: i64 = songs[1..].iter().map(|s| s.duration_ms).sum();
    assert!((before + left + rest..=after + left + rest).contains(&t.ends_at_utc_ms));

    // Repeat One: the song plays again, so the album ends with it. The stop point
    // moves at once, with an event.
    events.take();
    core.set_repeat(RepeatMode::One).unwrap();
    assert_eq!(
        last_sleep_event(&events).unwrap().stop_after_uid,
        Some(first.uid)
    );
    core.set_repeat(RepeatMode::Off).unwrap();
    // Shuffle: the album ends with the last of its songs in the new order.
    let q = core.set_shuffle(true).unwrap();
    assert_eq!(
        last_sleep_event(&events).unwrap().stop_after_uid,
        Some(q.up_next.last().unwrap().uid)
    );
    // Another album's song played next: the album ends with this song.
    let other = core
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .into_iter()
        .find(|s| s.album_id != Some(album.id))
        .unwrap();
    core.play_next(vec![other.id]).unwrap();
    assert_eq!(
        last_sleep_event(&events).unwrap().stop_after_uid,
        Some(first.uid)
    );

    // End of song after a skip: the new current song.
    core.set_sleep_timer(until(SleepTimerMode::EndOfSong))
        .unwrap();
    let now_on = core.skip_next(SkipOrigin::Button).unwrap().current.unwrap();
    assert_eq!(
        core.sleep_timer().unwrap().unwrap().stop_after_uid,
        Some(now_on.uid)
    );
    // Song and album timers never fade: they play to the end, then pause.
    assert_eq!(
        core.sleep_fade_plan(0, 1_000).unwrap().action,
        SleepFadeAction::KeepPlaying
    );
}

/// The end of a timer is never a skip: the play closes as ended by the system
/// (signal "system"), and the next song the morning after starts clean. Also
/// when the timer ran out while paused and nothing said so.
#[test]
fn the_end_of_a_timer_is_never_logged_as_a_skip() {
    let dir = temp_dir("sleep-log");
    let (core, events) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 3)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    core.set_autoplay(false).unwrap();
    let state = core.play(ids, 0, StartedFrom::Songs, None).unwrap();
    let uid = |c: &EncoreCore| c.queue().unwrap().current.unwrap().uid;
    core.playback_started(state.current.unwrap().uid, 0)
        .unwrap();
    core.playback_progress(90_000, 90_000).unwrap();

    // The timer ends: the player faded out, paused, and says so.
    core.set_sleep_timer_seconds(15).unwrap();
    core.playback_paused(90_000).unwrap();
    events.take();
    core.sleep_timer_ended(90_000).unwrap();
    assert_eq!(core.sleep_timer().unwrap(), None);
    assert_eq!(sleep_events(&events), [None]);
    // The next morning: Next. The night's last play stays ended by the system.
    core.skip_next(SkipOrigin::Button).unwrap();
    core.playback_started(uid(&core), 0).unwrap();

    // Song 2 is paused and a timer runs out with nothing said (the app was
    // stopped): the next song still doesn't make song 2 a skip.
    core.playback_progress(30_000, 30_000).unwrap();
    core.playback_paused(30_000).unwrap();
    let ran_out = SleepTimerRecord {
        mode: "minutes".into(),
        set_utc_ms: now_ms() - 10 * MINUTE,
        ends_at_utc_ms: Some(now_ms() - 1_000),
    };
    lock(&core.inner.registry)
        .unwrap()
        .set_sleep_timer(&ran_out)
        .unwrap();
    core.skip_next(SkipOrigin::Button).unwrap();
    core.playback_started(uid(&core), 0).unwrap();
    assert_eq!(core.sleep_timer().unwrap(), None);

    let p = core.inner.profile().unwrap();
    let mut log = p.store.lock().unwrap().recent_play_events(10).unwrap();
    log.reverse();
    let rows: Vec<(&str, Option<&str>, u64)> = log
        .iter()
        .map(|e| (e.end_reason.as_str(), e.signal.as_deref(), e.listened_ms))
        .collect();
    assert_eq!(
        rows,
        [
            ("endedBySystem", Some("system"), 90_000),
            ("endedBySystem", Some("system"), 30_000),
            ("inProgress", None, 0),
        ],
        "no skip anywhere"
    );
}
