//! The Subsonic API (with OpenSubsonic's additions), as far as Encore uses it. Every
//! endpoint and field here is checked against opensubsonic.github.io/docs/ (endpoints
//! and responses pages); see docs/adr/core-0021-opensubsonic-source.md.
//!
//! - Every call is `GET /rest/<endpoint>.view` with `v`, `c`, `f=json` and the
//!   sign-in in the query (Subsonic has no header auth). The `.view` suffix is the
//!   form every Subsonic server answers; Navidrome and gonic also take it without.
//! - A Subsonic error comes back inside the `subsonic-response` envelope (usually with
//!   HTTP 200), or as an XML document from the binary endpoints (`stream`, `download`,
//!   `getCoverArt`, `getAvatar`). Both are read and mapped to `SourceError`.
//! - URLs here carry the sign-in, so they are never logged: `Request`'s Debug shows
//!   only the path, and `StreamInfo`, `ArtworkInfo` and `SubsonicUser` hide theirs.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use super::dto::{
    ExtensionsDto, LyricsListDto, PlainLyricsDto, PlaylistDto, PlaylistsDto, UserDto,
};
use super::map;
use super::md5::md5_hex;
use crate::error::{Result, SourceError};
use crate::jellyfin::{
    AddressWarning, ArtworkInfo, RawLyrics, StreamCodec, StreamInfo, StreamQuality, StreamRequest,
    address::check_security,
};
use crate::model::{EditOutcome, PlaylistEntry, ServerEdit};
use crate::transport::{Method, Request, Response, Transport};

/// The protocol version sent as `v`. 1.15.0 is what gonic, Airsonic and
/// Airsonic-Advanced report and below Navidrome's 1.16.1; a Subsonic-derived server
/// answers error 30 ("server must upgrade") to a client version above its own, so the
/// lowest version with everything used here (token sign-in, 1.13.0; `createPlaylist`
/// returning the playlist, 1.14.0) plus that margin is the safe choice.
pub const API_VERSION: &str = "1.15.0";

/// Song IDs or positions per request, so a long playlist never makes an overlong URL.
const IDS_PER_REQUEST: usize = 150;

/// What `/rest/ping` says about a server (no sign-in needed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsonicInfo {
    pub base_url: Url,
    /// A hash of the canonical address (ADR core-0021 point 4): Subsonic servers have
    /// no ID of their own.
    pub server_id: String,
    /// The server's kind ("Navidrome", "gonic"), or "Subsonic" when it doesn't say.
    pub name: String,
    /// The server's own version when it says (OpenSubsonic), else the API version.
    pub version: String,
    pub open_subsonic: bool,
    /// The OpenSubsonic extensions it offers (`songLyrics`, `apiKeyAuthentication`...).
    pub extensions: Vec<String>,
    /// Set for plain http on the local network (as for Jellyfin, board 21).
    pub warning: Option<AddressWarning>,
}

#[derive(Clone, PartialEq, Eq)]
enum Auth {
    /// `t = md5(password + s)`.
    Token { token: String, salt: String },
    /// OpenSubsonic `apiKeyAuthentication`.
    ApiKey(String),
}

/// A saved sign-in: the user name and either a token with its salt or an API key.
/// Never the password. Keep it in the keychain only.
#[derive(Clone, PartialEq, Eq)]
pub struct SubsonicCredentials {
    /// The user name (for an API key, the one `tokenInfo` gave; may be empty if the
    /// server didn't say).
    pub user: String,
    auth: Auth,
}

impl fmt::Debug for SubsonicCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubsonicCredentials")
            .field("user", &self.user)
            .field("api_key", &self.uses_api_key())
            .finish_non_exhaustive()
    }
}

const TOKEN_PREFIX: &str = "subsonic1:";

impl SubsonicCredentials {
    pub fn uses_api_key(&self) -> bool {
        matches!(self.auth, Auth::ApiKey(_))
    }

    /// The query parameters that sign a request in: `u`, `t`, `s`, or `apiKey` alone
    /// (the API key extension forbids `u` beside it). For the companion server's
    /// sign-in check (ADR core-0021 point 9) and this client. Never log them.
    pub fn auth_params(&self) -> Vec<(&'static str, &str)> {
        match &self.auth {
            Auth::Token { token, salt } => {
                vec![("u", self.user.as_str()), ("t", token), ("s", salt)]
            }
            Auth::ApiKey(key) => vec![("apiKey", key.as_str())],
        }
    }

    /// One string for the keychain: `subsonic1:` and form-encoded fields (`u`, then
    /// `t` and `s`, or `k` for an API key). Never holds the password.
    pub fn to_token_string(&self) -> String {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        s.append_pair("u", &self.user);
        match &self.auth {
            Auth::Token { token, salt } => {
                s.append_pair("t", token);
                s.append_pair("s", salt);
            }
            Auth::ApiKey(key) => {
                s.append_pair("k", key);
            }
        }
        format!("{TOKEN_PREFIX}{}", s.finish())
    }

