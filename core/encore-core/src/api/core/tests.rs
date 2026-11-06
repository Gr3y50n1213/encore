//! The public API end to end on the real Jellyfin 12.0.0 recording: connect, sign
//! in with Quick Connect, sync, browse, search, play an album, report playback.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use encore_sources::{FixtureTransport, Transport};

use super::*;

const RECORDED: &str = include_str!("../../../../../tools/fixtures/jellyfin/recorded.json");

#[derive(Default)]
struct Events(Mutex<Vec<CoreEvent>>);

impl CoreListener for Events {
    fn on_event(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Events {
    fn kinds(&self) -> Vec<EventKind> {
        self.0.lock().unwrap().iter().map(|e| e.kind).collect()
    }
    fn take(&self) -> Vec<CoreEvent> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(name: &str) -> TempDir {
    let d = std::env::temp_dir().join(format!(
        "encore-api-{name}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir_all(&d).unwrap();
    TempDir(d)
}

fn core_on_recording(dir: &TempDir) -> (Arc<EncoreCore>, Arc<Events>) {
    let t: Arc<dyn Transport> = Arc::new(FixtureTransport::from_json(RECORDED).unwrap());
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    };
    // The recording was made with pages of 50.
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(t)).unwrap();
    let events = Arc::new(Events::default());
    core.set_listener(events.clone());
    (core, events)
}

fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

/// Connect and sign in with Quick Connect, as the recording did.
fn signed_in(core: &EncoreCore) -> SignedIn {
    let check = block(core.check_address("http://jellyfin.local:8096".into())).unwrap();
    assert_eq!(check.version, "12.0.0");
    assert!(check.plain_http_on_local_network);
    let start = block(core.quick_connect_start(check.base_url.clone())).unwrap();
    assert_eq!(start.code.chars().count(), 6);
    assert_eq!(start.expires_in_seconds, 300);
    block(core.quick_connect_wait()).unwrap()
}

/// The production path (`EncoreCore::new`: real HTTP, the real page size and poll
/// interval) against tools/ci/replay_jellyfin.py. Ignored because it needs that
/// server: CI starts it and runs this with `--ignored` (tools/ci/replay.sh).
#[test]
#[ignore = "needs ENCORE_REPLAY_URL: tools/ci/replay.sh"]
fn library_flow_over_http_against_the_replay_server() {
    let url = std::env::var("ENCORE_REPLAY_URL").expect("set ENCORE_REPLAY_URL");
    let dir = temp_dir("http");
    let core = EncoreCore::new(CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    })
    .unwrap();
    let check = block(core.check_address(url)).unwrap();
    assert_eq!(check.version, "12.0.0");
    assert!(check.plain_http_on_local_network);
    block(core.quick_connect_start(check.base_url)).unwrap();
    let signed_in = block(core.quick_connect_wait()).unwrap();
    assert_eq!(core.current_profile(), Some(signed_in.profile_id));
    let synced = block(core.sync_library()).unwrap();
    assert_eq!(
        (synced[0].artists, synced[0].albums, synced[0].songs),
        (26, 38, 150)
    );
    // A second sync is a delta, cursored by the replayed Date header.
    let again = block(core.sync_library()).unwrap();
    assert_eq!(again[0].removed, 0);
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);
}

/// Dart can release the last reference on another runtime's worker; dropping a
/// tokio runtime there panics unless it is shut down in the background.
#[test]
fn dropping_the_core_inside_another_runtime_does_not_panic() {
    let dir = temp_dir("drop");
    let (core, _) = core_on_recording(&dir);
    block(core.check_address("http://music.example.org".into())).unwrap_err();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap()
        .block_on(async move { drop(core) });
}

#[test]
fn quick_connect_creates_a_profile_and_hands_the_token_to_the_app() {
    let dir = temp_dir("signin");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    assert!(s.profile_id.starts_with("jf-"));
    assert_eq!(
        s.credential_key,
        format!("encore/{}/{}", s.profile_id, s.source_id)
    );
    assert_eq!(s.token, "anonymized-token");
    assert!(
        !format!("{s:?}").contains("anonymized-token"),
        "the token never reaches a log"
    );
    assert_eq!(core.current_profile(), Some(s.profile_id.clone()));
    // The token is not on disk: only the app's keychain holds it.
    let db = std::fs::read(
        dir.0
            .join("profiles")
            .join(&s.profile_id)
            .join("library.sqlite"),
    )
    .unwrap();
    assert!(
        !db.windows(b"anonymized-token".len())
            .any(|w| w == b"anonymized-token")
    );
}

#[test]
fn sync_then_browse_search_and_list_servers_offline() {
    let dir = temp_dir("browse");
    let (core, events) = core_on_recording(&dir);
    signed_in(&core);
    let results = block(core.sync_library()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        (
            results[0].artists,
            results[0].albums,
            results[0].songs,
            results[0].playlists
        ),
        (26, 38, 150, 1)
    );
    assert!(events.kinds().contains(&EventKind::SyncProgress));

    let servers = core.servers().unwrap();
    assert_eq!(servers[0].song_count, 150);
    assert!(servers[0].signed_in && servers[0].last_sync_utc_ms.is_some());

    let albums = core.albums(AlbumSort::Title, 0, 100).unwrap();
    assert_eq!(albums.len(), 38);
    let detail = core.album(albums[0].id).unwrap();
    assert_eq!(detail.songs.len() as u32, detail.album.song_count);
    assert!(!core.artists(0, 100).unwrap().is_empty());
    let playlist = &core.playlists(0, 10).unwrap()[0];
    assert!(!core.playlist(playlist.id).unwrap().entries.is_empty());
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);

