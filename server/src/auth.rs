//! Sign-in without a new password (plan section 5): a device sends its Jellyfin
//! token once, the server asks that Jellyfin who it belongs to, and hands back its
//! own tokens. The Jellyfin token is used for that one check and never stored.

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use encore_sources::jellyfin::{DeviceInfo, JellyfinClient};
use encore_sources::{SourceError, Transport};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::db::{Authed, DeviceRow, Refreshed};
use crate::error::ApiError;
use crate::{AppState, now_ms};

/// Who a media-server token belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUser {
    pub user_id: String,
    pub user_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// None of the trusted media servers has this ID.
    UnknownServer,
    /// The media server says the token isn't valid.
    Rejected,
    Unreachable(String),
}

pub type CheckFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CheckedUser, CheckError>> + Send + 'a>>;

/// Checks a token with the media server. A trait so tests can stand in for it.
pub trait MediaCheck: Send + Sync + std::fmt::Debug {
    fn check<'a>(&'a self, server_id: &'a str, token: &'a str) -> CheckFuture<'a>;
}

/// The real check: the trusted Jellyfin servers, through encore-sources' client.
#[derive(Debug)]
pub struct JellyfinCheck {
    urls: Vec<Url>,
    transport: Arc<dyn Transport>,
    /// Server ID to address, learnt from `/System/Info/Public`.
    known: Mutex<HashMap<String, Url>>,
}

impl JellyfinCheck {
    pub fn new(urls: Vec<Url>, transport: Arc<dyn Transport>) -> JellyfinCheck {
        JellyfinCheck {
            urls,
            transport,
            known: Mutex::new(HashMap::new()),
        }
    }

    fn client(&self, url: &Url) -> JellyfinClient {
        JellyfinClient::new(
            self.transport.clone(),
            url.clone(),
            DeviceInfo {
                client: "Encore companion".into(),
                device_name: "encore-server".into(),
                device_id: "encore-server".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            Duration::from_secs(10),
        )
    }

    async fn url_for(&self, server_id: &str) -> Result<Url, CheckError> {
        if let Some(u) = self
            .known
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(server_id)
        {
            return Ok(u.clone());
        }
        let mut last_error = None;
        for url in &self.urls {
            match self.client(url).public_info().await {
                Ok(info) => {
                    self.known
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(info.server_id.clone(), url.clone());
                    if info.server_id == server_id {
                        return Ok(url.clone());
                    }
                }
                Err(e) => last_error = Some(e.to_string()),
            }
        }
        Err(match last_error {
            Some(e) => CheckError::Unreachable(e),
            None => CheckError::UnknownServer,
        })
    }
}

impl MediaCheck for JellyfinCheck {
    fn check<'a>(&'a self, server_id: &'a str, token: &'a str) -> CheckFuture<'a> {
        Box::pin(async move {
            let url = self.url_for(server_id).await?;
            let me = self
                .client(&url)
                .signed_in(token.to_owned(), String::new())
                .me()
                .await
                .map_err(|e| match e {
                    SourceError::SessionExpired | SourceError::WrongCredentials => {
                        CheckError::Rejected
                    }
                    other => CheckError::Unreachable(other.to_string()),
                })?;
            // The user record names its server on current Jellyfin; it must agree.
            if !me.server_id.is_empty() && me.server_id != server_id {
                return Err(CheckError::Rejected);
            }
            Ok(CheckedUser {
                user_id: me.user_id,
                user_name: me.user_name,
            })
        })
    }
}

/// A request signed in with an access token (`Authorization: Bearer eat_...`).
#[derive(Debug, Clone)]
pub(crate) struct Device(pub Authed);

impl FromRequestParts<Arc<AppState>> for Device {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                ApiError::Unauthorized("Sign in to the companion server first.".into())
            })?;
        state
            .db
            .authed(token, now_ms())?
            .map(Device)
            .ok_or_else(|| {
                ApiError::Unauthorized(
                    "This device's sign-in has ended. Refresh it, or sign in again.".into(),
                )
            })
    }
}

#[derive(Deserialize)]
pub(crate) struct ExchangeBody {
    /// The media server's ID (from its `/System/Info/Public`).
    server_id: String,
    /// The device's Jellyfin token: checked once, never stored.
    media_token: String,
    /// The app's stable device ID.
    device_id: String,
    device_name: String,
    platform: String,
}

impl std::fmt::Debug for ExchangeBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeBody")
            .field("device_name", &self.device_name)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
pub(crate) struct TokenReply {
    device_id: String,
    user_name: Option<String>,
    access_token: String,
    access_expires_utc_ms: i64,
    refresh_token: String,
}

fn field(name: &str, value: &str, max: usize) -> Result<(), ApiError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(ApiError::BadRequest(format!(
            "`{name}` must be 1 to {max} characters"
        )));
    }
    Ok(())
}

