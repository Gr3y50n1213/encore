//! The sync loop with the companion server (plan section 5; ADR core-0010), one
//! per open profile:
//!
//! - the pusher sends this device's changes moments after they happen (it is
//!   woken by each edit, and checks every 30 s for plays);
//! - the puller keeps one long-poll pull open, so another device's like arrives
//!   within a second, and applies what comes;
//! - both back off while the server can't be reached, and stop when the server
//!   no longer accepts the device (local data stays).
//!
//! Tokens: the access token lives only in memory; the refresh token is in the
//! app's keychain, handed over in `open_profile` and handed back by a
//! `CredentialChanged` event whenever it rotates.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use encore_store::OutEvent;
use encore_sync::Hlc;

use super::{Inner, Profile, lock, now_ms, send_all};
use crate::api::types::*;
use crate::companion::{Client, CompanionError, PULL_WAIT_S, Tokens};
use crate::edits::{self, QueuedOp};

pub(super) const URL_KEY: &str = "companion.url";
pub(super) const DEVICE_KEY: &str = "companion.device_id";
const LAST_SYNCED_KEY: &str = "companion.last_synced";
const PUSH_BATCH: u32 = 500;

pub(crate) struct Session {
    pub(super) client: Client,
    pub(super) server_device_id: String,
    tokens: tokio::sync::Mutex<Tokens>,
    wake: tokio::sync::Notify,
    stopped: AtomicBool,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    status: Mutex<SyncStatus>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("server", &self.client.base().as_str())
            .finish_non_exhaustive()
    }
}

impl Session {
    pub(super) fn new(
        client: Client,
        server_device_id: String,
        tokens: Tokens,
        last_synced: Option<i64>,
    ) -> Arc<Session> {
        let server_url = Some(client.base().as_str().trim_end_matches('/').to_owned());
        Arc::new(Session {
            client,
            server_device_id,
            tokens: tokio::sync::Mutex::new(tokens),
            wake: tokio::sync::Notify::new(),
            stopped: AtomicBool::new(false),
            tasks: Mutex::new(Vec::new()),
            status: Mutex::new(SyncStatus {
                state: SyncState::Syncing,
                pending: 0,
                last_synced_utc_ms: last_synced,
                server_url,
                reason: None,
            }),
        })
    }

    /// Something changed here: send it soon.
    pub(super) fn wake(&self) {
        self.wake.notify_one();
    }

    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut t) = self.tasks.lock() {
            for task in t.drain(..) {
                task.abort();
            }
        }
    }

    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub(super) fn status(&self) -> SyncStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn local<T>(r: Result<T, impl std::fmt::Display>) -> Result<T, CompanionError> {
    r.map_err(|e| CompanionError::Refused(format!("on this device: {e}")))
}

pub(super) fn into_error(e: CompanionError) -> EncoreError {
    match e {
        CompanionError::SignedOut(reason) => EncoreError::SessionExpired { reason },
        CompanionError::Unreachable(reason) => EncoreError::Unreachable { reason },
        CompanionError::Refused(reason) => EncoreError::Server {
            status: 400,
            detail: reason.clone(),
            reason,
        },
    }
}

pub(super) fn credential(inner: &Inner, token: String) {
    let mut e = CoreEvent::of(EventKind::CredentialChanged);
    e.credential = Some(SourceToken {
        source_id: 0,
        token,
    });
    inner.emit(e);
}

/// Update the status, and tell the app when it changed.
fn set_status(
    inner: &Inner,
    p: &Profile,
    s: &Session,
    state: SyncState,
    reason: Option<String>,
    synced_now: bool,
) {
    let pending = lock(&p.store)
        .ok()
        .and_then(|st| st.pending_out_count(&inner.config.device_id).ok())
        .unwrap_or(0);
    let now = now_ms();
    if synced_now && let Ok(st) = lock(&p.store) {
        let _ = st.set_setting(LAST_SYNCED_KEY, &now.to_string());
    }
    let changed = {
        let Ok(mut cur) = s.status.lock() else { return };
        let mut next = cur.clone();
        next.state = state;
        next.reason = reason;
        next.pending = pending;
        if synced_now {
            next.last_synced_utc_ms = Some(now);
        }
        // "Last synced" moving on alone isn't news; a state or count change is.
        let news =
            (next.state, &next.reason, next.pending) != (cur.state, &cur.reason, cur.pending);
        *cur = next.clone();
        news.then_some(next)
    };
    if let Some(status) = changed {
        let mut e = CoreEvent::of(EventKind::SyncStatusChanged);
        e.sync_status = Some(status);
        inner.emit(e);
    }
}