    pub fn from_token_string(s: &str) -> Option<SubsonicCredentials> {
        let fields: HashMap<String, String> =
            url::form_urlencoded::parse(s.strip_prefix(TOKEN_PREFIX)?.as_bytes())
                .into_owned()
                .collect();
        let user = fields.get("u").cloned().unwrap_or_default();
        let auth = match (fields.get("t"), fields.get("s"), fields.get("k")) {
            (Some(t), Some(salt), None)
                if !t.is_empty() && !salt.is_empty() && !user.is_empty() =>
            {
                Auth::Token {
                    token: t.clone(),
                    salt: salt.clone(),
                }
            }
            (None, None, Some(k)) if !k.is_empty() => Auth::ApiKey(k.clone()),
            _ => return None,
        };
        Some(SubsonicCredentials { user, auth })
    }
}

/// The signed-in user (`getUser`).
#[derive(Clone, PartialEq, Eq)]
pub struct SubsonicUser {
    pub user_name: String,
    /// `getAvatar` for this user, when the server answered it with a picture. The URL
    /// carries the sign-in: never log it.
    pub avatar_url: Option<Url>,
}

impl fmt::Debug for SubsonicUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubsonicUser")
            .field("user_name", &self.user_name)
            .field("has_avatar", &self.avatar_url.is_some())
            .finish()
    }
}