    let title = detail.songs[0].title.clone();
    let found = core.search(title.clone()).unwrap();
    assert!(found.songs.iter().any(|s| s.title == title));
    core.add_recent_search(title.clone()).unwrap();
    assert_eq!(core.recent_searches().unwrap(), vec![title]);
    assert!(matches!(
        core.album(999_999),
        Err(EncoreError::NotFound { .. })
    ));
}

#[test]
fn play_an_album_report_it_and_preload_the_next_song() {
    let dir = temp_dir("play");
    let (core, events) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let album = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.song_count >= 2)
        .unwrap();
    let songs = core.album(album.id).unwrap().songs;
    let ids: Vec<i64> = songs.iter().map(|s| s.id).collect();
    events.take();

    let state = core
        .play(ids.clone(), 0, StartedFrom::Album, Some("al".into()))
        .unwrap();
    let current = state.current.clone().unwrap();
    assert_eq!(current.song.id, ids[0]);
    assert_eq!(state.up_next.len(), ids.len() - 1);
    assert_eq!(
        events.kinds(),
        [EventKind::QueueChanged, EventKind::NowPlaying]
    );

    core.playback_started(current.uid, -300).unwrap();
    // Near the end the core asks the player to preload the next song, once.
    let near_end = current.song.duration_ms - 5_000;
    core.playback_progress(near_end, near_end).unwrap();
    core.playback_progress(near_end + 1_000, near_end + 1_000)
        .unwrap();
    let preloads: Vec<CoreEvent> = events
        .take()
        .into_iter()
        .filter(|e| e.kind == EventKind::Preload)
        .collect();
    assert_eq!(preloads.len(), 1);
    assert_eq!(preloads[0].preload.as_ref().unwrap().song.id, ids[1]);

    let next = core
        .playback_ended(EndReason::Finished, current.song.duration_ms)
        .unwrap()
        .unwrap();
    assert_eq!(next.song.id, ids[1]);
    // The finished play is in the log with its section 5 fields.
    let p = core.inner.profile().unwrap();
    let log = p.store.lock().unwrap().recent_play_events(5).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(
        (
            log[0].end_reason.as_str(),
            log[0].started_from.as_str(),
            log[0].chosen_by.as_str()
        ),
        ("finished", "album", "user")
    );
    assert_eq!(log[0].utc_offset_minutes, -300);
    assert_eq!(log[0].listened_ms as i64, current.song.duration_ms);
    assert_eq!(log[0].device_id, "device-test");
}

