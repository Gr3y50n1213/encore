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
use encore_sources::jellyfin::{
    self, DeviceInfo, JellyfinClient, ServerInfo, Session, StreamCodec, StreamRequest,
};
use encore_sources::{JellyfinSource, MediaSource, Transport};
use encore_store::{NewSource, Page, SearchLimits, SourceRecord, Store, TopResult as StoreTop};
use sha2::{Digest, Sha256};

use super::types::*;
use crate::brand::APP_NAME;
use crate::playlog::{PlayLog, PlayStart};
use crate::queue::{Advance, NewItem, PreviousOutcome, Queue, QueueItem};
use crate::tokens::tokens;

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
}

struct Profile {
    id: String,
    store: Mutex<Store>,
    sources: Mutex<HashMap<i64, Arc<JellyfinSource>>>,
    playback: Mutex<Playback>,
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
        drop(store);
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
        let profile = Profile {
            id: id.to_owned(),
            store: Mutex::new(store),
            sources: Mutex::new(sources),
            playback: Mutex::new(Playback {
                queue: Queue::new(seed),
                log,
                playing_uid: None,
                last_listened_ms: 0,
                preloaded_for: None,
            }),
        };
        *lock(&self.profile)? = Some(Arc::new(profile));
        Ok(())
    }

    fn close_profile(&self) -> Result<(), EncoreError> {
        let old = lock(&self.profile)?.take();
        if let Some(p) = old {
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
        Ok(())
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
        }
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

impl EncoreCore {
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

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Start the core. `config.data_dir` must be a folder the app can write.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: CoreConfig) -> Result<Arc<EncoreCore>, EncoreError> {
        Self::build(config, QUICK_CONNECT_POLL, tokens().sync_page_size, None)
    }
}

