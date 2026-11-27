//! The companion server (plan section 5): profiles, sign-in by media-server token,
//! the synced event log, and a status page. It never stores music. Auth, sync and
//! status are Phase 4; mixes Phase 5; the live link (`/ws`: presence, handoff,
//! remote control) Phase 7; jobs and recaps arrive in their own phases.

mod auth;
mod config;
mod db;
mod error;
mod limit;
mod live;
mod recs;
mod sound;
mod sync;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, DefaultBodyLimit, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::sync::Notify;

pub use auth::{CheckError, CheckFuture, CheckedUser, JellyfinCheck, MediaCheck};
pub use config::{
    BIND_ENV, Config, DATA_DIR_ENV, DEFAULT_BIND, JELLYFIN_URLS_ENV, MEDIA_URLS_ENV, bind_address,
    jellyfin_urls,
};
pub use db::{DeviceRow, Event, Status};
pub use live::{MAX_MESSAGE_BYTES, MAX_PER_SECOND, PING_EVERY};
pub use recs::history_from;
pub use sync::{MAX_PULL, MAX_PUSH};

use error::ApiError;

/// The database file inside the data folder.
pub const DB_FILE: &str = "encore-server.sqlite";

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Everything a request handler shares.
#[derive(Debug)]
pub struct AppState {
    db: db::Db,
    check: Arc<dyn MediaCheck>,
    limiter: limit::RateLimiter,
    /// Every sign-in attempt, from all clients together.
    sign_ins: limit::RateLimiter,
    /// Token refreshes, per client.
    refreshes: limit::RateLimiter,
    /// One waiter per profile, woken when one of its devices pushes.
    waiters: Mutex<HashMap<i64, Arc<Notify>>>,
    /// Open live links (Phase 7).
    live: live::Hub,
    /// Heavy jobs running now, one of each kind per profile.
    jobs: Mutex<std::collections::HashSet<(i64, &'static str)>>,
    config: Config,
    started: Instant,
}

impl AppState {
    /// Open (or create) the database in `config.data_dir`.
    pub fn open(config: Config, check: Arc<dyn MediaCheck>) -> Result<AppState, String> {
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|e| format!("can't make {}: {e}", config.data_dir.display()))?;
        let path = config.data_dir.join(DB_FILE);
        let db = db::Db::open(&path).map_err(|e| format!("can't open {}: {e}", path.display()))?;
        Ok(AppState::with_db(db, config, check))
    }

    fn with_db(db: db::Db, config: Config, check: Arc<dyn MediaCheck>) -> AppState {
        AppState {
            db,
            check,
            limiter: limit::RateLimiter::new(10, Duration::from_secs(60)),
            sign_ins: limit::RateLimiter::new(60, Duration::from_secs(60)),
            refreshes: limit::RateLimiter::new(120, Duration::from_secs(60)),
            waiters: Mutex::new(HashMap::new()),
            live: live::Hub::default(),
            jobs: Mutex::new(std::collections::HashSet::new()),
            config,
            started: Instant::now(),
        }
    }

    fn ttl(&self) -> (i64, i64) {
        (
            self.config.access_ttl.as_millis() as i64,
            self.config.refresh_ttl.as_millis() as i64,
        )
    }

    fn waiter(&self, profile_id: i64) -> Arc<Notify> {
        self.waiters
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(profile_id)
            .or_default()
            .clone()
    }

    /// Start a heavy job (`kind`: mixes or training) for a profile, unless one is still
    /// running: work past its time limit keeps going in the background, so asking again
    /// must not pile up more (security review, 27 November 2026). The guard ends it.
    pub(crate) fn start_job(
        self: &Arc<Self>,
        profile_id: i64,
        kind: &'static str,
    ) -> Option<JobGuard> {
        let mut jobs = self.jobs.lock().unwrap_or_else(|p| p.into_inner());
        jobs.insert((profile_id, kind)).then(|| JobGuard {
            state: self.clone(),
            key: (profile_id, kind),
        })
    }

    fn wake(&self, profile_id: i64) {
        self.waiter(profile_id).notify_waiters();
    }

    /// A consistent copy of the database while the server runs.
    pub fn backup_to(&self, dest: &Path) -> Result<(), String> {
        self.db.backup_to(dest).map_err(|e| e.to_string())
    }
}

