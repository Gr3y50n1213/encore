//! Downloads and offline (Phase 8) through the public API, on the Jellyfin
//! recording, with a server stand-in that serves audio and covers (the recording
//! holds none) and can fail, cut a transfer short, or resume it.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use encore_sources::transport::{BoxFuture, Downloaded, Progress, Request, Response};
use encore_sources::{SourceError, Unreachable};

use super::*;

/// The bytes a song's file holds: different per song, the same every time.
fn audio_of(item: &str) -> Vec<u8> {
    let seed = item
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    (0..12_000u32)
        .map(|i| (seed.wrapping_add(i * 13) % 251) as u8)
        .collect()
}

fn item_of(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/Audio/")?;
    Some(rest.split('/').next()?.to_owned())
}

/// The recording for everything else; songs and covers from here.
#[derive(Debug)]
struct Server {
    inner: Arc<dyn Transport>,
    /// Songs that answer 500 this many more times.
    failing: Mutex<HashMap<String, u32>>,
    /// Songs whose next transfer stops halfway (the network dropped).
    cut: Mutex<HashSet<String>>,
    /// Every download's (item, resume_from).
    downloads: Mutex<Vec<(String, u64)>>,
    /// Set by a test to stop the run from inside a transfer.
    stop_on: Mutex<Option<(String, Arc<EncoreCore>)>>,
}

impl Server {
    fn new(inner: Arc<dyn Transport>) -> Arc<Server> {
        Arc::new(Server {
            inner,
            failing: Mutex::new(HashMap::new()),
            cut: Mutex::new(HashSet::new()),
            downloads: Mutex::new(Vec::new()),
            stop_on: Mutex::new(None),
        })
    }
}

impl Transport for Server {
    fn send(&self, req: Request) -> BoxFuture<'_, encore_sources::Result<Response>> {
        let path = req.url.path().to_owned();
        if path.starts_with("/Items/")
            && path.ends_with("/Images/Primary")
            && req.url.query().is_some_and(|q| q.contains("fillWidth"))
        {
            return Box::pin(async {
                Ok(Response {
                    status: 200,
                    body: vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4],
                    server_date_utc_ms: None,
                })
            });
        }
        let inner = self.inner.clone();
        Box::pin(async move { inner.send(req).await })
    }

    fn download<'a>(
        &'a self,
        req: Request,
        to: &'a std::path::Path,
        resume_from: u64,
        progress: Progress<'a>,
        cancel: &'a AtomicBool,
    ) -> BoxFuture<'a, encore_sources::Result<Downloaded>> {
        Box::pin(async move {
            let item = item_of(req.url.path()).expect("a song's address");
            self.downloads
                .lock()
                .unwrap()
                .push((item.clone(), resume_from));
            if let Some((stop_item, core)) = self.stop_on.lock().unwrap().clone()
                && stop_item == item
            {
                core.stop_downloads();
            }
            if cancel.load(Ordering::SeqCst) {
                std::fs::write(to, &audio_of(&item)[..100]).unwrap();
                return Err(SourceError::Cancelled);
            }
            {
                let mut f = self.failing.lock().unwrap();
                if let Some(n) = f.get_mut(&item)
                    && *n > 0
                {
                    *n -= 1;
                    return Ok(Downloaded {
                        status: 500,
                        bytes: 0,
                        content_type: None,
                        expected_bytes: None,
                    });
                }
            }
            let body = audio_of(&item);
            let total = body.len() as u64;
            if self.cut.lock().unwrap().remove(&item) {
                std::fs::write(to, &body[..body.len() / 2]).unwrap();
                progress(total / 2, Some(total));
                return Err(SourceError::Unreachable(Unreachable::TimedOut));
            }
            let (status, from) = if resume_from > 0 && resume_from < total {
                (206, resume_from)
            } else {
                (200, 0)
            };
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(status == 206)
                .truncate(status == 200)
                .open(to)
                .unwrap();
            std::io::Write::write_all(&mut file, &body[from as usize..]).unwrap();
            progress(total, Some(total));
            Ok(Downloaded {
                status,
                bytes: total,
                content_type: Some("audio/flac".into()),
                expected_bytes: Some(total),
            })
        })
    }
}

