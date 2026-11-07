//! The companion server (plan section 5): profiles, sign-in by media-server token,
//! the synced event log, and a status page. It never stores music. Auth, sync and
//! status are Phase 4; the WebSocket, jobs and recaps arrive in their own phases.

mod auth;
mod config;
mod db;
mod error;
mod limit;
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
    BIND_ENV, Config, DATA_DIR_ENV, DEFAULT_BIND, JELLYFIN_URLS_ENV, bind_address, jellyfin_urls,
};
pub use db::{DeviceRow, Event, Status};
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
    /// One waiter per profile, woken when one of its devices pushes.
    waiters: Mutex<HashMap<i64, Arc<Notify>>>,
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
            waiters: Mutex::new(HashMap::new()),
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

    fn wake(&self, profile_id: i64) {
        self.waiter(profile_id).notify_waiters();
    }

    /// A consistent copy of the database while the server runs.
    pub fn backup_to(&self, dest: &Path) -> Result<(), String> {
        self.db.backup_to(dest).map_err(|e| e.to_string())
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
        .route("/auth/exchange", post(auth::exchange))
        .route("/auth/refresh", post(auth::refresh))
        .route("/devices", get(auth::devices))
        .route("/devices/{id}", delete(auth::revoke))
        .route("/profile", delete(auth::delete_profile))
        .route("/sync/push", post(sync::push))
        .route("/sync/pull", get(sync::pull))
        .route("/admin/status", get(admin_status))
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .with_state(state)
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

/// `GET /admin/status`: counts only, and only from the home network.
async fn admin_status(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<AdminStatus>, ApiError> {
    if !is_local(addr.ip()) {
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
