//! `EncoreCore`: the one object every platform holds (docs/core/CORE-API.md).
//!
//! Threading: every call may be made from any thread. Calls marked async run on
//! the core's own small runtime, so they work the same from Swift, Kotlin and Dart.
//! Synchronous calls read the on-device mirror and return in milliseconds; they are
//! safe on the main thread. Events go to the listener on a core thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use encore_sources::http::HttpTransport;
use encore_sources::jellyfin::{self, DeviceInfo, JellyfinClient, ServerInfo, Session};
use encore_sources::{JellyfinSource, MediaSource, Transport};
use encore_store::{
    CachedLyrics, CachedPalette, NewSource, Page, SearchLimits, SourceRecord, Store,
    TopResult as StoreTop,
};
use sha2::{Digest, Sha256};

use super::types::*;
use crate::brand::APP_NAME;
use crate::edits::{self, Editor, Target};
use crate::playlog::{Close, PlayLog, PlayStart};
use crate::queue::{Advance, NewItem, PreviousOutcome, Queue, QueueItem};
use crate::signal::SkipOrigin as Moved;
use crate::tokens::tokens;
use encore_sync::{Hlc, HlcClock};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const QUICK_CONNECT_POLL: Duration = Duration::from_secs(3);
const DEFAULT_CONTAINERS: &[&str] = &["flac", "mp3", "m4a", "aac", "alac", "ogg", "opus", "wav"];

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> Result<MutexGuard<'_, T>, EncoreError> {
    m.lock().map_err(|_| EncoreError::internal("lock"))
}

/// A profile's folder name: stable for one account on one server.
fn profile_id(server_id: &str, user_id: &str) -> String {
    let digest = Sha256::digest(format!("{server_id}\u{1f}{user_id}").as_bytes());
    let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("jf-{hex}")
}

/// A profile ID names a folder, so it must be a plain name: an ID from the app can
/// never point outside the profiles folder.
fn checked_profile_id(id: &str) -> Result<&str, EncoreError> {
    let plain = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if plain {
        Ok(id)
    } else {
        Err(EncoreError::NotFound {
            reason: "That profile isn't on this device".into(),
        })
    }
}

fn credential_key(profile_id: &str, source_id: i64) -> String {
    format!("{}/{profile_id}/{source_id}", APP_NAME.to_ascii_lowercase())
}

fn host_of(address: &str) -> String {
    let typed = address.trim();
    let with_scheme = if typed.contains("://") {
        typed.to_owned()
    } else {
        format!("https://{typed}")
    };
    url::Url::parse(&with_scheme)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default()
}

struct PendingQuickConnect {
    client: JellyfinClient,
    info: ServerInfo,
    secret: String,
    started: Instant,
}

struct Playback {
    queue: Queue,
    log: PlayLog,
    playing_uid: Option<u64>,
    last_listened_ms: u64,
    preloaded_for: Option<u64>,
    /// What moved on from the current song, until its play closes.
    pending_origin: Option<Moved>,
    /// The last unplug, output switch, call or assistant (UTC milliseconds).
    last_interruption_utc_ms: Option<i64>,
    /// A radio's seed, held for the whole session (smart picks).
    radio_seed: Option<String>,
    /// The device's UTC offset, as the last `playback_started` reported it.
    utc_offset_minutes: i32,
    /// While Private listening is on: when something last played (it ends after
    /// `smartPicks.privateListeningEndsAfterHoursSilent` of silence).
    private_last_activity_utc_ms: Option<i64>,
    /// A handed-off song's uid and the listening time it brings (Phase 7).
    handoff_carry: Option<(u64, u64)>,
}

impl Playback {
    /// What is known about the play that is closing.
    fn closing(&mut self) -> Close {
        Close {
            origin: self.pending_origin.take(),
            last_interruption_utc_ms: self.last_interruption_utc_ms,
        }
    }
}

struct Profile {
    id: String,
    store: Mutex<Store>,
    sources: Mutex<HashMap<i64, Arc<JellyfinSource>>>,
    playback: Mutex<Playback>,
    /// The clock for library edits' events (plan section 5).
    edits_clock: Mutex<HlcClock>,
    /// Songs whose lyrics failed to fetch ahead this session, so the next batch
    /// moves on instead of asking for them again.
    lyrics_ahead_failed: Mutex<std::collections::HashSet<i64>>,
    /// Sync with the companion server, when the profile has one.
    companion: Mutex<Option<Arc<sync_loop::Session>>>,
    /// Smart picks' model, rebuilt when its inputs change.
    recs: Mutex<Option<recs::RecsCache>>,
    /// Handoff and remote control (Phase 7). A leaf lock.
    live: Mutex<live::Live>,
}

struct Inner {
    config: CoreConfig,
    listener: Mutex<Option<Arc<dyn CoreListener>>>,
    profile: Mutex<Option<Arc<Profile>>>,
    pins: Mutex<HashMap<String, String>>,
    quick: tokio::sync::Mutex<Option<PendingQuickConnect>>,
    cancel_quick: AtomicBool,
    poll_every: Duration,
    sync_page_size: u32,
    /// Tests replay a recording instead of the network.
    transport_override: Option<Arc<dyn Transport>>,
    /// The device registry: switcher order, names, avatars, PINs, Guest.
    registry: Mutex<encore_store::Registry>,
    /// Profiles unlocked for their next opening (each unlock is used once).
    unlocked: Mutex<std::collections::HashSet<String>>,
    /// What the platform last said about the network (Phase 8).
    connectivity: Mutex<downloads::Connectivity>,
    /// The download run in progress, if any (Phase 8).
    transfers: Mutex<downloads::Transfers>,
    /// Phase 9, WP2: the widget snapshot writer's wake-up and write lock.
    widgets: widgets::Widgets,
}

/// The Encore core. Create one per app.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct EncoreCore {
    inner: Arc<Inner>,
    /// The core's own runtime. Kept here, not in `Inner`, because tasks hold
    /// `Inner`: the runtime must never be dropped by a task running on it.
    rt: Option<tokio::runtime::Runtime>,
}

impl Drop for EncoreCore {
    /// `shutdown_background` is allowed anywhere, even inside another runtime's
    /// task (where Dart can release the last reference); a plain drop would panic.
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

impl std::fmt::Debug for EncoreCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncoreCore")
            .field("platform", &self.inner.config.platform)
            .finish_non_exhaustive()
    }
}