fn core_with_server(dir: &TempDir, server: Arc<Server>) -> (Arc<EncoreCore>, Arc<Events>) {
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    };
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(server)).unwrap();
    let events = Arc::new(Events::default());
    core.set_listener(events.clone());
    (core, events)
}

fn recording() -> Arc<dyn Transport> {
    Arc::new(FixtureTransport::from_json(RECORDED).unwrap())
}

/// Signed in and synced, with the first album that has a cover and 2+ songs.
fn ready(core: &EncoreCore) -> AlbumDetail {
    signed_in(core);
    block(core.sync_library()).unwrap();
    core.albums(AlbumSort::Title, 0, 200)
        .unwrap()
        .into_iter()
        .filter(|a| a.has_artwork && a.song_count >= 2)
        .map(|a| core.album(a.id).unwrap())
        .next()
        .expect("an album with a cover")
}

fn album(id: i64) -> DownloadTarget {
    DownloadTarget {
        kind: DownloadKind::Album,
        id,
        mix_id: None,
    }
}

fn song(id: i64) -> DownloadTarget {
    DownloadTarget {
        kind: DownloadKind::Song,
        id,
        mix_id: None,
    }
}

const PLENTY: u64 = 100_000_000_000;

fn files_in(dir: &TempDir) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.0.join("downloads"))
        .map(|d| {
            d.flatten()
                .filter(|f| f.path().is_file())
                .map(|f| f.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn an_album_downloads_plays_from_its_files_and_shows_its_cover_offline() {
    let dir = temp_dir("dl-album");
    let server = Server::new(recording());
    let (core, events) = core_with_server(&dir, server.clone());
    let a = ready(&core);
    let ids: Vec<i64> = a.songs.iter().map(|s| s.id).collect();

    let item = core.download(album(a.album.id)).unwrap();
    assert_eq!(
        (item.state, item.saved, item.total),
        (DownloadState::Waiting, 0, ids.len() as u32)
    );
    let marks = core.download_marks().unwrap();
    assert_eq!(marks.pending_song_ids.len(), ids.len());
    events.take();

    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(
        (run.saved, run.failed, run.remaining, run.stop),
        (ids.len() as u32, 0, 0, DownloadStop::Done)
    );
    let kinds = events.kinds();
    assert!(kinds.contains(&EventKind::DownloadProgress));
    assert!(kinds.contains(&EventKind::DownloadsChanged));
    let progress = events
        .take()
        .into_iter()
        .filter_map(|e| e.download)
        .next_back()
        .unwrap();
    assert_eq!(progress.item_title, a.album.title);

    let marks = core.download_marks().unwrap();
    assert_eq!(marks.saved_song_ids.len(), ids.len());
    assert!(marks.pending_song_ids.is_empty());
    let it = &marks.items[0];
    assert_eq!(
        (it.state, it.saved, it.total),
        (DownloadState::Saved, ids.len() as u32, ids.len() as u32)
    );

    // Each song plays from its file, with no token and no headers.
    let s = core.stream_for(ids[0], Network::Cellular).unwrap();
    assert!(s.local);
    assert!(s.url.starts_with("file://"), "{}", s.url);
    assert!(s.headers.is_empty());
    assert!(!s.cache_key.contains("token"));
    let path = url::Url::parse(&s.url).unwrap().to_file_path().unwrap();
    assert_eq!(
        std::fs::read(path).unwrap(),
        audio_of(
            &core
                .inner
                .profile()
                .unwrap()
                .store
                .lock()
                .unwrap()
                .track(ids[0])
                .unwrap()
                .source_item_id
        )
    );

    // The cover was kept, so the album shows offline.
    let art = core.artwork(ArtworkOf::Album, a.album.id, 300).unwrap();
    assert!(art.url.starts_with("file://"));
    let art_song = core.artwork(ArtworkOf::Song, ids[1], 300).unwrap();
    assert_eq!(art_song.url, art.url);

    let o = core.downloads_overview().unwrap();
    assert_eq!(o.song_count, ids.len() as u32);
    assert!(o.used_bytes >= 12_000 * ids.len() as u64);
    assert_eq!(o.settings.quality, StreamQuality::Original);
    assert!(o.settings.wifi_only);
    assert_eq!(
        files_in(&dir).len(),
        ids.len() + 1,
        "the songs and one cover"
    );

    // A second run has nothing to do.
    let again = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!((again.saved, again.stop), (0, DownloadStop::Done));

    // Removing it lets every file go.
    core.remove_download(album(a.album.id)).unwrap();
    assert!(files_in(&dir).is_empty(), "{:?}", files_in(&dir));
    assert!(core.download_marks().unwrap().items.is_empty());
    assert!(!core.stream_for(ids[0], Network::Wifi).unwrap().local);
}

#[test]
fn downloads_wait_for_wifi_the_limit_and_space_and_never_delete() {
    let dir = temp_dir("dl-wait");
    let (core, _) = core_with_server(&dir, Server::new(recording()));
    let a = ready(&core);
    core.download(album(a.album.id)).unwrap();

    let run = block(core.run_downloads(Network::Cellular, PLENTY)).unwrap();
    assert_eq!((run.saved, run.stop), (0, DownloadStop::WaitingForWifi));
    assert_eq!(
        core.downloads_overview().unwrap().stop,
        Some(DownloadStop::WaitingForWifi)
    );

    let mut s = core.download_settings().unwrap();
    s.wifi_only = false;
    s.limit_bytes = Some(1);
    core.set_download_settings(s.clone()).unwrap();
    let run = block(core.run_downloads(Network::Cellular, PLENTY)).unwrap();
    assert_eq!((run.saved, run.stop), (0, DownloadStop::StorageLimit));

    s.limit_bytes = None;
    core.set_download_settings(s).unwrap();
    let run = block(core.run_downloads(Network::Cellular, 1_000)).unwrap();
    assert_eq!(
        (run.saved, run.stop),
        (0, DownloadStop::LowSpace),
        "it keeps space free"
    );

    core.set_connectivity(false, Network::Wifi, vec![]).unwrap();
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(run.stop, DownloadStop::Offline);
    core.set_connectivity(true, Network::Wifi, vec![]).unwrap();
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(run.stop, DownloadStop::Done);
    assert_eq!(core.downloads_overview().unwrap().stop, None);
}

#[test]
fn a_failing_song_is_tried_three_times_then_marked_and_can_be_retried() {
    let dir = temp_dir("dl-fail");
    let server = Server::new(recording());
    let (core, _) = core_with_server(&dir, server.clone());
    let a = ready(&core);
    let bad = core
        .inner
        .profile()
        .unwrap()
        .store
        .lock()
        .unwrap()
        .track(a.songs[0].id)
        .unwrap()
        .source_item_id;
    server.failing.lock().unwrap().insert(bad.clone(), 10);
    core.download(album(a.album.id)).unwrap();

    // One try per run, so a broken song never holds up the rest.
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(
        (run.saved, run.failed, run.remaining),
        (a.songs.len() as u32 - 1, 0, 1)
    );
    block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(
        (run.failed, run.remaining),
        (1, 0),
        "given up after the third try"
    );
    let marks = core.download_marks().unwrap();
    assert_eq!(marks.failed_song_ids, [a.songs[0].id]);
    let it = &marks.items[0];
    assert_eq!(it.state, DownloadState::Failed);
    assert!(
        it.error.as_deref().is_some_and(|e| e.contains("500")),
        "{:?}",
        it.error
    );

    server.failing.lock().unwrap().clear();
    assert_eq!(core.retry_failed_downloads().unwrap(), 1);
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!((run.saved, run.remaining), (1, 0));
    assert_eq!(
        core.download_marks().unwrap().items[0].state,
        DownloadState::Saved
    );
}

#[test]
fn a_cut_transfer_resumes_where_it_stopped_and_a_stopped_run_carries_on() {
    let dir = temp_dir("dl-resume");
    let server = Server::new(recording());
    let (core, _) = core_with_server(&dir, server.clone());
    let a = ready(&core);
    let first = core
        .inner
        .profile()
        .unwrap()
        .store
        .lock()
        .unwrap()
        .track(a.songs[0].id)
        .unwrap()
        .source_item_id;
    server.cut.lock().unwrap().insert(first.clone());
    core.download(song(a.songs[0].id)).unwrap();

    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(run.stop, DownloadStop::ServerUnreachable);
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!((run.saved, run.stop), (1, DownloadStop::Done));
    let tries = server.downloads.lock().unwrap().clone();
    assert_eq!(
        tries,
        [(first.clone(), 0), (first.clone(), 6_000)],
        "the second try asked for the rest"
    );
    let s = core.stream_for(a.songs[0].id, Network::Wifi).unwrap();
    let path = url::Url::parse(&s.url).unwrap().to_file_path().unwrap();
    assert_eq!(
        std::fs::read(path).unwrap(),
        audio_of(&first),
        "the joined file is whole"
    );

    // Stopping a run (the job ended) keeps what it had and resumes next time.
    core.download(album(a.album.id)).unwrap();
    let second = core
        .inner
        .profile()
        .unwrap()
        .store
        .lock()
        .unwrap()
        .track(a.songs[1].id)
        .unwrap()
        .source_item_id;
    *server.stop_on.lock().unwrap() = Some((second.clone(), core.clone()));
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(run.stop, DownloadStop::Stopped);
    *server.stop_on.lock().unwrap() = None;
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!((run.stop, run.remaining), (DownloadStop::Done, 0));
    assert!(
        server.downloads.lock().unwrap().contains(&(second, 100)),
        "resumed after the 100 bytes kept"
    );
}

#[test]
fn offline_only_saved_and_cached_songs_play_and_the_queue_passes_over_the_rest() {
    let dir = temp_dir("dl-offline");
    let (core, events) = core_with_server(&dir, Server::new(recording()));
    let a = ready(&core);
    assert!(a.songs.len() >= 3, "a 3-song album");
    let ids: Vec<i64> = a.songs.iter().map(|s| s.id).collect();
    // Only the second song is downloaded; the third is in the player's cache.
    core.download(song(ids[1])).unwrap();
    block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    let third_key = core.stream_for(ids[2], Network::Wifi).unwrap().cache_key;
    events.take();
    core.set_connectivity(false, Network::Wifi, vec![third_key])
        .unwrap();
    assert!(events.kinds().contains(&EventKind::DownloadsChanged));

    let marks = core.download_marks().unwrap();
    assert!(!marks.online);
    let playable: HashSet<i64> = marks.playable_offline_song_ids.iter().copied().collect();
    assert_eq!(playable, HashSet::from([ids[1], ids[2]]));

    // A tap on a song that can't play is refused, with the reason.
    let err = core
        .play(ids.clone(), 0, StartedFrom::Album, None)
        .unwrap_err();
    assert!(matches!(err, EncoreError::NotAllowed { .. }), "{err:?}");
    assert!(err.to_string().contains("Not downloaded"));
    // Playing the album from the downloaded song queues only what can play.
    let q = core.play(ids.clone(), 1, StartedFrom::Album, None).unwrap();
    assert_eq!(q.current.unwrap().song.id, ids[1]);
    let up: Vec<i64> = q.up_next.iter().map(|e| e.song.id).collect();
    assert!(up.iter().all(|id| playable.contains(id)), "{up:?}");
    assert!(
        core.add_to_up_next(vec![ids[0]]).is_err(),
        "nothing playable to add"
    );

    // Online again, everything plays; a queued song that can't play offline is
    // passed over by Next when the network goes.
    core.set_connectivity(true, Network::Wifi, vec![]).unwrap();
    let q = core.play(ids.clone(), 1, StartedFrom::Album, None).unwrap();
    assert_eq!(q.up_next.len(), ids.len() - 2);
    core.add_to_up_next(vec![ids[0]]).unwrap();
    core.set_connectivity(false, Network::Wifi, vec![]).unwrap();
    let target = core.next_target().unwrap();
    assert!(
        target.song.as_ref().is_none_or(|e| e.song.id != ids[0]),
        "the unplayable song isn't next"
    );
    let next = core.next().unwrap();
    assert!(next.current.as_ref().is_some_and(|c| c.song.id != ids[0]));
}

#[test]
fn two_profiles_share_one_file_and_it_goes_only_when_both_let_go() {
    let dir = temp_dir("dl-share");
    let swap = Arc::new(Swap(Mutex::new(recording())));
    let server = Server::new(swap.clone());
    let (core, _) = core_with_server(&dir, server.clone());
    let a = ready(&core);
    core.download(album(a.album.id)).unwrap();
    block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    let files = files_in(&dir);
    let transfers = server.downloads.lock().unwrap().len();

    // Alex, on the same device and server, keeps the same album: no new transfer.
    *swap.0.lock().unwrap() = second_user();
    signed_in(&core);
    block(core.sync_library()).unwrap();
    core.download(album(a.album.id)).unwrap();
    let run = block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    assert_eq!(run.saved as usize, a.songs.len());
    assert_eq!(
        server.downloads.lock().unwrap().len(),
        transfers,
        "nothing fetched twice"
    );
    assert_eq!(files_in(&dir), files, "the same files");

    // Alex lets go: Sam still has them.
    core.remove_download(album(a.album.id)).unwrap();
    assert_eq!(files_in(&dir), files);
    // Removing Sam's profile from the device lets the files go.
    let profiles = core.profiles().unwrap();
    let sam = profiles
        .iter()
        .find(|p| !p.active)
        .unwrap()
        .profile_id
        .clone();
    core.remove_profile_from_device(sam).unwrap();
    assert!(files_in(&dir).is_empty(), "{:?}", files_in(&dir));
}

#[test]
fn liked_songs_follow_likes_and_a_song_inside_an_album_stays_with_it() {
    let dir = temp_dir("dl-liked");
    let (core, _) = core_with_server(&dir, Server::new(recording()));
    let a = ready(&core);
    let mut s = core.download_settings().unwrap();
    s.liked_songs = true;
    core.set_download_settings(s).unwrap();
    let liked_before = core.liked_songs(0, 10_000).unwrap().len();
    let fresh = a
        .songs
        .iter()
        .find(|x| !core.are_liked(vec![x.id]).unwrap()[0])
        .unwrap()
        .id;
    core.like(vec![fresh]).unwrap();
    let marks = core.download_marks().unwrap();
    let liked = marks
        .items
        .iter()
        .find(|i| i.target.kind == DownloadKind::LikedSongs)
        .unwrap();
    assert_eq!(liked.total as usize, liked_before + 1);
    assert!(liked.auto);
    core.unlike(vec![fresh]).unwrap();
    let marks = core.download_marks().unwrap();
    let liked = marks
        .items
        .iter()
        .find(|i| i.target.kind == DownloadKind::LikedSongs)
        .unwrap();
    assert_eq!(liked.total as usize, liked_before);

    // A song kept as part of an album can't be removed on its own.
    core.download(album(a.album.id)).unwrap();
    let err = core.remove_download(song(a.songs[0].id)).unwrap_err();
    assert!(err.to_string().contains(&a.album.title), "{err}");
}

#[test]
fn guest_downloads_nothing_and_search_finds_what_is_downloaded() {
    let dir = temp_dir("dl-guest");
    let (core, _) = core_with_server(&dir, Server::new(recording()));
    let a = ready(&core);
    core.download(song(a.songs[0].id)).unwrap();
    block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    let word = a.songs[0]
        .title
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let all = core.search(word.clone()).unwrap();
    let down = core.search_downloaded(word).unwrap();
    assert!(down.songs.iter().all(|s| s.id == a.songs[0].id));
    assert!(down.songs.len() <= all.songs.len());

    core.start_guest().unwrap();
    let err = core.download(song(a.songs[1].id)).unwrap_err();
    assert!(matches!(err, EncoreError::NotAllowed { .. }), "{err:?}");
}

#[test]
fn remove_all_clears_every_file_and_unplayed_ones_stay_until_their_days_pass() {
    let dir = temp_dir("dl-all");
    let (core, _) = core_with_server(&dir, Server::new(recording()));
    let a = ready(&core);
    core.download(album(a.album.id)).unwrap();
    block(core.run_downloads(Network::Wifi, PLENTY)).unwrap();
    let mut s = core.download_settings().unwrap();
    s.remove_unplayed = true;
    core.set_download_settings(s).unwrap();
    assert_eq!(
        core.remove_unplayed_downloads().unwrap(),
        0,
        "downloaded today"
    );
    assert_eq!(core.remove_all_downloads().unwrap(), 1);
    assert!(files_in(&dir).is_empty());
    assert_eq!(core.downloads_overview().unwrap().used_bytes, 0);
}