impl EncoreCore {
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
        std::fs::create_dir_all(Path::new(&config.data_dir).join("profiles")).map_err(|e| {
            EncoreError::Storage {
                reason: format!("couldn't use the data folder ({e})"),
            }
        })?;
        Ok(Arc::new(EncoreCore {
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
            }),
        }))
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
            out.push(ProfileSummary {
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
                    .collect(),
                profile_id: id,
            });
        }
        Ok(out)
    }

    /// Open a profile with the tokens the app read from its keychain. A play left
    /// open by a crash is closed as "ended by the system".
    pub fn open_profile(
        &self,
        profile_id: String,
        tokens: Vec<SourceToken>,
    ) -> Result<(), EncoreError> {
        self.inner.open_profile(&profile_id, tokens)
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
        if self.current_profile().as_deref() == Some(id) {
            self.inner.close_profile()?;
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
                let stats = crate::library::sync_source(
                    &p.store,
                    id,
                    source.as_ref(),
                    inner.sync_page_size,
                    now_ms(),
                    &progress,
                )
                .await?;
                results.push(SyncResult {
                    source_id: id,
                    artists: stats.artists as u32,
                    albums: stats.albums as u32,
                    songs: stats.tracks as u32,
                    playlists: stats.playlists as u32,
                    removed: stats.removed as u32,
                });
            }
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
        let items = Inner::new_items(&song_ids, started_from, context_id);
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.play(items, start_index as usize))?
            .1)
    }

    pub fn play_next(&self, song_ids: Vec<i64>) -> Result<QueueState, EncoreError> {
        let items = Inner::new_items(&song_ids, StartedFrom::Queue, None);
        Ok(self.inner.with_queue(|pb, _| pb.queue.play_next(items))?.1)
    }

    pub fn add_to_up_next(&self, song_ids: Vec<i64>) -> Result<QueueState, EncoreError> {
        let items = Inner::new_items(&song_ids, StartedFrom::Queue, None);
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.add_to_up_next(items))?
            .1)
    }

    /// Next (button, swipe, headset, lock screen). At the end of the queue the
    /// current song stays.
    pub fn next(&self) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| {
                pb.queue.advance(Advance::Skip);
            })?
            .1)
    }

    /// Previous. `position_ms` is where the current song is; the button restarts
    /// it after 3 s, the swipe (`from_swipe`) always goes back (board 42).
    pub fn previous(
        &self,
        position_ms: i64,
        from_swipe: bool,
    ) -> Result<PreviousResult, EncoreError> {
        let restart_after = tokens().previous_restart_after_ms;
        let (outcome, _) = self.inner.with_queue(|pb, _| {
            pb.queue
                .previous(position_ms.max(0) as u64, restart_after, from_swipe)
        })?;
        Ok(match outcome {
            PreviousOutcome::Restart => PreviousResult::Restarted,
            PreviousOutcome::Moved => PreviousResult::Moved,
        })
    }

    pub fn jump_to(&self, uid: i64) -> Result<QueueState, EncoreError> {
        Ok(self
            .inner
            .with_queue(|pb, _| pb.queue.jump_to(uid as u64))?
            .1)
    }

    pub fn replay_from_history(&self, uid: i64) -> Result<QueueState, EncoreError> {
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

    // ------------------------------------------------------------ playback reports

    /// The player started the current song (`uid` from the queue). Writes the play
    /// event at once (plan section 8). `utc_offset_minutes` is the device's offset
    /// right now (east positive).
    pub fn playback_started(&self, uid: i64, utc_offset_minutes: i32) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let mut pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
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
        };
        let previous = pb.last_listened_ms;
        pb.log.start(&store, &start, previous)?;
        pb.playing_uid = Some(item.uid);
        pb.last_listened_ms = 0;
        pb.preloaded_for = None;
        Ok(())
    }

    /// Progress from the player, about once a second. `listened_ms` counts only
    /// time actually played (not seeks). Emits Preload 20 s before the end.
    pub fn playback_progress(&self, position_ms: i64, listened_ms: i64) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let preload = {
            let mut pb = lock(&p.playback)?;
            let store = lock(&p.store)?;
            pb.last_listened_ms = listened_ms.max(0) as u64;
            let listened = pb.last_listened_ms;
            pb.log.progress(&store, listened, now_ms())?;
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
            pb.log
                .finish(&store, listened_ms.max(0) as u64, reason.into(), now_ms())?;
            pb.playing_uid = None;
            pb.last_listened_ms = 0;
        }
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
        Ok(())
    }

    // ------------------------------------------------------------ streams, artwork, lyrics

    /// Where to load a song from, for this network's quality setting.
    pub fn stream_for(&self, song_id: i64, network: Network) -> Result<StreamInfo, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        let track = store.track(song_id)?;
        let source = self.inner.source(&p, track.source_id)?;
        let t = tokens();
        let quality = match self.inner.quality(&store, network)? {
            StreamQuality::Original => jellyfin::StreamQuality::Original,
            StreamQuality::High => jellyfin::StreamQuality::Capped { kbps: t.high_kbps },
            StreamQuality::Normal => jellyfin::StreamQuality::Capped {
                kbps: t.normal_kbps,
            },
            StreamQuality::DataSaver => jellyfin::StreamQuality::Capped {
                kbps: t.data_saver_kbps,
            },
        };
        let codec = match store.setting("stream.codec")?.as_deref() {
            Some("opus") => StreamCodec::Opus,
            _ => StreamCodec::Aac,
        };
        let containers: Vec<String> = match store.setting("stream.containers")? {
            Some(list) => list.split(',').map(str::to_owned).collect(),
            None => DEFAULT_CONTAINERS.iter().map(|s| (*s).to_owned()).collect(),
        };
        let info = source.stream_info(
            &track.source_item_id,
            &StreamRequest {
                quality,
                codec,
                direct_play_containers: containers,
            },
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

    /// The song's lyrics as the server sent them (Jellyfin's JSON); None when it has
    /// none. Phase 3 adds the parsed model. Async.
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

#[cfg(test)]
mod tests;
