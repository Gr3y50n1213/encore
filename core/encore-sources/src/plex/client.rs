//! The Plex Media Server API, as far as Encore uses it (ADR core-0023). Every request
//! asks for JSON (`Accept: application/json`) and signs in with the `X-Plex-Token`
//! header; the `X-Plex-*` identity headers name the app and the install's random client
//! identifier only. URLs never carry the token except `StreamInfo::url_with_api_key`,
//! for players that can't send headers, and no Debug output shows a token.
//!
//! The endpoints and replies here were checked against a real Plex Media Server
//! 1.43.5 in CI (core.yml, job `plex`), unclaimed and offline; see the ADR for what
//! that could not cover.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use url::Url;

use super::dto::{Container, Directory, Envelope, Metadata};
use super::map;
use super::tv::{AccountToken, PlexIdentity, PlexTv};
use crate::error::{Result, SourceError};
use crate::jellyfin::address::check_security;
use crate::jellyfin::{
    AddressWarning, ArtworkInfo, RawLyrics, StreamInfo, StreamQuality, StreamRequest,
};
use crate::model::{EditOutcome, PlaylistEntry, ServerEdit};
use crate::transport::{Method, Request, Response, Transport};

/// Rating keys per playlist request, so a long list never makes an overlong URL.
const KEYS_PER_REQUEST: usize = 100;
/// A playlist's items are read this many at a time.
const PLAYLIST_PAGE: u32 = 500;
/// How often plex.tv may be asked for a server's addresses when none of the known ones
/// answers.
const LOOKUP_EVERY: Duration = Duration::from_secs(10 * 60);
/// The transcode target the music transcoder is asked to add to its profile.
const MP3_OVER_HTTP: &str = "add-transcode-target(type=musicProfile&context=streaming&protocol=http&container=mp3&audioCodec=mp3)";
/// The library's identifier in rating and play-count calls.
const LIBRARY: &str = "com.plexapp.plugins.library";

/// What `/identity` says about a server (no sign-in needed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexInfo {
    pub base_url: Url,
    /// The server's `machineIdentifier`.
    pub server_id: String,
    pub version: String,
    /// False for a server nobody has signed in to Plex with yet.
    pub claimed: bool,
    /// Set for plain http on the local network (as for Jellyfin, board 21).
    pub warning: Option<AddressWarning>,
}

/// A saved Plex sign-in: one server's access token, the server's ID and the addresses
/// plex.tv gave for it. Never the account token. Keep it in the keychain only.
#[derive(Clone, PartialEq, Eq)]
pub struct PlexCredentials {
    token: String,
    pub machine_id: String,
    /// In the order to try them; the first is the one that answered at sign-in.
    pub connections: Vec<Url>,
}

impl fmt::Debug for PlexCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexCredentials")
            .field("machine_id", &self.machine_id)
            .field("connections", &self.connections.len())
            .finish_non_exhaustive()
    }
}

const TOKEN_PREFIX: &str = "plex1:";

impl PlexCredentials {
    pub fn new(token: String, machine_id: String, connections: Vec<Url>) -> PlexCredentials {
        PlexCredentials {
            token,
            machine_id,
            connections,
        }
    }

    /// The server token, for the companion server's check (ADR core-0023 point 10) and
    /// this client's header. Never log it.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// One string for the keychain: `plex1:` and form-encoded fields (`t` the token,
    /// `m` the server's ID, `c` each address in order).
    pub fn to_token_string(&self) -> String {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        s.append_pair("t", &self.token);
        s.append_pair("m", &self.machine_id);
        for c in &self.connections {
            s.append_pair("c", c.as_str());
        }
        format!("{TOKEN_PREFIX}{}", s.finish())
    }