pub(super) fn last_synced(p: &Profile) -> Option<i64> {
    lock(&p.store)
        .ok()?
        .setting(LAST_SYNCED_KEY)
        .ok()??
        .parse()
        .ok()
}

/// A valid access token, refreshing (and handing the app the new refresh token)
/// when it is about to expire. One refresh at a time: two at once would look like
/// a copied token to the server.
async fn access(inner: &Inner, s: &Session, force: bool) -> Result<String, CompanionError> {
    let mut t = s.tokens.lock().await;
    if force || t.access.is_empty() || t.access_expires_utc_ms - now_ms() < 120_000 {
        let fresh = s.client.refresh(&t.refresh).await?;
        credential(inner, fresh.refresh.clone());
        *t = fresh;
    }
    Ok(t.access.clone())
}

/// Run `call` with an access token; if the server says the token is no good,
/// refresh once and try again.
pub(super) async fn signed<T, F, Fut>(
    inner: &Inner,
    s: &Session,
    call: F,
) -> Result<T, CompanionError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<T, CompanionError>>,
{
    let a = access(inner, s, false).await?;
    match call(a).await {
        Err(CompanionError::SignedOut(_)) => {
            let a = access(inner, s, true).await?;
            call(a).await
        }
        other => other,
    }
}

/// Send everything waiting. Returns how many went.
pub(super) async fn push_all(
    inner: &Inner,
    p: &Profile,
    s: &Session,
) -> Result<u32, CompanionError> {
    let mut sent = 0;
    loop {
        let batch: Vec<OutEvent> =
            local(lock(&p.store).map_err(|e| e.to_string()).and_then(|st| {
                st.pending_out(&inner.config.device_id, PUSH_BATCH)
                    .map_err(|e| e.to_string())
            }))?;
        if batch.is_empty() {
            return Ok(sent);
        }
        let accepted = signed(inner, s, |a| {
            let batch = &batch;
            async move { s.client.push(&a, batch).await }
        })
        .await?;
        let done: Vec<OutEvent> = batch
            .into_iter()
            .filter(|e| accepted.contains(&e.id))
            .collect();
        if done.is_empty() {
            return Err(CompanionError::Refused(
                "the companion server took none of this device's changes".into(),
            ));
        }
        sent += done.len() as u32;
        local(
            lock(&p.store)
                .map_err(|e| e.to_string())
                .and_then(|st| st.mark_sent(&done).map_err(|e| e.to_string())),
        )?;
    }
}

