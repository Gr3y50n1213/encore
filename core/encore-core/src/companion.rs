//! The companion server's client (plan section 5; docs/api/openapi.yaml): sign-in
//! by media-server token, token refresh, push, pull with long-poll, devices.
//!
//! Tokens never reach a log: errors carry the server's sentence, and `Tokens`
//! prints none of its fields.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use encore_sources::Transport;
use encore_sources::transport::{Method, Request};
use encore_store::{InEvent, OutEvent};
use serde_json::{Value, json};
use url::{Host, Url};

/// How long a long-poll pull waits on the server for something new.
pub(crate) const PULL_WAIT_S: u64 = 25;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Tokens {
    pub access: String,
    pub access_expires_utc_ms: i64,
    pub refresh: String,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("access_expires_utc_ms", &self.access_expires_utc_ms)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompanionError {
    /// The server no longer accepts this device (revoked, profile deleted, or the
    /// refresh token is gone). Sign in again.
    SignedOut(String),
    /// Not reachable now, or busy: try again later.
    Unreachable(String),
    /// The server refused the request as it is.
    Refused(String),
}

impl CompanionError {
    pub(crate) fn reason(&self) -> &str {
        match self {
            CompanionError::SignedOut(r)
            | CompanionError::Unreachable(r)
            | CompanionError::Refused(r) => r,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub created_utc_ms: i64,
    pub last_seen_utc_ms: i64,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pulled {
    pub events: Vec<InEvent>,
    pub cursor: i64,
    pub more: bool,
}

fn is_local(host: Host<&str>) -> bool {
    match host {
        Host::Ipv4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || (o[0] == 100 && (o[1] & 0xC0) == 64)
        }
        Host::Ipv6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xFE00) == 0xFC00 || (first & 0xFFC0) == 0xFE80
        }
        Host::Domain(d) => {
            if let Ok(ip) = d.parse::<IpAddr>() {
                return match ip {
                    IpAddr::V4(v4) => is_local(Host::Ipv4(v4)),
                    IpAddr::V6(v6) => is_local(Host::Ipv6(v6)),
                };
            }
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d == "localhost"
                || !d.contains('.')
                || [".local", ".home.arpa", ".lan", ".internal"]
                    .iter()
                    .any(|s| d.ends_with(s))
        }
    }
}

/// The companion's address as the user typed it: https anywhere, plain http only
/// on the home network (plan section 12). A missing scheme means http on a home
/// address and https elsewhere.
pub(crate) fn companion_url(typed: &str) -> Result<Url, String> {
    let t = typed.trim().trim_end_matches('/');
    if t.is_empty() {
        return Err("Type the companion server's address".into());
    }
    let with_scheme = if t.contains("://") {
        t.to_owned()
    } else {
        format!("http://{t}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|_| format!("{t:?} isn't an address"))?;
    let local = url.host().is_some_and(is_local);
    if !t.contains("://") && !local {
        url.set_scheme("https")
            .map_err(|_| format!("{t:?} isn't an address"))?;
    }
    match url.scheme() {
        "https" => {}
        "http" if local => {}
        "http" => {
            return Err(
                "Plain http is only allowed on your home network; use https for this address"
                    .into(),
            );
        }
        _ => return Err("Use an http or https address".into()),
    }
    if url.host().is_none() {
        return Err(format!("{t:?} isn't an address"));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

#[derive(Debug, Clone)]
pub(crate) struct Client {
    base: Url,
    transport: Arc<dyn Transport>,
}

fn tokens_from(v: &Value) -> Result<Tokens, CompanionError> {
    let s = |k: &str| v[k].as_str().map(str::to_owned);
    Ok(Tokens {
        access: s("access_token").ok_or_else(|| bad("no access token in the reply"))?,
        access_expires_utc_ms: v["access_expires_utc_ms"]
            .as_i64()
            .ok_or_else(|| bad("no expiry in the reply"))?,
        refresh: s("refresh_token").ok_or_else(|| bad("no refresh token in the reply"))?,
    })
}

fn bad(what: &str) -> CompanionError {
    CompanionError::Refused(format!(
        "That doesn't look like an Encore companion server ({what})"
    ))
}

impl Client {
    pub(crate) fn new(base: Url, transport: Arc<dyn Transport>) -> Client {
        Client { base, transport }
    }

    pub(crate) fn base(&self) -> &Url {
        &self.base
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, CompanionError> {
        let url = Url::parse(&format!(
            "{}{path}",
            self.base.as_str().trim_end_matches('/')
        ))
        .map_err(|e| CompanionError::Refused(e.to_string()))?;
        let mut headers = vec![("Accept".to_owned(), "application/json".to_owned())];
        if let Some(t) = token {
            headers.push(("Authorization".to_owned(), format!("Bearer {t}")));
        }
        if body.is_some() {
            headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
        }
        let resp = self
            .transport
            .send(Request {
                method,
                url,
                headers,
                body: body.map(|b| b.to_string().into_bytes()),
                timeout,
            })
            .await
            .map_err(|e| CompanionError::Unreachable(e.to_string()))?;
        let value: Value = if resp.body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&resp.body).unwrap_or(Value::Null)
        };
        let reason = || {
            value["reason"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("the companion server answered {}", resp.status))
        };
        match resp.status {
            200..=299 => Ok(value),
            401 => Err(CompanionError::SignedOut(reason())),
            408 | 429 | 500..=599 => Err(CompanionError::Unreachable(reason())),
            _ => Err(CompanionError::Refused(reason())),
        }
    }

    const QUICK: Duration = Duration::from_secs(15);

    /// `GET /health`: is this an Encore companion server?
    pub(crate) async fn health(&self) -> Result<String, CompanionError> {
        let v = self
            .send(Method::Get, "/health", None, None, Self::QUICK)
            .await?;
        match (v["status"].as_str(), v["server_version"].as_str()) {
            (Some("ok"), Some(version)) => Ok(version.to_owned()),
            _ => Err(bad("its health check didn't answer")),
        }
    }

    /// `POST /auth/exchange`: (the server's ID for this device, its tokens).
    pub(crate) async fn exchange(
        &self,
        server_id: &str,
        media_token: &str,
        device_id: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<(String, Tokens), CompanionError> {
        let v = self
            .send(
                Method::Post,
                "/auth/exchange",
                None,
                Some(json!({
                    "server_id": server_id, "media_token": media_token, "device_id": device_id,
                    "device_name": device_name, "platform": platform,
                })),
                Self::QUICK,
            )
            .await?;
        let id = v["device_id"]
            .as_str()
            .ok_or_else(|| bad("no device ID in the reply"))?;
        Ok((id.to_owned(), tokens_from(&v)?))
    }

    /// `POST /auth/refresh`: a new pair; the old refresh token is used up.
    pub(crate) async fn refresh(&self, refresh: &str) -> Result<Tokens, CompanionError> {
        let v = self
            .send(
                Method::Post,
                "/auth/refresh",
                None,
                Some(json!({ "refresh_token": refresh })),
                Self::QUICK,
            )
            .await?;
        tokens_from(&v)
    }

    /// `POST /sync/push`: the IDs the server now holds.
    pub(crate) async fn push(
        &self,
        access: &str,
        events: &[OutEvent],
    ) -> Result<Vec<String>, CompanionError> {
        let list: Vec<Value> = events
            .iter()
            .map(|e| {
                json!({ "id": e.id, "kind": e.kind, "hlc": e.hlc, "version": e.version,
                        "payload": e.payload, "created_utc_ms": e.created_utc_ms })
            })
            .collect();
        let v = self
            .send(
                Method::Post,
                "/sync/push",
                Some(access),
                Some(json!({ "events": list })),
                Duration::from_secs(30),
            )
            .await?;
        Ok(v["accepted"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| i.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// `GET /sync/pull`: other devices' events after `cursor`, waiting up to
    /// `wait_s` for one.
    pub(crate) async fn pull(
        &self,
        access: &str,
        cursor: i64,
        wait_s: u64,
    ) -> Result<Pulled, CompanionError> {
        let v = self
            .send(
                Method::Get,
                &format!("/sync/pull?cursor={cursor}&wait={wait_s}"),
                Some(access),
                None,
                Duration::from_secs(wait_s + 15),
            )
            .await?;
        let events = v["events"]
            .as_array()
            .ok_or_else(|| bad("no events in the pull"))?
            .iter()
            .filter_map(|e| {
                Some(InEvent {
                    id: e["id"].as_str()?.to_owned(),
                    device_id: e["device_id"].as_str()?.to_owned(),
                    kind: e["kind"].as_str()?.to_owned(),
                    hlc: e["hlc"].as_str()?.to_owned(),
                    version: e["version"].as_i64().unwrap_or(0),
                    payload: e["payload"].clone(),
                    created_utc_ms: e["created_utc_ms"].as_i64().unwrap_or(0),
                })
            })
            .collect();
        Ok(Pulled {
            events,
            cursor: v["cursor"].as_i64().unwrap_or(cursor),
            more: v["more"].as_bool().unwrap_or(false),
        })
    }

    /// `GET /devices`: (this device's ID, every device of the profile).
    pub(crate) async fn devices(
        &self,
        access: &str,
    ) -> Result<(String, Vec<Device>), CompanionError> {
        let v = self
            .send(Method::Get, "/devices", Some(access), None, Self::QUICK)
            .await?;
        let list = v["devices"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|d| {
                        Some(Device {
                            id: d["id"].as_str()?.to_owned(),
                            name: d["name"].as_str()?.to_owned(),
                            platform: d["platform"].as_str().unwrap_or("").to_owned(),
                            created_utc_ms: d["created_utc_ms"].as_i64().unwrap_or(0),
                            last_seen_utc_ms: d["last_seen_utc_ms"].as_i64().unwrap_or(0),
                            revoked: d["revoked"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok((v["this_device"].as_str().unwrap_or("").to_owned(), list))
    }

    /// `DELETE /devices/{id}`.
    pub(crate) async fn revoke(&self, access: &str, id: &str) -> Result<(), CompanionError> {
        let id: String = url::form_urlencoded::byte_serialize(id.as_bytes()).collect();
        self.send(
            Method::Delete,
            &format!("/devices/{id}"),
            Some(access),
            None,
            Self::QUICK,
        )
        .await
        .map(drop)
    }

    /// `DELETE /profile`: everything of this profile on the server (board 19's
    /// delete, called by `delete_profile_everywhere`).
    pub(crate) async fn delete_profile(&self, access: &str) -> Result<(), CompanionError> {
        self.send(Method::Delete, "/profile", Some(access), None, Self::QUICK)
            .await
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_only_on_the_home_network() {
        assert_eq!(
            companion_url("192.168.1.10:8097").unwrap().as_str(),
            "http://192.168.1.10:8097/"
        );
        assert_eq!(companion_url("encore:8097").unwrap().scheme(), "http");
        assert_eq!(companion_url("nas.local:8097/").unwrap().scheme(), "http");
        assert_eq!(
            companion_url("music.example.com").unwrap().scheme(),
            "https"
        );
        assert!(companion_url("http://music.example.com").is_err());
        assert!(
            companion_url("https://music.example.com/?x=1")
                .unwrap()
                .query()
                .is_none()
        );
        assert!(companion_url("ftp://x").is_err());
        assert!(companion_url("  ").is_err());
        assert_eq!(
            companion_url("http://[fd00::5]:8097").unwrap().scheme(),
            "http"
        );
    }
}
