//! HTTP behind a trait, so every adapter test runs from recorded fixtures with no
//! server (plan section 4; `tools/fixtures/`).

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::error::{Result, SourceError, Unreachable};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout: Duration,
}

// Hand-written so a token in a header or the URL can never reach a log line.
impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.url.path())
            .field(
                "headers",
                &self
                    .headers
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    /// The server's clock from the HTTP `Date` header, UTC milliseconds. The sync
    /// cursor uses it, so a wrong clock on the phone cannot skip changes.
    pub server_date_utc_ms: Option<i64>,
}

impl Response {
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(|e| SourceError::Protocol(e.to_string()))
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub trait Transport: Send + Sync + fmt::Debug {
    fn send(&self, req: Request) -> BoxFuture<'_, Result<Response>>;
}

/// One recorded exchange in a fixture file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureExchange {
    /// "https://host:port", when the exchange belongs to one address only.
    #[serde(default)]
    pub origin: Option<String>,
    pub method: String,
    pub path: String,
    /// Query parameters the request must carry (others are ignored).
    #[serde(default)]
    pub query: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub status: u16,
    /// The reply body as JSON.
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    /// Or as plain text.
    #[serde(default)]
    pub body_text: Option<String>,
    /// Or as bytes, in hex (an image).
    #[serde(default)]
    pub body_hex: Option<String>,
    /// Or a failure instead of a reply: "nameNotFound", "refused", "timedOut",
    /// "certificateUntrusted:<sha256>".
    #[serde(default)]
    pub error: Option<String>,
    /// The reply's HTTP `Date` header, as sent ("Thu, 06 Nov 2026 15:04:05 GMT") or
    /// in ISO 8601 form.
    #[serde(default)]
    pub date: Option<String>,
}

/// Replays fixture exchanges and remembers every request it was sent. When several
/// recorded exchanges match a request, they are replayed in recorded order (the
/// last one repeats), so a polled endpoint answers "not yet" until it said "yes".
#[derive(Debug)]
pub struct FixtureTransport {
    exchanges: Vec<FixtureExchange>,
    sent: Mutex<Vec<Request>>,
    used: Mutex<Vec<bool>>,
}

impl FixtureTransport {
    pub fn from_json(json: &str) -> Result<FixtureTransport> {
        let exchanges: Vec<FixtureExchange> = serde_json::from_str(json)
            .map_err(|e| SourceError::Protocol(format!("fixture: {e}")))?;
        Ok(FixtureTransport::new(exchanges))
    }

    pub fn new(exchanges: Vec<FixtureExchange>) -> FixtureTransport {
        let used = Mutex::new(vec![false; exchanges.len()]);
        FixtureTransport {
            exchanges,
            sent: Mutex::new(Vec::new()),
            used,
        }
    }

    /// The requests sent so far.
    pub fn sent(&self) -> Vec<Request> {
        self.sent.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn find(&self, req: &Request) -> Option<&FixtureExchange> {
        let pairs: Vec<(String, String)> = req
            .url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let origin = req.url.origin().ascii_serialization();
        let matching: Vec<usize> = self
            .exchanges
            .iter()
            .enumerate()
            .filter(|(_, x)| {
                x.origin.as_deref().is_none_or(|o| o == origin)
                    && x.method.eq_ignore_ascii_case(req.method.as_str())
                    && x.path == req.url.path()
                    && x.query.iter().all(|(k, v)| {
                        pairs
                            .iter()
                            .any(|(pk, pv)| pk.eq_ignore_ascii_case(k) && pv == v)
                    })
            })
            .map(|(i, _)| i)
            .collect();
        let mut used = self.used.lock().ok()?;
        let pick = matching
            .iter()
            .copied()
            .find(|&i| !used[i])
            .or_else(|| matching.last().copied())?;
        used[pick] = true;
        self.exchanges.get(pick)
    }
}

impl Transport for FixtureTransport {
    fn send(&self, req: Request) -> BoxFuture<'_, Result<Response>> {
        let found = self.find(&req).cloned();
        let path = req.url.path().to_owned();
        if let Ok(mut s) = self.sent.lock() {
            s.push(req);
        }
        Box::pin(async move {
            let x = found.ok_or_else(|| SourceError::Protocol(format!("no fixture for {path}")))?;
            if let Some(e) = x.error {
                return Err(match e.as_str() {
                    "nameNotFound" => SourceError::Unreachable(Unreachable::NameNotFound),
                    "refused" => SourceError::Unreachable(Unreachable::Refused),
                    "timedOut" => SourceError::Unreachable(Unreachable::TimedOut),
                    other => match other.strip_prefix("certificateUntrusted:") {
                        Some(sha) => SourceError::CertificateUntrusted {
                            sha256: sha.to_owned(),
                        },
                        None => SourceError::Unreachable(Unreachable::Other(other.to_owned())),
                    },
                });
            }
            let body = match (x.body, x.body_text, x.body_hex) {
                (Some(v), _, _) => serde_json::to_vec(&v).unwrap_or_default(),
                (None, Some(t), _) => t.into_bytes(),
                (None, None, Some(h)) => from_hex(&h).ok_or_else(|| {
                    SourceError::Protocol(format!("fixture: bad bodyHex for {path}"))
                })?,
                (None, None, None) => Vec::new(),
            };
            Ok(Response {
                status: if x.status == 0 { 200 } else { x.status },
                body,
                server_date_utc_ms: x.date.as_deref().and_then(|d| {
                    crate::jellyfin::time::parse_http_date(d)
                        .or_else(|| crate::jellyfin::time::parse_utc_ms(d))
                }),
            })
        })
    }
}

/// Lower- or upper-case hex to bytes; None if it isn't hex.
pub fn from_hex(h: &str) -> Option<Vec<u8>> {
    let h = h.trim();
    if !h.len().is_multiple_of(2) {
        return None;
    }
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(h.get(i..i + 2)?, 16).ok())
        .collect()
}