/// One pull (waiting up to `wait_s`), applied. Returns whether more are waiting.
pub(super) async fn pull_once(
    inner: &Inner,
    p: &Profile,
    s: &Session,
    wait_s: u64,
) -> Result<bool, CompanionError> {
    let cursor = local(
        lock(&p.store)
            .map_err(|e| e.to_string())
            .and_then(|st| st.sync_cursor().map_err(|e| e.to_string())),
    )?;
    let pulled = signed(inner, s, |a| async move {
        s.client.pull(&a, cursor, wait_s).await
    })
    .await?;
    if s.stopped() {
        return Ok(false);
    }
    if pulled.events.is_empty() {
        if pulled.cursor != cursor {
            let st = local(lock(&p.store))?;
            local(st.set_sync_cursor(pulled.cursor))?;
        }
        return Ok(pulled.more);
    }
    let (applied, change, reassert_ops) = {
        let st = local(lock(&p.store))?;
        let applied = local(st.apply_remote(&pulled.events))?;
        local(st.set_sync_cursor(pulled.cursor))?;
        let ids = |keys: &[&String]| -> Vec<i64> {
            keys.iter()
                .flat_map(|k| st.track_ids_for_key(k).unwrap_or_default())
                .collect()
        };
        let liked: Vec<&String> = applied
            .likes
            .iter()
            .filter(|(_, l)| *l)
            .map(|(k, _)| k)
            .collect();
        let unliked: Vec<&String> = applied
            .likes
            .iter()
            .filter(|(_, l)| !*l)
            .map(|(k, _)| k)
            .collect();
        let change = RemoteChange {
            liked_song_ids: ids(&liked),
            unliked_song_ids: ids(&unliked),
            playlist_ids: applied.playlists.clone(),
            history: applied.plays > 0,
            library_sync_suggested: applied.new_playlists,
        };
        // This device lost a like decision: send the winner to the media server.
        let mut ops = 0;
        for liked in [true, false] {
            let keys: Vec<String> = applied
                .reassert
                .iter()
                .filter(|(_, l)| *l == liked)
                .map(|(k, _)| k.clone())
                .collect();
            if keys.is_empty() {
                continue;
            }
            let mut by_source: Vec<(i64, Vec<String>)> = Vec::new();
            for (src, item, _) in local(st.items_for_keys(&keys))? {
                match by_source.iter_mut().find(|(s, _)| *s == src) {
                    Some((_, items)) => items.push(item),
                    None => by_source.push((src, vec![item])),
                }
            }
            for (src, items) in by_source {
                let op = serde_json::to_string(&vec![QueuedOp::Favorite {
                    item_ids: items,
                    favorite: liked,
                }])
                .unwrap_or_default();
                local(st.enqueue(src, &edits::new_id(), None, &op))?;
                ops += 1;
            }
        }
        (applied, change, ops)
    };
    if let Some(h) = applied
        .newest_hlc
        .as_deref()
        .and_then(|h| h.parse::<Hlc>().ok())
        && let Ok(mut clock) = lock(&p.edits_clock)
    {
        clock.observe(h, now_ms());
    }
    if reassert_ops > 0 {
        let _ = send_all(inner).await;
    }
    for id in &applied.playlists {
        let source = lock(&p.store)
            .ok()
            .and_then(|st| st.playlist_record(*id).ok().flatten())
            .and_then(|r| lock(&p.sources).ok()?.get(&r.source_id).cloned());
        if let Some(source) = source {
            let _ = edits::refresh_playlist(&p.store, source.as_ref(), *id).await;
        }
    }
    let mut e = CoreEvent::of(EventKind::RemoteChange);
    e.remote_change = Some(change);
    inner.emit(e);
    Ok(pulled.more)
}

/// The server no longer accepts this device: stop, forget the token, keep the
/// data and the address (so the app can offer "Connect again").
fn signed_out(inner: &Inner, p: &Profile, s: &Session, reason: String) {
    s.stopped.store(true, Ordering::SeqCst);
    if let Ok(st) = lock(&p.store) {
        let _ = st.set_setting(DEVICE_KEY, "");
    }
    credential(inner, String::new());
    set_status(inner, p, s, SyncState::SignedOut, Some(reason), false);
}

async fn rest(s: &Session, seconds: u64) {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {}
        _ = s.wake.notified() => {}
    }
}

async fn pusher(inner: Arc<Inner>, p: Arc<Profile>, s: Arc<Session>) {
    let mut backoff = 1;
    while !s.stopped() {
        match push_all(&inner, &p, &s).await {
            Ok(_) => {
                backoff = 1;
                set_status(&inner, &p, &s, SyncState::Idle, None, true);
                tokio::select! {
                    _ = s.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                }
                // Let a burst of changes go as one push.
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(CompanionError::SignedOut(r)) => return signed_out(&inner, &p, &s, r),
            Err(e) => {
                set_status(
                    &inner,
                    &p,
                    &s,
                    SyncState::Offline,
                    Some(e.reason().to_owned()),
                    false,
                );
                rest(&s, backoff).await;
                backoff = (backoff * 2).min(60);
            }
        }
    }
}

async fn puller(inner: Arc<Inner>, p: Arc<Profile>, s: Arc<Session>) {
    let mut backoff = 1;
    while !s.stopped() {
        match pull_once(&inner, &p, &s, PULL_WAIT_S).await {
            Ok(more) => {
                backoff = 1;
                if !more {
                    set_status(&inner, &p, &s, SyncState::Idle, None, true);
                }
            }
            Err(CompanionError::SignedOut(r)) => return signed_out(&inner, &p, &s, r),
            Err(e) => {
                set_status(
                    &inner,
                    &p,
                    &s,
                    SyncState::Offline,
                    Some(e.reason().to_owned()),
                    false,
                );
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(60);
            }
        }
    }
}

/// Start both loops on the core's runtime.
pub(super) fn start(
    rt: &tokio::runtime::Handle,
    inner: Arc<Inner>,
    p: Arc<Profile>,
    s: Arc<Session>,
) {
    let push = rt.spawn(pusher(inner.clone(), p.clone(), s.clone()));
    let pull = rt.spawn(puller(inner, p, s.clone()));
    if let Ok(mut t) = s.tasks.lock() {
        t.push(push);
        t.push(pull);
    }
}