    pub fn from_token_string(s: &str) -> Option<PlexCredentials> {
        let mut token = None;
        let mut machine_id = None;
        let mut connections = Vec::new();
        for (k, v) in url::form_urlencoded::parse(s.strip_prefix(TOKEN_PREFIX)?.as_bytes()) {
            match k.as_ref() {
                "t" => token = Some(v.into_owned()),
                "m" => machine_id = Some(v.into_owned()),
                "c" => connections.extend(Url::parse(&v).ok()),
                _ => {}
            }
        }
        Some(PlexCredentials {
            token: token.filter(|t| !t.is_empty())?,
            machine_id: machine_id.unwrap_or_default(),
            connections,
        })
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// An error status in plain words. `unauthorized` is what a 401 means for this call.
pub(crate) fn check(resp: Response, unauthorized: &SourceError) -> Result<Response> {
    match resp.status {
        200..=299 => Ok(resp),
        401 => Err(unauthorized.clone()),
        404 => Err(SourceError::NotFound),
        status => Err(SourceError::Server {
            status,
            message: resp.text().chars().take(300).collect(),
        }),
    }
}

fn read(resp: &Response) -> Result<Container> {
    if resp.body.is_empty() {
        return Ok(Container::default());
    }
    Ok(resp.json::<Envelope>()?.container)
}

pub struct PlexClient {
    transport: Arc<dyn Transport>,
    base: RwLock<Url>,
    identity: PlexIdentity,
    timeout: Duration,
    creds: Option<PlexCredentials>,
    /// The addresses to fall back on, in order.
    known: Mutex<Vec<Url>>,
    /// Whether plex.tv may be asked for new addresses, and when it last was.
    lookup: bool,
    last_lookup: Mutex<Option<Instant>>,
    /// Playlist IDs to their composite cover, learned in a sync.
    covers: Mutex<HashMap<String, String>>,
}

impl fmt::Debug for PlexClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexClient")
            .field("signed_in", &self.creds.is_some())
            .finish_non_exhaustive()
    }
}

impl PlexClient {
    pub fn new(
        transport: Arc<dyn Transport>,
        base_url: Url,
        identity: PlexIdentity,
        timeout: Duration,
    ) -> PlexClient {
        PlexClient {
            transport,
            base: RwLock::new(base_url),
            identity,
            timeout,
            creds: None,
            known: Mutex::new(Vec::new()),
            lookup: false,
            last_lookup: Mutex::new(None),
            covers: Mutex::new(HashMap::new()),
        }
    }

    /// A client that signs every request in with `creds`, falling back on its other
    /// addresses when the current one stops answering.
    pub fn signed_in(mut self, creds: PlexCredentials) -> PlexClient {
        *lock(&self.known) = creds.connections.clone();
        self.creds = Some(creds);
        self
    }

    /// Let the client ask plex.tv for the server's addresses when none of the known
    /// ones answers (ADR core-0023 point 5). The app's sources turn this on; the
    /// companion server and the tests don't.
    pub fn with_address_lookup(mut self, on: bool) -> PlexClient {
        self.lookup = on;
        self
    }

    /// The address in use now.
    pub fn base_url(&self) -> Url {
        self.base
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn credentials(&self) -> Option<&PlexCredentials> {
        self.creds.as_ref()
    }

    fn token(&self) -> Result<&str> {
        self.creds
            .as_ref()
            .map(|c| c.token.as_str())
            .ok_or(SourceError::SessionExpired)
    }

    fn url_at(base: &Url, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let mut u = Url::parse(&format!("{}{path}", base.as_str().trim_end_matches('/')))
            .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
        if !query.is_empty() {
            u.query_pairs_mut().extend_pairs(query);
        }
        Ok(u)
    }

    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        Self::url_at(&self.base_url(), path, query)
    }

    /// The identity headers, with the token when signed in.
    fn headers(&self) -> Vec<(String, String)> {
        let mut h = self.identity.headers();
        if let Some(c) = &self.creds {
            h.push(("X-Plex-Token".into(), c.token.clone()));
        }
        h
    }

    async fn send_to(&self, method: Method, url: Url) -> Result<Response> {
        self.transport
            .send(Request {
                method,
                url,
                headers: self.headers(),
                body: None,
                timeout: self.timeout,
            })
            .await
    }

    /// One signed-in request to `path` on the current address; if that address doesn't
    /// answer, the next one that is this server is used, and the request sent again.
    async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<Response> {
        match self.send_to(method, self.url(path, query)?).await {
            Err(SourceError::Unreachable(why)) => {
                if self.fail_over().await {
                    self.send_to(method, self.url(path, query)?).await
                } else {
                    Err(SourceError::Unreachable(why))
                }
            }
            other => other,
        }
    }