#[test]
fn private_listening_records_nothing() {
    let dir = temp_dir("private");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let id = core.songs(SongSort::Title, 0, 1).unwrap()[0].id;
    core.set_listening_mode(ListeningMode::Private).unwrap();
    let state = core.play(vec![id], 0, StartedFrom::Songs, None).unwrap();
    core.playback_started(state.current.unwrap().uid, 0)
        .unwrap();
    core.lyric_line_tapped(1, 1000).unwrap();
    core.playback_ended(EndReason::Finished, 10_000).unwrap();
    let p = core.inner.profile().unwrap();
    assert_eq!(
        p.store.lock().unwrap().listening_row_counts().unwrap(),
        (0, 0)
    );
}

#[test]
fn queue_controls_from_the_api() {
    let dir = temp_dir("queue");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 5)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    core.play(ids.clone(), 0, StartedFrom::Songs, None).unwrap();
    let s = core.next().unwrap();
    assert_eq!(s.current.as_ref().unwrap().song.id, ids[1]);
    assert_eq!(
        core.previous(10_000, false).unwrap(),
        PreviousResult::Restarted
    );
    assert_eq!(core.previous(10_000, true).unwrap(), PreviousResult::Moved);
    let s = core.set_repeat(RepeatMode::One).unwrap();
    assert_eq!(s.repeat, RepeatMode::One);
    let last = s.up_next.last().unwrap().uid;
    let s = core.remove_from_queue(last).unwrap();
    assert!(s.can_undo);
    let s = core.undo_queue().unwrap();
    assert_eq!(s.up_next.len(), 4);
    let s = core.clear_up_next().unwrap();
    assert!(s.up_next.is_empty());
}

#[test]
fn streams_use_the_quality_setting_and_never_log_the_token() {
    let dir = temp_dir("stream");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let id = core.songs(SongSort::Title, 0, 1).unwrap()[0].id;
    let wifi = core.stream_for(id, Network::Wifi).unwrap();
    assert!(
        wifi.url.contains("/stream?static=true"),
        "Wi-Fi defaults to the original file"
    );
    assert!(wifi.url_with_api_key.contains("ApiKey="));
    assert!(!format!("{wifi:?}").contains("ApiKey"));
    let cell = core.stream_for(id, Network::Cellular).unwrap();
    assert!(cell.url.contains("/universal") && cell.url.contains("maxStreamingBitrate=320000"));
    core.set_stream_quality(Network::Cellular, StreamQuality::DataSaver)
        .unwrap();
    assert!(
        core.stream_for(id, Network::Cellular)
            .unwrap()
            .url
            .contains("maxStreamingBitrate=96000")
    );
    let art = core.artwork(ArtworkOf::Song, id, 300).unwrap();
    assert!(art.url.contains("fillWidth=300"));
}

