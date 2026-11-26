//! Sign-in without a new password (plan section 5): a device sends its Jellyfin
//! token once, the server asks that Jellyfin who it belongs to, and hands back its
//! own tokens. The Jellyfin token is used for that one check and never stored. An
//! Emby token comes the same way (ADR core-0022): the trusted address says it is Emby,
//! and Emby says whose the token is.

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
use encore_sources::jellyfin::{DeviceInfo, Flavor, JellyfinClient};
use encore_sources::plex::{PlexClient, PlexCredentials, PlexIdentity, client_identifier};
use encore_sources::subsonic::{SubsonicClient, SubsonicCredentials, server_id_for};
use encore_sources::{SourceError, Transport};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    /// The media server lets this companion in without any sign-in (a Plex server that
    /// isn't claimed, or whose "allowed without auth" networks include the companion),
    /// so it would vouch for any token: no sign-in can be checked there.
    Unverifiable,
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
    /// Server ID to address and kind (Jellyfin or Emby), learnt from
    /// `/System/Info/Public`.
    known: Mutex<HashMap<String, (Url, Flavor)>>,
    /// Emby names a token's user through this server's own session there, one per
    /// device name, so two Emby checks never run at once (ADR core-0022).
    emby_turn: tokio::sync::Mutex<()>,
}

impl JellyfinCheck {
    pub fn new(urls: Vec<Url>, transport: Arc<dyn Transport>) -> JellyfinCheck {
        JellyfinCheck {
            urls,
            transport,
            known: Mutex::new(HashMap::new()),
            emby_turn: tokio::sync::Mutex::new(()),
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

    async fn url_for(&self, server_id: &str) -> Result<(Url, Flavor), CheckError> {
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
                        .insert(info.server_id.clone(), (url.clone(), info.flavor));
                    if info.server_id == server_id {
                        return Ok((url.clone(), info.flavor));
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

impl JellyfinCheck {
    /// A Navidrome or other OpenSubsonic sign-in (ADR core-0021, point 9): its user name and
    /// salted token are tried on the trusted addresses, and kept nowhere. The account is
    /// named by the address that accepted it, never by what the device says, so a Subsonic
    /// sign-in can only ever reach its own profile.
    async fn check_subsonic(&self, creds: SubsonicCredentials) -> Result<CheckedUser, CheckError> {
        let mut rejected = false;
        let mut last_error = None;
        for url in &self.urls {
            let client = SubsonicClient::new(
                self.transport.clone(),
                url.clone(),
                "Encore companion".into(),
                Duration::from_secs(10),
            )
            .signed_in(creds.clone());
            match client.me().await {
                Ok(me) => {
                    return Ok(CheckedUser {
                        user_id: format!("subsonic:{}:{}", server_id_for(url), me.user_name),
                        user_name: me.user_name,
                    });
                }
                Err(SourceError::SessionExpired | SourceError::WrongCredentials) => rejected = true,
                // Not a Subsonic server (a Jellyfin address in the same list): try the next.
                Err(SourceError::Protocol(_)) => {}
                Err(e) => last_error = Some(e.to_string()),
            }
        }
        Err(if rejected {
            CheckError::Rejected
        } else if let Some(e) = last_error {
            CheckError::Unreachable(e)
        } else {
            CheckError::UnknownServer
        })
    }
}

/// The first 16 bytes of SHA-256 over a label and a value, in hex.
fn short_hash(label: &str, value: &str) -> String {
    Sha256::digest(format!("{label}\u{1f}{value}").as_bytes())[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl JellyfinCheck {
    /// A Plex sign-in (ADR core-0023, point 10): the server token is tried on the trusted
    /// Plex server whose ID it was issued for, and kept nowhere. Plex Media Server has no
    /// "who am I" call, so the account is named by that server and a hash of the token:
    /// `plex:<server>:<token hash>`. A token can only ever reach its own profile; two
    /// devices share a profile when the server gave them the same token (ADR, point 10,
    /// says when that is). A server that would take any token can't vouch for one.
    async fn check_plex(
        &self,
        server_id: &str,
        creds: PlexCredentials,
    ) -> Result<CheckedUser, CheckError> {
        let identity = PlexIdentity {
            product: "Encore companion".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            platform: "server".into(),
            client_id: client_identifier("encore-server"),
        };
        let mut rejected = false;
        let mut open = false;
        let mut last_error = None;
        for url in &self.urls {
            let plain = PlexClient::new(
                self.transport.clone(),
                url.clone(),
                identity.clone(),
                Duration::from_secs(10),
            );
            let info = match plain.server_info().await {
                Ok(info) => info,
                // Not a Plex server (a Jellyfin or Subsonic address in the same list).
                Err(SourceError::Protocol(_) | SourceError::NotFound) => continue,
                Err(e) => {
                    last_error = Some(e.to_string());
                    continue;
                }
            };
            // Only the server the token was issued for, and the device signed in to.
            if info.server_id != creds.machine_id || info.server_id != server_id {
                continue;
            }
            match plain.lets_anyone_in().await {
                Ok(false) => {}
                Ok(true) => {
                    open = true;
                    continue;
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    continue;
                }
            }
            let signed = PlexClient::new(
                self.transport.clone(),
                url.clone(),
                identity.clone(),
                Duration::from_secs(10),
            )
            .signed_in(creds.clone());
            match signed.check_sign_in().await {
                Ok(_) => {
                    return Ok(CheckedUser {
                        user_id: format!(
                            "plex:{}:{}",
                            short_hash("plex-server", &info.server_id),
                            short_hash("plex-token", creds.token())
                        ),
                        user_name: "Plex".into(),
                    });
                }
                Err(SourceError::WrongCredentials | SourceError::SessionExpired) => rejected = true,
                Err(e) => last_error = Some(e.to_string()),
            }
        }
        Err(if rejected {
            CheckError::Rejected
        } else if open {
            CheckError::Unverifiable
        } else if let Some(e) = last_error {
            CheckError::Unreachable(e)
        } else {
            CheckError::UnknownServer
        })
    }
}

impl MediaCheck for JellyfinCheck {
    fn check<'a>(&'a self, server_id: &'a str, token: &'a str) -> CheckFuture<'a> {
        Box::pin(async move {
            if let Some(creds) = SubsonicCredentials::from_token_string(token) {
                return self.check_subsonic(creds).await;
            }
            if let Some(creds) = PlexCredentials::from_token_string(token) {
                return self.check_plex(server_id, creds).await;
            }
            let (url, flavor) = self.url_for(server_id).await?;
            // Emby has no /Users/Me: the token's user is the one Emby records for this
            // server's own session, made by the request itself, so one check at a time.
            let _turn = match flavor {
                Flavor::Emby => Some(self.emby_turn.lock().await),
                Flavor::Jellyfin => None,
            };
            let me = self
                .client(&url)
                .with_flavor(flavor)
                .signed_in(token.to_owned(), String::new())
                .me()
                .await
                .map_err(|e| match e {
                    SourceError::SessionExpired | SourceError::WrongCredentials => {
                        CheckError::Rejected
                    }
                    other => CheckError::Unreachable(other.to_string()),
                })?;
            // The user record names its server on current Jellyfin and on Emby; it must
            // agree.
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
            CheckError::Unverifiable => ApiError::Forbidden(
                "This Plex server lets the companion in without a sign-in, so it can't check \
                 who you are. Claim the server, and take the companion's address out of \
                 Plex's networks allowed without auth."
                    .into(),
            ),
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