    pub(crate) async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Container> {
        let resp = self.request(Method::Get, path, query).await?;
        read(&check(resp, &SourceError::SessionExpired)?)
    }

    async fn write(&self, method: Method, path: &str, query: &[(&str, &str)]) -> Result<Container> {
        let resp = self.request(method, path, query).await?;
        read(&check(resp, &SourceError::SessionExpired)?)
    }

    /// `GET /identity` at `base`, without a sign-in: the server's ID if it is Plex.
    async fn identity_at(&self, base: &Url) -> Result<Container> {
        let resp = self
            .transport
            .send(Request {
                method: Method::Get,
                url: Self::url_at(base, "/identity", &[])?,
                headers: self.identity.headers(),
                body: None,
                timeout: self.timeout,
            })
            .await?;
        let resp = match check(resp, &SourceError::WrongCredentials) {
            Err(SourceError::NotFound) => {
                return Err(SourceError::Protocol("this isn't a Plex server".into()));
            }
            other => other?,
        };
        let c = resp
            .json::<Envelope>()
            .map_err(|_| SourceError::Protocol("this isn't a Plex server".into()))?
            .container;
        if c.machine_identifier.as_deref().is_none_or(str::is_empty) {
            return Err(SourceError::Protocol("this isn't a Plex server".into()));
        }
        Ok(c)
    }

    /// Is a Plex Media Server at this address? `/identity` answers without a sign-in on
    /// every Plex server, claimed or not.
    pub async fn server_info(&self) -> Result<PlexInfo> {
        let base = self.base_url();
        let c = self.identity_at(&base).await?;
        Ok(PlexInfo {
            warning: check_security(&base)?,
            server_id: c.machine_identifier.unwrap_or_default(),
            version: c.version.unwrap_or_default(),
            claimed: c.claimed.unwrap_or(true),
            base_url: base,
        })
    }

    /// Does the server take this sign-in, and is it the server it should be? Checked
    /// with `/library/sections` (refused with 401 for a token it doesn't know) and
    /// `/identity` (its ID must match the saved one).
    pub async fn check_sign_in(&self) -> Result<PlexInfo> {
        let info = self.server_info().await?;
        if let Some(c) = &self.creds
            && !c.machine_id.is_empty()
            && c.machine_id != info.server_id
        {
            return Err(SourceError::WrongCredentials);
        }
        let resp = self.request(Method::Get, "/library/sections", &[]).await?;
        check(resp, &SourceError::WrongCredentials)?;
        Ok(info)
    }

    /// Whether this address lets anyone in without a sign-in (an unclaimed server, or
    /// one whose "allowed without auth" networks include the caller): a token no server
    /// issued is tried, and a server that takes it can't vouch for any token. The
    /// companion server refuses to trust such a server for sign-ins.
    pub async fn lets_anyone_in(&self) -> Result<bool> {
        let probe = format!("encore-check-{}", crate::subsonic::client::random_salt());
        let resp = self
            .transport
            .send(Request {
                method: Method::Get,
                url: self.url("/library/sections", &[])?,
                headers: {
                    let mut h = self.identity.headers();
                    h.push(("X-Plex-Token".into(), probe));
                    h
                },
                body: None,
                timeout: self.timeout,
            })
            .await?;
        Ok((200..=299).contains(&resp.status))
    }