fn limited(state: &AppState, addr: SocketAddr) -> Result<(), ApiError> {
    state
        .limiter
        .take(addr.ip(), Instant::now())
        .map_err(|retry_after_s| ApiError::TooManyRequests { retry_after_s })
}

/// `POST /auth/exchange`.
pub(crate) async fn exchange(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<ExchangeBody>,
) -> Result<Json<TokenReply>, ApiError> {
    limited(&state, addr)?;
    field("server_id", &body.server_id, 128)?;
    field("media_token", &body.media_token, 1024)?;
    field("device_id", &body.device_id, 128)?;
    field("device_name", &body.device_name, 100)?;
    field("platform", &body.platform, 20)?;
    let user = state
        .check
        .check(&body.server_id, &body.media_token)
        .await
        .map_err(|e| match e {
            CheckError::UnknownServer => ApiError::Forbidden(
                "This companion server isn't set up for that media server.".into(),
            ),
            CheckError::Rejected => ApiError::Unauthorized(
                "The media server didn't accept that sign-in. Sign in to it again.".into(),
            ),
            CheckError::Unreachable(e) => {
                ApiError::Unavailable(format!("The media server can't be reached ({e})."))
            }
        })?;
    let now = now_ms();
    let profile = state
        .db
        .upsert_profile(&body.server_id, &user.user_id, &user.user_name, now)?;
    let (device_id, t) = state.db.sign_in_device(
        profile,
        &body.device_id,
        &body.device_name,
        &body.platform,
        now,
        state.ttl(),
    )?;
    Ok(Json(TokenReply {
        device_id,
        user_name: Some(user.user_name),
        access_token: t.access,
        access_expires_utc_ms: t.access_expires_utc_ms,
        refresh_token: t.refresh,
    }))
}

#[derive(Deserialize)]
pub(crate) struct RefreshBody {
    refresh_token: String,
}

/// `POST /auth/refresh`: a new pair; the old refresh token is used up.
pub(crate) async fn refresh(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RefreshBody>,
) -> Result<Json<TokenReply>, ApiError> {
    limited(&state, addr)?;
    match state.db.refresh(&body.refresh_token, now_ms(), state.ttl())? {
        Refreshed::Ok { tokens, authed } => Ok(Json(TokenReply {
            device_id: authed.device_id,
            user_name: None,
            access_token: tokens.access,
            access_expires_utc_ms: tokens.access_expires_utc_ms,
            refresh_token: tokens.refresh,
        })),
        Refreshed::Invalid => Err(ApiError::Unauthorized(
            "This device's sign-in has ended. Sign in again.".into(),
        )),
        Refreshed::Reused => Err(ApiError::Unauthorized(
            "This sign-in was used from somewhere else, so the device was signed out. Sign in again."
                .into(),
        )),
    }
}

#[derive(Serialize)]
pub(crate) struct DevicesReply {
    this_device: String,
    devices: Vec<DeviceReply>,
}

/// A device and, from the live link, whether it is online and what it plays.
#[derive(Debug, Serialize)]
pub(crate) struct DeviceReply {
    #[serde(flatten)]
    row: DeviceRow,
    online: bool,
    presence: Option<serde_json::Value>,
}

/// `GET /devices`: the profile's devices, for the Devices page.
pub(crate) async fn devices(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
) -> Result<Json<DevicesReply>, ApiError> {
    let online = state.live.online(d.profile_id);
    let devices = state
        .db
        .devices(d.profile_id)?
        .into_iter()
        .map(|row| DeviceReply {
            online: online.contains_key(&row.id),
            presence: online.get(&row.id).cloned().flatten(),
            row,
        })
        .collect();
    Ok(Json(DevicesReply {
        devices,
        this_device: d.device_id,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct RenameBody {
    name: String,
}

/// `PATCH /devices/{id}`: board 12's Rename, for any device of the profile.
pub(crate) async fn rename(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Path(id): Path<String>,
    Json(body): Json<RenameBody>,
) -> Result<StatusCode, ApiError> {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::BadRequest(
            "A device name is 1 to 100 characters.".into(),
        ));
    }
    if state.db.rename_device(d.profile_id, &id, name)? {
        state.live.devices_changed(d.profile_id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("No such device on this profile.".into()))
    }
}

/// `DELETE /devices/{id}`: sign that device out everywhere (plan section 12, "Lost
/// phone").
pub(crate) async fn revoke(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if state.db.revoke_device(d.profile_id, &id, now_ms())? {
        state.live.kick(d.profile_id, Some(&id));
        state.live.devices_changed(d.profile_id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("No such device on this profile.".into()))
    }
}

/// `DELETE /profile`: board 19's delete, on the server: every event and device
/// of this profile. The media-server account and its music are never touched.
pub(crate) async fn delete_profile(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
) -> Result<StatusCode, ApiError> {
    state.db.delete_profile(d.profile_id)?;
    state.wake(d.profile_id);
    state.live.kick(d.profile_id, None);
    Ok(StatusCode::NO_CONTENT)
}
