//! Phase 4's "done when", core part, end to end: two real cores (a phone and a
//! Mac, each with its own data folder) signed in to the recorded Jellyfin, and a
//! real `encore-server` on loopback between them.
//!
//! - A like on the phone shows on the Mac within a second (timed).
//! - An unlike follows, and a play on the phone joins the Mac's history.
//! - Revoking the phone from the Mac signs it out, and its data stays.
//!
//! The media-server check is a stand-in that accepts the recording's token; the
//! real check (`JellyfinCheck`) is tested on Jellyfin's replies in the server's
//! unit tests.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use encore_core::api::{
    CoreConfig, CoreEvent, CoreListener, EncoreCore, EndReason, EventKind, SongSort, SourceToken,
    StartedFrom, SyncState,
};
use encore_server::{AppState, CheckFuture, CheckedUser, Config, MediaCheck};
use encore_sources::transport::{BoxFuture, Request, Response};
use encore_sources::{FixtureTransport, Transport};

const RECORDED: &str = include_str!("../../tools/fixtures/jellyfin/recorded.json");

/// Accepts any token on any server as one user (the recording's).
#[derive(Debug)]
struct AnyToken;

impl MediaCheck for AnyToken {
    fn check<'a>(&'a self, _server_id: &'a str, _token: &'a str) -> CheckFuture<'a> {
        Box::pin(async {
            Ok(CheckedUser {
                user_id: "recorded-user".into(),
                user_name: "Recorded".into(),
            })
        })
    }
}

/// The companion over real HTTP on loopback; everything else from the recording.
#[derive(Debug)]
struct Split {
    companion: SocketAddr,
    http: encore_sources::http::HttpTransport,
    recording: FixtureTransport,
}

impl Transport for Split {
    fn send(&self, req: Request) -> BoxFuture<'_, encore_sources::Result<Response>> {
        let to_companion = req.url.port() == Some(self.companion.port())
            && req.url.host_str() == Some("127.0.0.1");
        if to_companion {
            self.http.send(req)
        } else {
            self.recording.send(req)
        }
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<CoreEvent>>);