impl Inner {
    fn device(&self) -> DeviceInfo {
        DeviceInfo {
            client: APP_NAME.to_owned(),
            device_name: self.config.device_name.clone(),
            device_id: self.config.device_id.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    fn emit(&self, event: CoreEvent) {
        let listener = self.listener.lock().ok().and_then(|l| l.clone());
        if let Some(l) = listener {
            l.on_event(event);
        }
    }

    fn transport_for(&self, pinned: Option<&str>) -> Result<Arc<dyn Transport>, EncoreError> {
        if let Some(t) = &self.transport_override {
            return Ok(t.clone());
        }
        let t = HttpTransport::new(pinned, Duration::from_secs(tokens().probe_timeout_seconds))?;
        Ok(Arc::new(t))
    }

    fn pin_for(&self, address: &str) -> Option<String> {
        self.pins.lock().ok()?.get(&host_of(address)).cloned()
    }

    fn profile(&self) -> Result<Arc<Profile>, EncoreError> {
        lock(&self.profile)?
            .clone()
            .ok_or_else(EncoreError::no_profile)
    }

    fn profiles_dir(&self) -> PathBuf {
        Path::new(&self.config.data_dir).join("profiles")
    }

    fn store_path(&self, profile_id: &str) -> PathBuf {
        self.profiles_dir().join(profile_id).join("library.sqlite")
    }

    async fn check_address(&self, address: String) -> Result<ServerCheck, EncoreError> {
        let t = tokens();
        let plan = jellyfin::plan_address(&address, &t.https_ports, t.http_port)?;
        let transport = self.transport_for(self.pin_for(&address).as_deref())?;
        let info = jellyfin::probe(
            transport,
            &plan,
            &self.device(),
            Duration::from_secs(t.probe_timeout_seconds),
        )
        .await?;
        Ok(ServerCheck {
            base_url: info.base_url.to_string(),
            name: info.name,
            version: info.version,
            server_id: info.server_id,
            plain_http_on_local_network: info.warning.is_some(),
        })
    }

    async fn client_for(
        &self,
        base_url: &str,
    ) -> Result<(JellyfinClient, ServerInfo), EncoreError> {
        let url = url::Url::parse(base_url).map_err(|e| EncoreError::InvalidAddress {
            reason: e.to_string(),
        })?;
        let transport = self.transport_for(self.pin_for(base_url).as_deref())?;
        let client = JellyfinClient::new(transport, url, self.device(), REQUEST_TIMEOUT);
        let info = client.public_info().await?;
        Ok((client, info))
    }

    /// Store the account, open its profile and return what the app keeps.
    fn complete_sign_in(
        &self,
        client: JellyfinClient,
        info: ServerInfo,
        session: Session,
    ) -> Result<SignedIn, EncoreError> {
        let id = profile_id(&info.server_id, &session.user_id);
        let path = self.store_path(&id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| EncoreError::Storage {
                reason: format!("couldn't create the profile folder ({e})"),
            })?;
        }
        let pin = self.pin_for(info.base_url.as_str());
        let store = Store::open(&path)?;
        let source_id = store.add_source(&NewSource {
            kind: "jellyfin",
            name: &info.name,
            base_url: info.base_url.as_str(),
            server_id: &info.server_id,
            server_version: Some(&info.version),
            user_id: &session.user_id,
            user_name: &session.user_name,
            pinned_cert_sha256: pin.as_deref(),
            created_utc_ms: now_ms(),
        })?;
        // A fresh sign-in: the media server no longer says it ended.
        store.set_setting(profiles::SESSION_ENDED_KEY, "")?;
        drop(store);
        // Signing in proves who it is: a PIN doesn't stop it opening.
        lock(&self.unlocked)?.insert(id.clone());
        self.open_profile(
            &id,
            vec![SourceToken {
                source_id,
                token: session.token.clone(),
            }],
        )?;
        // The client used to sign in already holds the session.
        let source = JellyfinSource {
            client: client.signed_in(session.token.clone(), session.user_id.clone()),
            quick_connect: true,
        };
        if let Ok(p) = self.profile() {
            lock(&p.sources)?.insert(source_id, Arc::new(source));
        }
        Ok(SignedIn {
            credential_key: credential_key(&id, source_id),
            profile_id: id,
            source_id,
            token: session.token,
            user_name: session.user_name,
        })
    }

    fn source_from(
        &self,
        record: &SourceRecord,
        token: &str,
    ) -> Result<JellyfinSource, EncoreError> {
        let url = url::Url::parse(&record.base_url).map_err(|e| EncoreError::InvalidAddress {
            reason: e.to_string(),
        })?;
        let transport = self.transport_for(record.pinned_cert_sha256.as_deref())?;
        let client = JellyfinClient::new(transport, url, self.device(), REQUEST_TIMEOUT)
            .signed_in(token.to_owned(), record.user_id.clone());
        Ok(JellyfinSource {
            client,
            quick_connect: true,
        })
    }

    fn open_profile(&self, id: &str, tokens_in: Vec<SourceToken>) -> Result<(), EncoreError> {
        self.take_unlock(id)?;
        self.close_profile()?;
        let path = self.store_path(checked_profile_id(id)?);
        if !path.exists() {
            return Err(EncoreError::NotFound {
                reason: "That profile isn't on this device".into(),
            });
        }
        let store = Store::open(&path)?;
        let mut log = PlayLog::new(&store, &self.config.device_id, tokens().checkpoint_seconds)?;
        log.recover(&store, now_ms())?;
        let mut sources = HashMap::new();
        let records = store.sources()?;
        for t in tokens_in {
            if let Some(r) = records.iter().find(|r| r.id == t.source_id) {
                sources.insert(r.id, Arc::new(self.source_from(r, &t.token)?));
            }
        }
        let seed = u64::from_le_bytes(
            Sha256::digest(format!("{id}{}", now_ms()).as_bytes())[..8]
                .try_into()
                .unwrap_or([7; 8]),
        );
        let last_edit = [store.newest_hlc()?, store.newest_event_hlc()?]
            .into_iter()
            .flatten()
            .filter_map(|h| h.parse::<Hlc>().ok())
            .max()
            .unwrap_or_default();
        let profile = Profile {
            id: id.to_owned(),
            edits_clock: Mutex::new(HlcClock::resume(last_edit)),
            lyrics_ahead_failed: Mutex::new(Default::default()),
            companion: Mutex::new(None),
            recs: Mutex::new(None),
            live: Mutex::new(Default::default()),
            store: Mutex::new(store),
            sources: Mutex::new(sources),
            playback: Mutex::new(Playback {
                queue: Queue::new(seed),
                log,
                playing_uid: None,
                last_listened_ms: 0,
                preloaded_for: None,
                pending_origin: None,
                last_interruption_utc_ms: None,
                radio_seed: None,
                utc_offset_minutes: 0,
                private_last_activity_utc_ms: None,
                handoff_carry: None,
            }),
        };
        *lock(&self.profile)? = Some(Arc::new(profile));
        {
            let reg = lock(&self.registry)?;
            reg.ensure(id, now_ms())?;
            reg.set_state("last_active", Some(id))?;
        }
        self.after_open()
    }

    fn close_profile(&self) -> Result<(), EncoreError> {
        self.take_profile().map(drop)
    }

    /// Close the open profile and hand it back. The database stays open until the
    /// last holder (a background send, say) lets go of it.
    fn take_profile(&self) -> Result<Option<Arc<Profile>>, EncoreError> {
        self.before_close();
        let old = lock(&self.profile)?.take();
        if let Some(p) = &old {
            // Stop syncing: unsent changes stay queued and go out next time.
            if let Some(s) = lock(&p.companion)?.take() {
                s.stop();
            }
            let mut pb = lock(&p.playback)?;
            let store = lock(&p.store)?;
            let listened = pb.last_listened_ms;
            pb.log.finish(
                &store,
                listened,
                crate::playlog::EndReason::EndedBySystem,
                now_ms(),
            )?;
        }
        Ok(old)
    }

    fn song(store: &Store, id: i64) -> Option<Song> {
        store.track(id).ok().map(Song::from)
    }

    fn entry(store: &Store, item: &QueueItem) -> Option<QueueEntry> {
        Some(QueueEntry {
            uid: item.uid as i64,
            song: Self::song(store, item.track_id)?,
            chosen_by: match item.chosen_by {
                crate::queue::ChosenBy::User => ChosenBy::User,
                crate::queue::ChosenBy::App => ChosenBy::App,
            },
        })
    }

    fn queue_state(store: &Store, pb: &Playback, can_undo: bool) -> QueueState {
        let v = pb.queue.view();
        let list = |items: &[QueueItem]| {
            items
                .iter()
                .filter_map(|i| Self::entry(store, i))
                .collect::<Vec<_>>()
        };
        QueueState {
            history: list(v.history),
            current: v.current.and_then(|i| Self::entry(store, i)),
            up_next: list(v.up_next),
            autoplay: list(v.autoplay),
            shuffle: v.shuffle,
            repeat: v.repeat.into(),
            autoplay_enabled: v.autoplay_enabled,
            can_undo,
        }
    }

    /// Run a queue change, then tell the listener what changed.
    fn with_queue<R>(
        &self,
        change: impl FnOnce(&mut Playback, &Store) -> R,
    ) -> Result<(R, QueueState), EncoreError> {
        let p = self.profile()?;
        let mut pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        let before = pb.queue.current().map(|i| i.uid);
        let r = change(&mut pb, &store);
        let mut probe = pb.queue.clone();
        let can_undo = probe.undo();
        let state = Self::queue_state(&store, &pb, can_undo);
        let after = pb.queue.current().map(|i| i.uid);
        drop(store);
        drop(pb);
        let mut e = CoreEvent::of(EventKind::QueueChanged);
        e.queue = Some(state.clone());
        self.emit(e);
        if before != after {
            let mut e = CoreEvent::of(EventKind::NowPlaying);
            e.now_playing = state.current.clone();
            self.emit(e);
            // Phase 9, WP2: now playing changed.
            self.widgets_soon();
        }
        // Phase 9, WP1: a song or album timer's stop point moves with the queue.
        self.sleep_queue_changed();
        Ok((r, state))
    }

    fn new_items(song_ids: &[i64], from: StartedFrom, context_id: Option<String>) -> Vec<NewItem> {
        song_ids
            .iter()
            .map(|&id| NewItem {
                track_id: id,
                chosen_by: crate::queue::ChosenBy::User,
                started_from: from.into(),
                context_id: context_id.clone(),
            })
            .collect()
    }

    fn source(&self, p: &Profile, source_id: i64) -> Result<Arc<JellyfinSource>, EncoreError> {
        lock(&p.sources)?
            .get(&source_id)
            .cloned()
            .ok_or_else(|| EncoreError::SessionExpired {
                reason: "Sign in to this server again to play from it".into(),
            })
    }

    fn quality(&self, store: &Store, network: Network) -> Result<StreamQuality, EncoreError> {
        let key = match network {
            Network::Wifi => "stream.quality.wifi",
            Network::Cellular => "stream.quality.cellular",
        };
        let stored = store.setting(key)?;
        let name = stored.unwrap_or_else(|| match network {
            Network::Wifi => tokens().default_wifi.clone(),
            Network::Cellular => tokens().default_cellular.clone(),
        });
        Ok(match name.as_str() {
            "high" => StreamQuality::High,
            "normal" => StreamQuality::Normal,
            "dataSaver" => StreamQuality::DataSaver,
            _ => StreamQuality::Original,
        })
    }
}

/// The songs for these IDs, in order; IDs no longer in the library are left out
/// (unavailable songs are never added).
fn songs_of(store: &Store, ids: &[i64]) -> Result<Vec<encore_store::TrackRow>, EncoreError> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        match store.track(*id) {
            Ok(t) => out.push(t),
            Err(encore_store::StoreError::NotFound(_)) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(out)
}

fn target_of(t: &DropTarget) -> Result<Target, EncoreError> {
    let needs_id = || {
        t.id.ok_or_else(|| EncoreError::NotFound {
            reason: "That playlist isn't in your library".into(),
        })
    };
    Ok(match t.kind {
        DropTargetKind::Playlist => Target::Playlist(needs_id()?),
        DropTargetKind::NewPlaylist => Target::NewPlaylist,
        DropTargetKind::LikedSongs => Target::LikedSongs,
        DropTargetKind::UpNext => Target::UpNext,
        DropTargetKind::PlayNext => Target::PlayNext,
        DropTargetKind::Album => Target::Album(t.id.unwrap_or(0)),
    })
}

fn edit_result(a: edits::Applied, now: i64) -> EditResult {
    EditResult {
        undo_until_utc_ms: a.edit_id.as_ref().map(|_| now + tokens().undo_ms),
        edit_id: a.edit_id,
        added: a.added,
        already_there: a.already_there,
        playlist_id: a.playlist_id,
    }
}

/// Send the waiting edits of every signed-in server.
async fn send_all(inner: &Inner) -> Result<SentEdits, EncoreError> {
    let p = inner.profile()?;
    let sources: Vec<(i64, Arc<JellyfinSource>)> = lock(&p.sources)?
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    let mut total = SentEdits {
        sent: 0,
        refused: 0,
        waiting: 0,
    };
    for (id, source) in sources {
        let f = edits::flush(&p.store, id, source.as_ref(), now_ms()).await?;
        total.sent += f.sent;
        total.refused += f.refused;
        total.waiting += f.waiting;
    }
    Ok(total)
}

impl EncoreCore {
    /// Offline, a queued song that can't play is refused like a tap in a list.
    fn check_uid_playable(&self, uid: i64) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let pb = lock(&p.playback)?;
        let v = pb.queue.view();
        let track = v
            .history
            .iter()
            .chain(v.up_next)
            .chain(v.autoplay)
            .find(|i| i.uid == uid as u64)
            .map(|i| i.track_id);
        match track {
            Some(t) if !pb.queue.can_play(t) => Err(downloads::not_downloaded()),
            _ => Ok(()),
        }
    }

