//! Jellyfin's HTTP API, as far as Encore uses it. Endpoint names are checked
//! against the 12.0.0 OpenAPI document; `/Artists` and `/MusicGenres` are not used
//! (deprecated and removed in 12.0), artists come from `/Items`.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use url::Url;

use super::address::{AddressPlan, AddressWarning, check_security};
use super::auth::{DeviceInfo, authorization_header};
use super::dto::{
    AuthenticationResult, ItemsResult, PlaylistCreated, PublicSystemInfo, QuickConnectResult,
    UserDto,
};
use crate::error::{Result, SourceError};
use crate::model::{EditOutcome, ServerEdit};
use crate::transport::{Method, Request, Response, Transport};

/// The oldest Jellyfin this adapter supports (lyrics endpoint, `MediaBrowser` auth).
pub const MIN_VERSION: (u32, u32, u32) = (10, 10, 0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub base_url: Url,
    pub name: String,
    pub version: String,
    pub server_id: String,
    pub warning: Option<AddressWarning>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub user_name: String,
    pub server_id: String,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("user_name", &self.user_name)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickConnectCode {
    /// The 6 characters the user approves on another device.
    pub code: String,
    /// Kept by the app to poll; not shown.
    pub secret: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamQuality {
    /// The file as it is (gapless info intact).
    Original,
    /// Ask the server to keep the stream under this bitrate (it transcodes if needed).
    Capped { kbps: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamCodec {
    Aac,
    Opus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRequest {
    pub quality: StreamQuality,
    pub codec: StreamCodec,
    /// Containers this device plays without transcoding (for example flac, mp3, m4a).
    pub direct_play_containers: Vec<String>,
}

/// Where the player loads a song from. Use `url` plus `headers` when the player can
/// send headers; `url_with_api_key` otherwise. Never log either.
#[derive(Clone, PartialEq, Eq)]
pub struct StreamInfo {
    pub url: String,
    pub url_with_api_key: String,
    pub headers: Vec<(String, String)>,
    pub may_transcode: bool,
}

impl fmt::Debug for StreamInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamInfo")
            .field("may_transcode", &self.may_transcode)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ArtworkInfo {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl fmt::Debug for ArtworkInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArtworkInfo").finish_non_exhaustive()
    }
}

/// Who a token belongs to (`GET /Users/Me`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    pub user_id: String,
    pub user_name: String,
    /// The server's ID as the user record gives it (may be empty on old servers).
    pub server_id: String,
    /// Set when the user has a profile image (`user_image`).
    pub image_tag: Option<String>,
}
/// Lyrics as the server sent them; Phase 3 parses them into the lyrics model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLyrics {
    /// "jellyfin-json": Jellyfin's LyricDto (lines with start ticks and word cues).
    pub format: String,
    pub body: String,
}

pub(crate) fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim().split(['.', '-', '+']).map(|p| {
        p.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
    });
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

pub struct JellyfinClient {
    transport: Arc<dyn Transport>,
    base: Url,
    device: DeviceInfo,
    timeout: Duration,
    session: Option<(String, String)>,
}

impl fmt::Debug for JellyfinClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JellyfinClient")
            .field("signed_in", &self.session.is_some())
            .finish_non_exhaustive()
    }
}

impl JellyfinClient {
    pub fn new(
        transport: Arc<dyn Transport>,
        base: Url,
        device: DeviceInfo,
        timeout: Duration,
    ) -> JellyfinClient {
        JellyfinClient {
            transport,
            base,
            device,
            timeout,
            session: None,
        }
    }

