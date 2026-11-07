//! A WebSocket client for the companion server's live link (Phase 7; plan section
//! 11). `ws://` on the home network, `wss://` with the same TLS checks as every
//! other connection (`http::tls_config`). The socket runs on its own task; the
//! caller gets two channels of text messages, and either side closing ends it.
//! Pings from the server are answered by the library.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{self, Message};
use url::Url;

use crate::error::{Result, SourceError, Unreachable};

/// At most this many bytes in one message, either way (the server's limit too).
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;

/// An open link: send text on `tx`; text arrives on `rx`, which ends when the link
/// closes (`closed_because` then says why, when the server said).
#[derive(Debug)]
pub struct Link {
    pub tx: mpsc::Sender<String>,
    pub rx: mpsc::Receiver<String>,
    pub closed_because: Arc<std::sync::Mutex<Option<String>>>,
}

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

fn unreachable(what: impl std::fmt::Display) -> SourceError {
    SourceError::Unreachable(Unreachable::Other(what.to_string()))
}

/// Open `url` (http or https; `/ws` is the caller's path) with `headers` (the
/// Authorization header), within `timeout`.
pub async fn connect(
    url: &Url,
    headers: &[(String, String)],
    pinned_sha256: Option<&str>,
    timeout: Duration,
) -> Result<Link> {
    tokio::time::timeout(timeout, open(url, headers, pinned_sha256))
        .await
        .map_err(|_| SourceError::Unreachable(Unreachable::TimedOut))?
}

async fn open(url: &Url, headers: &[(String, String)], pinned: Option<&str>) -> Result<Link> {
    let secure = match url.scheme() {
        "https" | "wss" => true,
        "http" | "ws" => false,
        other => return Err(unreachable(format!("can't open a live link over {other}"))),
    };
    let host = url
        .host_str()
        .ok_or_else(|| unreachable("the address has no host"))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = url
        .port_or_known_default()
        .unwrap_or(if secure { 443 } else { 80 });
    let tcp = tokio::net::TcpStream::connect((host.as_str(), port))
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::ConnectionRefused => SourceError::Unreachable(Unreachable::Refused),
            _ => unreachable(e.kind()),
        })?;
    let _ = tcp.set_nodelay(true);
    let stream: Box<dyn Io> = if secure {
        let (config, rejected) = crate::http::tls_config(pinned)?;
        let name = rustls::pki_types::ServerName::try_from(host.clone())
            .map_err(|_| unreachable("the host name isn't valid"))?;
        let tls = tokio_rustls::TlsConnector::from(Arc::new(config))
            .connect(name, tcp)
            .await
            .map_err(|e| {
                if let Some(sha256) = rejected.lock().ok().and_then(|mut r| r.take()) {
                    SourceError::CertificateUntrusted { sha256 }
                } else {
                    SourceError::Unreachable(Unreachable::Tls(e.to_string()))
                }
            })?;
        Box::new(tls)
    } else {
        Box::new(tcp)
    };
    let mut ws_url = url.clone();
    let _ = ws_url.set_scheme(if secure { "wss" } else { "ws" });
    let mut req = ws_url.as_str().into_client_request().map_err(unreachable)?;
    for (k, v) in headers {
        let name = tungstenite::http::HeaderName::try_from(k.as_str()).map_err(unreachable)?;
        let value = tungstenite::http::HeaderValue::try_from(v.as_str()).map_err(unreachable)?;
        req.headers_mut().insert(name, value);
    }
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let (ws, _) = tokio_tungstenite::client_async_with_config(req, stream, Some(config))
        .await
        .map_err(|e| match e {
            tungstenite::Error::Http(r) if r.status() == 401 => SourceError::SessionExpired,
            tungstenite::Error::Http(r) => {
                unreachable(format!("the server answered {}", r.status()))
            }
            other => unreachable(other),
        })?;
    let (out_tx, mut out_rx) = mpsc::channel::<String>(256);
    let (in_tx, in_rx) = mpsc::channel::<String>(256);
    let closed = Arc::new(std::sync::Mutex::new(None));
    let closed_task = closed.clone();
    tokio::spawn(async move {
        let (mut sink, mut stream) = ws.split();
        loop {
            tokio::select! {
                msg = stream.next() => match msg {
                    Some(Ok(Message::Text(t))) => {
                        if in_tx.send(t.to_string()).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(frame))) => {
                        if let (Some(f), Ok(mut c)) = (frame, closed_task.lock()) {
                            *c = Some(f.reason.to_string());
                        }
                        break;
                    }
                    // Pings: tungstenite queues the pong; the next write or flush sends it.
                    Some(Ok(Message::Ping(_))) => {
                        if sink.flush().await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                },
                out = out_rx.recv() => match out {
                    Some(text) => {
                        if sink.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        let _ = sink.send(Message::Close(None)).await;
                        break;
                    }
                },
            }
        }
    });
    Ok(Link {
        tx: out_tx,
        rx: in_rx,
        closed_because: closed,
    })
}
