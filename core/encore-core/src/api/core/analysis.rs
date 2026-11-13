//! Sound analysis in the core (Phase 10; docs/adr/core-0019): what each song sounds like, made
//! once from about 30 seconds of its audio, a song at a time, in the background.
//!
//! The platform schedules `run_sound_analysis` as a background job (Android's WorkManager while
//! charging on an unmetered network) and tells the core about power and the network each time; the
//! core decides what is worth doing next and stops when it should. A song's audio is read with
//! small range requests on its original file (no transcoding, so a weak media server is only
//! asked to read and send a megabyte), analysed on a blocking thread, and saved as one row. A
//! song that cannot be read (a format with no decoder, a damaged file) is saved as unavailable
//! with the reason and left alone; one the server failed to serve is not marked at all and is
//! tried again later.

use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use encore_analysis::{MODEL_NAME, Ranged, VERSION, Yamnet, analyse_reader};
use encore_sources::ranged::RangeReader;
use encore_store::{SoundRow, Store};

use super::{EncoreCore, Inner, Profile, lock, now_ms};
use crate::api::types::*;

const ENABLED_KEY: &str = "sound.enabled";
const WIFI_ONLY_KEY: &str = "sound.wifi_only";
const CHARGING_ONLY_KEY: &str = "sound.charging_only";

// Starting values (docs/adr/core-0019), to be tuned on real libraries.
/// A pause after each song, so a weak media server is never worked flat out.
const SONG_GAP: Duration = if cfg!(test) {
    Duration::ZERO
} else {
    Duration::from_millis(250)
};
/// The server failing this many songs in a row ends the run.
const SERVER_FAILURES_BEFORE_STOP: u32 = 3;
/// Progress events at most this often.
const PROGRESS_EVERY: Duration = Duration::from_secs(2);
/// What a song costs before this device has measured any: about 1.2 MB over the network, 3 s of work.
const DEFAULT_BYTES_PER_SONG: u64 = 1_200_000;
const DEFAULT_MS_PER_SONG: u64 = 3_000;

/// The analysis in progress, and what this session has measured (device-wide).
#[derive(Debug, Default)]
pub(super) struct SoundState {
    running: bool,
    cancel: Arc<AtomicBool>,
    /// The model the platform loaded with `set_sound_model`.
    model: Option<Arc<Yamnet>>,
    /// Why the last run stopped, while songs still wait.
    last_stop: Option<SoundStop>,
    songs: u32,
    bytes: u64,
    ms: u64,
}

/// The ranged reader as the decoder wants it, counting the bytes the server sent (shared, so the
/// run can read the total after the decoder has taken the reader).
struct Reader {
    inner: RangeReader,
    fetched: Arc<std::sync::atomic::AtomicU64>,
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.fetched.store(self.inner.cost().1, Ordering::Relaxed);
        Ok(n)
    }
}

impl Seek for Reader {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(to)
    }
}

impl Ranged for Reader {
    fn total_len(&self) -> Option<u64> {
        self.inner.len()
    }
}

struct RunGuard<'a>(&'a Inner);

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut s) = lock(&self.0.sound) {
            s.running = false;
        }
    }
}

impl Inner {
    fn sound_settings_of(&self, store: &Store) -> Result<SoundSettings, EncoreError> {
        Ok(SoundSettings {
            enabled: store.setting(ENABLED_KEY)?.as_deref() == Some("1"),
            wifi_only: store.setting(WIFI_ONLY_KEY)?.as_deref() != Some("0"),
            charging_only: store.setting(CHARGING_ONLY_KEY)?.as_deref() != Some("0"),
        })
    }

    fn sound_status_of(&self, p: &Profile) -> Result<SoundStatus, EncoreError> {
        let (settings, counts) = {
            let store = lock(&p.store)?;
            (self.sound_settings_of(&store)?, store.sound_counts()?)
        };
        let s = lock(&self.sound)?;
        let remaining = counts
            .songs
            .saturating_sub(counts.analysed + counts.unavailable);
        let (bytes_each, ms_each) = if s.songs > 0 {
            (s.bytes / u64::from(s.songs), s.ms / u64::from(s.songs))
        } else {
            (DEFAULT_BYTES_PER_SONG, DEFAULT_MS_PER_SONG)
        };
        Ok(SoundStatus {
            settings,
            songs: counts.songs,
            analysed: counts.analysed,
            unavailable: counts.unavailable,
            running: s.running,
            model_loaded: s.model.is_some(),
            bytes_estimate: u64::from(remaining) * bytes_each,
            minutes_estimate: (u64::from(remaining) * ms_each).div_ceil(60_000) as u32,
            last_stop: s.last_stop.filter(|_| remaining > 0),
        })
    }