    /// Run one library edit with an `Editor`, then send it in the background.
    fn edit(
        &self,
        f: impl FnOnce(&mut Editor<'_>, &Store) -> Result<edits::Applied, EncoreError>,
    ) -> Result<EditResult, EncoreError> {
        let p = self.inner.profile()?;
        let result = {
            let store = lock(&p.store)?;
            let mut clock = lock(&p.edits_clock)?;
            let now = now_ms();
            let mut ed = Editor {
                store: &store,
                clock: &mut clock,
                device_id: &self.inner.config.device_id,
                now_utc_ms: now,
                undo_ms: tokens().undo_ms,
            };
            edit_result(f(&mut ed, &store)?, now)
        };
        self.send_soon();
        // A downloaded playlist or Liked songs follows the edit (Phase 8).
        let _ = self
            .inner
            .reconcile_downloads(&p, Some(&["playlist", "liked"]));
        Ok(result)
    }

    /// Up Next and Play Next: the queue changes, with an edit ID for its Undo like
    /// every other edit. Nothing is sent anywhere (the queue is this device's).
    fn queue_add(
        &self,
        target: Target,
        song_ids: &[i64],
        position: Option<u32>,
    ) -> Result<EditResult, EncoreError> {
        let p = self.inner.profile()?;
        let ids: Vec<i64> = {
            let store = lock(&p.store)?;
            songs_of(&store, song_ids)?.iter().map(|t| t.id).collect()
        };
        if ids.is_empty() {
            return Err(edits::EditError::Refused(edits::Refusal::NothingToAdd).into());
        }
        let ids = self.inner.offline_songs(&p, &ids)?;
        let n = ids.len() as u32;
        let items = Inner::new_items(&ids, StartedFrom::Queue, None);
        self.inner.with_queue(|pb, _| match (target, position) {
            (Target::PlayNext, _) => pb.queue.play_next(items),
            (_, Some(at)) => pb.queue.insert_up_next(items, at as usize),
            _ => pb.queue.add_to_up_next(items),
        })?;
        let store = lock(&p.store)?;
        let mut clock = lock(&p.edits_clock)?;
        let now = now_ms();
        let edit_id = Editor {
            store: &store,
            clock: &mut clock,
            device_id: &self.inner.config.device_id,
            now_utc_ms: now,
            undo_ms: tokens().undo_ms,
        }
        .queue_edit()?;
        Ok(EditResult {
            edit_id: Some(edit_id),
            added: n,
            already_there: 0,
            playlist_id: None,
            undo_until_utc_ms: Some(now + tokens().undo_ms),
        })
    }

    /// Start sending the waiting edits in the background; failures wait for the
    /// next chance (another edit, a sync, or `send_edits`).
    /// Pick up sync with the companion where it was, with the refresh token from
    /// the keychain (`open_profile`). No address or device kept: nothing to do.
    fn resume_companion(&self, refresh: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let (url, device) = {
            let st = lock(&p.store)?;
            (
                st.setting(sync_loop::URL_KEY)?,
                st.setting(sync_loop::DEVICE_KEY)?,
            )
        };
        let (Some(url), Some(device)) = (
            url.filter(|u| !u.is_empty()),
            device.filter(|d| !d.is_empty()),
        ) else {
            return Ok(());
        };
        let base = url::Url::parse(&url).map_err(|e| EncoreError::InvalidAddress {
            reason: e.to_string(),
        })?;
        let client = crate::companion::Client::new(base, self.inner.transport_for(None)?);
        let tokens = crate::companion::Tokens {
            access: String::new(),
            access_expires_utc_ms: 0,
            refresh,
        };
        let s = sync_loop::Session::new(client, device, tokens, sync_loop::last_synced(&p));
        *lock(&p.companion)? = Some(s.clone());
        if let Some(rt) = &self.rt {
            sync_loop::start(rt.handle(), self.inner.clone(), p, s);
        }
        Ok(())
    }

    /// Something to sync changed: wake the companion pusher.
    fn sync_soon(&self) {
        if let Ok(p) = self.inner.profile()
            && let Ok(c) = lock(&p.companion)
            && let Some(s) = c.as_ref()
        {
            s.wake();
        }
    }
    fn send_soon(&self) {
        self.sync_soon();
        let Some(rt) = self.rt.as_ref() else { return };
        let inner = self.inner.clone();
        rt.spawn(async move {
            let _ = send_all(&inner).await;
        });
    }

    /// Make, or read back, the palette of an album's or playlist's cover.
    async fn palette_for(
        &self,
        kind: &'static str,
        id: i64,
    ) -> Result<Option<ArtworkPalette>, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            inner.make_palette(&p, kind, id).await
        })
        .await
    }

    /// Run on the core's runtime, so callers on any executor (Swift, Kotlin, Dart)
    /// get the same behaviour; reqwest needs tokio underneath.
    async fn spawn<T: Send + 'static>(
        &self,
        fut: impl std::future::Future<Output = Result<T, EncoreError>> + Send + 'static,
    ) -> Result<T, EncoreError> {
        let rt = self
            .rt
            .as_ref()
            .ok_or_else(|| EncoreError::internal("runtime stopped"))?;
        rt.spawn(fut).await.map_err(EncoreError::internal)?
    }
}

impl Inner {
    /// Make, or read back, the palette of an album's or playlist's cover: the
    /// database first (no network), else one small PNG from the source, kept
    /// for good. The app's `album_palette` and the widget writer
    /// (`widgets.rs`) both come here, so a palette is made once per cover.
    async fn make_palette(
        &self,
        p: &Profile,
        kind: &'static str,
        id: i64,
    ) -> Result<Option<ArtworkPalette>, EncoreError> {
        let settings = &tokens().palette;
        let (found, cached) = {
            let store = lock(&p.store)?;
            (
                store.artwork_tag(kind, id)?,
                store.cached_palette(kind, id)?,
            )
        };
        let Some((source_id, item, tag)) = found else {
            return Err(EncoreError::NotFound {
                reason: format!("That {kind} isn't in your library"),
            });
        };
        let read = |c: &CachedPalette| -> Result<Option<ArtworkPalette>, EncoreError> {
            let Some(json) = &c.palette_json else {
                return Ok(None);
            };
            let palette: crate::palette::Palette =
                serde_json::from_str(json).map_err(EncoreError::internal)?;
            Ok(Some(palette.into()))
        };
        if let Some(c) = &cached
            && c.artwork_tag == tag
        {
            return read(c);
        }
        let fetched = match (&tag, self.source(p, source_id)) {
            // No cover: nothing to fetch.
            (None, _) => Ok(None),
            (Some(t), Ok(source)) => source
                .cover_png(&item, settings.sample_size_px, Some(t))
                .await
                .map_err(EncoreError::from),
            (Some(_), Err(e)) => Err(e),
        };
        let bytes = match (fetched, &cached) {
            (Ok(b), _) => b,
            // Offline or signed out: the last palette, even of an older cover.
            (Err(_), Some(c)) => return read(c),
            (Err(e), None) => return Err(e),
        };
        // A cover that can't be read counts as no cover: the profile accent.
        let palette =
            bytes.and_then(|b| crate::palette::palette_of_png(&b, settings).ok().flatten());
        let palette_json = match &palette {
            Some(pal) => Some(serde_json::to_string(pal).map_err(EncoreError::internal)?),
            None => None,
        };
        lock(&p.store)?.save_palette(
            kind,
            id,
            &CachedPalette {
                artwork_tag: tag,
                palette_json,
                made_utc_ms: now_ms(),
            },
        )?;
        Ok(palette.map(Into::into))
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Start the core. `config.data_dir` must be a folder the app can write.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: CoreConfig) -> Result<Arc<EncoreCore>, EncoreError> {
        Self::build(config, QUICK_CONNECT_POLL, tokens().sync_page_size, None)
    }
}

impl EncoreCore {
    /// For end-to-end tests outside this crate (the companion server's): a core whose
    /// every request goes through `transport`, polling Quick Connect without delay,
    /// with the recording's page size. Not part of the app API.
    #[doc(hidden)]
    pub fn with_transport_for_tests(
        config: CoreConfig,
        transport: Arc<dyn Transport>,
    ) -> Result<Arc<EncoreCore>, EncoreError> {
        Self::build(config, Duration::ZERO, 50, Some(transport))
    }

    pub(crate) fn build(
        config: CoreConfig,
        poll_every: Duration,
        sync_page_size: u32,
        transport_override: Option<Arc<dyn Transport>>,
    ) -> Result<Arc<EncoreCore>, EncoreError> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("encore-core")
            .enable_all()
            .build()
            .map_err(EncoreError::internal)?;
        std::fs::create_dir_all(&config.data_dir).map_err(|e| EncoreError::Storage {
            reason: format!("couldn't use the data folder ({e})"),
        })?;
        let registry =
            encore_store::Registry::open(&Path::new(&config.data_dir).join("device.sqlite"))?;
        std::fs::create_dir_all(Path::new(&config.data_dir).join("profiles")).map_err(|e| {
            EncoreError::Storage {
                reason: format!("couldn't use the data folder ({e})"),
            }
        })?;
        let core = Arc::new(EncoreCore {
            rt: Some(rt),
            inner: Arc::new(Inner {
                config,
                listener: Mutex::new(None),
                profile: Mutex::new(None),
                pins: Mutex::new(HashMap::new()),
                quick: tokio::sync::Mutex::new(None),
                cancel_quick: AtomicBool::new(false),
                poll_every,
                sync_page_size,
                transport_override,
                registry: Mutex::new(registry),
                unlocked: Mutex::new(Default::default()),
                connectivity: Mutex::new(Default::default()),
                transfers: Mutex::new(Default::default()),
                widgets: Default::default(),
            }),
        });
        // Phase 9, WP2: the debounced widget snapshot writer.
        widgets::start_writer(&core);
        Ok(core)
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Receive events (queue changed, now playing, preload, sync progress).
    pub fn set_listener(&self, listener: Arc<dyn CoreListener>) {
        if let Ok(mut l) = self.inner.listener.lock() {
            *l = Some(listener);
        }
    }

    /// Stop sending events and let go of the listener (in Dart this closes the
    /// events stream). Events after this are dropped until `set_listener`.
    pub fn clear_listener(&self) {
        if let Ok(mut l) = self.inner.listener.lock() {
            *l = None;
        }
    }

    // ------------------------------------------------------------ connect