/// The source's ID: SHA-256 of the canonical address (scheme, lower-case host,
/// port, path without a trailing slash), first 16 bytes in hex.
pub fn server_id_for(base: &Url) -> String {
    let host = base.host_str().unwrap_or("").to_ascii_lowercase();
    let port = base
        .port_or_known_default()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let canonical = format!(
        "{}://{}:{}{}",
        base.scheme(),
        host,
        port,
        base.path().trim_end_matches('/')
    );
    Sha256::digest(canonical.as_bytes())[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A fresh salt (16 hex characters). The protocol only needs it unpredictable and at
/// least six characters; it is not a key. It comes from the standard library's
/// randomly seeded hasher keys and the clock, mixed with SHA-256, so no random-number
/// crate is needed.
fn random_salt() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    for i in 0..4u64 {
        let mut s = std::collections::hash_map::RandomState::new().build_hasher();
        s.write_u64(i);
        s.write_u128(nanos);
        h.update(s.finish().to_le_bytes());
    }
    h.update(nanos.to_le_bytes());
    h.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A Subsonic error code to the crate's error. `unauthorized` is what a refused
/// sign-in means for this call (WrongCredentials when signing in, SessionExpired
/// after). Codes: opensubsonic.github.io/docs/responses/error/.
pub(crate) fn map_code(
    code: i64,
    message: &str,
    server_version: &str,
    unauthorized: &SourceError,
) -> SourceError {
    let message = format!(
        "Subsonic error {code}: {}",
        message.chars().take(300).collect::<String>()
    );
    match code {
        // Wrong user name or password; invalid API key.
        40 | 44 => unauthorized.clone(),
        // Token sign-in not supported (LDAP), this sign-in kind not supported, or
        // conflicting kinds: a sign-in the server won't take. While signing in that is
        // shown with the server's words; later it means "sign in again".
        41..=43 => match unauthorized {
            SourceError::WrongCredentials => SourceError::Server {
                status: 401,
                message,
            },
            other => other.clone(),
        },
        // Not allowed for this user: as HTTP 403, so the outbox drops the write (and
        // makes a playlist it can't change into the user's own).
        50 => SourceError::Server {
            status: 403,
            message,
        },
        70 => SourceError::NotFound,
        30 => SourceError::UnsupportedVersion {
            found: server_version.to_owned(),
            minimum: API_VERSION.to_owned(),
        },
        // Missing parameter, client too old: requests this client got wrong.
        10 | 20 => SourceError::Server {
            status: 400,
            message,
        },
        _ => SourceError::Server {
            status: 500,
            message,
        },
    }
}

/// The `subsonic-response` object of a JSON reply, if it is one.
fn envelope_of(body: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(body)
        .ok()?
        .get_mut("subsonic-response")
        .map(Value::take)
}

/// The error code and message of an XML error document (what the binary endpoints
/// send), if `body` is one.
fn xml_error(body: &[u8]) -> Option<(i64, String)> {
    let text = std::str::from_utf8(body.get(..body.len().min(64 * 1024))?).ok()?;
    if !text.trim_start().starts_with('<') || !text.contains("subsonic-response") {
        return None;
    }
    let attr = |name: &str| -> Option<String> {
        let start = text.find("<error")?;
        let tag = &text[start..];
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
        let rest = &tag[at..];
        Some(rest[..rest.find('"')?].to_owned())
    };
    let code = attr("code").and_then(|c| c.parse().ok()).unwrap_or(0);
    Some((code, attr("message").unwrap_or_default()))
}

/// Whether a binary endpoint's body is really an error document.
pub(crate) fn is_error_document(body: &[u8]) -> bool {
    let head = &body[..body.len().min(512)];
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start();
    (t.starts_with('<') || t.starts_with('{')) && text.contains("subsonic-response")
}

/// Check a reply: the envelope when its status is "ok", else the mapped error.
pub(crate) fn check(resp: &Response, unauthorized: &SourceError) -> Result<Value> {
    if let Some(env) = envelope_of(&resp.body) {
        if env.get("status").and_then(Value::as_str) == Some("ok") {
            return Ok(env);
        }
        let err = env.get("error");
        let code = err
            .and_then(|e| e.get("code"))
            .and_then(|c| {
                c.as_i64()
                    .or_else(|| c.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(0);
        let message = err
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let version = env
            .get("serverVersion")
            .or_else(|| env.get("version"))
            .and_then(Value::as_str)
            .unwrap_or("");
        return Err(map_code(code, message, version, unauthorized));
    }
    if let Some((code, message)) = xml_error(&resp.body) {
        return Err(map_code(code, &message, "", unauthorized));
    }
    Err(match resp.status {
        401 | 403 => unauthorized.clone(),
        404 => SourceError::NotFound,
        200..=299 => SourceError::Protocol("the reply isn't a Subsonic response".into()),
        status => SourceError::Server {
            status,
            message: resp.text().chars().take(500).collect(),
        },
    })
}

/// `key` of an envelope read as `T`; missing reads as `T::default()`.
fn payload<T: DeserializeOwned + Default>(env: &Value, key: &str) -> Result<T> {
    match env.get(key) {
        None | Some(Value::Null) => Ok(T::default()),
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|e| SourceError::Protocol(e.to_string()))
        }
    }
}

/// Find where an entry is now. Its ID is `"<position>:<song ID>"` (map module): the
/// position is used when that song is still there, else the same song nearest to it,
/// so an ID made before an earlier edit moved things still finds its song. `taken`
/// positions are skipped (one entry, one position).
pub(crate) fn resolve_entry(
    entry_id: &str,
    entries: &[PlaylistEntry],
    taken: &[usize],
) -> Option<usize> {
    let (pos, song) = map::parse_entry_id(entry_id)?;
    let free = |i: &usize| !taken.contains(i) && entries[*i].track_item_id == song;
    if pos < entries.len() && free(&pos) {
        return Some(pos);
    }
    (0..entries.len())
        .filter(free)
        .min_by_key(|i| i.abs_diff(pos))
}

pub struct SubsonicClient {
    transport: Arc<dyn Transport>,
    base: Url,
    client_name: String,
    timeout: Duration,
    creds: Option<SubsonicCredentials>,
    /// The server's OpenSubsonic extensions, asked once (`getOpenSubsonicExtensions`
    /// is public) when first needed.
    extensions: Mutex<Option<Arc<Vec<String>>>>,
    /// Album and playlist IDs to their `coverArt` IDs, learned in a sync. Where an ID
    /// isn't known, the item ID itself is asked for (Navidrome and Subsonic resolve it).
    cover_ids: Mutex<HashMap<String, String>>,
}

impl fmt::Debug for SubsonicClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubsonicClient")
            .field("signed_in", &self.creds.is_some())
            .finish_non_exhaustive()
    }
}

/// Lock a mutex even if a panicking thread poisoned it (the data is a cache).
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl SubsonicClient {
    /// `base_url`: the server's address as the user typed it, without `/rest`.
    pub fn new(
        transport: Arc<dyn Transport>,
        base_url: Url,
        client_name: String,
        timeout: Duration,
    ) -> SubsonicClient {
        SubsonicClient {
            transport,
            base: base_url,
            client_name,
            timeout,
            creds: None,
            extensions: Mutex::new(None),
            cover_ids: Mutex::new(HashMap::new()),
        }
    }

    /// A client that signs every request in with `creds`.
    pub fn signed_in(mut self, creds: SubsonicCredentials) -> SubsonicClient {
        self.creds = Some(creds);
        self
    }

    /// The extensions `server_info` already found, so they aren't asked again.
    pub fn with_extensions(self, extensions: Vec<String>) -> SubsonicClient {
        *lock(&self.extensions) = Some(Arc::new(extensions));
        self
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    pub fn credentials(&self) -> Option<&SubsonicCredentials> {
        self.creds.as_ref()
    }

    pub(crate) fn user_name(&self) -> &str {
        self.creds.as_ref().map(|c| c.user.as_str()).unwrap_or("")
    }

    pub(crate) fn remember_cover(&self, item_id: &str, cover_id: Option<&str>) {
        if let Some(c) = cover_id.filter(|c| !c.is_empty() && *c != item_id) {
            lock(&self.cover_ids).insert(item_id.to_owned(), c.to_owned());
        }
    }

    fn cover_id(&self, item_id: &str) -> String {
        lock(&self.cover_ids)
            .get(item_id)
            .cloned()
            .unwrap_or_else(|| item_id.to_owned())
    }

    /// `<base>/rest/<endpoint>.view?<query>&v&c&f=json&<sign-in>`.
    fn url_with(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
        creds: Option<&SubsonicCredentials>,
    ) -> Result<Url> {
        let mut u = Url::parse(&format!(
            "{}/rest/{endpoint}.view",
            self.base.as_str().trim_end_matches('/')
        ))
        .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
        {
            let mut q = u.query_pairs_mut();
            for (k, v) in query {
                q.append_pair(k, v);
            }
            q.append_pair("v", API_VERSION);
            q.append_pair("c", &self.client_name);
            q.append_pair("f", "json");
            if let Some(c) = creds {
                for (k, v) in c.auth_params() {
                    q.append_pair(k, v);
                }
            }
        }
        Ok(u)
    }

    /// A signed-in URL; SessionExpired when there is no sign-in.
    pub(crate) fn url(&self, endpoint: &str, query: &[(&str, &str)]) -> Result<Url> {
        let creds = self.creds.as_ref().ok_or(SourceError::SessionExpired)?;
        self.url_with(endpoint, query, Some(creds))
    }

    async fn get(&self, url: Url) -> Result<Response> {
        self.transport
            .send(Request {
                method: Method::Get,
                url,
                headers: Vec::new(),
                body: None,
                timeout: self.timeout,
            })
            .await
    }

    /// One signed-in API call; the envelope on success.
    pub(crate) async fn call(&self, endpoint: &str, query: &[(&str, &str)]) -> Result<Value> {
        let resp = self.get(self.url(endpoint, query)?).await?;
        check(&resp, &SourceError::SessionExpired)
    }

    /// One signed-in call, reading `key` of the reply.
    pub(crate) async fn fetch<T: DeserializeOwned + Default>(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
        key: &str,
    ) -> Result<T> {
        payload(&self.call(endpoint, query).await?, key)
    }

    /// `ping` without a sign-in, then `getOpenSubsonicExtensions` (public by the
    /// spec). A Subsonic server answers `ping` with a `subsonic-response` even when it
    /// refuses the missing sign-in, so the envelope is what identifies it.
    pub async fn server_info(&self) -> Result<SubsonicInfo> {
        let resp = self.get(self.url_with("ping", &[], None)?).await?;
        let Some(env) = envelope_of(&resp.body) else {
            return Err(match resp.status {
                200..=299 | 404 => SourceError::Protocol("this isn't a Subsonic server".into()),
                status => SourceError::Server {
                    status,
                    message: resp.text().chars().take(500).collect(),
                },
            });
        };
        let field = |k: &str| {
            env.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let api_version = field("version").unwrap_or_default();
        // A server that needs a newer client protocol than ours says so before anything.
        if let Err(e @ SourceError::UnsupportedVersion { .. }) =
            check(&resp, &SourceError::WrongCredentials)
        {
            return Err(e);
        }
        let open_subsonic = env
            .get("openSubsonic")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let extensions = if open_subsonic {
            self.ask_extensions().await.unwrap_or_default()
        } else {
            Vec::new()
        };
        *lock(&self.extensions) = Some(Arc::new(extensions.clone()));
        Ok(SubsonicInfo {
            warning: check_security(&self.base)?,
            base_url: self.base.clone(),
            server_id: server_id_for(&self.base),
            name: field("type").unwrap_or_else(|| "Subsonic".to_owned()),
            version: field("serverVersion").unwrap_or(api_version),
            open_subsonic,
            extensions,
        })
    }

    async fn ask_extensions(&self) -> Result<Vec<String>> {
        let resp = self
            .get(self.url_with("getOpenSubsonicExtensions", &[], self.creds.as_ref())?)
            .await?;
        let env = check(&resp, &SourceError::SessionExpired)?;
        let list: ExtensionsDto =
            serde_json::from_value(env).map_err(|e| SourceError::Protocol(e.to_string()))?;
        Ok(list
            .open_subsonic_extensions
            .into_iter()
            .filter_map(|e| e.name)
            .collect())
    }

    /// The server's OpenSubsonic extensions, asked once. A plain Subsonic server
    /// answers with an error or a reply without the list: none, and that is kept. A
    /// server that couldn't be reached gives none this time and is asked again later.
    pub async fn extensions(&self) -> Arc<Vec<String>> {
        if let Some(e) = lock(&self.extensions).clone() {
            return e;
        }
        let list = match self.ask_extensions().await {
            Ok(list) => list,
            Err(SourceError::Unreachable(_) | SourceError::CertificateUntrusted { .. }) => {
                return Arc::new(Vec::new());
            }
            Err(_) => Vec::new(),
        };
        let list = Arc::new(list);
        *lock(&self.extensions) = Some(list.clone());
        list
    }

    async fn has_extension(&self, name: &str) -> bool {
        self.extensions().await.iter().any(|e| e == name)
    }

    /// Sign in with a user name and password: a random salt, `token = md5(password +
    /// salt)`, checked with `ping`. Only the token and salt are kept, never the
    /// password (ADR core-0021 point 3).
    pub async fn sign_in(&self, user: &str, password: &str) -> Result<SubsonicCredentials> {
        let salt = random_salt();
        let token = md5_hex(format!("{password}{salt}").as_bytes());
        let creds = SubsonicCredentials {
            user: user.to_owned(),
            auth: Auth::Token { token, salt },
        };
        let resp = self.get(self.url_with("ping", &[], Some(&creds))?).await?;
        check(&resp, &SourceError::WrongCredentials)?;
        Ok(creds)
    }

    /// Sign in with an OpenSubsonic API key (`apiKeyAuthentication`): checked with
    /// `ping`; the user name comes from `tokenInfo` (empty when the server can't say).
    pub async fn sign_in_with_key(&self, key: &str) -> Result<SubsonicCredentials> {
        let mut creds = SubsonicCredentials {
            user: String::new(),
            auth: Auth::ApiKey(key.trim().to_owned()),
        };
        let resp = self.get(self.url_with("ping", &[], Some(&creds))?).await?;
        check(&resp, &SourceError::WrongCredentials)?;
        let resp = self
            .get(self.url_with("tokenInfo", &[], Some(&creds))?)
            .await?;
        if let Ok(env) = check(&resp, &SourceError::WrongCredentials) {
            let info: UserDto = payload(&env, "tokenInfo")?;
            creds.user = info.username.unwrap_or_default();
        }
        Ok(creds)
    }

    /// `getUser` for the signed-in user, and `getAvatar` when the server answers it
    /// with a picture (one small request; a server without avatars sends an error or a
    /// placeholder, and an error gives None).
    pub async fn me(&self) -> Result<SubsonicUser> {
        let name = self.user_name().to_owned();
        if name.is_empty() {
            return Err(SourceError::Protocol(
                "the server didn't say whose API key this is".into(),
            ));
        }
        let user: UserDto = self
            .fetch("getUser", &[("username", name.as_str())], "user")
            .await?;
        let user_name = user.username.filter(|u| !u.is_empty()).unwrap_or(name);
        let avatar = self.url("getAvatar", &[("username", user_name.as_str())])?;
        let avatar_url = match self.get(avatar.clone()).await {
            Ok(r)
                if (200..=299).contains(&r.status)
                    && !r.body.is_empty()
                    && !is_error_document(&r.body) =>
            {
                Some(avatar)
            }
            _ => None,
        };
        Ok(SubsonicUser {
            user_name,
            avatar_url,
        })
    }

    /// `getIndexes`' `lastModified` (ms since 1970), the moment the library last
    /// changed. Asked with `ifModifiedSince` far in the future, so the server sends
    /// only the time and not the whole index.
    pub async fn last_modified(&self) -> Result<Option<i64>> {
        let idx: super::dto::IndexesDto = self
            .fetch(
                "getIndexes",
                &[("ifModifiedSince", "99999999999999")],
                "indexes",
            )
            .await?;
        Ok(idx.last_modified.filter(|m| *m > 0))
    }

    /// `getPlaylist`: the playlist with its songs.
    pub(crate) async fn playlist(&self, playlist_id: &str) -> Result<PlaylistDto> {
        self.fetch("getPlaylist", &[("id", playlist_id)], "playlist")
            .await
    }

    /// `getPlaylists`: this user's playlists and those shared with them.
    pub(crate) async fn playlists(&self) -> Result<Vec<PlaylistDto>> {
        let list: PlaylistsDto = self.fetch("getPlaylists", &[], "playlists").await?;
        Ok(list.playlist)
    }

    /// A playlist's songs as the server has them, with entry IDs `"<position>:<song>"`.
    pub async fn playlist_entries(&self, playlist_id: &str) -> Result<Vec<PlaylistEntry>> {
        Ok(map::to_entries(&self.playlist(playlist_id).await?))
    }

    /// Lyrics: OpenSubsonic `getLyricsBySongId` when the server has `songLyrics`
    /// (synced lines become LRC, format "lrc"), otherwise `getSong` for the artist and
    /// title and `getLyrics` (plain text, format "plain"). None when there are none.
    pub async fn lyrics(&self, item_id: &str) -> Result<Option<RawLyrics>> {
        let found = if self.has_extension("songLyrics").await {
            self.fetch::<LyricsListDto>("getLyricsBySongId", &[("id", item_id)], "lyricsList")
                .await
                .map(|list| {
                    map::pick_lyrics(&list.structured_lyrics).map(|l| {
                        let (format, body) = map::lyrics_text(l);
                        RawLyrics {
                            format: format.into(),
                            body,
                        }
                    })
                })
        } else {
            self.plain_lyrics(item_id).await
        };
        match found {
            Err(SourceError::NotFound) => Ok(None),
            other => other,
        }
    }

    async fn plain_lyrics(&self, item_id: &str) -> Result<Option<RawLyrics>> {
        let song: super::dto::Child = self.fetch("getSong", &[("id", item_id)], "song").await?;
        let (Some(artist), Some(title)) = (song.artist, song.title) else {
            return Ok(None);
        };
        let l: PlainLyricsDto = self
            .fetch(
                "getLyrics",
                &[("artist", artist.as_str()), ("title", title.as_str())],
                "lyrics",
            )
            .await?;
        Ok(l.value
            .filter(|v| !v.trim().is_empty())
            .map(|body| RawLyrics {
                format: "plain".into(),
                body,
            }))
    }

    /// Where to stream a song from: `stream` with `format=raw` for the original file,
    /// or with `maxBitRate` (kbps) and `format` for a transcode under a cap. Both URLs
    /// are the same and carry the sign-in; there are no headers.
    pub fn stream(&self, item_id: &str, req: &StreamRequest) -> Result<StreamInfo> {
        let (url, may_transcode) = match req.quality {
            StreamQuality::Original => (
                self.url("stream", &[("id", item_id), ("format", "raw")])?,
                false,
            ),
            StreamQuality::Capped { kbps } => {
                let kbps = kbps.to_string();
                let format = match req.codec {
                    StreamCodec::Aac => "aac",
                    StreamCodec::Opus => "opus",
                };
                (
                    self.url(
                        "stream",
                        &[
                            ("id", item_id),
                            ("maxBitRate", kbps.as_str()),
                            ("format", format),
                            ("estimateContentLength", "true"),
                        ],
                    )?,
                    true,
                )
            }
        };
        Ok(StreamInfo {
            url: url.to_string(),
            url_with_api_key: url.to_string(),
            headers: Vec::new(),
            may_transcode,
        })
    }

    /// A seekable reader over the song's original file (`download`, ADR core-0021
    /// point 6), for sound analysis: each read is one small `Range` request. Use it
    /// from a blocking thread.
    pub fn range_reader(
        &self,
        item_id: &str,
        runtime: tokio::runtime::Handle,
    ) -> Result<crate::ranged::RangeReader> {
        let url = self.url("download", &[("id", item_id)])?;
        Ok(crate::ranged::RangeReader::new(
            self.transport.clone(),
            url,
            Vec::new(),
            runtime,
        ))
    }

    /// Save a song to `to`: the original through `download` (or `stream?format=raw`
    /// when this user may not download), a transcode through `stream`. A part kept
    /// from an earlier try is resumed where the server allows.
    pub async fn download(
        &self,
        item_id: &str,
        req: &StreamRequest,
        to: &std::path::Path,
        resume_from: u64,
        progress: crate::transport::Progress<'_>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::transport::Downloaded> {
        let url = match req.quality {
            StreamQuality::Original => self.url("download", &[("id", item_id)])?,
            StreamQuality::Capped { .. } => Url::parse(&self.stream(item_id, req)?.url)
                .map_err(|e| SourceError::InvalidAddress(e.to_string()))?,
        };
        match self
            .download_from(url, to, resume_from, progress, cancel)
            .await
        {
            Err(SourceError::Server { status: 403, .. })
                if req.quality == StreamQuality::Original =>
            {
                let raw = Url::parse(&self.stream(item_id, req)?.url)
                    .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
                self.download_from(raw, to, resume_from, progress, cancel)
                    .await
            }
            other => other,
        }
    }

    async fn download_from(
        &self,
        url: Url,
        to: &std::path::Path,
        resume_from: u64,
        progress: crate::transport::Progress<'_>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::transport::Downloaded> {
        let got = self
            .transport
            .download(
                Request {
                    method: Method::Get,
                    url,
                    headers: Vec::new(),
                    body: None,
                    timeout: self.timeout,
                },
                to,
                resume_from,
                progress,
                cancel,
            )
            .await?;
        match got.status {
            200..=299 => {}
            401 | 403 => return Err(SourceError::SessionExpired),
            404 => return Err(SourceError::NotFound),
            status => {
                return Err(SourceError::Server {
                    status,
                    message: String::new(),
                });
            }
        }
        // A Subsonic error arrives as a small XML or JSON document with status 200, and
        // the transport has written it to the file. Read it, take it back out, map it.
        let start = if got.status == 206 { resume_from } else { 0 };
        if got.bytes.saturating_sub(start) <= 64 * 1024
            && let Some(doc) = read_tail(to, start)
            && is_error_document(&doc)
        {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(to) {
                let _ = f.set_len(start);
            }
            let resp = Response {
                status: 200,
                body: doc,
                server_date_utc_ms: None,
                content_range_total: None,
            };
            return Err(check(&resp, &SourceError::SessionExpired)
                .err()
                .unwrap_or_else(|| SourceError::Protocol("an error reply".into())));
        }
        Ok(got)
    }

    /// `getCoverArt` at `size_px` square (the server scales it).
    pub fn artwork(&self, item_id: &str, size_px: u32) -> Result<ArtworkInfo> {
        self.cover_info(&self.cover_id(item_id), size_px.clamp(16, 4096))
    }

    fn cover_info(&self, cover_id: &str, px: u32) -> Result<ArtworkInfo> {
        let px = px.to_string();
        let url = self.url("getCoverArt", &[("id", cover_id), ("size", px.as_str())])?;
        Ok(ArtworkInfo {
            url: url.to_string(),
            headers: Vec::new(),
        })
    }

    async fn image(&self, cover_id: &str, px: u32) -> Result<Option<Vec<u8>>> {
        let url = Url::parse(&self.cover_info(cover_id, px)?.url)
            .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
        let resp = self.get(url).await?;
        if (200..=299).contains(&resp.status) && !is_error_document(&resp.body) {
            return Ok(Some(resp.body).filter(|b| !b.is_empty()));
        }
        match check(&resp, &SourceError::SessionExpired) {
            Err(SourceError::NotFound) => Ok(None),
            Err(e) => Err(e),
            Ok(_) => Err(SourceError::Protocol("no image in the reply".into())),
        }
    }

    /// The cover for colour sampling, at most `size_px`. `tag` is the album's or
    /// playlist's `artwork_tag`, which for Subsonic is its cover art ID. Subsonic can't
    /// ask for a format: the bytes are the server's own (JPEG or PNG, usually JPEG).
    pub async fn cover_png(
        &self,
        item_id: &str,
        size_px: u32,
        tag: Option<&str>,
    ) -> Result<Option<Vec<u8>>> {
        let id = tag
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| self.cover_id(item_id));
        self.image(&id, size_px.clamp(8, 1024)).await
    }

    /// The cover's bytes at `size_px`, for a downloaded album shown offline.
    pub async fn artwork_bytes(&self, item_id: &str, size_px: u32) -> Result<Option<Vec<u8>>> {
        self.image(&self.cover_id(item_id), size_px.clamp(16, 4096))
            .await
    }

    /// `createPlaylist`; the new ID from the reply (API 1.14.0 and later), else the
    /// newest playlist of that name this user owns.
    async fn create_playlist(&self, name: &str, item_ids: &[String]) -> Result<String> {
        let first = &item_ids[..item_ids.len().min(IDS_PER_REQUEST)];
        let mut q: Vec<(&str, &str)> = vec![("name", name)];
        q.extend(first.iter().map(|id| ("songId", id.as_str())));
        let env = self.call("createPlaylist", &q).await?;
        let made: PlaylistDto = payload(&env, "playlist")?;
        let id = match made.id.filter(|i| !i.is_empty()) {
            Some(id) => id,
            None => {
                let user = self.user_name();
                self.playlists()
                    .await?
                    .into_iter()
                    .filter(|p| p.name.as_deref() == Some(name))
                    .filter(|p| {
                        p.owner
                            .as_deref()
                            .is_none_or(|o| user.is_empty() || o.eq_ignore_ascii_case(user))
                    })
                    .max_by(|a, b| a.created.cmp(&b.created))
                    .and_then(|p| p.id)
                    .ok_or_else(|| SourceError::Protocol("no ID for the new playlist".into()))?
            }
        };
        self.append(&id, &item_ids[first.len()..]).await?;
        Ok(id)
    }

    /// `updatePlaylist` with `songIdToAdd`: songs at the end, in chunks.
    async fn append(&self, playlist_id: &str, item_ids: &[String]) -> Result<()> {
        for chunk in item_ids.chunks(IDS_PER_REQUEST) {
            let mut q: Vec<(&str, &str)> = vec![("playlistId", playlist_id)];
            q.extend(chunk.iter().map(|id| ("songIdToAdd", id.as_str())));
            self.call("updatePlaylist", &q).await?;
        }
        Ok(())
    }

    /// Replace a playlist's songs with `songs`, in this order: `createPlaylist` with
    /// `playlistId` sets the list (the Subsonic way to update one, keeping its name),
    /// and any songs past the first chunk are appended.
    async fn rewrite(&self, playlist_id: &str, songs: &[String]) -> Result<()> {
        let first = &songs[..songs.len().min(IDS_PER_REQUEST)];
        let mut q: Vec<(&str, &str)> = vec![("playlistId", playlist_id)];
        q.extend(first.iter().map(|id| ("songId", id.as_str())));
        self.call("createPlaylist", &q).await?;
        self.append(playlist_id, &songs[first.len()..]).await
    }

    /// Send one playlist or favourite write (plan section 26). Subsonic has no entry
    /// IDs and no insert or move, so:
    /// - adding at the end is `updatePlaylist?songIdToAdd`;
    /// - adding at a position and moving read the list and write it back in the new
    ///   order (`createPlaylist?playlistId`), which is exact;
    /// - removing maps entry IDs to positions on the list as it is now and sends
    ///   `updatePlaylist?songIndexToRemove` (positions from 0), highest first so a
    ///   later chunk's positions still hold;
    /// - favourites are `star` / `unstar`.
    pub async fn edit(&self, edit: &ServerEdit) -> Result<EditOutcome> {
        match edit {
            ServerEdit::CreatePlaylist { name, item_ids } => {
                return Ok(EditOutcome::Created(
                    self.create_playlist(name, item_ids).await?,
                ));
            }
            ServerEdit::AddToPlaylist {
                playlist_id,
                item_ids,
                position,
            } => {
                let at_end = match position {
                    None => true,
                    Some(p) => {
                        let entries = self.playlist_entries(playlist_id).await?;
                        let p = *p as usize;
                        if p >= entries.len() {
                            true
                        } else {
                            let mut songs: Vec<String> =
                                entries.into_iter().map(|e| e.track_item_id).collect();
                            songs.splice(p..p, item_ids.iter().cloned());
                            self.rewrite(playlist_id, &songs).await?;
                            false
                        }
                    }
                };
                if at_end {
                    self.append(playlist_id, item_ids).await?;
                }
            }
            ServerEdit::RemoveFromPlaylist {
                playlist_id,
                entry_ids,
            } => {
                let entries = self.playlist_entries(playlist_id).await?;
                let mut positions: Vec<usize> = Vec::new();
                for id in entry_ids {
                    // An entry that is no longer there is already removed.
                    if let Some(p) = resolve_entry(id, &entries, &positions) {
                        positions.push(p);
                    }
                }
                positions.sort_unstable_by(|a, b| b.cmp(a));
                for chunk in positions.chunks(IDS_PER_REQUEST) {
                    let strings: Vec<String> = chunk.iter().map(usize::to_string).collect();
                    let mut q: Vec<(&str, &str)> = vec![("playlistId", playlist_id.as_str())];
                    q.extend(strings.iter().map(|s| ("songIndexToRemove", s.as_str())));
                    self.call("updatePlaylist", &q).await?;
                }
            }
            ServerEdit::MoveInPlaylist {
                playlist_id,
                entry_id,
                new_index,
            } => {
                let entries = self.playlist_entries(playlist_id).await?;
                let from = resolve_entry(entry_id, &entries, &[]).ok_or(SourceError::NotFound)?;
                let to = (*new_index as usize).min(entries.len().saturating_sub(1));
                if from != to {
                    let mut songs: Vec<String> =
                        entries.into_iter().map(|e| e.track_item_id).collect();
                    let song = songs.remove(from);
                    songs.insert(to, song);
                    self.rewrite(playlist_id, &songs).await?;
                }
            }
            ServerEdit::RenamePlaylist { playlist_id, name } => {
                self.call(
                    "updatePlaylist",
                    &[
                        ("playlistId", playlist_id.as_str()),
                        ("name", name.as_str()),
                    ],
                )
                .await?;
            }
            ServerEdit::DeletePlaylist { playlist_id } => {
                self.call("deletePlaylist", &[("id", playlist_id.as_str())])
                    .await?;
            }
            ServerEdit::SetFavorite { item_id, favorite } => {
                let endpoint = if *favorite { "star" } else { "unstar" };
                self.call(endpoint, &[("id", item_id.as_str())]).await?;
            }
        }
        Ok(EditOutcome::Done)
    }
}

/// The file from `start` on, at most 64 KiB.
fn read_tail(path: &std::path::Path, start: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.take(64 * 1024).read_to_end(&mut buf).ok()?;
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_ids_are_canonical() {
        let a = server_id_for(&Url::parse("https://Music.Example/navidrome/").unwrap());
        let b = server_id_for(&Url::parse("https://music.example:443/navidrome").unwrap());
        let c = server_id_for(&Url::parse("http://music.example/navidrome").unwrap());
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn salts_are_fresh() {
        let (a, b) = (random_salt(), random_salt());
        assert_eq!(a.len(), 16);
        assert_ne!(a, b);
    }

    #[test]
    fn xml_errors_are_read() {
        let body = br#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="failed" version="1.15.0">
  <error code="70" message="Song not found"/>
</subsonic-response>"#;
        assert_eq!(xml_error(body), Some((70, "Song not found".into())));
        assert!(is_error_document(body));
        assert!(!is_error_document(b"\xff\xd8\xff\xe0 a jpeg"));
    }

    #[test]
    fn stale_entry_ids_find_their_song() {
        let e = |s: &str| PlaylistEntry {
            entry_id: None,
            track_item_id: s.into(),
        };
        let entries = vec![e("a"), e("b"), e("a"), e("c")];
        assert_eq!(resolve_entry("2:a", &entries, &[]), Some(2));
        // "b" was at 3 before an earlier removal; it is at 1 now.
        assert_eq!(resolve_entry("3:b", &entries, &[]), Some(1));
        assert_eq!(resolve_entry("0:a", &entries, &[0]), Some(2));
        assert_eq!(resolve_entry("0:z", &entries, &[]), None);
    }
}