    /// Try the other known addresses, then (when allowed, at most every ten minutes)
    /// the ones plex.tv lists now; switch to the first that is this server. True when
    /// one was found.
    async fn fail_over(&self) -> bool {
        let Some(creds) = &self.creds else {
            return false;
        };
        let current = self.base_url();
        let known: Vec<Url> = lock(&self.known).clone();
        let mut tried: Vec<Url> = vec![current.clone()];
        for url in known {
            if tried.contains(&url) {
                continue;
            }
            tried.push(url.clone());
            if self.is_this_server(&url, &creds.machine_id).await {
                return self.switch_to(url);
            }
        }
        if !self.lookup {
            return false;
        }
        {
            let mut last = lock(&self.last_lookup);
            if last.is_some_and(|t| t.elapsed() < LOOKUP_EVERY) {
                return false;
            }
            *last = Some(Instant::now());
        }
        let tv = PlexTv::new(self.transport.clone(), self.identity.clone(), self.timeout);
        let Ok(servers) = tv.servers(&AccountToken::new(creds.token.clone())).await else {
            return false;
        };
        let Some(server) = servers
            .into_iter()
            .find(|s| s.machine_id == creds.machine_id)
        else {
            return false;
        };
        let fresh: Vec<Url> = server.connections.into_iter().map(|c| c.uri).collect();
        *lock(&self.known) = fresh.clone();
        for url in fresh {
            if tried.contains(&url) {
                continue;
            }
            if self.is_this_server(&url, &creds.machine_id).await {
                return self.switch_to(url);
            }
        }
        false
    }

    async fn is_this_server(&self, url: &Url, machine_id: &str) -> bool {
        check_security(url).is_ok()
            && self
                .identity_at(url)
                .await
                .is_ok_and(|c| c.machine_identifier.as_deref() == Some(machine_id))
    }

    fn switch_to(&self, url: Url) -> bool {
        *self
            .base
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = url;
        true
    }

    /// The music libraries (`type=artist`).
    pub(crate) async fn music_sections(&self) -> Result<Vec<Directory>> {
        Ok(self
            .get("/library/sections", &[])
            .await?
            .directory
            .into_iter()
            .filter(|d| d.kind.as_deref() == Some("artist") && d.key.is_some())
            .collect())
    }

    /// One page of a section's items of one type (8 artists, 9 albums, 10 tracks).
    pub(crate) async fn page(
        &self,
        section: &str,
        item_type: u8,
        start: u64,
        size: u32,
        filter: &[(&str, &str)],
    ) -> Result<Container> {
        let (t, s, n) = (item_type.to_string(), start.to_string(), size.to_string());
        let mut q: Vec<(&str, &str)> = vec![
            ("type", t.as_str()),
            ("X-Plex-Container-Start", s.as_str()),
            ("X-Plex-Container-Size", n.as_str()),
        ];
        q.extend_from_slice(filter);
        self.get(&format!("/library/sections/{section}/all"), &q)
            .await
    }

    /// This user's audio playlists (Plex lists only the user's own).
    pub(crate) async fn playlists(&self) -> Result<Vec<Metadata>> {
        Ok(self
            .get("/playlists", &[("playlistType", "audio")])
            .await?
            .metadata)
    }

    pub(crate) fn remember_cover(&self, playlist_id: &str, cover: Option<&str>) {
        if let Some(c) = cover.filter(|c| !c.is_empty()) {
            lock(&self.covers).insert(playlist_id.to_owned(), c.to_owned());
        }
    }

    /// A playlist's items, every page.
    pub(crate) async fn playlist_items(&self, playlist_id: &str) -> Result<Vec<Metadata>> {
        let mut all = Vec::new();
        let size = PLAYLIST_PAGE.to_string();
        loop {
            let start = all.len().to_string();
            let page = self
                .get(
                    &format!("/playlists/{playlist_id}/items"),
                    &[
                        ("X-Plex-Container-Start", start.as_str()),
                        ("X-Plex-Container-Size", size.as_str()),
                    ],
                )
                .await?;
            let n = page.metadata.len();
            all.extend(page.metadata);
            let total = page.total_size.and_then(|t| usize::try_from(t).ok());
            if n < PLAYLIST_PAGE as usize || n == 0 || total.is_some_and(|t| all.len() >= t) {
                return Ok(all);
            }
        }
    }

    pub async fn playlist_entries(&self, playlist_id: &str) -> Result<Vec<PlaylistEntry>> {
        Ok(map::to_entries(&self.playlist_items(playlist_id).await?))
    }