    pub(super) fn emit_sound(&self, p: &Profile) {
        if let Ok(status) = self.sound_status_of(p) {
            let mut e = CoreEvent::of(EventKind::SoundAnalysisChanged);
            e.sound = Some(status);
            self.emit(e);
        }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Give the core the sound model (`yamnet-v1.bin`, shipped with the app) by path. Call it once
    /// at start; analysis does nothing without it.
    pub fn set_sound_model(&self, path: String) -> Result<(), EncoreError> {
        let bytes = std::fs::read(&path).map_err(|e| EncoreError::Storage {
            reason: format!("couldn't read the sound model ({e})"),
        })?;
        let model = Yamnet::from_bytes(&bytes).map_err(|e| EncoreError::Storage {
            reason: e.to_string(),
        })?;
        lock(&self.inner.sound)?.model = Some(Arc::new(model));
        if let Ok(p) = self.inner.profile() {
            self.inner.emit_sound(&p);
        }
        Ok(())
    }

    /// Settings > Sound analysis (off until the person turns it on).
    pub fn sound_settings(&self) -> Result<SoundSettings, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        self.inner.sound_settings_of(&store)
    }

    pub fn set_sound_settings(&self, settings: SoundSettings) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        {
            let store = lock(&p.store)?;
            let flag = |on: bool| if on { "1" } else { "0" };
            store.set_setting(ENABLED_KEY, flag(settings.enabled))?;
            store.set_setting(WIFI_ONLY_KEY, flag(settings.wifi_only))?;
            store.set_setting(CHARGING_ONLY_KEY, flag(settings.charging_only))?;
        }
        self.inner.emit_sound(&p);
        Ok(())
    }

    /// How far the library is analysed, with an estimate of the rest.
    pub fn sound_status(&self) -> Result<SoundStatus, EncoreError> {
        let p = self.inner.profile()?;
        self.inner.sound_status_of(&p)
    }

    /// Forget every analysis (Settings > Sound analysis > Delete). Returns how many songs.
    pub fn delete_sound_analysis(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let n = lock(&p.store)?.delete_sounds()?;
        self.inner.emit_sound(&p);
        Ok(n)
    }

    /// Try the songs that could not be read again. Returns how many are waiting again.
    pub fn retry_unreadable_sounds(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let n = lock(&p.store)?.retry_unavailable_sounds()?;
        self.inner.emit_sound(&p);
        Ok(n)
    }

    /// Analyse what waits, one song at a time, until done or blocked (off, no model, Wi-Fi only,
    /// charging only, battery saver, heat, offline, a failing server, sign-in). `network` and
    /// `power` are what the device is like now. Call it from the platform's background job;
    /// `stop_sound_analysis` ends it, keeping what was made. Async.
    pub async fn run_sound_analysis(
        &self,
        network: Network,
        power: PowerState,
    ) -> Result<SoundRun, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move { Self::run_sound_inner(inner, network, power).await })
            .await
    }

    /// End the run in progress after the song it is on.
    pub fn stop_sound_analysis(&self) {
        if let Ok(s) = self.inner.sound.lock() {
            s.cancel.store(true, Ordering::SeqCst);
        }
    }
}