/// A running heavy job; dropping it (when the work ends) lets the next one start.
#[derive(Debug)]
pub(crate) struct JobGuard {
    state: Arc<AppState>,
    key: (i64, &'static str),
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.state
            .jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.key);
    }
}

/// `encore-server backup <file>`: a consistent copy of the database in
/// `data_dir`, safe while the server runs (the database is in WAL mode).
pub fn backup_database(data_dir: &Path, dest: &Path) -> Result<(), String> {
    let path = data_dir.join(DB_FILE);
    if !path.exists() {
        return Err(format!("no database at {}", path.display()));
    }
    let db = db::Db::open(&path).map_err(|e| e.to_string())?;
    db.backup_to(dest).map_err(|e| e.to_string())
}

/// The reply to `GET /health`. It names no user, device or library.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Health {
    pub status: &'static str,
    pub server_version: &'static str,
    pub core_version: String,
}

/// All routes. Serve with `into_make_service_with_connect_info::<SocketAddr>()`:
/// sign-in's rate limit and the status page need the client's address.
pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        // Sign-in bodies are small, and are read only after the rate limit.
        .route(
            "/auth/exchange",
            post(auth::exchange).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/auth/refresh",
            post(auth::refresh).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route("/devices", get(auth::devices))
        .route("/devices/{id}", delete(auth::revoke).patch(auth::rename))
        .route("/ws", get(live::ws))
        .route("/profile", delete(auth::delete_profile))
        .route("/sync/push", post(sync::push))
        .route("/sync/pull", get(sync::pull))
        .route("/admin/status", get(admin_status))
        .route("/recs/home", post(recs::home))
        .route("/recs/model", post(recs::model))
        .route("/sound/lookup", post(sound::lookup))
        .route("/sound/claim", post(sound::claim))
        .route("/sound", axum::routing::put(sound::put))
        .route("/catalog/hash", get(recs::catalog_hash))
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        // A big library's song list is larger than any other request.
        .route(
            "/catalog",
            axum::routing::put(recs::put_catalog).layer(DefaultBodyLimit::max(64 * 1024 * 1024)),
        )
        .layer(axum::middleware::from_fn(time_limit))
        .with_state(state)
}

/// No request may take longer than this, so a slow client can't hold one open for
/// ever (security review, 27 November 2026). The long poll waits at most 25 s, a big
/// song list uploads well within it, and a WebSocket's upgrade answers at once.
const REQUEST_LIMIT: Duration = Duration::from_secs(120);

async fn time_limit(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match tokio::time::timeout(REQUEST_LIMIT, next.run(req)).await {
        Ok(r) => r,
        Err(_) => ApiError::Unavailable("The request took too long.".into()).into_response(),
    }
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        server_version: env!("CARGO_PKG_VERSION"),
        core_version: encore_core::core_version(),
    })
}

/// Loopback and private addresses: the home network (plan section 12: "admin
/// page bound to the LAN by default").
fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || (o[0] == 100 && (o[1] & 0xC0) == 64)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xFE00) == 0xFC00 || (first & 0xFFC0) == 0xFE80
        }
    }
}

#[derive(Debug, Serialize)]
struct AdminStatus {
    status: &'static str,
    server_version: &'static str,
    uptime_s: u64,
    #[serde(flatten)]
    counts: Status,
}

/// Headers a reverse proxy or tunnel adds. A request carrying one came through a
/// proxy, whose own address (often this machine's) says nothing about where the
/// request started, so it is not taken as from the home network.
const PROXY_HEADERS: [&str; 6] = [
    "forwarded",
    "x-forwarded-for",
    "x-real-ip",
    "cf-connecting-ip",
    "true-client-ip",
    "tailscale-user-login",
];

/// `GET /admin/status`: counts only, and only from the home network, never through a
/// proxy or tunnel (security review, 27 November 2026).
async fn admin_status(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
) -> Result<Json<AdminStatus>, ApiError> {
    let proxied = PROXY_HEADERS.iter().any(|h| headers.contains_key(*h));
    if proxied || !is_local(addr.ip()) {
        return Err(ApiError::Forbidden(
            "The status page answers only on the home network.".into(),
        ));
    }
    Ok(Json(AdminStatus {
        status: "ok",
        server_version: env!("CARGO_PKG_VERSION"),
        uptime_s: state.started.elapsed().as_secs(),
        counts: state.db.status()?,
    }))
}
