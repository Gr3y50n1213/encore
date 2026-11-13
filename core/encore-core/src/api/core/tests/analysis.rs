//! Sound analysis (Phase 10) through the public API, on the Jellyfin recording, with a server
//! stand-in that serves generated audio with real `Range` support (the recording holds no audio),
//! serves garbage for most songs (so most analyses end at once as "unreadable") and can fail.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use encore_sources::transport::{BoxFuture, Request, Response};

use super::*;

const MODEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../encore-analysis/model/yamnet-v1.bin"
);

/// A 9.5 s mono WAV at 16 kHz: a tone and a click every half second, different per song.
fn wav_of(item: &str) -> Vec<u8> {
    let seed = item
        .bytes()
        .fold(11u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    let freq = 200.0 + (seed % 900) as f32;
    let rate = 16_000u32;
    let n = (9.5 * rate as f32) as usize;
    let data: Vec<u8> = (0..n)
        .flat_map(|i| {
            let t = i as f32 / rate as f32;
            let click = if (i % 8_000) < 300 { 0.5 } else { 0.0 };
            let v = 0.2 * (2.0 * std::f32::consts::PI * freq * t).sin() + click;
            ((v.clamp(-1.0, 1.0) * 32_000.0) as i16).to_le_bytes()
        })
        .collect();
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

fn song_item(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/Audio/")?;
    let (item, tail) = rest.split_once('/')?;
    (tail == "stream").then(|| item.to_owned())
}

/// The recording for everything but audio.
#[derive(Debug)]
struct Server {
    inner: Arc<dyn Transport>,
    /// Songs that serve real audio; the others serve bytes that are not audio.
    real: Mutex<HashSet<String>>,
    /// Every audio request's item and `Range` header.
    seen: Mutex<Vec<(String, String)>>,
    /// Answer 503 to everything.
    broken: Mutex<bool>,
    /// Stop the run from inside the first request for this item.
    stop_on: Mutex<Option<(String, Arc<EncoreCore>)>>,
    cache: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

impl Server {
    fn new(inner: Arc<dyn Transport>) -> Arc<Server> {
        Arc::new(Server {
            inner,
            real: Mutex::new(HashSet::new()),
            seen: Mutex::new(Vec::new()),
            broken: Mutex::new(false),
            stop_on: Mutex::new(None),
            cache: Mutex::new(HashMap::new()),
        })
    }

    fn body(&self, item: &str) -> Arc<Vec<u8>> {
        if !self.real.lock().unwrap().contains(item) {
            return Arc::new(vec![0x42; 3_000]);
        }
        self.cache
            .lock()
            .unwrap()
            .entry(item.to_owned())
            .or_insert_with(|| Arc::new(wav_of(item)))
            .clone()
    }
}

impl Transport for Server {
    fn send(&self, req: Request) -> BoxFuture<'_, encore_sources::Result<Response>> {
        let Some(item) = song_item(req.url.path()) else {
            let inner = self.inner.clone();
            return Box::pin(async move { inner.send(req).await });
        };
        let range = req
            .headers
            .iter()
            .find(|(k, _)| k == "Range")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        self.seen
            .lock()
            .unwrap()
            .push((item.clone(), range.clone()));
        if let Some((stop_item, core)) = self.stop_on.lock().unwrap().clone()
            && stop_item == item
        {
            core.stop_sound_analysis();
        }
        let broken = *self.broken.lock().unwrap();
        let body = self.body(&item);
        Box::pin(async move {
            let empty = |status| Response {
                status,
                body: Vec::new(),
                server_date_utc_ms: None,
                content_range_total: None,
            };
            if broken {
                return Ok(empty(503));
            }
            let total = body.len() as u64;
            let Some((a, b)) = range
                .strip_prefix("bytes=")
                .and_then(|r| r.split_once('-'))
                .and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))
            else {
                return Ok(Response {
                    status: 200,
                    body: body.to_vec(),
                    server_date_utc_ms: None,
                    content_range_total: None,
                });
            };
            if a >= total {
                return Ok(empty(416));
            }
            let end = b.min(total - 1);
            Ok(Response {
                status: 206,
                body: body[a as usize..=end as usize].to_vec(),
                server_date_utc_ms: None,
                content_range_total: Some(total),
            })
        })
    }
}

