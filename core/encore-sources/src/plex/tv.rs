//! plex.tv, as far as signing in needs it (ADR core-0023 points 2 to 4): the PIN link,
//! the account's ID, and the account's servers with their addresses.
//!
//! - `POST /api/v2/pins` makes a short code the user enters at plex.tv/link (or approves
//!   on Plex's own sign-in page, opened with the code); `GET /api/v2/pins/{id}` answers
//!   with the account's token once that is done, and 404 once the code has expired.
//! - `GET /api/v2/user` names the account (its number and user name, nothing else is
//!   kept). `GET /api/v2/resources` lists the servers this account may use, each with its
//!   own access token and its addresses.
//!
//! Every request names the app ("Encore") and a client identifier that is random per
//! install (`client_identifier`), never the device's name or anything about the person.
//! The account token is held only while signing in; only the chosen server's token is
//! kept (`PlexServerResource::access_token`, into the keychain).
//!
//! These calls are tested on hand-made fixtures only (tools/fixtures/plex/plex-tv.json):
//! no test or CI job contacts the real plex.tv.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::{Result, SourceError};
use crate::subsonic::dto::{flag, int, many, text};
use crate::transport::{Method, Request, Response, Transport};

/// Where plex.tv is. Tests point a `PlexTv` at the same address through a fixture.
pub const PLEX_TV: &str = "https://plex.tv";
/// The page where a short code is entered.
pub const LINK_PAGE: &str = "https://plex.tv/link";
/// Plex's own sign-in page, which takes the code in its address.
pub const AUTH_PAGE: &str = "https://app.plex.tv/auth#";

/// What Encore calls itself to Plex: the product name and its headers.
#[derive(Clone, PartialEq, Eq)]
pub struct PlexIdentity {
    /// "Encore" (the brand constant).
    pub product: String,
    pub version: String,
    /// "android", "ios"... (the platform's name, not the device's).
    pub platform: String,
    /// Random per install: see [`client_identifier`].
    pub client_id: String,
}

impl fmt::Debug for PlexIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexIdentity")
            .field("product", &self.product)
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl PlexIdentity {
    /// The `X-Plex-*` headers every request carries. The device name is the product's
    /// name, so the account's device list never shows a person's device name.
    pub fn headers(&self) -> Vec<(String, String)> {
        vec![
            ("Accept".into(), "application/json".into()),
            ("X-Plex-Product".into(), self.product.clone()),
            ("X-Plex-Version".into(), self.version.clone()),
            ("X-Plex-Client-Identifier".into(), self.client_id.clone()),
            ("X-Plex-Platform".into(), self.platform.clone()),
            ("X-Plex-Device-Name".into(), self.product.clone()),
        ]
    }
}

/// The Plex client identifier for an install: SHA-256 of a fixed label and the app's
/// per-install random device ID, first 16 bytes in hex. Stable for the install, so
/// the account's device list shows one entry for it, and not the device ID itself, so
/// nothing links what Plex sees to the companion server or another media server.
pub fn client_identifier(device_id: &str) -> String {
    Sha256::digest(format!("encore-plex-client\u{1f}{device_id}").as_bytes())[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A link code waiting for the user.
#[derive(Clone, PartialEq, Eq)]
pub struct PlexPin {
    /// Kept to poll; not shown.
    pub id: String,
    /// The short code to enter at plex.tv/link.
    pub code: String,
    /// How long plex.tv keeps the code.
    pub expires_in_seconds: u32,
    /// Plex's sign-in page with the code filled in, for a "Open in browser" button.
    pub auth_url: String,
}

impl fmt::Debug for PlexPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexPin")
            .field("expires_in_seconds", &self.expires_in_seconds)
            .finish_non_exhaustive()
    }
}

/// The account token plex.tv hands over once the code is approved. Held only while
/// signing in; never stored, never logged.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountToken(pub(crate) String);

impl fmt::Debug for AccountToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccountToken(..)")
    }
}

impl AccountToken {
    pub fn new(token: String) -> AccountToken {
        AccountToken(token)
    }
}

/// Who signed in: the account's number (stable) and its user name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexAccount {
    pub id: String,
    pub user_name: String,
}

/// One address of a server, as plex.tv lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexConnection {
    pub uri: Url,
    pub local: bool,
    pub relay: bool,
}

/// A server the account may use.
#[derive(Clone, PartialEq, Eq)]
pub struct PlexServerResource {
    pub name: String,
    /// The server's `machineIdentifier`.
    pub machine_id: String,
    pub version: String,
    /// The account owns it (else it was shared with the account).
    pub owned: bool,
    /// This account's token for this server: the only token kept.
    pub access_token: String,
    /// In the order to try them (`order_connections`).
    pub connections: Vec<PlexConnection>,
}

