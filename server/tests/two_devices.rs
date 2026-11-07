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
    let d = std::env::temp_dir().join(format!("encore-e2e-{name}-{}-{nanos}", std::process::id()));
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