fn recording() -> Arc<dyn Transport> {
    Arc::new(FixtureTransport::from_json(RECORDED).unwrap())
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

const WIFI_CHARGING: PowerState = PowerState {
    charging: true,
    battery_saver: false,
    hot: false,
};

fn on() -> SoundSettings {
    SoundSettings {
        enabled: true,
        wifi_only: true,
        charging_only: true,
    }
}

/// The songs analysis would take first, as their item IDs on the server.
fn first_items(core: &EncoreCore, n: u32) -> Vec<String> {
    let p = core.inner.profile().unwrap();
    let store = p.store.lock().unwrap();
    store
        .pending_sounds(
            u32::from(encore_analysis::VERSION),
            encore_analysis::MODEL_NAME,
            n,
        )
        .unwrap()
        .into_iter()
        .map(|s| s.source_item_id)
        .collect()
}

fn run(core: &EncoreCore, network: Network, power: PowerState) -> SoundRun {
    block(core.run_sound_analysis(network, power)).unwrap()
}

#[test]
fn it_waits_to_be_asked_and_for_its_model() {
    let dir = temp_dir("sound-off");
    let server = Server::new(recording());
    let (core, _events) = core_with_server(&dir, server);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    // Off until the person turns it on, whatever else is true.
    assert!(!core.sound_settings().unwrap().enabled);
    assert_eq!(
        run(&core, Network::Wifi, WIFI_CHARGING).stop,
        SoundStop::NotEnabled
    );
    // On, but the platform has not given it the model.
    core.set_sound_settings(on()).unwrap();
    assert_eq!(
        run(&core, Network::Wifi, WIFI_CHARGING).stop,
        SoundStop::NoModel
    );
    assert!(!core.sound_status().unwrap().model_loaded);
    // A model path that is not a model is refused with words, not a crash.
    assert!(core.set_sound_model("/no/such/file".into()).is_err());
    let junk = dir.0.join("junk.bin");
    std::fs::write(&junk, b"not a model").unwrap();
    assert!(
        core.set_sound_model(junk.to_string_lossy().into_owned())
            .is_err()
    );
}

#[test]
fn it_waits_for_wifi_power_and_a_calm_device() {
    let dir = temp_dir("sound-conditions");
    let server = Server::new(recording());
    let (core, _events) = core_with_server(&dir, server.clone());
    signed_in(&core);
    block(core.sync_library()).unwrap();
    core.set_sound_model(MODEL.into()).unwrap();
    core.set_sound_settings(on()).unwrap();
    let cases = [
        (Network::Cellular, WIFI_CHARGING, SoundStop::WaitingForWifi),
        (
            Network::Wifi,
            PowerState {
                charging: false,
                ..WIFI_CHARGING
            },
            SoundStop::WaitingForPower,
        ),
        (
            Network::Wifi,
            PowerState {
                battery_saver: true,
                ..WIFI_CHARGING
            },
            SoundStop::Paused,
        ),
        (
            Network::Wifi,
            PowerState {
                hot: true,
                ..WIFI_CHARGING
            },
            SoundStop::Paused,
        ),
    ];
    for (network, power, stop) in cases {
        let r = run(&core, network, power);
        assert_eq!((r.stop, r.analysed, r.unreadable), (stop, 0, 0));
    }
    // Wi-Fi only and charging only are the person's to turn off.
    core.set_sound_settings(SoundSettings {
        enabled: true,
        wifi_only: false,
        charging_only: false,
    })
    .unwrap();
    server.real.lock().unwrap().clear();
    *server.broken.lock().unwrap() = true;
    let r = run(
        &core,
        Network::Cellular,
        PowerState {
            charging: false,
            ..WIFI_CHARGING
        },
    );
    assert_eq!(
        r.stop,
        SoundStop::ServerBusy,
        "it went ahead: nothing blocked it"
    );
    // Offline.
    core.set_connectivity(false, Network::Wifi, vec![]).unwrap();
    assert_eq!(
        run(&core, Network::Wifi, WIFI_CHARGING).stop,
        SoundStop::Offline
    );
    assert!(!server.seen.lock().unwrap().is_empty());
}

#[test]
fn it_analyses_what_it_can_marks_the_rest_and_does_not_repeat_itself() {
    let dir = temp_dir("sound-run");
    let server = Server::new(recording());
    let (core, events) = core_with_server(&dir, server.clone());
    signed_in(&core);
    block(core.sync_library()).unwrap();
    core.set_sound_model(MODEL.into()).unwrap();
    core.set_sound_settings(on()).unwrap();

    // Three songs serve real audio; every other song serves bytes that are not audio.
    let real = first_items(&core, 3);
    server.real.lock().unwrap().extend(real.iter().cloned());
    let before = core.sound_status().unwrap();
    assert!(before.songs > 3 && before.analysed == 0 && before.bytes_estimate > 0);

    events.take();
    let r = run(&core, Network::Wifi, WIFI_CHARGING);
    assert_eq!(r.stop, SoundStop::Done);
    assert_eq!(r.analysed, 3);
    assert_eq!(r.unreadable, before.songs - 3);
    assert_eq!(r.remaining, 0);

    let status = core.sound_status().unwrap();
    assert_eq!(
        (status.analysed, status.unavailable, status.running),
        (3, before.songs - 3, false)
    );
    assert!(events.kinds().contains(&EventKind::SoundAnalysisChanged));
    let last = events
        .take()
        .into_iter()
        .rev()
        .find_map(|e| e.sound)
        .expect("a status came with the event");
    assert_eq!(last.analysed, 3);

    // A row has the profile, and the embedding made by the model; a bad one has the reason.
    let p = core.inner.profile().unwrap();
    let store = p.store.lock().unwrap();
    let rows = store.sounds_ok().unwrap();
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert_eq!(row.model.as_deref(), Some(encore_analysis::MODEL_NAME));
        let profile = encore_analysis::SoundProfile::decode(row.profile.as_ref().unwrap()).unwrap();
        assert!(profile.lufs < 0.0);
        let e = encore_analysis::Embedding::decode(row.embedding.as_ref().unwrap()).unwrap();
        assert_eq!(e.pooled.len(), encore_analysis::EMBEDDING_LEN);
    }
    drop(store);

    // Only the bytes asked for were fetched: ranges of at most 256 KB, never a whole file at once.
    let seen = server.seen.lock().unwrap().clone();
    assert!(seen.iter().all(|(_, range)| range.starts_with("bytes=")));

    // Run again: nothing is left, nothing is asked for.
    let asked = seen.len();
    let again = run(&core, Network::Wifi, WIFI_CHARGING);
    assert_eq!(
        (again.stop, again.analysed, again.unreadable),
        (SoundStop::Done, 0, 0)
    );
    assert_eq!(server.seen.lock().unwrap().len(), asked);

    // "Try the unreadable ones again" puts them back; "Delete" forgets everything.
    assert_eq!(core.retry_unreadable_sounds().unwrap(), before.songs - 3);
    assert_eq!(core.sound_status().unwrap().unavailable, 0);
    assert_eq!(core.delete_sound_analysis().unwrap(), 3);
    assert_eq!(core.sound_status().unwrap().analysed, 0);
}