impl CoreListener for Events {
    fn on_event(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Events {
    /// The `n`th event (from 1) that `matches`, waiting up to `limit` for it.
    fn wait_nth(
        &self,
        limit: Duration,
        n: usize,
        matches: impl Fn(&CoreEvent) -> bool,
    ) -> Option<CoreEvent> {
        let t = Instant::now();
        while t.elapsed() < limit {
            if let Some(e) = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| matches(e))
                .nth(n - 1)
            {
                return Some(e.clone());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    /// Wait (up to `limit`) for an event that `matches`; how long it took.
    fn wait_for(&self, limit: Duration, matches: impl Fn(&CoreEvent) -> bool) -> Option<Duration> {
        let t = Instant::now();
        while t.elapsed() < limit {
            if self.0.lock().unwrap().iter().any(&matches) {
                return Some(t.elapsed());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    fn latest_token(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|e| e.credential.as_ref().map(|c| c.token.clone()))
    }
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(name: &str) -> TempDir {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Every test names its devices "phone" and "mac", and tests run at once: a
    // counter keeps two of them from ever sharing a folder.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!(
        "encore-e2e-{name}-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    TempDir(d)
}

fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

/// Start a companion server on a free loopback port, on its own runtime.
fn start_server(data: &TempDir) -> (SocketAddr, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let config = Config::from_env(|k| match k {
        "ENCORE_JELLYFIN_URLS" => Some("http://jellyfin.local:8096".into()),
        "ENCORE_DATA_DIR" => Some(data.0.to_string_lossy().into_owned()),
        _ => None,
    })
    .unwrap();
    let state = Arc::new(AppState::open(config, Arc::new(AnyToken)).unwrap());
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    rt.spawn(async move {
        axum::serve(
            listener,
            encore_server::app(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    (addr, rt)
}

fn device(name: &str, companion: SocketAddr) -> (TempDir, Arc<EncoreCore>, Arc<Events>, String) {
    let dir = temp_dir(name);
    let transport = Arc::new(Split {
        companion,
        http: encore_sources::http::HttpTransport::new(None, Duration::from_secs(5)).unwrap(),
        recording: FixtureTransport::from_json(RECORDED).unwrap(),
    });
    let core = EncoreCore::with_transport_for_tests(
        CoreConfig {
            data_dir: dir.0.to_string_lossy().into_owned(),
            device_id: format!("{name}-device"),
            device_name: name.into(),
            platform: "test".into(),
        },
        transport,
    )
    .unwrap();
    let events = Arc::new(Events::default());
    core.set_listener(events.clone());
    let check = block(core.check_address("http://jellyfin.local:8096".into())).unwrap();
    block(core.quick_connect_start(check.base_url)).unwrap();
    let signed = block(core.quick_connect_wait()).unwrap();
    block(core.sync_library()).unwrap();
    (dir, core, events, signed.profile_id)
}

#[test]
fn a_like_on_the_phone_shows_on_the_mac_within_a_second() {
    let server_dir = temp_dir("server");
    let (addr, _server) = start_server(&server_dir);
    let url = format!("http://127.0.0.1:{}", addr.port());
    let (_pd, phone, phone_events, profile) = device("phone", addr);
    let (_md, mac, mac_events, _) = device("mac", addr);

    let status = block(phone.connect_companion(url.clone())).unwrap();
    assert_eq!(status.server_url.as_deref(), Some(url.as_str()));
    block(mac.connect_companion(url.clone())).unwrap();
    assert!(
        phone_events
            .latest_token()
            .is_some_and(|t| t.starts_with("ert_"))
    );
    // Both loops running (each has pulled once, so the Mac is waiting on the server).
    for e in [&phone_events, &mac_events] {
        assert!(
            e.wait_for(Duration::from_secs(10), |e| e
                .sync_status
                .as_ref()
                .is_some_and(|s| s.state == SyncState::Idle))
                .is_some(),
            "sync is idle"
        );
    }

    let songs = phone.songs(SongSort::Title, 0, 1000).unwrap();
    let song = songs
        .iter()
        .find(|s| !phone.are_liked(vec![s.id]).unwrap()[0])
        .expect("a song that isn't liked yet")
        .id;
    assert!(!mac.are_liked(vec![song]).unwrap()[0]);

    // The like.
    let liked_at = Instant::now();
    phone.like(vec![song]).unwrap();
    let seen = mac_events.wait_for(Duration::from_secs(5), |e| {
        e.kind == EventKind::RemoteChange
            && e.remote_change
                .as_ref()
                .is_some_and(|c| c.liked_song_ids.contains(&song))
    });
    let latency = liked_at.elapsed();
    assert!(seen.is_some(), "the Mac never heard of the like");
    println!(
        "a like on the phone showed on the Mac in {:.1} ms",
        latency.as_secs_f64() * 1e3
    );
    assert!(latency < Duration::from_secs(1), "{latency:?}");
    assert!(
        mac.are_liked(vec![song]).unwrap()[0],
        "the Mac's heart is filled"
    );

    // The unlike follows the same way.
    phone.unlike(vec![song]).unwrap();
    assert!(
        mac_events
            .wait_for(Duration::from_secs(5), |e| e
                .remote_change
                .as_ref()
                .is_some_and(|c| c.unliked_song_ids.contains(&song)))
            .is_some()
    );
    assert!(!mac.are_liked(vec![song]).unwrap()[0]);

    // A play on the phone joins the Mac's history.
    let state = phone
        .play(vec![songs[0].id], 0, StartedFrom::Songs, None)
        .unwrap();
    phone
        .playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    phone.playback_ended(EndReason::Finished, 200_000).unwrap();
    block(phone.sync_now()).unwrap();
    assert!(
        mac_events
            .wait_for(Duration::from_secs(5), |e| e
                .remote_change
                .as_ref()
                .is_some_and(|c| c.history))
            .is_some(),
        "the Mac got the play"
    );
    let export = _md.0.join("mac-export.json");
    let summary = mac
        .export_profile(profile.clone(), export.to_string_lossy().into_owned())
        .unwrap();
    assert_eq!(summary.plays, 1, "the phone's play is in the Mac's log");

    // From the Mac, sign the lost phone out: it stops, says so, keeps its data.
    let devices = block(mac.companion_devices()).unwrap();
    assert_eq!(devices.len(), 2);
    let phone_device = devices.iter().find(|d| !d.this_device).unwrap();
    assert_eq!(phone_device.name, "phone");
    block(mac.revoke_device(phone_device.device_id.clone())).unwrap();
    phone.like(vec![song]).unwrap();
    assert!(
        phone_events
            .wait_for(Duration::from_secs(10), |e| e
                .sync_status
                .as_ref()
                .is_some_and(|s| s.state == SyncState::SignedOut))
            .is_some(),
        "the phone hears it was signed out"
    );
    assert_eq!(
        phone_events.latest_token().as_deref(),
        Some(""),
        "forget the token"
    );
    assert!(phone.are_liked(vec![song]).unwrap()[0], "local data stays");
    assert_eq!(phone.sync_status().unwrap().state, SyncState::SignedOut);
}

#[test]
fn a_profile_reopened_with_its_kept_token_syncs_again() {
    let server_dir = temp_dir("server-reopen");
    let (addr, _server) = start_server(&server_dir);
    let url = format!("http://127.0.0.1:{}", addr.port());
    let (_pd, phone, phone_events, profile) = device("phone", addr);
    let (_md, mac, mac_events, _) = device("mac", addr);
    block(phone.connect_companion(url.clone())).unwrap();
    block(mac.connect_companion(url)).unwrap();

    // The app closes; the keychain kept the latest refresh token.
    let summary = phone
        .profiles()
        .unwrap()
        .into_iter()
        .find(|p| p.profile_id == profile)
        .unwrap();
    assert!(
        summary.credentials.iter().any(|c| c.source_id == 0),
        "a companion key"
    );
    phone.close_profile().unwrap();
    let token = phone_events.latest_token().unwrap();
    // Liked while the phone was closed: it arrives when the phone reopens.
    let song = mac.songs(SongSort::Title, 0, 5).unwrap()[3].id;
    mac.like(vec![song]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    // The Jellyfin token comes from the keychain too; the recording accepts any.
    phone
        .open_profile(
            profile,
            vec![
                SourceToken {
                    source_id: 1,
                    token: "anonymized-token".into(),
                },
                SourceToken {
                    source_id: 0,
                    token,
                },
            ],
        )
        .unwrap();
    assert!(
        phone_events
            .wait_for(Duration::from_secs(10), |e| e
                .remote_change
                .as_ref()
                .is_some_and(|c| c.liked_song_ids.contains(&song)))
            .is_some(),
        "the like made while closed arrives"
    );
    assert!(phone.are_liked(vec![song]).unwrap()[0]);
    let _ = mac_events;
}

#[test]
fn deleting_the_profile_everywhere_signs_every_device_out_and_clears_this_one() {
    let server_dir = temp_dir("server-delete");
    let (addr, _server) = start_server(&server_dir);
    let url = format!("http://127.0.0.1:{}", addr.port());
    let (_pd, phone, phone_events, _) = device("phone", addr);
    let (md, mac, mac_events, profile) = device("mac", addr);
    block(phone.connect_companion(url.clone())).unwrap();
    block(mac.connect_companion(url)).unwrap();
    let song = mac.songs(SongSort::Title, 0, 5).unwrap()[1].id;
    mac.like(vec![song]).unwrap();
    block(mac.sync_now()).unwrap();

    // The confirm sheet asks for the profile's name.
    let wrong = block(mac.delete_profile_everywhere(profile.clone(), "nope".into()));
    assert!(matches!(
        wrong,
        Err(encore_core::api::EncoreError::NotAllowed { .. })
    ));
    let name = mac.profiles().unwrap()[0].display_name.clone();
    block(mac.delete_profile_everywhere(profile.clone(), name)).unwrap();

    assert!(mac.current_profile().is_none());
    assert!(
        !md.0.join("profiles").join(&profile).exists(),
        "this device's copy is gone"
    );
    assert!(mac.profiles().unwrap().is_empty());
    assert_eq!(mac_events.latest_token().as_deref(), Some(""));
    // The phone is signed out by the server, and keeps its own copy until removed.
    phone.like(vec![song]).unwrap();
    assert!(
        phone_events
            .wait_for(Duration::from_secs(10), |e| e
                .sync_status
                .as_ref()
                .is_some_and(|s| s.state == SyncState::SignedOut))
            .is_some()
    );
    assert!(phone.current_profile().is_some());
}

/// The like within a second through a server outside this process: the Docker
/// image in CI (core.yml, the server-image job), whose sign-in check asks the
/// replay Jellyfin over real HTTP. `ENCORE_COMPANION_URL` is its address on
/// 127.0.0.1.
#[test]
#[ignore = "needs ENCORE_COMPANION_URL: the server-image job in core.yml"]
fn a_like_shows_on_the_mac_within_a_second_through_the_docker_image() {
    let url = std::env::var("ENCORE_COMPANION_URL").expect("set ENCORE_COMPANION_URL");
    let port = url::Url::parse(&url).unwrap().port().expect("a port");
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let (_pd, phone, _pe, _) = device("phone", addr);
    let (_md, mac, mac_events, _) = device("mac", addr);
    block(phone.connect_companion(url.clone())).unwrap();
    block(mac.connect_companion(url)).unwrap();
    assert!(
        mac_events
            .wait_for(Duration::from_secs(15), |e| e
                .sync_status
                .as_ref()
                .is_some_and(|s| s.state == SyncState::Idle))
            .is_some()
    );
    let song = phone
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .into_iter()
        .find(|s| !phone.are_liked(vec![s.id]).unwrap()[0])
        .unwrap()
        .id;
    let liked_at = Instant::now();
    phone.like(vec![song]).unwrap();
    let seen = mac_events.wait_for(Duration::from_secs(5), |e| {
        e.remote_change
            .as_ref()
            .is_some_and(|c| c.liked_song_ids.contains(&song))
    });
    let latency = liked_at.elapsed();
    assert!(seen.is_some(), "the Mac never heard of the like");
    println!(
        "through the Docker image: a like on the phone showed on the Mac in {:.1} ms",
        latency.as_secs_f64() * 1e3
    );
    assert!(latency < Duration::from_secs(1), "{latency:?}");
    assert!(mac.are_liked(vec![song]).unwrap()[0]);
}

/// Phase 5, the owner's choice: the mixes are built on the companion server, and
/// every device of the profile shows the same set; with the server gone, a device
/// builds its own.
#[test]
fn the_mixes_come_from_the_companion_server_and_the_device_builds_them_without_it() {
    let server_dir = temp_dir("server-mixes");
    let (addr, server) = start_server(&server_dir);
    let url = format!("http://127.0.0.1:{}", addr.port());
    let (_pd, phone, _pe, _) = device("phone", addr);
    let (_md, mac, _me, _) = device("mac", addr);
    block(phone.connect_companion(url.clone())).unwrap();
    block(mac.connect_companion(url)).unwrap();

    // The phone listens to 60 songs; its plays and its song list reach the server.
    let ids: Vec<i64> = phone
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    for k in 0..60 {
        let state = phone
            .play(vec![ids[k % ids.len()]], 0, StartedFrom::Songs, None)
            .unwrap();
        phone
            .playback_started(state.current.unwrap().uid, 60)
            .unwrap();
        phone.playback_ended(EndReason::Finished, 200_000).unwrap();
    }
    block(phone.sync_now()).unwrap();
    block(phone.sync_library()).unwrap();
    block(mac.sync_now()).unwrap();

    let on_phone = block(phone.home()).unwrap().made_for_you;
    let on_mac = block(mac.home()).unwrap().made_for_you;
    assert!(
        on_phone.from_companion && on_mac.from_companion,
        "both from the server"
    );
    assert!(!on_phone.mixes.is_empty(), "{on_phone:?}");
    let titles = |s: &encore_core::api::MixShelf| {
        s.mixes
            .iter()
            .map(|m| (m.mix_id.clone(), m.title.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        titles(&on_phone),
        titles(&on_mac),
        "every device shows the same set"
    );

    // The server goes away: the phone builds its own set, from the same log.
    drop(server);
    let alone = block(phone.regenerate_mixes()).unwrap().made_for_you;
    assert!(!alone.from_companion);
    assert!(!alone.mixes.is_empty());
}

/// Phase 7's "done when", core part: phone-to-desktop transfer starts in under 1
/// second (here: from the tap to the Mac's confirmation, through a real server and
/// real WebSockets; the Mac's "player" is ready at once, as with a cached track).
/// Then the play counts once, the phone pulls it back, the Mac remote-controls the
/// phone, a failed handoff leaves the phone playing, and a rename shows everywhere.
#[test]
fn playback_moves_between_devices_and_is_counted_once() {
    use encore_core::api::{
        AlbumSort, DeviceState, OutputState, RemoteCommand, RemoteCommandKind, SleepTimerMode,
        SleepTimerSpec,
    };
    let server_dir = temp_dir("server-handoff");
    let (addr, _server) = start_server(&server_dir);
    let url = format!("http://127.0.0.1:{}", addr.port());
    let (_pd, phone, phone_events, profile) = device("phone", addr);
    let (md, mac, mac_events, _) = device("mac", addr);
    block(phone.connect_companion(url.clone())).unwrap();
    block(mac.connect_companion(url)).unwrap();

    // Both live links up: each sees the other.
    let ready = |c: &EncoreCore| {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(15) {
            let o = c.output_devices().unwrap();
            if o.state == OutputState::Ready && o.devices.len() == 2 {
                return o;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("no other device: {:?}", c.output_devices().unwrap());
    };
    let mac_id = ready(&phone)
        .devices
        .into_iter()
        .find(|d| !d.this_device)
        .unwrap()
        .device_id;
    let phone_id = ready(&mac)
        .devices
        .into_iter()
        .find(|d| !d.this_device)
        .unwrap()
        .device_id;

    // The phone plays an album, 30 s in.
    let album = phone
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.song_count >= 3)
        .unwrap();
    let songs = phone.album(album.id).unwrap().songs;
    let state = phone
        .play(
            songs.iter().map(|s| s.id).collect(),
            0,
            StartedFrom::Album,
            Some("ctx".into()),
        )
        .unwrap();
    phone
        .playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    phone.playback_progress(30_000, 30_000).unwrap();

    // The Mac sees it playing, and offers "Continue from phone" once.
    let offered = mac_events
        .wait_nth(Duration::from_secs(5), 1, |e| {
            e.kind == EventKind::ContinueAvailable
        })
        .expect("Continue from phone");
    assert_eq!(offered.device.as_ref().unwrap().name, "phone");
    // The 30 s position follows the first presence within moments.
    let t = Instant::now();
    let row = loop {
        let row = mac
            .output_devices()
            .unwrap()
            .devices
            .into_iter()
            .find(|d| d.device_id == phone_id)
            .unwrap();
        if row.position_ms >= 30_000 || t.elapsed() > Duration::from_secs(2) {
            break row;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(row.state, DeviceState::Playing);
    assert_eq!(row.song.as_ref().unwrap().title, songs[0].title);
    assert!(row.position_ms >= 30_000, "{}", row.position_ms);

    // Push: the Mac's player loads the song and starts at the position it's given.
    let platform = |core: Arc<EncoreCore>, events: Arc<Events>, nth: usize, fail: bool| {
        std::thread::spawn(move || {
            let e = events
                .wait_nth(Duration::from_secs(5), nth, |e| {
                    e.kind == EventKind::HandoffIncoming
                })
                .expect("HandoffIncoming");
            let info = e.handoff.unwrap();
            if fail {
                core.handoff_failed(info.transfer_id.clone(), "Can't play that here".into())
                    .unwrap();
                return (info, -1);
            }
            let pos = core.handoff_ready(info.transfer_id.clone()).unwrap();
            core.playback_started(info.entry.as_ref().unwrap().uid, 60)
                .unwrap();
            (info, pos)
        })
    };
    // A sleep timer on the phone (Phase 9) goes with the music.
    phone
        .set_sleep_timer(SleepTimerSpec {
            mode: SleepTimerMode::Minutes,
            minutes: 30,
        })
        .unwrap();
    let mac_player = platform(mac.clone(), mac_events.clone(), 1, false);
    let tapped = Instant::now();
    block(phone.hand_off_to(mac_id.clone())).unwrap();
    let took = tapped.elapsed();
    let (info, pos) = mac_player.join().unwrap();
    println!(
        "handoff: from the tap to the Mac's confirmation in {:.1} ms (the Mac started at {pos} ms)",
        took.as_secs_f64() * 1e3
    );
    assert!(took < Duration::from_secs(1), "{took:?}");
    assert_eq!(info.entry.as_ref().unwrap().song.title, songs[0].title);
    assert!(info.playing && info.unknown_songs == 0);
    assert!((30_000..32_000).contains(&pos), "{pos}");
    assert!(
        phone_events
            .wait_nth(Duration::from_secs(2), 1, |e| e.kind
                == EventKind::HandoffConfirmed)
            .is_some(),
        "the phone fades out"
    );
    assert_eq!(
        phone.sleep_timer().unwrap(),
        None,
        "the phone's sleep timer ended with the handoff"
    );
    let q = mac.queue().unwrap();
    assert_eq!(q.current.unwrap().song.title, songs[0].title);
    assert_eq!(q.up_next.len(), songs.len() - 1);

    // Counted once: the phone's part is "handedOff", the Mac's play carries its time.
    mac.playback_ended(EndReason::Finished, 170_000).unwrap();
    block(phone.sync_now()).unwrap();
    block(mac.sync_now()).unwrap();
    let export = md.0.join("handoff-export.json");
    let t = Instant::now();
    let plays = loop {
        mac.export_profile(profile.clone(), export.to_string_lossy().into_owned())
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&export).unwrap()).unwrap();
        let plays = v["plays"].as_array().cloned().unwrap_or_default();
        if plays.len() >= 2 || t.elapsed() > Duration::from_secs(5) {
            break plays;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let handed: Vec<_> = plays
        .iter()
        .filter(|p| p["end_reason"] == "handedOff")
        .collect();
    let finished: Vec<_> = plays
        .iter()
        .filter(|p| p["end_reason"] == "finished")
        .collect();
    assert_eq!((handed.len(), finished.len()), (1, 1), "{plays:#?}");
    assert_eq!(
        finished[0]["listened_ms"], 200_000,
        "30 s on the phone + 170 s on the Mac"
    );

    // Pull: the Mac plays the next song; the phone pulls it back.
    let next = mac.queue().unwrap().current.unwrap();
    mac.playback_started(next.uid, 60).unwrap();
    mac.playback_progress(5_000, 5_000).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let phone_player = platform(phone.clone(), phone_events.clone(), 1, false);
    block(phone.pull_from(mac_id.clone())).unwrap();
    let (pulled, _) = phone_player.join().unwrap();
    assert_eq!(pulled.entry.unwrap().song.title, next.song.title);
    assert!(
        mac_events
            .wait_nth(Duration::from_secs(2), 1, |e| e.kind
                == EventKind::HandoffConfirmed)
            .is_some(),
        "the Mac fades out"
    );

    // Remote control from the Mac: pause goes to the phone's player; a queue edit
    // is done by the phone's core.
    block(mac.remote_command(
        phone_id.clone(),
        RemoteCommand::of(RemoteCommandKind::Pause),
    ))
    .unwrap();
    assert!(
        phone_events
            .wait_nth(Duration::from_secs(2), 1, |e| e.remote_command
                == Some(RemoteCommand::of(RemoteCommandKind::Pause)))
            .is_some()
    );
    let remote = block(mac.remote_queue(phone_id.clone())).unwrap();
    assert_eq!(remote.current.unwrap().song.title, next.song.title);
    let extra = mac.songs(SongSort::Title, 0, 1).unwrap()[0].clone();
    block(mac.remote_command(
        phone_id.clone(),
        RemoteCommand {
            song_ids: vec![extra.id],
            ..RemoteCommand::of(RemoteCommandKind::PlayNext)
        },
    ))
    .unwrap();
    assert_eq!(phone.queue().unwrap().up_next[0].song.title, extra.title);

    // A handoff the Mac can't take: the phone keeps playing and says why.
    phone.playback_resumed(6_000).unwrap();
    let mac_refuses = platform(mac.clone(), mac_events.clone(), 2, true);
    let refused = block(phone.hand_off_to(mac_id.clone()));
    mac_refuses.join().unwrap();
    assert!(refused.is_err());
    let failed = phone_events
        .wait_nth(Duration::from_secs(2), 1, |e| {
            e.kind == EventKind::HandoffFailed
        })
        .unwrap();
    assert_eq!(
        failed.handoff.unwrap().reason.as_deref(),
        Some("Can't play that here")
    );
    let me = phone
        .output_devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|d| d.this_device)
        .unwrap();
    assert_eq!(me.state, DeviceState::Playing, "the phone kept playing");

    // Rename from the Mac; the phone sees it.
    block(mac.rename_device(mac_id.clone(), "Living Room Mac".into())).unwrap();
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(5) {
        if phone
            .output_devices()
            .unwrap()
            .devices
            .iter()
            .any(|d| d.name == "Living Room Mac")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        phone
            .output_devices()
            .unwrap()
            .devices
            .iter()
            .any(|d| d.name == "Living Room Mac")
    );
}