    /// Check what the user typed (board 21, item 3). Async.
    pub async fn check_address(&self, address: String) -> Result<ServerCheck, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move { inner.check_address(address).await })
            .await
    }

    /// Trust this exact certificate for the server at `address` (after the user
    /// saw the fingerprint). Then check the address again.
    pub fn trust_certificate(&self, address: String, sha256: String) {
        if let Ok(mut pins) = self.inner.pins.lock() {
            pins.insert(host_of(&address), sha256.to_ascii_lowercase());
        }
    }

    /// Sign in with a username and password. The password is sent once and never
    /// kept. Opens the account's profile. Async.
    pub async fn sign_in(
        &self,
        base_url: String,
        username: String,
        password: String,
    ) -> Result<SignedIn, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let (client, info) = inner.client_for(&base_url).await?;
            let session = client.sign_in(&username, &password).await?;
            inner.complete_sign_in(client, info, session)
        })
        .await
    }

    /// Start Quick Connect: show the code, then call `quick_connect_wait`. Async.
    pub async fn quick_connect_start(
        &self,
        base_url: String,
    ) -> Result<QuickConnectStart, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let (client, info) = inner.client_for(&base_url).await?;
            if !client.quick_connect_enabled().await? {
                return Err(EncoreError::QuickConnectDisabled {
                    reason: "Quick Connect is turned off on this server".into(),
                });
            }
            let code = client.quick_connect_start().await?;
            inner.cancel_quick.store(false, Ordering::SeqCst);
            *inner.quick.lock().await = Some(PendingQuickConnect {
                client,
                info,
                secret: code.secret,
                started: Instant::now(),
            });
            Ok(QuickConnectStart {
                code: code.code,
                expires_in_seconds: tokens().quick_connect_expiry_seconds as u32,
            })
        })
        .await
    }

    /// Wait until the code is approved on another device, then sign in. Fails with
    /// QuickConnectExpired after the expiry, or QuickConnectCancelled. Async.
    pub async fn quick_connect_wait(&self) -> Result<SignedIn, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let expiry = Duration::from_secs(tokens().quick_connect_expiry_seconds);
            loop {
                if inner.cancel_quick.load(Ordering::SeqCst) {
                    *inner.quick.lock().await = None;
                    return Err(EncoreError::QuickConnectCancelled {
                        reason: "Quick Connect was cancelled".into(),
                    });
                }
                let approved = {
                    let guard = inner.quick.lock().await;
                    let pending =
                        guard
                            .as_ref()
                            .ok_or_else(|| EncoreError::QuickConnectExpired {
                                reason: "Start Quick Connect again".into(),
                            })?;
                    if pending.started.elapsed() > expiry {
                        return Err(EncoreError::QuickConnectExpired {
                            reason: "The code expired. Start again for a new one".into(),
                        });
                    }
                    pending.client.quick_connect_poll(&pending.secret).await?
                };
                if approved {
                    let pending = inner
                        .quick
                        .lock()
                        .await
                        .take()
                        .ok_or_else(|| EncoreError::internal("quick connect"))?;
                    let session = pending.client.quick_connect_finish(&pending.secret).await?;
                    return inner.complete_sign_in(pending.client, pending.info, session);
                }
                tokio::time::sleep(inner.poll_every).await;
            }
        })
        .await
    }

    /// Stop waiting for a Quick Connect approval (Cancel on board 21).
    pub fn quick_connect_cancel(&self) {
        self.inner.cancel_quick.store(true, Ordering::SeqCst);
    }

    // ------------------------------------------------------------ profiles

    /// The profiles on this device.
    pub fn profiles(&self) -> Result<Vec<ProfileSummary>, EncoreError> {
        let dir = self.inner.profiles_dir();
        let mut out = Vec::new();
        let mut ended = std::collections::HashSet::new();
        let entries = std::fs::read_dir(&dir).map_err(|e| EncoreError::Storage {
            reason: e.to_string(),
        })?;
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        ids.sort();
        for id in ids {
            let path = self.inner.store_path(&id);
            if !path.exists() {
                continue;
            }
            let store = Store::open(&path)?;
            let sources = store.sources()?;
            if store
                .setting(profiles::SESSION_ENDED_KEY)?
                .is_some_and(|v| v == "1")
            {
                ended.insert(id.clone());
            }
            out.push(ProfileSummary {
                initials: String::new(),
                avatar_url: None,
                locked: false,
                active: false,
                needs_sign_in: false,
                display_name: sources
                    .first()
                    .map(|s| s.user_name.clone())
                    .unwrap_or_else(|| id.clone()),
                credentials: sources
                    .iter()
                    .map(|s| CredentialKey {
                        source_id: s.id,
                        key: credential_key(&id, s.id),
                    })
                    // The companion server's token, when the profile has one.
                    .chain(
                        store
                            .setting(sync_loop::URL_KEY)?
                            .filter(|u| !u.is_empty())
                            .map(|_| CredentialKey {
                                source_id: 0,
                                key: credential_key(&id, 0),
                            }),
                    )
                    .collect(),
                profile_id: id,
            });
        }
        self.arrange_profiles(out, &ended)
    }

    /// Open a profile with the tokens the app read from its keychain. A play left
    /// open by a crash is closed as "ended by the system".
    pub fn open_profile(
        &self,
        profile_id: String,
        tokens: Vec<SourceToken>,
    ) -> Result<(), EncoreError> {
        // `source_id` 0 is the companion server's token; the rest are media servers'.
        let (companion, media): (Vec<SourceToken>, Vec<SourceToken>) =
            tokens.into_iter().partition(|t| t.source_id == 0);
        self.inner.open_profile(&profile_id, media)?;
        if let Some(t) = companion.into_iter().find(|t| !t.token.is_empty()) {
            self.resume_companion(t.token)?;
        }
        Ok(())
    }

    pub fn close_profile(&self) -> Result<(), EncoreError> {
        self.inner.close_profile()
    }

    /// The open profile's ID, if any.
    pub fn current_profile(&self) -> Option<String> {
        self.inner.profile().ok().map(|p| p.id.clone())
    }

    /// Write a consistent copy of a profile's database to `dest_path`, a file the
    /// app chose. Safe while the profile is open and playing. It holds the library
    /// mirror, history and settings; tokens are never in it (they stay in the
    /// keychain). Blocking: call off the main thread.
    pub fn backup_profile(&self, profile_id: String, dest_path: String) -> Result<(), EncoreError> {
        let path = self.inner.store_path(checked_profile_id(&profile_id)?);
        let dest = Path::new(&dest_path);
        if let Ok(p) = self.inner.profile()
            && p.id == profile_id
        {
            return Ok(lock(&p.store)?.backup_to(dest)?);
        }
        if !path.exists() {
            return Err(EncoreError::NotFound {
                reason: "That profile isn't on this device".into(),
            });
        }
        Ok(Store::open(&path)?.backup_to(dest)?)
    }

    /// Export everything the profile holds as one open JSON file (board 19, item
    /// 5): its play log, likes, playlists, settings and recent searches. The format
    /// is in docs/core/EXPORT-FORMAT.md; tokens are never in it. It reads through its
    /// own connection, so playback and browsing carry on while it runs. Refused in
    /// Guest mode ("Nothing is saved in Guest mode"). Blocking: call off the main
    /// thread, with the board's progress row showing until it returns.
    pub fn export_profile(
        &self,
        profile_id: String,
        dest_path: String,
    ) -> Result<ExportSummary, EncoreError> {
        let path = self.inner.store_path(checked_profile_id(&profile_id)?);
        if let Ok(p) = self.inner.profile()
            && p.id == profile_id
            && lock(&p.playback)?.log.mode() == crate::playlog::ListeningMode::Guest
        {
            return Err(EncoreError::NotAllowed {
                reason: "Nothing is saved in Guest mode".into(),
            });
        }
        if !path.exists() {
            return Err(EncoreError::NotFound {
                reason: "That profile isn't on this device".into(),
            });
        }
        let store = Store::open(&path)?;
        let name = store.sources()?.first().map(|s| s.user_name.clone());
        let counts = store.export_to(
            Path::new(&dest_path),
            &encore_store::ExportHeader {
                profile_id: &profile_id,
                profile_name: name.as_deref(),
                app_version: env!("CARGO_PKG_VERSION"),
                exported_utc_ms: now_ms(),
            },
        )?;
        Ok(ExportSummary {
            plays: counts.plays,
            likes: counts.likes,
            playlists: counts.playlists,
            playlist_songs: counts.playlist_songs,
            bytes: counts.bytes,
        })
    }

    /// Connect this profile to a companion server (plan section 5), at the address
    /// the user typed: https anywhere, plain http only on the home network. The
    /// profile's media-server token is sent once so the companion can check it;
    /// the companion gives back its own tokens. A `CredentialChanged` event hands
    /// the app the token to keep in the keychain (`source_id` 0). Sync starts at
    /// once and runs in the background. Async.
    pub async fn connect_companion(&self, url: String) -> Result<SyncStatus, EncoreError> {
        let base = crate::companion::companion_url(&url)
            .map_err(|reason| EncoreError::InvalidAddress { reason })?;
        let handle = self
            .rt
            .as_ref()
            .ok_or_else(|| EncoreError::internal("runtime stopped"))?
            .handle()
            .clone();
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            if lock(&p.playback)?.log.mode() == crate::playlog::ListeningMode::Guest {
                return Err(EncoreError::NotAllowed {
                    reason: "Nothing syncs in Guest mode".into(),
                });
            }
            let (server_id, media_token) = {
                let records = lock(&p.store)?.sources()?;
                let sources = lock(&p.sources)?;
                records
                    .iter()
                    .find_map(|r| {
                        let t = sources.get(&r.id)?.client.token_for_companion_sign_in()?;
                        Some((r.server_id.clone(), t.to_owned()))
                    })
                    .ok_or_else(|| EncoreError::SessionExpired {
                        reason: "Sign in to your media server first".into(),
                    })?
            };
            let client = crate::companion::Client::new(base.clone(), inner.transport_for(None)?);
            client.health().await.map_err(sync_loop::into_error)?;
            let (device, tokens) = client
                .exchange(
                    &server_id,
                    &media_token,
                    &inner.config.device_id,
                    &inner.config.device_name,
                    &inner.config.platform,
                )
                .await
                .map_err(sync_loop::into_error)?;
            {
                let st = lock(&p.store)?;
                st.set_setting(sync_loop::URL_KEY, base.as_str())?;
                st.set_setting(sync_loop::DEVICE_KEY, &device)?;
            }
            sync_loop::credential(&inner, tokens.refresh.clone());
            if let Some(old) = lock(&p.companion)?.take() {
                old.stop();
            }
            let s = sync_loop::Session::new(client, device, tokens, sync_loop::last_synced(&p));
            *lock(&p.companion)? = Some(s.clone());
            sync_loop::start(&handle, inner.clone(), p.clone(), s.clone());
            Ok(s.status())
        })
        .await
    }

    /// Stop syncing this profile with its companion server. This device is signed
    /// out there when it can be reached; everything stays on this device. A
    /// `CredentialChanged` event with an empty token says to delete the keychain
    /// entry. Async.
    pub async fn disconnect_companion(&self) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let session = lock(&p.companion)?.take();
            if let Some(s) = session {
                s.stop();
                let own = s.server_device_id.clone();
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    sync_loop::signed(&inner, &s, |a| {
                        let (s, own) = (&s, &own);
                        async move { s.client.revoke(&a, own).await }
                    }),
                )
                .await;
            }
            {
                let st = lock(&p.store)?;
                st.set_setting(sync_loop::URL_KEY, "")?;
                st.set_setting(sync_loop::DEVICE_KEY, "")?;
                st.set_sync_cursor(0)?;
            }
            sync_loop::credential(&inner, String::new());
            Ok(())
        })
        .await
    }

    /// Board 19's "Delete profile and its data", for the open profile: its
    /// history, likes, playlists made here, settings and tokens, on the companion
    /// server (every device signed in to it stops syncing) and on this device. The
    /// media-server account and its music are never touched. `typed_name` must be
    /// the profile's name as `profiles()` shows it. If the companion server can't be
    /// reached, nothing is deleted and the error says so: try again when it's back.
    /// After this, delete every keychain entry of the profile. Async.
    pub async fn delete_profile_everywhere(
        &self,
        profile_id: String,
        typed_name: String,
    ) -> Result<(), EncoreError> {
        checked_profile_id(&profile_id)?;
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            if p.id != profile_id {
                return Err(EncoreError::NotAllowed {
                    reason: "Open the profile first, then delete it".into(),
                });
            }
            let name = lock(&p.store)?
                .sources()?
                .first()
                .map(|s| s.user_name.clone())
                .unwrap_or_else(|| profile_id.clone());
            if typed_name.trim() != name {
                return Err(EncoreError::NotAllowed {
                    reason: format!("Type the profile's name, {name}, to delete it"),
                });
            }
            let session = lock(&p.companion)?.clone();
            if let Some(s) = session {
                match sync_loop::signed(&inner, &s, |a| {
                    let s = &s;
                    async move { s.client.delete_profile(&a).await }
                })
                .await
                {
                    // Signed out there already: nothing of this device left to delete.
                    Ok(()) | Err(crate::companion::CompanionError::SignedOut(_)) => {}
                    Err(e) => return Err(sync_loop::into_error(e)),
                }
            }
            drop(p);
            let old = inner.take_profile()?;
            if let Some(p) = old {
                for _ in 0..100 {
                    if Arc::strong_count(&p) == 1 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
            let dir = inner.profiles_dir().join(&profile_id);
            std::fs::remove_dir_all(&dir).map_err(|e| EncoreError::Storage {
                reason: format!("couldn't delete this device's copy ({e})"),
            })?;
            // Files only this profile held go too.
            let _ = inner.collect_download_garbage();
            // Phase 9, WP2: and its widget snapshot.
            inner.remove_widgets(&profile_id);
            sync_loop::credential(&inner, String::new());
            Ok(())
        })
        .await
    }
    /// Where sync stands, for the sync status row.
    pub fn sync_status(&self) -> Result<SyncStatus, EncoreError> {
        let p = self.inner.profile()?;
        let pending = lock(&p.store)?.pending_out_count(&self.inner.config.device_id)?;
        if let Some(s) = lock(&p.companion)?.as_ref() {
            let mut st = s.status();
            st.pending = pending;
            return Ok(st);
        }
        let url = lock(&p.store)?
            .setting(sync_loop::URL_KEY)?
            .filter(|u| !u.is_empty());
        Ok(SyncStatus {
            state: SyncState::NotConnected,
            pending,
            last_synced_utc_ms: sync_loop::last_synced(&p),
            server_url: url.map(|u| u.trim_end_matches('/').to_owned()),
            reason: None,
        })
    }

    /// Send and receive now, without waiting (pull to refresh). Async.
    pub async fn sync_now(&self) -> Result<SyncStatus, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let s = lock(&p.companion)?
                .clone()
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "This profile isn't connected to a companion server".into(),
                })?;
            sync_loop::push_all(&inner, &p, &s)
                .await
                .map_err(sync_loop::into_error)?;
            while sync_loop::pull_once(&inner, &p, &s, 0)
                .await
                .map_err(sync_loop::into_error)?
            {}
            Ok(s.status())
        })
        .await
    }

    /// The profile's devices on the companion server (the Devices list). Async.
    pub async fn companion_devices(&self) -> Result<Vec<CompanionDevice>, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let s = lock(&p.companion)?
                .clone()
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "This profile isn't connected to a companion server".into(),
                })?;
            let (this, list) = sync_loop::signed(&inner, &s, |a| {
                let s = &s;
                async move { s.client.devices(&a).await }
            })
            .await
            .map_err(sync_loop::into_error)?;
            Ok(list
                .into_iter()
                .map(|d| CompanionDevice {
                    this_device: d.id == this,
                    device_id: d.id,
                    name: d.name,
                    platform: d.platform,
                    added_utc_ms: d.created_utc_ms,
                    last_seen_utc_ms: d.last_seen_utc_ms,
                    signed_out: d.revoked,
                })
                .collect())
        })
        .await
    }

    /// Sign one of the profile's devices out everywhere (a lost phone, plan section
    /// 12). Async.
    pub async fn revoke_device(&self, device_id: String) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let s = lock(&p.companion)?
                .clone()
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "This profile isn't connected to a companion server".into(),
                })?;
            sync_loop::signed(&inner, &s, |a| {
                let (s, id) = (&s, &device_id);
                async move { s.client.revoke(&a, id).await }
            })
            .await
            .map_err(sync_loop::into_error)
        })
        .await
    }
    /// Replace a profile's database with a backup made by `backup_profile`. The
    /// backup is checked first (intact, an Encore profile, not from a newer app); if
    /// it fails, nothing changes. The replaced database is kept beside it as
    /// `library.sqlite.before-restore`. The profile is closed if it was open; open it
    /// again with its tokens. Blocking: call off the main thread.
    pub fn restore_profile(
        &self,
        profile_id: String,
        from_path: String,
    ) -> Result<(), EncoreError> {
        let id = checked_profile_id(&profile_id)?;
        let from = Path::new(&from_path);
        Store::check_backup(from)?;
        if self.current_profile().as_deref() == Some(id)
            && let Some(p) = self.inner.take_profile()?
        {
            // The database file must really be closed before it is replaced
            // (Windows refuses to replace an open file): wait for any background
            // send still holding the profile, briefly.
            for _ in 0..100 {
                if Arc::strong_count(&p) == 1 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            drop(p);
        }
        let path = self.inner.store_path(id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| EncoreError::Storage {
                reason: format!("couldn't use the data folder ({e})"),
            })?;
        }
        drop(Store::restore(&path, from)?);
        Ok(())
    }

    // ------------------------------------------------------------ servers and sync

    /// The rows of board 18.
    pub fn servers(&self) -> Result<Vec<ServerRow>, EncoreError> {
        let p = self.inner.profile()?;
        let signed: Vec<i64> = lock(&p.sources)?.keys().copied().collect();
        let store = lock(&p.store)?;
        let mut rows = Vec::new();
        for s in store.sources()? {
            rows.push(ServerRow {
                song_count: store.track_count(s.id)?,
                signed_in: signed.contains(&s.id),
                source_id: s.id,
                kind: s.kind,
                name: s.name,
                base_url: s.base_url,
                version: s.server_version,
                user_name: s.user_name,
                last_sync_utc_ms: s.last_sync_utc_ms,
            });
        }
        Ok(rows)
    }

    /// Sync every signed-in source of the open profile: a full sync the first time,
    /// a delta after. Progress arrives as SyncProgress events. Async.
    pub async fn sync_library(&self) -> Result<Vec<SyncResult>, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let sources: Vec<(i64, Arc<JellyfinSource>)> = lock(&p.sources)?
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            let mut results = Vec::new();
            for (id, source) in sources {
                let progress_inner = inner.clone();
                let progress = move |s: jellyfin::SyncProgress| {
                    let mut e = CoreEvent::of(EventKind::SyncProgress);
                    e.sync_source_id = Some(id);
                    e.sync_done = s.done as i64;
                    e.sync_total = s.total as i64;
                    progress_inner.emit(e);
                };
                // Edits made here go first, so the pull can't put back what they
                // changed. If the server can't be reached they keep waiting.
                let _ = edits::flush(&p.store, id, source.as_ref(), now_ms()).await;
                let stats: Result<_, EncoreError> = crate::library::sync_source(
                    &p.store,
                    id,
                    source.as_ref(),
                    inner.sync_page_size,
                    now_ms(),
                    &progress,
                )
                .await
                .map_err(Into::into);
                // The media server ended the sign-in: the switcher says "Sign in again".
                if let Err(EncoreError::SessionExpired { .. }) = &stats {
                    lock(&p.store)?.set_setting(profiles::SESSION_ENDED_KEY, "1")?;
                }
                let stats = stats?;
                // The user's image, for the switcher's avatar (best effort).
                if let Ok(me) = source.client.me().await {
                    let avatar = me
                        .image_tag
                        .as_deref()
                        .map(|tag| (source.client.base_url().as_str(), me.user_id.as_str(), tag));
                    lock(&inner.registry)?.set_avatar(&p.id, avatar)?;
                }
                {
                    // Favourites set or cleared elsewhere become likes here.
                    let store = lock(&p.store)?;
                    let pending = edits::pending_favorite_keys(&store, id)?;
                    store.reconcile_likes(id, &pending, now_ms())?;
                }
                results.push(SyncResult {
                    source_id: id,
                    artists: stats.artists as u32,
                    albums: stats.albums as u32,
                    songs: stats.tracks as u32,
                    playlists: stats.playlists as u32,
                    removed: stats.removed as u32,
                });
            }
            // The companion server builds the mixes from this song list.
            let session = lock(&p.companion)?.clone();
            if let Some(s) = session {
                let _ = recs::upload_catalog(&inner, &p, &s).await;
            }
            // Downloaded albums and playlists follow the library (Phase 8).
            let _ = inner.reconcile_downloads(&p, None);
            // Phase 9, WP2: titles, covers and the server's reach may have changed.
            inner.widgets_soon();
            Ok(results)
        })
        .await
    }

    /// Sign out of a server: its token is revoked there and dropped here. The
    /// mirror stays, so songs stay listed until the user removes the server. Async.
    pub async fn sign_out(&self, source_id: i64) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let source = lock(&p.sources)?.remove(&source_id);
            // Phase 9, WP2: the widgets' "Server not reachable" follows.
            inner.widgets_soon();
            if let Some(s) = source {
                s.client.sign_out().await?;
            }
            Ok(())
        })
        .await
    }

    /// Remove a server: its local index goes, nothing on the server is touched
    /// (board 18, item 2). History stays.
    pub fn remove_server(&self, source_id: i64) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        lock(&p.sources)?.remove(&source_id);
        lock(&p.store)?.remove_source(source_id)?;
        // Its downloads go with its index (board 18, item 2).
        self.inner.reconcile_downloads(&p, None)?;
        // Phase 9, WP2: its songs and covers leave the widgets.
        self.inner.widgets_soon();
        Ok(())
    }

    // ------------------------------------------------------------ browse (synchronous, offline)

    pub fn albums(
        &self,
        sort: AlbumSort,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<AlbumSummary>, EncoreError> {
        let p = self.inner.profile()?;
        let sort = match sort {
            AlbumSort::Title => encore_store::AlbumSort::Title,
            AlbumSort::Artist => encore_store::AlbumSort::Artist,
            AlbumSort::Year => encore_store::AlbumSort::Year,
            AlbumSort::RecentlyAdded => encore_store::AlbumSort::RecentlyAdded,
        };
        Ok(lock(&p.store)?
            .albums(sort, Page { offset, limit })?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn album(&self, album_id: i64) -> Result<AlbumDetail, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        Ok(AlbumDetail {
            album: store.album(album_id)?.into(),
            songs: store
                .album_tracks(album_id)?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub fn songs(&self, sort: SongSort, offset: u32, limit: u32) -> Result<Vec<Song>, EncoreError> {
        let p = self.inner.profile()?;
        let sort = match sort {
            SongSort::Title => encore_store::TrackSort::Title,
            SongSort::Artist => encore_store::TrackSort::Artist,
            SongSort::Album => encore_store::TrackSort::Album,
            SongSort::RecentlyAdded => encore_store::TrackSort::RecentlyAdded,
        };
        Ok(lock(&p.store)?
            .tracks(sort, Page { offset, limit })?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn artists(&self, offset: u32, limit: u32) -> Result<Vec<ArtistSummary>, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?
            .artists(Page { offset, limit })?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn artist(&self, artist_id: i64) -> Result<ArtistDetail, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        Ok(ArtistDetail {
            artist: store.artist(artist_id)?.into(),
            albums: store
                .artist_albums(artist_id)?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub fn artist_songs(
        &self,
        artist_id: i64,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Song>, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?
            .artist_tracks(artist_id, Page { offset, limit })?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn playlists(&self, offset: u32, limit: u32) -> Result<Vec<PlaylistSummary>, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?
            .playlists(Page { offset, limit })?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn playlist(&self, playlist_id: i64) -> Result<PlaylistDetail, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        Ok(PlaylistDetail {
            playlist: store.playlist(playlist_id)?.into(),
            entries: store
                .playlist_entries(playlist_id)?
                .into_iter()
                .map(|e| PlaylistEntry {
                    position: e.position,
                    song: e.track.map(Into::into),
                })
                .collect(),
        })
    }

    pub fn genres(&self) -> Result<Vec<GenreSummary>, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?
            .genres()?
            .into_iter()
            .map(|g| GenreSummary {
                name: g.name,
                song_count: g.track_count,
            })
            .collect())
    }

    pub fn genre_songs(
        &self,
        genre: String,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Song>, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?
            .genre_tracks(&genre, Page { offset, limit })?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn song(&self, song_id: i64) -> Result<Song, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?.track(song_id)?.into())
    }

    // ------------------------------------------------------------ search

    /// Search the on-device mirror as the user types (board 07). Synchronous.
    pub fn search(&self, query: String) -> Result<SearchResults, EncoreError> {
        let p = self.inner.profile()?;
        let t = tokens();
        let limits = SearchLimits {
            songs: t.search_songs,
            albums: t.search_albums,
            artists: t.search_artists,
            playlists: t.search_playlists,
            lyrics: t.search_lyrics,
        };
        let r = lock(&p.store)?.search(&query, &limits, t.typo_max_edits)?;
        let top = r.top.map(|top| match top {
            StoreTop::Song(s) => TopResult {
                kind: ResultKind::Song,
                song: Some(s.into()),
                album: None,
                artist: None,
                playlist: None,
            },
            StoreTop::Album(a) => TopResult {
                kind: ResultKind::Album,
                song: None,
                album: Some(a.into()),
                artist: None,
                playlist: None,
            },
            StoreTop::Artist(a) => TopResult {
                kind: ResultKind::Artist,
                song: None,
                album: None,
                artist: Some(a.into()),
                playlist: None,
            },
            StoreTop::Playlist(pl) => TopResult {
                kind: ResultKind::Playlist,
                song: None,
                album: None,
                artist: None,
                playlist: Some(pl.into()),
            },
        });
        Ok(SearchResults {
            top,
            songs: r.songs.into_iter().map(Into::into).collect(),
            albums: r.albums.into_iter().map(Into::into).collect(),
            artists: r.artists.into_iter().map(Into::into).collect(),
            playlists: r.playlists.into_iter().map(Into::into).collect(),
            lyrics: r
                .lyrics
                .into_iter()
                .map(|h| LyricMatch {
                    song: h.track.into(),
                    line: h.text,
                    line_index: h.line_index,
                    start_ms: h.start_ms,
                })
                .collect(),
        })
    }

    /// Save a query (the return key). Recent searches stay on this device.
    pub fn add_recent_search(&self, query: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?.add_recent_search(&query, now_ms(), tokens().recent_searches_max)?)
    }

    pub fn recent_searches(&self) -> Result<Vec<String>, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?.recent_searches()?)
    }

    pub fn remove_recent_search(&self, query: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?.remove_recent_search(&query)?)
    }

    pub fn clear_recent_searches(&self) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?.clear_recent_searches()?)
    }

    // ------------------------------------------------------------ queue

    pub fn queue(&self) -> Result<QueueState, EncoreError> {
        let p = self.inner.profile()?;
        let pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        let mut probe = pb.queue.clone();
        Ok(Inner::queue_state(&store, &pb, probe.undo()))
    }

    /// Play a list (an album, a playlist, search results) from `start_index`.
    pub fn play(
        &self,
        song_ids: Vec<i64>,
        start_index: u32,
        started_from: StartedFrom,
        context_id: Option<String>,
    ) -> Result<QueueState, EncoreError> {
        self.inner.guest_tick(true)?;
        // Offline (Phase 8): only what can play joins the queue; a tap on a song
        // that can't is refused with the reason.
        let (song_ids, start_index) = match self.inner.profile() {
            Ok(p) => self
                .inner
                .offline_list(&p, &song_ids, start_index as usize)?,
            Err(_) => (song_ids, start_index),
        };
        // Anything played by hand ends a radio.
        if let Ok(p) = self.inner.profile() {
            lock(&p.playback)?.radio_seed = None;
        }
        let items = Inner::new_items(&song_ids, started_from, context_id);
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.play(items, start_index as usize))?
            .1)
    }

    pub fn play_next(&self, song_ids: Vec<i64>) -> Result<QueueState, EncoreError> {
        let song_ids = self
            .inner
            .offline_songs(&*self.inner.profile()?, &song_ids)?;
        let items = Inner::new_items(&song_ids, StartedFrom::Queue, None);
        Ok(self.inner.with_queue(|pb, _| pb.queue.play_next(items))?.1)
    }

    pub fn add_to_up_next(&self, song_ids: Vec<i64>) -> Result<QueueState, EncoreError> {
        let song_ids = self
            .inner
            .offline_songs(&*self.inner.profile()?, &song_ids)?;
        let items = Inner::new_items(&song_ids, StartedFrom::Queue, None);
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.add_to_up_next(items))?
            .1)
    }

    /// Next from the button. The same as `skip_next(SkipOrigin::Button)`.
    pub fn next(&self) -> Result<QueueState, EncoreError> {
        self.skip_next(SkipOrigin::Button)
    }

    /// Next from anywhere: the button, a swipe, a headset, a car, the lock screen
    /// or a key. Every origin runs this same code and teaches the same lesson
    /// (board 42, rule 15); the origin is stored on the play it ends. At the end of
    /// the queue the current song stays (`next_target` says so beforehand).
    pub fn skip_next(&self, origin: SkipOrigin) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| {
                if pb.queue.peek_skip().is_some() {
                    pb.pending_origin = Some(origin.into());
                }
                pb.queue.advance(Advance::Skip);
            })?
            .1)
    }

    /// Previous from the button (`from_swipe` false) or a swipe (true). The same
    /// as `skip_previous` with that origin.
    pub fn previous(
        &self,
        position_ms: i64,
        from_swipe: bool,
    ) -> Result<PreviousResult, EncoreError> {
        let origin = if from_swipe {
            SkipOrigin::Swipe
        } else {
            SkipOrigin::Button
        };
        self.skip_previous(origin, position_ms)
    }

    /// Previous from anywhere. `position_ms` is where the current song is. A swipe
    /// always goes back ("back means back"); every other origin restarts the song
    /// after `previousButtonRestartAfterSeconds` (3 s). At the first song it
    /// restarts, or wraps on Repeat All.
    pub fn skip_previous(
        &self,
        origin: SkipOrigin,
        position_ms: i64,
    ) -> Result<PreviousResult, EncoreError> {
        let restart_after = tokens().previous_restart_after_ms;
        let always_back = origin == SkipOrigin::Swipe;
        let (outcome, _) = self.inner.with_queue(|pb, _| {
            let outcome = pb
                .queue
                .previous(position_ms.max(0) as u64, restart_after, always_back);
            if outcome == PreviousOutcome::Moved {
                pb.pending_origin = Some(origin.into());
            }
            outcome
        })?;
        Ok(match outcome {
            PreviousOutcome::Restart => PreviousResult::Restarted,
            PreviousOutcome::Moved => PreviousResult::Moved,
        })
    }

    /// What a swipe forward would show and land on, so the UI can draw the
    /// neighbour or the "End of queue" rubber band (board 42, rule 8).
    pub fn next_target(&self) -> Result<SkipTarget, EncoreError> {
        let p = self.inner.profile()?;
        let pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        let song = pb.queue.peek_skip().and_then(|i| Inner::entry(&store, i));
        Ok(SkipTarget {
            end_of_queue: song.is_none(),
            song,
        })
    }

    /// What a swipe back would show and land on (with Repeat All at the first
    /// song: the last one).
    pub fn previous_target(&self) -> Result<SkipTarget, EncoreError> {
        let p = self.inner.profile()?;
        let pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        let song = pb
            .queue
            .peek_previous()
            .and_then(|i| Inner::entry(&store, i));
        Ok(SkipTarget {
            end_of_queue: song.is_none(),
            song,
        })
    }

    /// Headphones unplugged, the output switched, a call or an assistant. A skip in
    /// the next `interruptionWindowSeconds` (3 s) is not a skip (section 21).
    pub fn playback_interrupted(&self, kind: InterruptionKind) -> Result<(), EncoreError> {
        let _ = kind; // every kind counts the same today; kept for later rules
        let p = self.inner.profile()?;
        lock(&p.playback)?.last_interruption_utc_ms = Some(now_ms());
        Ok(())
    }

    /// The listener dragged the progress bar. Counted on the play; never a skip.
    /// Seeking back before the preload point lets Preload come again.
    pub fn playback_seeked(&self, from_ms: i64, to_ms: i64) -> Result<(), EncoreError> {
        let _ = from_ms;
        self.inner.note_playback(None, Some(to_ms));
        let p = self.inner.profile()?;
        let mut pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        pb.log.seek(&store)?;
        if let Some(uid) = pb.playing_uid
            && let Some(current) = pb.queue.current().filter(|i| i.uid == uid)
        {
            let duration = store.track(current.track_id)?.duration_ms as i64;
            if duration - to_ms > tokens().preload_before_end_ms as i64 {
                pb.preloaded_for = None;
            }
        }
        Ok(())
    }

    pub fn jump_to(&self, uid: i64) -> Result<QueueState, EncoreError> {
        self.check_uid_playable(uid)?;
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.jump_to(uid as u64))?
            .1)
    }

    pub fn replay_from_history(&self, uid: i64) -> Result<QueueState, EncoreError> {
        self.check_uid_playable(uid)?;
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.replay_from_history(uid as u64))?
            .1)
    }

    pub fn remove_from_queue(&self, uid: i64) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.remove(uid as u64))?
            .1)
    }

    pub fn move_in_queue(&self, uid: i64, to_index: u32) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.move_item(uid as u64, to_index as usize))?
            .1)
    }

    /// Empty Up Next; Autoplay stays (board 11).
    pub fn clear_up_next(&self) -> Result<QueueState, EncoreError> {
        Ok(self.inner.with_queue(|pb, _| pb.queue.clear_up_next())?.1)
    }

    /// Undo the last queue change (the Undo toast).
    pub fn undo_queue(&self) -> Result<QueueState, EncoreError> {
        Ok(self.inner.with_queue(|pb, _| pb.queue.undo())?.1)
    }

    pub fn set_shuffle(&self, on: bool) -> Result<QueueState, EncoreError> {
        Ok(self.inner.with_queue(|pb, _| pb.queue.set_shuffle(on))?.1)
    }

    pub fn set_repeat(&self, mode: RepeatMode) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.set_repeat(mode.into()))?
            .1)
    }

    /// The queue's Autoplay switch (board 11). Off: the queue ends with its last
    /// song, and the Autoplay section empties.
    pub fn set_autoplay(&self, enabled: bool) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| {
                pb.queue.set_autoplay_enabled(enabled);
                if !enabled {
                    pb.queue.set_autoplay(Vec::new());
                }
            })?
            .1)
    }

    // ------------------------------------------------------------ playback reports

    /// The player started the current song (`uid` from the queue). Writes the play
    /// event at once (plan section 8). `utc_offset_minutes` is the device's offset
    /// right now (east positive).
    pub fn playback_started(&self, uid: i64, utc_offset_minutes: i32) -> Result<(), EncoreError> {
        self.inner.guest_tick(true)?;
        // Phase 9, WP1: a sleep timer that ran out while paused closes that play
        // as ended by the system first, so this start can't make it a skip.
        self.inner.sleep_before_start();
        let p = self.inner.profile()?;
        let mut pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        // Private listening ends after 2 hours of silence (board 38): this play is
        // recorded again if it started after that.
        let now = now_ms();
        if let Some(last) = pb.private_last_activity_utc_ms {
            if now - last > tokens().private_ends_after_ms {
                let listened = pb.last_listened_ms;
                pb.log
                    .set_mode(&store, crate::playlog::ListeningMode::Normal, listened, now)?;
                pb.private_last_activity_utc_ms = None;
            } else {
                pb.private_last_activity_utc_ms = Some(now);
            }
        }
        let item = pb
            .queue
            .current()
            .filter(|i| i.uid == uid as u64)
            .cloned()
            .ok_or_else(|| EncoreError::NotFound {
                reason: "That song isn't the current one in the queue".into(),
            })?;
        let track = store.track(item.track_id)?;
        let source = store.source(track.source_id)?;
        let start = PlayStart {
            song_key: track.song_key.clone(),
            source_kind: source.kind,
            source_item_id: track.source_item_id.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album_title.clone(),
            duration_ms: track.duration_ms,
            started_from: item.started_from,
            context_id: item.context_id.clone(),
            chosen_by: item.chosen_by,
            started_utc_ms: now_ms(),
            utc_offset_minutes,
            carried_listened_ms: 0,
        };
        let mut start = start;
        if let Some((carry_uid, carried)) = pb.handoff_carry.take()
            && carry_uid == item.uid
        {
            start.carried_listened_ms = carried;
        }
        let previous = pb.last_listened_ms;
        let closing = pb.closing();
        pb.log.start(&store, &start, previous, &closing)?;
        pb.playing_uid = Some(item.uid);
        pb.last_listened_ms = 0;
        pb.preloaded_for = None;
        pb.utc_offset_minutes = utc_offset_minutes;
        // Let go of both locks before the background refill takes them.
        drop(store);
        drop(pb);
        self.inner.note_playback(Some(true), Some(0));
        // Keep Autoplay filled, in the background (the next-song decision reads it).
        if let Some(rt) = &self.rt {
            let inner = self.inner.clone();
            rt.spawn(async move {
                let _ = tokio::task::block_in_place(|| inner.refill_autoplay(false));
            });
        }
        Ok(())
    }

    /// Progress from the player, about once a second. `listened_ms` counts only
    /// time actually played (not seeks). Emits Preload 20 s before the end.
    pub fn playback_progress(&self, position_ms: i64, listened_ms: i64) -> Result<(), EncoreError> {
        self.inner.guest_tick(true)?;
        let p = self.inner.profile()?;
        let mut drifted = false;
        let preload = {
            let mut pb = lock(&p.playback)?;
            let store = lock(&p.store)?;
            pb.last_listened_ms = listened_ms.max(0) as u64;
            let listened = pb.last_listened_ms;
            pb.log.progress(&store, listened, now_ms())?;
            if let Ok(mut live) = lock(&p.live) {
                drifted = live.note_position(position_ms);
            }
            let Some(uid) = pb.playing_uid else {
                return Ok(());
            };
            let Some(current) = pb.queue.current().filter(|i| i.uid == uid).cloned() else {
                return Ok(());
            };
            let duration = store.track(current.track_id)?.duration_ms as i64;
            let near_end =
                duration > 0 && duration - position_ms <= tokens().preload_before_end_ms as i64;
            if near_end && pb.preloaded_for != Some(uid) {
                pb.preloaded_for = Some(uid);
                pb.queue
                    .peek_after_end()
                    .and_then(|next| Inner::entry(&store, next))
            } else {
                None
            }
        };
        if drifted {
            self.inner.publish_presence();
        }
        if let Some(next) = preload {
            let mut e = CoreEvent::of(EventKind::Preload);
            e.preload = Some(next);
            self.inner.emit(e);
        }
        Ok(())
    }

    /// The current song stopped. Finished and Error move the queue on (Repeat One
    /// replays a finished song); the others leave it. Returns the new current song.
    pub fn playback_ended(
        &self,
        reason: EndReason,
        listened_ms: i64,
    ) -> Result<Option<QueueEntry>, EncoreError> {
        {
            let p = self.inner.profile()?;
            let mut pb = lock(&p.playback)?;
            let store = lock(&p.store)?;
            if matches!(reason, EndReason::Finished | EndReason::Error) {
                // The app moves on by itself.
                pb.pending_origin = Some(Moved::Auto);
            }
            let closing = pb.closing();
            pb.log.close(
                &store,
                listened_ms.max(0) as u64,
                reason.into(),
                now_ms(),
                &closing,
            )?;
            pb.playing_uid = None;
            pb.last_listened_ms = 0;
        }
        self.inner.note_playback(Some(false), None);
        let advance = match reason {
            EndReason::Finished => Some(Advance::Ended),
            EndReason::Error => Some(Advance::Skip),
            _ => None,
        };
        let state = match advance {
            Some(how) => {
                self.inner
                    .with_queue(|pb, _| {
                        pb.queue.advance(how);
                    })?
                    .1
            }
            None => self.queue()?,
        };
        Ok(state.current)
    }

    /// A lyric line was tapped (plan section 5: lyric-line jumps are logged).
    pub fn lyric_line_tapped(
        &self,
        line_index: u32,
        line_start_ms: i64,
    ) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let mut pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        pb.log
            .lyric_jump(&store, line_index, line_start_ms.max(0) as u64, now_ms())?;
        Ok(())
    }

    /// Private listening, Guest and Learn-from-my-listening-off record nothing.
    pub fn set_listening_mode(&self, mode: ListeningMode) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let mut pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        let listened = pb.last_listened_ms;
        pb.log.set_mode(&store, mode.into(), listened, now_ms())?;
        pb.private_last_activity_utc_ms = (mode == ListeningMode::Private).then(now_ms);
        drop(store);
        drop(pb);
        // Phase 9, WP2: Guest empties the widgets at once; Private and Learning
        // off stop updating now playing and last played.
        if mode == ListeningMode::Guest {
            self.inner.widgets_now();
        } else {
            self.inner.widgets_soon();
        }
        Ok(())
    }

    // ------------------------------------------------------------ playlists and likes

    /// What dropping (or sending) these songs on `target` would do, before it is
    /// done (board 43): the count, how many are already there (by song, so a song
    /// on two albums counts once), whether to ask first, the name a new playlist
    /// would get, or why it's refused. Synchronous.
    pub fn drop_preview(
        &self,
        target: DropTarget,
        song_ids: Vec<i64>,
    ) -> Result<DropPreview, EncoreError> {
        let p = self.inner.profile()?;
        let guest = lock(&p.playback)?.log.mode() == crate::playlog::ListeningMode::Guest;
        let store = lock(&p.store)?;
        let songs = songs_of(&store, &song_ids)?;
        let preview = edits::preview(
            &store,
            target_of(&target)?,
            &songs,
            guest,
            tokens().confirm_over_songs,
        )?;
        Ok(DropPreview {
            count: preview.count,
            already_there: preview.already_there,
            needs_confirm: preview.needs_confirm,
            suggested_name: preview.suggested_name,
            refusal: preview.refusal.map(Into::into),
            refusal_reason: preview.refusal.map(|r| r.sentence().to_owned()),
        })
    }

    /// Add songs to a playlist, a new playlist, Liked Songs or the queue: **one
    /// call** with the whole list in the order shown, one event, one undo entry,
    /// however many songs (section 26). `position` is the line in a playlist or in
    /// Up Next (None: the end). Duplicates (by song) are skipped unless
    /// `allow_duplicates` ("Add anyway"). Works offline: the change shows at once
    /// and is sent when the server can be reached. Synchronous; sending happens in
    /// the background.
    pub fn add_to(
        &self,
        target: DropTarget,
        song_ids: Vec<i64>,
        position: Option<u32>,
        allow_duplicates: bool,
    ) -> Result<EditResult, EncoreError> {
        let p = self.inner.profile()?;
        let target_kind = target_of(&target)?;
        let guest = lock(&p.playback)?.log.mode() == crate::playlog::ListeningMode::Guest;
        if guest
            && matches!(
                target_kind,
                Target::Playlist(_) | Target::NewPlaylist | Target::LikedSongs
            )
        {
            return Err(edits::EditError::Refused(edits::Refusal::GuestCannotSave).into());
        }
        if matches!(target_kind, Target::UpNext | Target::PlayNext) {
            return self.queue_add(target_kind, &song_ids, position);
        }
        let result = {
            let store = lock(&p.store)?;
            let songs = songs_of(&store, &song_ids)?;
            let mut clock = lock(&p.edits_clock)?;
            let now = now_ms();
            let applied = Editor {
                store: &store,
                clock: &mut clock,
                device_id: &self.inner.config.device_id,
                now_utc_ms: now,
                undo_ms: tokens().undo_ms,
            }
            .add(target_kind, &songs, position, allow_duplicates)?;
            edit_result(applied, now)
        };
        self.send_soon();
        // A downloaded playlist or Liked songs follows the edit (Phase 8).
        let _ = self
            .inner
            .reconcile_downloads(&p, Some(&["playlist", "liked"]));
        Ok(result)
    }

    /// Like songs: Liked Songs, and a favourite on the server. The same as
    /// `add_to` with Liked Songs.
    pub fn like(&self, song_ids: Vec<i64>) -> Result<EditResult, EncoreError> {
        self.add_to(
            DropTarget {
                kind: DropTargetKind::LikedSongs,
                id: None,
            },
            song_ids,
            None,
            false,
        )
    }

    /// Unlike songs; every copy of each song (on any album) stops being liked.
    pub fn unlike(&self, song_ids: Vec<i64>) -> Result<EditResult, EncoreError> {
        self.edit(|ed, store| {
            let songs = songs_of(store, &song_ids)?;
            Ok(ed.unlike(&songs)?)
        })
    }

    /// Whether each song is liked, in order (the heart on rows and Now Playing).
    pub fn are_liked(&self, song_ids: Vec<i64>) -> Result<Vec<bool>, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        let liked = store.liked_keys()?;
        song_ids
            .iter()
            .map(|id| Ok(liked.contains(&store.track(*id)?.song_key)))
            .collect()
    }

    /// Liked Songs, newest like first; a song on several albums appears once.
    pub fn liked_songs(&self, offset: u32, limit: u32) -> Result<Vec<Song>, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        Ok(store
            .liked_songs(offset, limit)?
            .into_iter()
            .map(Song::from)
            .collect())
    }

    /// Remove rows from one of your playlists (a row swipe; Undo for 5 s).
    pub fn remove_from_playlist(
        &self,
        playlist_id: i64,
        positions: Vec<u32>,
    ) -> Result<EditResult, EncoreError> {
        self.edit(|ed, _| {
            let id = ed.remove(playlist_id, &positions)?;
            Ok(edits::Applied {
                edit_id: Some(id),
                added: 0,
                already_there: 0,
                playlist_id: Some(playlist_id),
            })
        })
    }

    /// Reorder inside a playlist (always a move).
    pub fn move_in_playlist(
        &self,
        playlist_id: i64,
        from_position: u32,
        to_position: u32,
    ) -> Result<EditResult, EncoreError> {
        self.edit(|ed, _| {
            let id = ed.move_row(playlist_id, from_position, to_position)?;
            Ok(edits::Applied {
                edit_id: Some(id),
                added: 0,
                already_there: 0,
                playlist_id: Some(playlist_id),
            })
        })
    }

    /// Rename a playlist (the name field a new playlist opens with). An empty name
    /// keeps the old one.
    pub fn rename_playlist(
        &self,
        playlist_id: i64,
        name: String,
    ) -> Result<EditResult, EncoreError> {
        self.edit(|ed, _| {
            let id = ed.rename(playlist_id, &name)?;
            Ok(edits::Applied {
                edit_id: Some(id),
                added: 0,
                already_there: 0,
                playlist_id: Some(playlist_id),
            })
        })
    }

    /// Undo an edit (the toast's Undo, or Command/Ctrl+Z), within
    /// `gestures.dragAndDrop.shortcutUndoMs` (10 s). Undo is one event too; it
    /// can't itself be undone. Refused with `NotAllowed` when too late.
    pub fn undo(&self, edit_id: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let undone = {
            let store = lock(&p.store)?;
            let mut clock = lock(&p.edits_clock)?;
            Editor {
                store: &store,
                clock: &mut clock,
                device_id: &self.inner.config.device_id,
                now_utc_ms: now_ms(),
                undo_ms: tokens().undo_ms,
            }
            .undo(&edit_id)?
        };
        if undone == edits::Undone::Queue {
            self.undo_queue()?;
        }
        self.send_soon();
        let _ = self
            .inner
            .reconcile_downloads(&p, Some(&["playlist", "liked"]));
        Ok(())
    }

    /// Edits not yet accepted by their server (for a "Waiting to sync" note).
    pub fn pending_edits(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        Ok(lock(&p.store)?.pending_outbox_count()?)
    }

    /// Send the waiting edits now (when the network comes back, say). They are
    /// also sent after every edit and before every sync. Async.
    pub async fn send_edits(&self) -> Result<SentEdits, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move { send_all(&inner).await }).await
    }

    // ------------------------------------------------------------ streams, artwork, lyrics

    /// Where to load a song from, for this network's quality setting.
    pub fn stream_for(&self, song_id: i64, network: Network) -> Result<StreamInfo, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        let track = store.track(song_id)?;
        let quality = self.inner.quality(&store, network)?;
        let cache_key = self.inner.cache_key(&store, &track, quality);
        // A downloaded song plays from its file, online or not (Phase 8).
        if let Some(path) = self.inner.local_file(&store, song_id)? {
            let url = downloads_file_url(&path);
            return Ok(StreamInfo {
                song_id,
                url_with_api_key: url.clone(),
                url,
                headers: Vec::new(),
                may_transcode: false,
                local: true,
                cache_key,
            });
        }
        let source = self.inner.source(&p, track.source_id)?;
        let info = source.stream_info(
            &track.source_item_id,
            &self.inner.stream_request(&store, quality)?,
        )?;
        Ok(StreamInfo {
            song_id,
            url: info.url,
            url_with_api_key: info.url_with_api_key,
            headers: info
                .headers
                .into_iter()
                .map(|(name, value)| HttpHeader { name, value })
                .collect(),
            may_transcode: info.may_transcode,
            local: false,
            cache_key,
        })
    }

    pub fn stream_quality(&self, network: Network) -> Result<StreamQuality, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        self.inner.quality(&store, network)
    }

    pub fn set_stream_quality(
        &self,
        network: Network,
        quality: StreamQuality,
    ) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let key = match network {
            Network::Wifi => "stream.quality.wifi",
            Network::Cellular => "stream.quality.cellular",
        };
        let value = match quality {
            StreamQuality::Original => "original",
            StreamQuality::High => "high",
            StreamQuality::Normal => "normal",
            StreamQuality::DataSaver => "dataSaver",
        };
        Ok(lock(&p.store)?.set_setting(key, value)?)
    }

    /// What this device's player can play without transcoding, and the codec to
    /// ask for when a transcode is needed. Each platform sets this once.
    pub fn set_playback_capabilities(
        &self,
        direct_play_containers: Vec<String>,
        transcode_codec: TranscodeCodec,
    ) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        store.set_setting("stream.containers", &direct_play_containers.join(","))?;
        store.set_setting(
            "stream.codec",
            match transcode_codec {
                TranscodeCodec::Aac => "aac",
                TranscodeCodec::Opus => "opus",
            },
        )?;
        Ok(())
    }

    /// The image for an item at exactly `size_px` (plan section 4).
    pub fn artwork(
        &self,
        of: ArtworkOf,
        id: i64,
        size_px: u32,
    ) -> Result<ArtworkRequest, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        // A downloaded album shows its kept cover, so covers show offline too
        // (the platform's loader reads `file://`).
        let album_id = match of {
            ArtworkOf::Album => Some(id),
            ArtworkOf::Song => store.track(id).ok().and_then(|t| t.album_id),
            _ => None,
        };
        if let Some(a) = album_id
            && let Some(path) = self.inner.local_cover(&store, a)?
        {
            return Ok(ArtworkRequest {
                url: downloads_file_url(&path),
                headers: Vec::new(),
            });
        }
        let (source_id, item) = match of {
            ArtworkOf::Album => {
                let a = store.album(id)?;
                (a.source_id, a.source_item_id)
            }
            ArtworkOf::Song => {
                let t = store.track(id)?;
                match t.album_id.map(|a| store.album(a)).transpose()? {
                    Some(a) if a.has_artwork => (a.source_id, a.source_item_id),
                    _ => (t.source_id, t.source_item_id),
                }
            }
            ArtworkOf::Artist => {
                let a = store.artist(id)?;
                (a.source_id, a.source_item_id)
            }
            ArtworkOf::Playlist => {
                let pl = store.playlist(id)?;
                (pl.source_id, pl.source_item_id)
            }
        };
        let source = self.inner.source(&p, source_id)?;
        let a = source.artwork(&item, size_px)?;
        Ok(ArtworkRequest {
            url: a.url,
            headers: a
                .headers
                .into_iter()
                .map(|(name, value)| HttpHeader { name, value })
                .collect(),
        })
    }

    /// A song's lyrics, read and ready to draw (board 10); None when it has none.
    /// Kept per song: once fetched they show offline. They are fetched again after
    /// `lyrics.refreshAfterDays`, or sooner when the server's copy of the song
    /// changes. When the server can't be reached the kept copy is used, however old.
    /// A file that can't be read counts as no lyrics, so the view never fails. Async.
    pub async fn lyrics(&self, song_id: i64) -> Result<Option<SongLyrics>, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let t = tokens();
            let (track, cached) = {
                let store = lock(&p.store)?;
                (store.track(song_id)?, store.cached_lyrics(song_id)?)
            };
            let now = now_ms();
            let from_cache = |c: &CachedLyrics| -> Result<Option<SongLyrics>, EncoreError> {
                let Some(json) = &c.model_json else {
                    return Ok(None);
                };
                let model: encore_lyrics::Lyrics =
                    serde_json::from_str(json).map_err(EncoreError::internal)?;
                Ok(Some(SongLyrics::from_model(
                    song_id,
                    model,
                    c.fetched_utc_ms,
                )))
            };
            if let Some(c) = &cached
                && now - c.fetched_utc_ms < t.lyrics_refresh_after_ms
            {
                return from_cache(c);
            }
            if !track.has_lyrics {
                // The server says it has none: no request.
                return Ok(None);
            }
            let fetched = match inner.source(&p, track.source_id) {
                Ok(source) => source
                    .lyrics(&track.source_item_id)
                    .await
                    .map_err(EncoreError::from),
                Err(e) => Err(e),
            };
            let fetched = match (fetched, &cached) {
                (Ok(f), _) => f,
                // Offline or signed out: the kept copy, however old.
                (Err(_), Some(c)) => return from_cache(c),
                (Err(e), None) => return Err(e),
            };
            let rules = encore_lyrics::Rules {
                instrumental_gap_ms: t.lyrics_gap_ms,
                last_line_ms: t.lyrics_last_line_ms,
                song_duration_ms: Some(track.duration_ms as i64).filter(|d| *d > 0),
            };
            let parsed = fetched.and_then(|raw| {
                encore_lyrics::parse(&raw.body, encore_lyrics::Origin::MediaServer, &rules)
                    .ok()
                    .flatten()
            });
            let (format, model_json) = match &parsed {
                Some(l) => (
                    serde_json::to_value(l.format)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    Some(serde_json::to_string(l).map_err(EncoreError::internal)?),
                ),
                None => ("none".to_owned(), None),
            };
            {
                let store = lock(&p.store)?;
                store.save_lyrics(
                    song_id,
                    &CachedLyrics {
                        origin: "mediaServer".into(),
                        format,
                        model_json,
                        fetched_utc_ms: now,
                    },
                )?;
                // Searchable from now on (board 07).
                let lines: Vec<(String, Option<i64>)> = parsed
                    .iter()
                    .flat_map(|l| l.lines.iter())
                    .map(|line| (line.text.clone(), line.start_ms))
                    .collect();
                store.index_lyrics(song_id, &lines)?;
            }
            Ok(parsed.map(|l| SongLyrics::from_model(song_id, l, now)))
        })
        .await
    }

    /// Fetch lyrics for songs that have them and aren't kept yet, trying up to
    /// `max_songs`, so search finds songs by their lyrics before they were ever
    /// opened. Call it after a sync, on Wi-Fi; call again while it returns more
    /// than 0. Returns how many songs it tried. A song that fails is skipped for
    /// the rest of the session; it stops early when the server can't be reached.
    /// Async.
    pub async fn fetch_lyrics_ahead(&self, max_songs: u32) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let ids: Vec<i64> = {
            let failed = lock(&p.lyrics_ahead_failed)?;
            let wanted = max_songs.saturating_add(failed.len() as u32);
            lock(&p.store)?
                .songs_without_kept_lyrics(wanted)?
                .into_iter()
                .filter(|id| !failed.contains(id))
                .take(max_songs as usize)
                .collect()
        };
        let mut tried = 0;
        for id in ids {
            tried += 1;
            match self.lyrics(id).await {
                Ok(_) => {}
                Err(e @ (EncoreError::Unreachable { .. } | EncoreError::SessionExpired { .. })) => {
                    return if tried == 1 { Err(e) } else { Ok(tried - 1) };
                }
                // One song's trouble doesn't stop the rest.
                Err(_) => {
                    lock(&p.lyrics_ahead_failed)?.insert(id);
                }
            }
        }
        Ok(tried)
    }

    /// The colours for an album's gradient (board 04); None when it has no cover
    /// (use the profile accent). Made once per cover from a small PNG of it, the same
    /// on every device; made again when the cover changes. Works offline once made;
    /// when the server can't be reached the last palette is used. Async.
    pub async fn album_palette(
        &self,
        album_id: i64,
    ) -> Result<Option<ArtworkPalette>, EncoreError> {
        self.palette_for("album", album_id).await
    }

    /// The same for a playlist's cover. Async.
    pub async fn playlist_palette(
        &self,
        playlist_id: i64,
    ) -> Result<Option<ArtworkPalette>, EncoreError> {
        self.palette_for("playlist", playlist_id).await
    }

    /// The song's lyrics exactly as the server sent them (Jellyfin's JSON); None
    /// when it has none. For diagnostics; draw `lyrics` instead. Async.
    pub async fn lyrics_raw(&self, song_id: i64) -> Result<Option<String>, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let (source_id, item) = {
                let store = lock(&p.store)?;
                let t = store.track(song_id)?;
                (t.source_id, t.source_item_id)
            };
            let source = inner.source(&p, source_id)?;
            Ok(source.lyrics(&item).await?.map(|l| l.body))
        })
        .await
    }
}

mod downloads;
mod live;
mod profiles;
mod recs;
// Phase 9, WP1: the sleep timer.
mod sleep;
mod sync_loop;
#[cfg(test)]
mod tests;
// Phase 9, WP2: the widget snapshot and `play_mix`.
mod widgets;

/// A local path as a `file://` address for the player and the image loader.
fn downloads_file_url(path: &Path) -> String {
    url::Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", path.display()))
}