#[test]
fn a_failing_server_stops_the_run_and_marks_nothing() {
    let dir = temp_dir("sound-busy");
    let server = Server::new(recording());
    let (core, _events) = core_with_server(&dir, server.clone());
    signed_in(&core);
    block(core.sync_library()).unwrap();
    core.set_sound_model(MODEL.into()).unwrap();
    core.set_sound_settings(on()).unwrap();
    *server.broken.lock().unwrap() = true;
    let r = run(&core, Network::Wifi, WIFI_CHARGING);
    assert_eq!(r.stop, SoundStop::ServerBusy);
    assert_eq!((r.analysed, r.unreadable), (0, 0));
    // The songs are fine, so none is marked: they are all still waiting.
    let status = core.sound_status().unwrap();
    assert_eq!((status.analysed, status.unavailable), (0, 0));
    assert!(status.last_stop == Some(SoundStop::ServerBusy));
    // The server recovers: the next run does them.
    *server.broken.lock().unwrap() = false;
    let first = first_items(&core, 1);
    server.real.lock().unwrap().extend(first);
    let r = run(&core, Network::Wifi, WIFI_CHARGING);
    assert_eq!(r.stop, SoundStop::Done);
    assert_eq!(r.analysed, 1);
}

#[test]
fn stopping_ends_the_run_after_the_song_it_is_on() {
    let dir = temp_dir("sound-stop");
    let server = Server::new(recording());
    let (core, _events) = core_with_server(&dir, server.clone());
    signed_in(&core);
    block(core.sync_library()).unwrap();
    core.set_sound_model(MODEL.into()).unwrap();
    core.set_sound_settings(on()).unwrap();
    let second = first_items(&core, 2)[1].clone();
    *server.stop_on.lock().unwrap() = Some((second, core.clone()));
    let r = run(&core, Network::Wifi, WIFI_CHARGING);
    assert_eq!(r.stop, SoundStop::Stopped);
    // Two songs were looked at (both unreadable: the stand-in serves garbage), then it stopped.
    assert_eq!(r.analysed + r.unreadable, 2);
    assert!(r.remaining > 0);
    assert!(!core.sound_status().unwrap().running);
}