#[test]
fn a_backup_taken_while_playing_restores_on_another_device() {
    let dir = temp_dir("backup");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 3)
        .unwrap()
        .iter()
        .map(|x| x.id)
        .collect();
    let state = core.play(ids, 0, StartedFrom::Songs, None).unwrap();
    core.playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    core.playback_ended(EndReason::Finished, 200_000).unwrap();
    // The second song is still playing while the backup is taken.
    core.playback_started(core.queue().unwrap().current.unwrap().uid, 60)
        .unwrap();
    let file = dir.0.join("encore-backup.sqlite");
    core.backup_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    let db = std::fs::read(&file).unwrap();
    assert!(
        !db.windows(b"anonymized-token".len())
            .any(|w| w == b"anonymized-token"),
        "a backup never holds the token"
    );

    // A new device: a fresh data folder, restored from the file, opened with the
    // token from the keychain.
    let other = temp_dir("restored");
    let (core2, _) = core_on_recording(&other);
    core2
        .restore_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    core2
        .open_profile(
            s.profile_id.clone(),
            vec![SourceToken {
                source_id: s.source_id,
                token: s.token.clone(),
            }],
        )
        .unwrap();
    assert_eq!(core2.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);
    let p = core2.inner.profile().unwrap();
    let log = p.store.lock().unwrap().recent_play_events(5).unwrap();
    // The finished play, and the one that was playing, closed as ended by the system.
    assert_eq!(log.len(), 2, "{log:?}");
    assert!(log.iter().any(|e| e.end_reason == "finished"));
    assert!(log.iter().any(|e| e.end_reason == "endedBySystem"));

    // A bad file changes nothing, and an ID can't point outside the profiles folder.
    let junk = dir.0.join("junk.sqlite");
    std::fs::write(&junk, b"junk").unwrap();
    assert!(matches!(
        core2.restore_profile(s.profile_id.clone(), junk.to_string_lossy().into_owned()),
        Err(EncoreError::Storage { .. })
    ));
    assert_eq!(core2.current_profile(), Some(s.profile_id.clone()));
    for id in ["../escape", "a/b", "", "..", "x\\y"] {
        assert!(
            core2
                .backup_profile(id.into(), file.to_string_lossy().into_owned())
                .is_err()
        );
        assert!(core2.open_profile(id.into(), vec![]).is_err());
    }
}

#[test]
fn lyrics_are_read_cached_and_shown_offline() {
    let dir = temp_dir("lyrics");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let songs = core.songs(SongSort::Title, 0, 1000).unwrap();
    let with: Vec<i64> = songs
        .iter()
        .filter(|x| x.has_lyrics)
        .map(|x| x.id)
        .collect();
    let without = songs.iter().find(|x| !x.has_lyrics).unwrap().id;
    assert!(!with.is_empty());

    // The recording holds lyrics for one song: line-synced, every line timed and
    // ended, in order. Songs whose lyrics weren't recorded fail to fetch.
    let mut found = None;
    for id in &with {
        if let Ok(Some(l)) = block(core.lyrics(*id)) {
            found = Some(l);
            break;
        }
    }
    let l = found.expect("the recorded song's lyrics");
    assert_eq!(l.sync, LyricsSync::Line);
    assert_eq!(l.format, LyricsFormat::Jellyfin);
    assert_eq!(l.origin, LyricsOrigin::MediaServer);
    assert!(l.lines.len() > 10);
    assert!(
        l.lines
            .iter()
            .all(|x| x.start_ms.is_some() && x.end_ms >= x.start_ms)
    );
    assert!(l.lines.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));

    // A song the server says has no lyrics: None, without asking.
    assert_eq!(block(core.lyrics(without)).unwrap(), None);

    // Offline (reopened without a token, so the server can't be asked): the kept
    // copy still shows.
    core.close_profile().unwrap();
    core.open_profile(s.profile_id, vec![]).unwrap();
    assert_eq!(block(core.lyrics(l.song_id)).unwrap(), Some(l.clone()));
    assert!(matches!(
        block(core.lyrics(999_999)),
        Err(EncoreError::NotFound { .. })
    ));
}

#[test]
fn a_profile_reopens_with_the_token_from_the_keychain() {
    let dir = temp_dir("reopen");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    core.close_profile().unwrap();
    assert!(matches!(core.servers(), Err(EncoreError::NoProfile { .. })));

    let profiles = core.profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].credentials[0].key, s.credential_key);
    // Without the token: the library is browsable offline, the server says sign in again.
    core.open_profile(s.profile_id.clone(), vec![]).unwrap();
    assert!(!core.servers().unwrap()[0].signed_in);
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);
    let id = core.songs(SongSort::Title, 0, 1).unwrap()[0].id;
    assert!(matches!(
        core.stream_for(id, Network::Wifi),
        Err(EncoreError::SessionExpired { .. })
    ));
    // With it: streaming works again.
    core.open_profile(
        s.profile_id,
        vec![SourceToken {
            source_id: s.source_id,
            token: s.token,
        }],
    )
    .unwrap();
    assert!(core.stream_for(id, Network::Wifi).is_ok());
}