    /// Lyrics: the track's lyric stream (`streamType` 4, from a sidecar `.lrc` or
    /// `.txt`, or the server's own lyrics agent), fetched by its key. Synced lyrics come as
    /// LRC (format "lrc"), others as plain lines. None when the track has none.
    pub async fn lyrics(&self, item_id: &str) -> Result<Option<RawLyrics>> {
        let key = map::rating_key(item_id);
        let meta = match self.get(&format!("/library/metadata/{key}"), &[]).await {
            Err(SourceError::NotFound) => return Ok(None),
            other => other?,
        };
        let streams: Vec<_> = meta
            .metadata
            .iter()
            .flat_map(|m| m.media.iter())
            .flat_map(|md| md.part.iter())
            .flat_map(|p| p.stream.iter())
            .filter(|s| s.stream_type == Some(4) && s.key.as_deref().is_some_and(|k| !k.is_empty()))
            .collect();
        let synced = |s: &super::dto::StreamDto| {
            [s.codec.as_deref(), s.format.as_deref()]
                .iter()
                .any(|f| f.is_some_and(|f| f.eq_ignore_ascii_case("lrc")))
        };
        let Some(stream) = streams
            .iter()
            .copied()
            .find(|s| synced(s))
            .or_else(|| streams.first().copied())
        else {
            return Ok(None);
        };
        // The file as it is: asked for as text, not JSON (the server would wrap it).
        let mut headers = self.headers();
        for (k, v) in &mut headers {
            if k == "Accept" {
                *v = "text/plain, */*".into();
            }
        }
        let resp = self
            .transport
            .send(Request {
                method: Method::Get,
                url: self.url(stream.key.as_deref().unwrap_or(""), &[])?,
                headers,
                body: None,
                timeout: self.timeout,
            })
            .await?;
        let body = match check(resp, &SourceError::SessionExpired) {
            Err(SourceError::NotFound) => return Ok(None),
            other => other?.text(),
        };
        let body = body.trim_start_matches('\u{feff}').to_owned();
        if body.trim().is_empty() {
            return Ok(None);
        }
        let looks_timed = body.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with('[') && l[1..].chars().next().is_some_and(|c| c.is_ascii_digit())
        });
        Ok(Some(RawLyrics {
            format: if synced(stream) || looks_timed {
                "lrc"
            } else {
                "plain"
            }
            .into(),
            body,
        }))
    }

    /// The file of a track: `/library/parts/<id>/file` (the server sends its own
    /// Content-Type).
    fn part_url(&self, item_id: &str, query: &[(&str, &str)]) -> Result<Url> {
        let part = map::part_id(item_id).ok_or(SourceError::NotFound)?;
        self.url(&format!("/library/parts/{part}/file"), query)
    }

    /// The music transcoder for a bitrate cap: MP3 over plain progressive HTTP, which
    /// every platform player reads. Plex's generic profile has no music target for plain
    /// HTTP, so the request adds one (`X-Plex-Client-Profile-Extra`); without it the
    /// server answers 400 ("No conversion profile found for protocol http", seen in CI).
    /// The platform is named "Generic" here so that profile is the one adapted.
    fn transcode_url(&self, item_id: &str, kbps: u32) -> Result<Url> {
        let path = format!("/library/metadata/{}", map::rating_key(item_id));
        let kbps = kbps.clamp(32, 320).to_string();
        let session = crate::subsonic::client::random_salt();
        self.url(
            "/music/:/transcode/universal/start.mp3",
            &[
                ("path", path.as_str()),
                ("mediaIndex", "0"),
                ("partIndex", "0"),
                ("protocol", "http"),
                ("directPlay", "0"),
                ("directStream", "0"),
                ("musicBitrate", kbps.as_str()),
                ("maxAudioBitrate", kbps.as_str()),
                ("session", session.as_str()),
                ("X-Plex-Client-Identifier", self.identity.client_id.as_str()),
                ("X-Plex-Product", self.identity.product.as_str()),
                ("X-Plex-Platform", "Generic"),
                ("X-Plex-Client-Profile-Extra", MP3_OVER_HTTP),
            ],
        )
    }

    /// Where to stream a track from: its file as it is, or the transcoder under a cap.
    /// `url` with `headers` (the token is a header); `url_with_api_key` carries it in
    /// the query for players that can't send headers. Never log either.
    pub fn stream(&self, item_id: &str, req: &StreamRequest) -> Result<StreamInfo> {
        let token = self.token()?.to_owned();
        let (url, may_transcode) = match req.quality {
            StreamQuality::Original => (self.part_url(item_id, &[])?, false),
            StreamQuality::Capped { kbps } => (self.transcode_url(item_id, kbps)?, true),
        };
        let mut keyed = url.clone();
        keyed.query_pairs_mut().append_pair("X-Plex-Token", &token);
        Ok(StreamInfo {
            url: url.to_string(),
            url_with_api_key: keyed.to_string(),
            headers: self
                .headers()
                .into_iter()
                .filter(|(k, _)| k != "Accept")
                .collect(),
            may_transcode,
        })
    }

    /// A seekable reader over the track's file, for sound analysis: each read is one
    /// small `Range` request (the server answers 206). Use it from a blocking thread.
    pub fn range_reader(
        &self,
        item_id: &str,
        runtime: tokio::runtime::Handle,
    ) -> Result<crate::ranged::RangeReader> {
        self.token()?;
        Ok(crate::ranged::RangeReader::new(
            self.transport.clone(),
            self.part_url(item_id, &[])?,
            self.headers()
                .into_iter()
                .filter(|(k, _)| k != "Accept")
                .collect(),
            runtime,
        ))
    }

    /// Save a track to `to`: the file itself (`download=1`), or a transcode under a cap.
    pub async fn download(
        &self,
        item_id: &str,
        req: &StreamRequest,
        to: &std::path::Path,
        resume_from: u64,
        progress: crate::transport::Progress<'_>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::transport::Downloaded> {
        self.token()?;
        let url = match req.quality {
            StreamQuality::Original => self.part_url(item_id, &[("download", "1")])?,
            StreamQuality::Capped { kbps } => self.transcode_url(item_id, kbps)?,
        };
        let got = self
            .transport
            .download(
                Request {
                    method: Method::Get,
                    url,
                    headers: self
                        .headers()
                        .into_iter()
                        .filter(|(k, _)| k != "Accept")
                        .collect(),
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
            200..=299 => Ok(got),
            401 => Err(SourceError::SessionExpired),
            404 => Err(SourceError::NotFound),
            status => Err(SourceError::Server {
                status,
                message: String::new(),
            }),
        }
    }

    /// The picture of an item: a playlist's composite when the sync saw one, else the
    /// item's own thumb (`/library/metadata/<key>/thumb`, the current one).
    fn thumb_of(&self, item_id: &str) -> String {
        let key = map::rating_key(item_id);
        lock(&self.covers)
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("/library/metadata/{key}/thumb"))
    }

    /// The photo transcoder: the picture scaled to `px` square, as JPEG or PNG.
    fn photo_url(&self, thumb: &str, px: u32, png: bool) -> Result<Url> {
        let px = px.to_string();
        let mut q: Vec<(&str, &str)> = vec![
            ("width", px.as_str()),
            ("height", px.as_str()),
            ("minSize", "1"),
            ("upscale", "1"),
            ("url", thumb),
        ];
        if png {
            q.push(("format", "png"));
        }
        self.url("/photo/:/transcode", &q)
    }

    /// The cover at `size_px` square (the photo transcoder scales it).
    pub fn artwork(&self, item_id: &str, size_px: u32) -> Result<ArtworkInfo> {
        self.token()?;
        let url = self.photo_url(&self.thumb_of(item_id), size_px.clamp(16, 4096), false)?;
        Ok(ArtworkInfo {
            url: url.to_string(),
            headers: self
                .headers()
                .into_iter()
                .filter(|(k, _)| k != "Accept")
                .collect(),
        })
    }

    async fn image(&self, thumb: &str, px: u32, png: bool) -> Result<Option<Vec<u8>>> {
        let url = self.photo_url(thumb, px, png)?;
        let resp = match self.send_to(Method::Get, url).await {
            Err(SourceError::Unreachable(why)) => {
                if !self.fail_over().await {
                    return Err(SourceError::Unreachable(why));
                }
                self.send_to(Method::Get, self.photo_url(thumb, px, png)?)
                    .await?
            }
            other => other?,
        };
        match check(resp, &SourceError::SessionExpired) {
            Ok(r) if !r.body.is_empty() && !r.body.starts_with(b"<") => Ok(Some(r.body)),
            Ok(_) | Err(SourceError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The cover as PNG for colour sampling, `size_px` square. `tag` is the album's
    /// `thumb` path when the sync saw one (it changes with the picture).
    pub async fn cover_png(
        &self,
        item_id: &str,
        size_px: u32,
        tag: Option<&str>,
    ) -> Result<Option<Vec<u8>>> {
        let thumb = tag
            .filter(|t| t.starts_with('/'))
            .map(str::to_owned)
            .unwrap_or_else(|| self.thumb_of(item_id));
        self.image(&thumb, size_px.clamp(8, 1024), true).await
    }

    /// The cover's bytes at `size_px`, for a downloaded album shown offline.
    pub async fn artwork_bytes(&self, item_id: &str, size_px: u32) -> Result<Option<Vec<u8>>> {
        self.image(&self.thumb_of(item_id), size_px.clamp(16, 4096), false)
            .await
    }

    /// `server://<machine>/com.plexapp.plugins.library/library/metadata/<keys>`: how
    /// Plex names library items to put in a playlist.
    fn items_uri(&self, item_ids: &[String]) -> Result<String> {
        let machine = self
            .creds
            .as_ref()
            .map(|c| c.machine_id.as_str())
            .filter(|m| !m.is_empty())
            .ok_or_else(|| SourceError::Protocol("no server ID for this sign-in".into()))?;
        let keys: Vec<&str> = item_ids.iter().map(|i| map::rating_key(i)).collect();
        Ok(format!(
            "server://{machine}/{LIBRARY}/library/metadata/{}",
            keys.join(",")
        ))
    }

    /// `PUT /playlists/<id>/items?uri=`: songs at the end, in chunks.
    async fn append(&self, playlist_id: &str, item_ids: &[String]) -> Result<()> {
        for chunk in item_ids.chunks(KEYS_PER_REQUEST) {
            let uri = self.items_uri(chunk)?;
            self.write(
                Method::Put,
                &format!("/playlists/{playlist_id}/items"),
                &[("uri", uri.as_str())],
            )
            .await?;
        }
        Ok(())
    }

    /// `PUT /playlists/<id>/items/<entry>/move?after=<entry>` (no `after`: to the top).
    async fn move_after(&self, playlist_id: &str, entry: &str, after: Option<&str>) -> Result<()> {
        let q: Vec<(&str, &str)> = after.map(|a| ("after", a)).into_iter().collect();
        self.write(
            Method::Put,
            &format!("/playlists/{playlist_id}/items/{entry}/move"),
            &q,
        )
        .await
        .map(|_| ())
    }

    /// `POST /playlists?type=audio&smart=0&title=&uri=`; the new ID from the reply.
    async fn create_playlist(&self, name: &str, item_ids: &[String]) -> Result<String> {
        let first = &item_ids[..item_ids.len().min(KEYS_PER_REQUEST)];
        let uri = if first.is_empty() {
            None
        } else {
            Some(self.items_uri(first)?)
        };
        let mut q: Vec<(&str, &str)> = vec![("type", "audio"), ("smart", "0"), ("title", name)];
        if let Some(u) = &uri {
            q.push(("uri", u.as_str()));
        }
        let made = self.write(Method::Post, "/playlists", &q).await?;
        let id = made
            .metadata
            .first()
            .and_then(|m| m.rating_key.clone())
            .filter(|i| !i.is_empty())
            .ok_or_else(|| SourceError::Protocol("no ID for the new playlist".into()))?;
        self.append(&id, &item_ids[first.len()..]).await?;
        Ok(id)
    }

    /// Send one playlist or favourite write (plan section 26). Plex has entry IDs
    /// (`playlistItemID`), removals by entry and moves "after an entry", so:
    /// - adding at the end is `PUT .../items?uri=`; adding at a position adds at the end
    ///   and moves the new entries into place, one after the other;
    /// - a move puts the entry after the one that will precede it (or at the top);
    /// - removing deletes each entry (one already gone counts as removed);
    /// - a like rates the song 5 stars, an unlike clears its rating (`/:/rate`).
    ///
    /// Plex keeps a song once per playlist: adding one that is already there is ignored
    /// by the server, and the next sync shows the list as the server has it.
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
                let before = match position {
                    Some(_) => Some(self.playlist_entries(playlist_id).await?),
                    None => None,
                };
                self.append(playlist_id, item_ids).await?;
                if let (Some(p), Some(before)) = (position, before)
                    && (*p as usize) < before.len()
                {
                    let after_add = self.playlist_entries(playlist_id).await?;
                    let fresh: Vec<String> = after_add
                        .iter()
                        .skip(before.len())
                        .filter_map(|e| e.entry_id.clone())
                        .collect();
                    let mut after: Option<String> = match *p as usize {
                        0 => None,
                        i => before[i - 1].entry_id.clone(),
                    };
                    for entry in fresh {
                        self.move_after(playlist_id, &entry, after.as_deref())
                            .await?;
                        after = Some(entry);
                    }
                }
            }
            ServerEdit::RemoveFromPlaylist {
                playlist_id,
                entry_ids,
            } => {
                for entry in entry_ids {
                    match self
                        .write(
                            Method::Delete,
                            &format!("/playlists/{playlist_id}/items/{entry}"),
                            &[],
                        )
                        .await
                    {
                        Ok(_) | Err(SourceError::NotFound) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
            ServerEdit::MoveInPlaylist {
                playlist_id,
                entry_id,
                new_index,
            } => {
                let entries = self.playlist_entries(playlist_id).await?;
                let from = entries
                    .iter()
                    .position(|e| e.entry_id.as_deref() == Some(entry_id.as_str()))
                    .ok_or(SourceError::NotFound)?;
                let to = (*new_index as usize).min(entries.len().saturating_sub(1));
                if from != to {
                    let mut rest: Vec<&PlaylistEntry> = entries.iter().collect();
                    rest.remove(from);
                    let after = match to {
                        0 => None,
                        i => rest.get(i - 1).and_then(|e| e.entry_id.clone()),
                    };
                    self.move_after(playlist_id, entry_id, after.as_deref())
                        .await?;
                }
            }
            ServerEdit::RenamePlaylist { playlist_id, name } => {
                self.write(
                    Method::Put,
                    &format!("/playlists/{playlist_id}"),
                    &[("title", name.as_str())],
                )
                .await?;
            }
            ServerEdit::DeletePlaylist { playlist_id } => {
                match self
                    .write(Method::Delete, &format!("/playlists/{playlist_id}"), &[])
                    .await
                {
                    Ok(_) | Err(SourceError::NotFound) => {}
                    Err(e) => return Err(e),
                }
            }
            ServerEdit::SetFavorite { item_id, favorite } => {
                let rating = if *favorite { "10" } else { "-1" };
                self.write(
                    Method::Put,
                    "/:/rate",
                    &[
                        ("key", map::rating_key(item_id)),
                        ("identifier", LIBRARY),
                        ("rating", rating),
                    ],
                )
                .await?;
            }
        }
        Ok(EditOutcome::Done)
    }

    /// Count one play of a track on the server (`/:/scrobble`): what another Plex app
    /// does at the end of a song. Encore itself doesn't report plays to media servers
    /// (the companion keeps the history); this is here for the CI test that stands in
    /// for "played in another app".
    pub async fn scrobble(&self, item_id: &str) -> Result<()> {
        self.write(
            Method::Get,
            "/:/scrobble",
            &[("key", map::rating_key(item_id)), ("identifier", LIBRARY)],
        )
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_round_trip_and_hide_the_token() {
        let c = PlexCredentials::new(
            "server-token-abc".into(),
            "a1b2c3".into(),
            vec![
                Url::parse("https://192-168-1-20.abc.plex.direct:32400").unwrap(),
                Url::parse("http://192.168.1.20:32400").unwrap(),
            ],
        );
        let s = c.to_token_string();
        assert!(s.starts_with("plex1:t=server-token-abc&m=a1b2c3&c="));
        assert_eq!(PlexCredentials::from_token_string(&s), Some(c.clone()));
        assert!(!format!("{c:?}").contains("server-token-abc"));
        assert_eq!(PlexCredentials::from_token_string("plex1:m=x"), None);
        assert_eq!(PlexCredentials::from_token_string("subsonic1:t=x"), None);
    }
}