impl fmt::Debug for PlexServerResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexServerResource")
            .field("name", &self.name)
            .field("machine_id", &self.machine_id)
            .field("owned", &self.owned)
            .field("connections", &self.connections.len())
            .finish_non_exhaustive()
    }
}

/// The order to try a server's addresses in: on the local network before away from it,
/// https before plain http, Plex's relay last. Plain http away from the local network
/// is dropped (board 21: never plain http over the internet).
pub fn order_connections(mut list: Vec<PlexConnection>) -> Vec<PlexConnection> {
    list.retain(|c| crate::jellyfin::address::check_security(&c.uri).is_ok());
    list.sort_by_key(|c| (c.relay, !c.local, c.uri.scheme() != "https"));
    list.dedup_by(|a, b| a.uri == b.uri);
    list
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinDto {
    #[serde(default, deserialize_with = "text")]
    id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    code: Option<String>,
    #[serde(default, deserialize_with = "int")]
    expires_in: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    auth_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserDto {
    #[serde(default, deserialize_with = "text")]
    id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    username: Option<String>,
    #[serde(default, deserialize_with = "text")]
    title: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionDto {
    #[serde(default, deserialize_with = "text")]
    uri: Option<String>,
    #[serde(default, deserialize_with = "flag")]
    local: Option<bool>,
    #[serde(default, deserialize_with = "flag")]
    relay: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceDto {
    #[serde(default, deserialize_with = "text")]
    name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    product_version: Option<String>,
    #[serde(default, deserialize_with = "text")]
    client_identifier: Option<String>,
    #[serde(default, deserialize_with = "text")]
    provides: Option<String>,
    #[serde(default, deserialize_with = "flag")]
    owned: Option<bool>,
    #[serde(default, deserialize_with = "text")]
    access_token: Option<String>,
    #[serde(default, deserialize_with = "many")]
    connections: Vec<ConnectionDto>,
}

/// plex.tv for one install.
pub struct PlexTv {
    transport: Arc<dyn Transport>,
    base: Url,
    identity: PlexIdentity,
    timeout: Duration,
}

impl fmt::Debug for PlexTv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlexTv").finish_non_exhaustive()
    }
}

impl PlexTv {
    pub fn new(transport: Arc<dyn Transport>, identity: PlexIdentity, timeout: Duration) -> PlexTv {
        PlexTv {
            transport,
            // A constant that parses.
            base: Url::parse(PLEX_TV).unwrap_or_else(|_| unreachable!()),
            identity,
            timeout,
        }
    }

    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let mut u = self
            .base
            .join(path)
            .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
        if !query.is_empty() {
            u.query_pairs_mut().extend_pairs(query);
        }
        Ok(u)
    }

    async fn send(
        &self,
        method: Method,
        url: Url,
        token: Option<&AccountToken>,
    ) -> Result<Response> {
        let mut headers = self.identity.headers();
        if let Some(t) = token {
            headers.push(("X-Plex-Token".into(), t.0.clone()));
        }
        self.transport
            .send(Request {
                method,
                url,
                headers,
                body: None,
                timeout: self.timeout,
            })
            .await
    }

    fn check(resp: Response, unauthorized: SourceError) -> Result<Response> {
        match resp.status {
            200..=299 => Ok(resp),
            401 | 403 => Err(unauthorized),
            404 => Err(SourceError::NotFound),
            status => Err(SourceError::Server {
                status,
                message: resp.text().chars().take(300).collect(),
            }),
        }
    }

    /// A new short link code (`strong=false`: four characters to type at plex.tv/link).
    pub async fn start_pin(&self) -> Result<PlexPin> {
        let url = self.url("/api/v2/pins", &[("strong", "false")])?;
        let resp = self.send(Method::Post, url, None).await?;
        let pin: PinDto = Self::check(resp, SourceError::WrongCredentials)?.json()?;
        let id = pin
            .id
            .filter(|i| !i.is_empty())
            .ok_or_else(|| SourceError::Protocol("plex.tv sent no PIN".into()))?;
        let code = pin
            .code
            .filter(|c| !c.is_empty())
            .ok_or_else(|| SourceError::Protocol("plex.tv sent no code".into()))?;
        let mut auth = url::form_urlencoded::Serializer::new(String::new());
        auth.append_pair("clientID", &self.identity.client_id);
        auth.append_pair("code", &code);
        auth.append_pair("context[device][product]", &self.identity.product);
        Ok(PlexPin {
            auth_url: format!("{AUTH_PAGE}?{}", auth.finish()),
            id,
            code,
            expires_in_seconds: pin
                .expires_in
                .and_then(|s| u32::try_from(s).ok())
                .filter(|s| *s > 0)
                .unwrap_or(900),
        })
    }

    /// Has the code been approved? The account token once it has, None while it waits;
    /// `QuickConnectExpired` once plex.tv no longer knows the code.
    pub async fn poll_pin(&self, pin: &PlexPin) -> Result<Option<AccountToken>> {
        let url = self.url(
            &format!("/api/v2/pins/{}", pin.id),
            &[("code", pin.code.as_str())],
        )?;
        let resp = self.send(Method::Get, url, None).await?;
        let dto: PinDto = match Self::check(resp, SourceError::QuickConnectExpired) {
            Err(SourceError::NotFound) => return Err(SourceError::QuickConnectExpired),
            other => other?.json()?,
        };
        Ok(dto
            .auth_token
            .filter(|t| !t.trim().is_empty())
            .map(AccountToken))
    }

    /// The signed-in account: its number and user name (`/api/v2/user`; the rest of
    /// the reply, its e-mail and picture among it, is not read).
    pub async fn account(&self, token: &AccountToken) -> Result<PlexAccount> {
        let url = self.url("/api/v2/user", &[])?;
        let resp = self.send(Method::Get, url, Some(token)).await?;
        let u: UserDto = Self::check(resp, SourceError::WrongCredentials)?.json()?;
        let id =
            u.id.filter(|i| !i.is_empty())
                .ok_or_else(|| SourceError::Protocol("plex.tv sent no account".into()))?;
        Ok(PlexAccount {
            user_name: u
                .username
                .or(u.title)
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| "Plex".to_owned()),
            id,
        })
    }

    /// The Plex Media Servers this token may use, with their addresses in the order to
    /// try them. Players and other devices on the account are left out.
    pub async fn servers(&self, token: &AccountToken) -> Result<Vec<PlexServerResource>> {
        let url = self.url(
            "/api/v2/resources",
            &[
                ("includeHttps", "1"),
                ("includeRelay", "1"),
                ("includeIPv6", "1"),
            ],
        )?;
        let resp = self.send(Method::Get, url, Some(token)).await?;
        let list: Vec<serde_json::Value> =
            Self::check(resp, SourceError::WrongCredentials)?.json()?;
        Ok(list
            .into_iter()
            .filter_map(|v| serde_json::from_value::<ResourceDto>(v).ok())
            .filter(|r| {
                r.provides
                    .as_deref()
                    .is_some_and(|p| p.split(',').any(|x| x.trim() == "server"))
            })
            .filter_map(|r| {
                let connections = r
                    .connections
                    .into_iter()
                    .filter_map(|c| {
                        Some(PlexConnection {
                            uri: Url::parse(c.uri.as_deref()?).ok()?,
                            local: c.local.unwrap_or(false),
                            relay: c.relay.unwrap_or(false),
                        })
                    })
                    .collect();
                Some(PlexServerResource {
                    name: r
                        .name
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or_else(|| "Plex Media Server".to_owned()),
                    machine_id: r.client_identifier.filter(|m| !m.is_empty())?,
                    version: r.product_version.unwrap_or_default(),
                    owned: r.owned.unwrap_or(false),
                    access_token: r.access_token.filter(|t| !t.is_empty())?,
                    connections: order_connections(connections),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(uri: &str, local: bool, relay: bool) -> PlexConnection {
        PlexConnection {
            uri: Url::parse(uri).unwrap(),
            local,
            relay,
        }
    }

    #[test]
    fn addresses_are_tried_local_first_https_first_relay_last() {
        let ordered = order_connections(vec![
            conn("https://relay.example.plex.direct:8443", false, true),
            conn("https://203-0-113-9.abc.plex.direct:32400", false, false),
            conn("http://203.0.113.9:32400", false, false),
            conn("http://192.168.1.20:32400", true, false),
            conn("https://192-168-1-20.abc.plex.direct:32400", true, false),
        ]);
        let uris: Vec<&str> = ordered.iter().map(|c| c.uri.as_str()).collect();
        assert_eq!(
            uris,
            [
                "https://192-168-1-20.abc.plex.direct:32400/",
                "http://192.168.1.20:32400/",
                "https://203-0-113-9.abc.plex.direct:32400/",
                "https://relay.example.plex.direct:8443/",
            ],
            "plain http over the internet is never used"
        );
    }

    #[test]
    fn the_client_identifier_is_stable_and_not_the_device_id() {
        let a = client_identifier("device-1");
        assert_eq!(a, client_identifier("device-1"));
        assert_ne!(a, client_identifier("device-2"));
        assert_eq!(a.len(), 32);
        assert!(!a.contains("device"));
    }
}