impl EncoreCore {
    async fn run_sound_inner(
        inner: Arc<Inner>,
        network: Network,
        power: PowerState,
    ) -> Result<SoundRun, EncoreError> {
        // This runs on the core's own runtime (`spawn`), so this is its handle: the ranged readers
        // block on it from the blocking threads the analysis runs on.
        let handle = tokio::runtime::Handle::current();
        let none = |stop| SoundRun {
            analysed: 0,
            unreadable: 0,
            remaining: 0,
            stop,
        };
        let Ok(p) = inner.profile() else {
            return Ok(none(SoundStop::NoProfile));
        };
        if inner.can_download(&p).is_err() {
            return Ok(none(SoundStop::NoProfile));
        }
        let settings = inner.sound_settings_of(&*lock(&p.store)?)?;
        if !settings.enabled {
            return Ok(none(SoundStop::NotEnabled));
        }
        let (cancel, model) = {
            let mut s = lock(&inner.sound)?;
            let Some(model) = s.model.clone() else {
                return Ok(none(SoundStop::NoModel));
            };
            if s.running {
                return Ok(none(SoundStop::AlreadyRunning));
            }
            s.running = true;
            s.cancel = Arc::new(AtomicBool::new(false));
            (s.cancel.clone(), model)
        };
        let _guard = RunGuard(&inner);
        inner.emit_sound(&p);

        let (mut analysed, mut unreadable) = (0u32, 0u32);
        let mut tried = std::collections::HashSet::new();
        let mut server_failures = 0u32;
        let mut last_emit = Instant::now();
        let stop = loop {
            if cancel.load(Ordering::SeqCst) {
                break SoundStop::Stopped;
            }
            // The profile switched: this run was for the other one.
            match inner.profile() {
                Ok(now) if Arc::ptr_eq(&now, &p) => {}
                _ => break SoundStop::Stopped,
            }
            if !lock(&inner.connectivity)?.online {
                break SoundStop::Offline;
            }
            // Settings can change while it runs.
            let settings = inner.sound_settings_of(&*lock(&p.store)?)?;
            if !settings.enabled {
                break SoundStop::NotEnabled;
            }
            if settings.wifi_only && network == Network::Cellular {
                break SoundStop::WaitingForWifi;
            }
            if settings.charging_only && !power.charging {
                break SoundStop::WaitingForPower;
            }
            if power.battery_saver || power.hot {
                break SoundStop::Paused;
            }
            let next = lock(&p.store)?
                .pending_sounds(u32::from(VERSION), MODEL_NAME, 16)?
                .into_iter()
                .find(|s| !tried.contains(&s.track_id));
            let Some(song) = next else {
                break SoundStop::Done;
            };
            tried.insert(song.track_id);
            let Ok(source) = inner.source(&p, song.source_id) else {
                break SoundStop::SignInNeeded;
            };
            let reader = match source
                .client
                .range_reader(&song.source_item_id, handle.clone())
            {
                Ok(r) => r,
                Err(_) => break SoundStop::SignInNeeded,
            };
            let ext = song.container.clone();
            let model = model.clone();
            let started = Instant::now();
            let fetched = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let counted = fetched.clone();
            let result = tokio::task::spawn_blocking(move || {
                let reader = Reader {
                    inner: reader,
                    fetched: counted,
                };
                analyse_reader(reader, ext.as_deref(), Some(&model))
            })
            .await
            .map_err(EncoreError::internal)?;
            let bytes = fetched.load(Ordering::Relaxed);
            match result {
                Ok(a) => {
                    server_failures = 0;
                    let row = SoundRow {
                        song_key: song.song_key.clone(),
                        version: u32::from(VERSION),
                        state: "ok".into(),
                        reason: None,
                        profile: Some(a.profile.encode()),
                        model: a.embedding.as_ref().map(|_| MODEL_NAME.to_owned()),
                        embedding: a.embedding.as_ref().map(|e| e.encode()),
                        analysed_utc_ms: now_ms(),
                    };
                    lock(&p.store)?.put_sound(&row)?;
                    analysed += 1;
                    let mut s = lock(&inner.sound)?;
                    s.songs += 1;
                    s.bytes += bytes;
                    s.ms += started.elapsed().as_millis() as u64;
                }
                Err(e) if e.is_permanent() => {
                    server_failures = 0;
                    let row = SoundRow {
                        song_key: song.song_key.clone(),
                        version: u32::from(VERSION),
                        state: "unavailable".into(),
                        reason: Some(e.to_string()),
                        profile: None,
                        model: None,
                        embedding: None,
                        analysed_utc_ms: now_ms(),
                    };
                    lock(&p.store)?.put_sound(&row)?;
                    unreadable += 1;
                }
                Err(_) => {
                    // The song is fine; fetching it failed. Not marked: tried again next run.
                    server_failures += 1;
                    if server_failures >= SERVER_FAILURES_BEFORE_STOP {
                        break SoundStop::ServerBusy;
                    }
                }
            }
            if last_emit.elapsed() >= PROGRESS_EVERY {
                last_emit = Instant::now();
                inner.emit_sound(&p);
            }
            tokio::time::sleep(SONG_GAP).await;
        };
        let remaining = {
            let c = lock(&p.store)?.sound_counts()?;
            c.songs.saturating_sub(c.analysed + c.unavailable)
        };
        if let Ok(mut s) = lock(&inner.sound) {
            s.last_stop = Some(stop);
        }
        // The analysis changed what the picks can know: let the engine see it.
        inner.emit_sound(&p);
        Ok(SoundRun {
            analysed,
            unreadable,
            remaining,
            stop,
        })
    }
}