    /// A client that sends `token` and acts as `user_id`.
    pub fn signed_in(mut self, token: String, user_id: String) -> JellyfinClient {
        self.session = Some((token, user_id));
        self
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// The token, for one purpose only: the companion server's sign-in, which
    /// sends it once so the companion can check it with this server (plan section
    /// 5). Never log it or store it anywhere but the keychain.
    pub fn token_for_companion_sign_in(&self) -> Option<&str> {
        self.token()
    }

    fn token(&self) -> Option<&str> {
        self.session.as_ref().map(|(t, _)| t.as_str())
    }

    pub(crate) fn user_id(&self) -> Result<&str> {
        self.session
            .as_ref()
            .map(|(_, u)| u.as_str())
            .ok_or(SourceError::SessionExpired)
    }

    pub(crate) fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let mut u = Url::parse(&format!(
            "{}{path}",
            self.base.as_str().trim_end_matches('/')
        ))
        .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
        if !query.is_empty() {
            let mut q = u.query_pairs_mut();
            for (k, v) in query {
                q.append_pair(k, v);
            }
        }
        Ok(u)
    }

    fn auth_header(&self) -> (String, String) {
        (
            "Authorization".to_owned(),
            authorization_header(&self.device, self.token()),
        )
    }

    async fn send(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
    ) -> Result<Response> {
        let mut headers = vec![self.auth_header()];
        if body.is_some() {
            headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
        }
        self.transport
            .send(Request {
                method,
                url,
                headers,
                body: body.map(|b| serde_json::to_vec(&b).unwrap_or_default()),
                timeout: self.timeout,
            })
            .await
    }

    /// Turn an error status into a plain reason. `unauthorized` is what a 401 means
    /// for this call.
    fn check(resp: Response, unauthorized: SourceError) -> Result<Response> {
        match resp.status {
            200..=299 => Ok(resp),
            401 | 403 => Err(unauthorized),
            404 => Err(SourceError::NotFound),
            status => Err(SourceError::Server {
                status,
                message: resp.text().chars().take(500).collect(),
            }),
        }
    }

    pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let resp = self.send(Method::Get, self.url(path, query)?, None).await?;
        Self::check(resp, SourceError::SessionExpired)?.json()
    }

    /// `GET /System/Info/Public`: is this Jellyfin, and a version we support?
    pub async fn public_info(&self) -> Result<ServerInfo> {
        let info: PublicSystemInfo =
            self.get_json("/System/Info/Public", &[])
                .await
                .map_err(|e| match e {
                    SourceError::Protocol(_) | SourceError::NotFound => SourceError::NotJellyfin,
                    other => other,
                })?;
        let product = info.product_name.unwrap_or_default();
        let id = info.id.unwrap_or_default();
        if !product.to_ascii_lowercase().contains("jellyfin") || id.is_empty() {
            return Err(SourceError::NotJellyfin);
        }
        let version = info.version.unwrap_or_default();
        let parsed = parse_version(&version).ok_or(SourceError::NotJellyfin)?;
        if parsed < MIN_VERSION {
            let (a, b, c) = MIN_VERSION;
            return Err(SourceError::UnsupportedVersion {
                found: version,
                minimum: format!("{a}.{b}.{c}"),
            });
        }
        Ok(ServerInfo {
            warning: check_security(&self.base)?,
            base_url: self.base.clone(),
            name: info.server_name.unwrap_or_else(|| "Jellyfin".to_owned()),
            version,
            server_id: id,
        })
    }

    fn session_from(result: AuthenticationResult) -> Result<Session> {
        let user = result
            .user
            .ok_or_else(|| SourceError::Protocol("no user in the sign-in reply".into()))?;
        Ok(Session {
            token: result
                .access_token
                .ok_or_else(|| SourceError::Protocol("no token in the sign-in reply".into()))?,
            user_id: user
                .id
                .ok_or_else(|| SourceError::Protocol("no user id".into()))?,
            user_name: user.name.unwrap_or_default(),
            server_id: result.server_id.unwrap_or_default(),
        })
    }

    /// `POST /Users/AuthenticateByName`. The password is sent once and never kept.
    pub async fn sign_in(&self, username: &str, password: &str) -> Result<Session> {
        let body = serde_json::json!({ "Username": username, "Pw": password });
        let resp = self
            .send(
                Method::Post,
                self.url("/Users/AuthenticateByName", &[])?,
                Some(body),
            )
            .await?;
        Self::session_from(Self::check(resp, SourceError::WrongCredentials)?.json()?)
    }

    /// `GET /QuickConnect/Enabled`.
    pub async fn quick_connect_enabled(&self) -> Result<bool> {
        self.get_json("/QuickConnect/Enabled", &[]).await
    }

    /// `POST /QuickConnect/Initiate`: a code to approve on another device.
    pub async fn quick_connect_start(&self) -> Result<QuickConnectCode> {
        let resp = self
            .send(Method::Post, self.url("/QuickConnect/Initiate", &[])?, None)
            .await?;
        let r: QuickConnectResult = Self::check(resp, SourceError::QuickConnectDisabled)?.json()?;
        Ok(QuickConnectCode {
            code: r
                .code
                .ok_or_else(|| SourceError::Protocol("no Quick Connect code".into()))?,
            secret: r
                .secret
                .ok_or_else(|| SourceError::Protocol("no Quick Connect secret".into()))?,
        })
    }

    /// `GET /QuickConnect/Connect`: has the code been approved yet?
    pub async fn quick_connect_poll(&self, secret: &str) -> Result<bool> {
        let resp = self
            .send(
                Method::Get,
                self.url("/QuickConnect/Connect", &[("secret", secret)])?,
                None,
            )
            .await?;
        let r: QuickConnectResult = match Self::check(resp, SourceError::QuickConnectDisabled) {
            Err(SourceError::NotFound) => return Err(SourceError::QuickConnectExpired),
            other => other?.json()?,
        };
        Ok(r.authenticated.unwrap_or(false))
    }

    /// `POST /Users/AuthenticateWithQuickConnect` once the code is approved.
    pub async fn quick_connect_finish(&self, secret: &str) -> Result<Session> {
        let body = serde_json::json!({ "Secret": secret });
        let resp = self
            .send(
                Method::Post,
                self.url("/Users/AuthenticateWithQuickConnect", &[])?,
                Some(body),
            )
            .await?;
        let checked = match Self::check(resp, SourceError::QuickConnectExpired) {
            Err(SourceError::NotFound) => return Err(SourceError::QuickConnectExpired),
            other => other?,
        };
        Self::session_from(checked.json()?)
    }

    /// `POST /Sessions/Logout`: the server forgets this device's token (board 18,
    /// "Sign out"). An already-expired session counts as signed out.
    pub async fn sign_out(&self) -> Result<()> {
        let resp = self
            .send(Method::Post, self.url("/Sessions/Logout", &[])?, None)
            .await?;
        match Self::check(resp, SourceError::SessionExpired) {
            Ok(_) | Err(SourceError::SessionExpired) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// One page of `GET /Items` for the signed-in user.
    pub async fn items(&self, query: &[(&str, &str)]) -> Result<ItemsResult> {
        let user = self.user_id()?.to_owned();
        let mut q: Vec<(&str, &str)> = vec![("userId", user.as_str())];
        q.extend_from_slice(query);
        let resp = self
            .send(Method::Get, self.url("/Items", &q)?, None)
            .await?;
        let resp = Self::check(resp, SourceError::SessionExpired)?;
        let mut page: ItemsResult = resp.json()?;
        page.server_date_utc_ms = resp.server_date_utc_ms;
        Ok(page)
    }

    /// `GET /Playlists/{id}/Items`: a playlist's songs with their entry IDs.
    pub async fn playlist_items(&self, playlist_id: &str) -> Result<ItemsResult> {
        let user = self.user_id()?.to_owned();
        self.get_json(
            &format!("/Playlists/{playlist_id}/Items"),
            &[
                ("userId", user.as_str()),
                ("enableImages", "false"),
                ("enableUserData", "false"),
            ],
        )
        .await
    }

    /// `GET /Audio/{id}/Lyrics`. None when the song has no lyrics.
    pub async fn lyrics(&self, item_id: &str) -> Result<Option<RawLyrics>> {
        let resp = self
            .send(
                Method::Get,
                self.url(&format!("/Audio/{item_id}/Lyrics"), &[])?,
                None,
            )
            .await?;
        match Self::check(resp, SourceError::SessionExpired) {
            Ok(r) => Ok(Some(RawLyrics {
                format: "jellyfin-json".into(),
                body: r.text(),
            })),
            Err(SourceError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Where to stream a song from (plan section 8: original on fast networks, a
    /// transcode under a cap otherwise).
    pub fn stream(&self, item_id: &str, req: &StreamRequest) -> Result<StreamInfo> {
        let token = self.token().ok_or(SourceError::SessionExpired)?.to_owned();
        let user = self.user_id()?.to_owned();
        let (url, may_transcode) = match req.quality {
            StreamQuality::Original => (
                self.url(&format!("/Audio/{item_id}/stream"), &[("static", "true")])?,
                false,
            ),
            StreamQuality::Capped { kbps } => {
                let bits = (u64::from(kbps) * 1000).to_string();
                let (codec, container) = match req.codec {
                    StreamCodec::Aac => ("aac", "aac"),
                    StreamCodec::Opus => ("opus", "ogg"),
                };
                let containers = req.direct_play_containers.join(",");
                let url = self.url(
                    &format!("/Audio/{item_id}/universal"),
                    &[
                        ("userId", user.as_str()),
                        ("deviceId", self.device.device_id.as_str()),
                        ("container", containers.as_str()),
                        ("maxStreamingBitrate", bits.as_str()),
                        ("audioBitRate", bits.as_str()),
                        ("audioCodec", codec),
                        ("transcodingContainer", container),
                        ("transcodingProtocol", "http"),
                    ],
                )?;
                (url, true)
            }
        };
        let mut keyed = url.clone();
        keyed.query_pairs_mut().append_pair("ApiKey", &token);
        Ok(StreamInfo {
            url: url.to_string(),
            url_with_api_key: keyed.to_string(),
            headers: vec![self.auth_header()],
            may_transcode,
        })
    }

    /// Send one playlist or favourite write (plan section 26). Jellyfin 12.0:
    /// `POST /Playlists`, `POST /Playlists/{id}/Items?ids=&position=`,
    /// `DELETE /Playlists/{id}/Items?entryIds=`,
    /// `POST /Playlists/{id}/Items/{entryId}/Move/{index}`, `POST /Playlists/{id}`
    /// (rename), `DELETE /Items/{id}`, `POST`/`DELETE /UserFavoriteItems/{id}`.
    pub async fn edit(&self, edit: &ServerEdit) -> Result<EditOutcome> {
        let user = self.user_id()?.to_owned();
        let (method, url, body) = match edit {
            ServerEdit::CreatePlaylist { name, item_ids } => (
                Method::Post,
                self.url("/Playlists", &[])?,
                Some(serde_json::json!({
                    "Name": name,
                    "Ids": item_ids,
                    "UserId": user,
                    "MediaType": "Audio",
                })),
            ),
            ServerEdit::AddToPlaylist {
                playlist_id,
                item_ids,
                position,
            } => {
                let ids = item_ids.join(",");
                let pos = position.map(|p| p.to_string());
                let mut q = vec![("ids", ids.as_str()), ("userId", user.as_str())];
                if let Some(p) = &pos {
                    q.push(("position", p.as_str()));
                }
                (
                    Method::Post,
                    self.url(&format!("/Playlists/{playlist_id}/Items"), &q)?,
                    None,
                )
            }
            ServerEdit::RemoveFromPlaylist {
                playlist_id,
                entry_ids,
            } => {
                let ids = entry_ids.join(",");
                (
                    Method::Delete,
                    self.url(
                        &format!("/Playlists/{playlist_id}/Items"),
                        &[("entryIds", ids.as_str())],
                    )?,
                    None,
                )
            }
            ServerEdit::MoveInPlaylist {
                playlist_id,
                entry_id,
                new_index,
            } => (
                Method::Post,
                self.url(
                    &format!("/Playlists/{playlist_id}/Items/{entry_id}/Move/{new_index}"),
                    &[],
                )?,
                None,
            ),
            ServerEdit::RenamePlaylist { playlist_id, name } => (
                Method::Post,
                self.url(&format!("/Playlists/{playlist_id}"), &[])?,
                Some(serde_json::json!({ "Name": name })),
            ),
            ServerEdit::DeletePlaylist { playlist_id } => (
                Method::Delete,
                self.url(&format!("/Items/{playlist_id}"), &[])?,
                None,
            ),
            ServerEdit::SetFavorite { item_id, favorite } => (
                if *favorite {
                    Method::Post
                } else {
                    Method::Delete
                },
                self.url(
                    &format!("/UserFavoriteItems/{item_id}"),
                    &[("userId", user.as_str())],
                )?,
                None,
            ),
        };
        let resp = self.send(method, url, body).await?;
        let resp = Self::check(resp, SourceError::SessionExpired)?;
        Ok(match edit {
            ServerEdit::CreatePlaylist { .. } => {
                let created: PlaylistCreated = resp.json()?;
                EditOutcome::Created(
                    created.id.ok_or_else(|| {
                        SourceError::Protocol("no ID for the new playlist".into())
                    })?,
                )
            }
            _ => EditOutcome::Done,
        })
    }

    /// The IDs of every song this user marked as a favourite, page by page.
    pub async fn favorite_ids(&self, page_size: u32) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        let mut start: u64 = 0;
        let limit = page_size.max(1).to_string();
        loop {
            let start_s = start.to_string();
            let page = self
                .items(&[
                    ("includeItemTypes", "Audio"),
                    ("recursive", "true"),
                    ("filters", "IsFavorite"),
                    ("enableImages", "false"),
                    ("enableUserData", "false"),
                    ("enableTotalRecordCount", "true"),
                    ("sortBy", "SortName"),
                    ("startIndex", start_s.as_str()),
                    ("limit", limit.as_str()),
                ])
                .await?;
            let items = page.items.unwrap_or_default();
            let n = items.len() as u64;
            ids.extend(items.into_iter().filter_map(|i| i.id));
            start += n;
            if n == 0 || start >= page.total_record_count.unwrap_or(0).max(0) as u64 {
                break;
            }
        }
        Ok(ids)
    }

    /// How often this user played each song in any app, and when last: Jellyfin's
    /// own user data (`filters=IsPlayed`, `enableUserData`), the smart-picks prior
    /// for a profile with no Encore history yet. (item ID, plays, last played).
    pub async fn play_counts(&self, page_size: u32) -> Result<Vec<(String, u32, Option<i64>)>> {
        let mut out = Vec::new();
        let mut start: u64 = 0;
        let limit = page_size.max(1).to_string();
        loop {
            let start_s = start.to_string();
            let page = self
                .items(&[
                    ("includeItemTypes", "Audio"),
                    ("recursive", "true"),
                    ("filters", "IsPlayed"),
                    ("enableImages", "false"),
                    ("enableUserData", "true"),
                    ("enableTotalRecordCount", "true"),
                    ("sortBy", "SortName"),
                    ("startIndex", start_s.as_str()),
                    ("limit", limit.as_str()),
                ])
                .await?;
            let items = page.items.unwrap_or_default();
            let n = items.len() as u64;
            for i in items {
                let (Some(id), Some(u)) = (i.id, i.user_data) else {
                    continue;
                };
                let last = u
                    .last_played_date
                    .as_deref()
                    .and_then(super::time::parse_utc_ms);
                out.push((
                    id,
                    u.play_count.unwrap_or(u32::from(u.played.unwrap_or(false))),
                    last,
                ));
            }
            start += n;
            if n == 0 || start >= page.total_record_count.unwrap_or(0).max(0) as u64 {
                break;
            }
        }
        Ok(out)
    }

    /// The primary image as PNG, at most `size_px` on its longer side, for colour
    /// sampling (board 04). PNG because it is lossless: every device decodes the
    /// same pixels, so every device makes the same palette. `tag` (the image tag)
    /// lets the server and caches tell covers apart. None when there is no cover.
    pub async fn cover_png(
        &self,
        item_id: &str,
        size_px: u32,
        tag: Option<&str>,
    ) -> Result<Option<Vec<u8>>> {
        let px = size_px.clamp(8, 1024).to_string();
        let mut q = vec![
            ("maxWidth", px.as_str()),
            ("maxHeight", px.as_str()),
            ("format", "Png"),
        ];
        if let Some(t) = tag {
            q.push(("tag", t));
        }
        let resp = self
            .send(
                Method::Get,
                self.url(&format!("/Items/{item_id}/Images/Primary"), &q)?,
                None,
            )
            .await?;
        match Self::check(resp, SourceError::SessionExpired) {
            Ok(r) => Ok(Some(r.body)),
            Err(SourceError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The primary image at exactly `size_px` square (plan section 4).
    /// `GET /Users/Me`: who this client's token belongs to. A 401 means the token
    /// is not (or no longer) valid: `SessionExpired`.
    pub async fn me(&self) -> Result<Me> {
        let user: UserDto = self.get_json("/Users/Me", &[]).await?;
        Ok(Me {
            user_id: user
                .id
                .filter(|i| !i.is_empty())
                .ok_or_else(|| SourceError::Protocol("no user id in /Users/Me".into()))?,
            user_name: user.name.unwrap_or_default(),
            server_id: user.server_id.unwrap_or_default(),
            image_tag: user.primary_image_tag.filter(|t| !t.is_empty()),
        })
    }

    /// Where a user's profile image is, square, for avatars (board 14). Never log
    /// the headers.
    pub fn user_image(&self, user_id: &str, image_tag: &str, size_px: u32) -> Result<ArtworkInfo> {
        let px = size_px.clamp(16, 1024).to_string();
        let url = self.url(
            &format!("/Users/{user_id}/Images/Primary"),
            &[
                ("tag", image_tag),
                ("fillWidth", px.as_str()),
                ("fillHeight", px.as_str()),
                ("quality", "90"),
            ],
        )?;
        Ok(ArtworkInfo {
            url: url.to_string(),
            headers: vec![self.auth_header()],
        })
    }
    pub fn artwork(&self, item_id: &str, size_px: u32) -> Result<ArtworkInfo> {
        let px = size_px.clamp(16, 4096).to_string();
        let url = self.url(
            &format!("/Items/{item_id}/Images/Primary"),
            &[
                ("fillWidth", px.as_str()),
                ("fillHeight", px.as_str()),
                ("quality", "90"),
            ],
        )?;
        Ok(ArtworkInfo {
            url: url.to_string(),
            headers: vec![self.auth_header()],
        })
    }
}

/// Try each address of a plan in order and return the first Jellyfin that
/// answers. When none does, the most useful reason wins: an untrusted certificate
/// (the user can trust it), then "not Jellyfin" or "too old", then why the first
/// address failed.
pub async fn probe(
    transport: Arc<dyn Transport>,
    plan: &AddressPlan,
    device: &DeviceInfo,
    timeout: Duration,
) -> Result<ServerInfo> {
    let mut first_error: Option<SourceError> = None;
    let mut best: Option<SourceError> = None;
    for base in &plan.candidates {
        let client = JellyfinClient::new(transport.clone(), base.clone(), device.clone(), timeout);
        match client.public_info().await {
            Ok(info) => return Ok(info),
            Err(e) => {
                let rank = |e: &SourceError| match e {
                    SourceError::CertificateUntrusted { .. } => 3,
                    SourceError::NotJellyfin | SourceError::UnsupportedVersion { .. } => 2,
                    _ => 0,
                };
                if rank(&e) > best.as_ref().map(rank).unwrap_or(0) {
                    best = Some(e.clone());
                }
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
    }
    Err(best
        .or(first_error)
        .unwrap_or_else(|| SourceError::InvalidAddress("no address to try".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("12.0.0"), Some((12, 0, 0)));
        assert_eq!(parse_version("10.10.7"), Some((10, 10, 7)));
        assert_eq!(parse_version("10.11.0-rc2"), Some((10, 11, 0)));
        assert_eq!(parse_version("12.4"), Some((12, 4, 0)));
        assert!(parse_version("10.9.11").unwrap() < MIN_VERSION);
        assert_eq!(parse_version("x"), None);
    }
}
