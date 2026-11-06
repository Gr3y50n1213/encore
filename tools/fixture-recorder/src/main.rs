//! Records the Jellyfin adapter's HTTP exchanges from a real server and writes them,
//! anonymized, to tools/fixtures/jellyfin/ (plan section 4: every adapter ships
//! recorded fixtures; docs/adr/core-0005).
//!
//!   cargo run -p fixture-recorder -- [--server http://localhost:8096] [--limit 150]
//!
//! It signs in with Quick Connect, so no password is typed anywhere: it prints a
//! 6-character code, you approve it in Jellyfin (your profile, Quick Connect), and
//! it continues by itself. At the end it signs out, which removes its token.
//!
//! What it records, through the same adapter code the app runs: the server probe,
//! Quick Connect, a first library sync, a second (delta) sync, and lyrics for one
//! song that has them and one that does not. Before writing, everything that names
//! the library is replaced (anonymize.rs), and a leak check refuses to write if any
//! original value survives.
//!
//! Rewrites, so the fixture stays small (written down in RECORDING.md too):
//! - each item type is cut to the first `--limit` items, and `TotalRecordCount` is
//!   cut to match, so paging stops there;
//! - the ID listing used to find removals is answered from the recorded items
//!   instead of the whole library.

mod anonymize;

use std::collections::{BTreeMap, HashSet};
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use encore_sources::http::HttpTransport;
use encore_sources::jellyfin::{self, DeviceInfo, JellyfinClient};
use encore_sources::transport::{BoxFuture, Request, Response, Transport};
use encore_sources::{Result as SourceResult, SourceError};
use serde_json::{Value, json};
use url::Url;

use crate::anonymize::{Anonymizer, SAFE_PARAMS, looks_like_id};

/// One exchange as it happened.
#[derive(Debug, Clone)]
struct Exchange {
    method: &'static str,
    url: Url,
    outcome: std::result::Result<Response, SourceError>,
}

#[derive(Debug)]
struct Recorder {
    inner: Arc<dyn Transport>,
    base_path: String,
    limit: u64,
    log: Mutex<Vec<Exchange>>,
    kept: Mutex<Vec<String>>,
}

fn query(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

impl Recorder {
    fn is_items(&self, url: &Url) -> bool {
        url.path() == format!("{}/Items", self.base_path)
    }

    /// Cut a single-type `/Items` page to the first `limit` items overall.
    fn trim(&self, url: &Url, resp: Response) -> Response {
        let Ok(mut v) = serde_json::from_slice::<Value>(&resp.body) else {
            return resp;
        };
        let start: u64 = query(url, "startIndex")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let keep = self.limit.saturating_sub(start) as usize;
        if let Some(items) = v.get_mut("Items").and_then(Value::as_array_mut) {
            items.truncate(keep);
            if let Ok(mut kept) = self.kept.lock() {
                kept.extend(
                    items
                        .iter()
                        .filter_map(|i| i.get("Id").and_then(Value::as_str).map(str::to_owned)),
                );
            }
        }
        if let Some(total) = v.get("TotalRecordCount").and_then(Value::as_u64) {
            v["TotalRecordCount"] = json!(total.min(self.limit));
        }
        Response {
            status: resp.status,
            body: serde_json::to_vec(&v).unwrap_or_default(),
            server_date_utc_ms: resp.server_date_utc_ms,
        }
    }

    /// The ID listing, answered from what was recorded.
    fn id_listing(&self) -> Response {
        let kept = self.kept.lock().map(|k| k.clone()).unwrap_or_default();
        let unique: Vec<String> = {
            let mut seen = HashSet::new();
            kept.into_iter()
                .filter(|id| seen.insert(id.clone()))
                .collect()
        };
        let body = json!({
            "Items": unique.iter().map(|id| json!({"Id": id})).collect::<Vec<_>>(),
            "TotalRecordCount": unique.len(),
            "StartIndex": 0
        });
        Response {
            status: 200,
            body: serde_json::to_vec(&body).unwrap_or_default(),
            server_date_utc_ms: None,
        }
    }
}

impl Transport for Recorder {
    fn send(&self, req: Request) -> BoxFuture<'_, SourceResult<Response>> {
        Box::pin(async move {
            let method = req.method.as_str();
            let url = req.url.clone();
            let types = query(&url, "includeItemTypes").unwrap_or_default();
            let outcome = if self.is_items(&url) && types.contains(',') {
                Ok(self.id_listing())
            } else {
                match self.inner.send(req).await {
                    Ok(r) if self.is_items(&url) && r.status == 200 => Ok(self.trim(&url, r)),
                    other => other,
                }
            };
            if let Ok(mut log) = self.log.lock() {
                log.push(Exchange {
                    method,
                    url,
                    outcome: outcome.clone(),
                });
            }
            outcome
        })
    }
}

/// 256 bits from the OS: std's hash maps are keyed from OS randomness.
fn random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    for (i, chunk) in key.chunks_mut(8).enumerate() {
        chunk.copy_from_slice(&RandomState::new().hash_one(i).to_le_bytes());
    }
    key
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Args {
    server: String,
    limit: u64,
    page_size: u32,
    out: PathBuf,
}

fn args() -> std::result::Result<Args, String> {
    let mut a = Args {
        server: "http://localhost:8096".into(),
        limit: 150,
        page_size: 50,
        out: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/jellyfin")),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--server" => a.server = value()?,
            "--limit" => a.limit = value()?.parse().map_err(|_| "--limit needs a number")?,
            "--page-size" => {
                a.page_size = value()?.parse().map_err(|_| "--page-size needs a number")?
            }
            "--out" => a.out = PathBuf::from(value()?),
            "-h" | "--help" => {
                return Err(include_str!("main.rs")
                    .lines()
                    .take(20)
                    .map(|l| l.trim_start_matches("//!"))
                    .collect::<Vec<_>>()
                    .join("\n"));
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(a)
}

/// Replace the host, keeping the scheme and port, so plain http on the LAN stays
/// plain http on the LAN.
fn anonymous_origin(url: &Url) -> String {
    let local = url
        .host()
        .map(|h| jellyfin::address::is_local_host(&h))
        .unwrap_or(false);
    let host = if local {
        "jellyfin.local"
    } else {
        "jellyfin.example"
    };
    match url.port() {
        Some(p) => format!("{}://{host}:{p}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    }
}

fn to_fixture(a: &mut Anonymizer, x: &Exchange) -> Value {
    let path: String = x
        .url
        .path()
        .split('/')
        .map(|s| {
            if looks_like_id(s) {
                a.id(s)
            } else {
                s.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let mut q = BTreeMap::new();
    for (k, v) in x.url.query_pairs() {
        let v = if SAFE_PARAMS.contains(&k.as_ref()) {
            v.into_owned()
        } else {
            a.value(&v)
        };
        q.insert(k.into_owned(), v);
    }
    let mut out =
        json!({"origin": anonymous_origin(&x.url), "method": x.method, "path": path, "query": q});
    match &x.outcome {
        Ok(r) => {
            out["status"] = json!(r.status);
            if let Some(ms) = r.server_date_utc_ms {
                out["date"] = json!(jellyfin::time::format_utc(ms));
            }
            match serde_json::from_slice::<Value>(&r.body) {
                Ok(v) => out["body"] = a.json(&v),
                Err(_) if r.body.is_empty() => {}
                Err(_) => out["bodyText"] = json!("(non-JSON reply removed)"),
            }
        }
        Err(e) => {
            out["error"] = json!(match e {
                SourceError::Unreachable(encore_sources::Unreachable::NameNotFound) =>
                    "nameNotFound".to_owned(),
                SourceError::Unreachable(encore_sources::Unreachable::Refused) =>
                    "refused".to_owned(),
                SourceError::Unreachable(encore_sources::Unreachable::TimedOut) =>
                    "timedOut".to_owned(),
                SourceError::CertificateUntrusted { .. } => "certificateUntrusted:0000".to_owned(),
                other => format!("{other}"),
            });
        }
    }
    out
}

async fn run() -> std::result::Result<(), String> {
    let a = args()?;
    let key = random_key();
    let device = DeviceInfo {
        client: "Encore".into(),
        device_name: "Encore fixture recorder".into(),
        device_id: hex(&key[..8]),
        version: env!("CARGO_PKG_VERSION").into(),
    };
    let timeout = Duration::from_secs(15);
    let http: Arc<dyn Transport> =
        Arc::new(HttpTransport::new(None, Duration::from_secs(5)).map_err(|e| e.to_string())?);
    let plan = jellyfin::plan_address(&a.server, &[443, 8920], 8096).map_err(|e| e.to_string())?;
    let base_path = plan
        .candidates
        .first()
        .map(|u| u.path().trim_end_matches('/').to_owned())
        .unwrap_or_default();
    let recorder = Arc::new(Recorder {
        inner: http,
        base_path,
        limit: a.limit,
        log: Mutex::new(Vec::new()),
        kept: Mutex::new(Vec::new()),
    });
    let transport: Arc<dyn Transport> = recorder.clone();

    let info = jellyfin::probe(transport.clone(), &plan, &device, timeout)
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "Found {} (Jellyfin {}) at {}",
        info.name, info.version, info.base_url
    );
    let client = JellyfinClient::new(
        transport.clone(),
        info.base_url.clone(),
        device.clone(),
        timeout,
    );
    if !client
        .quick_connect_enabled()
        .await
        .map_err(|e| e.to_string())?
    {
        return Err("Quick Connect is off on this server. Turn it on in Dashboard > General, then run this again.".into());
    }
    let code = client
        .quick_connect_start()
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "\nApprove this code in Jellyfin (your profile > Quick Connect):\n\n    {}\n\nWaiting up to 5 minutes...",
        code.code
    );
    let mut approved = false;
    for _ in 0..100 {
        if client
            .quick_connect_poll(&code.secret)
            .await
            .map_err(|e| e.to_string())?
        {
            approved = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    if !approved {
        return Err("The code wasn't approved in time. Nothing was written.".into());
    }
    let session = client
        .quick_connect_finish(&code.secret)
        .await
        .map_err(|e| e.to_string())?;
    println!("Signed in as {}.", session.user_name);
    let client = client.signed_in(session.token.clone(), session.user_id.clone());

    let progress = |p: jellyfin::SyncProgress| print!("\rFirst sync: {} of {}   ", p.done, p.total);
    let first = jellyfin::sync_library(&client, None, &HashSet::new(), a.page_size, &progress)
        .await
        .map_err(|e| e.to_string())?;
    println!();
    let known: HashSet<String> = first
        .artists
        .iter()
        .map(|x| x.source_item_id.clone())
        .chain(first.albums.iter().map(|x| x.source_item_id.clone()))
        .chain(first.tracks.iter().map(|x| x.source_item_id.clone()))
        .chain(first.playlists.iter().map(|x| x.source_item_id.clone()))
        .collect();
    let second =
        jellyfin::sync_library(&client, first.cursor.as_ref(), &known, a.page_size, &|_| {})
            .await
            .map_err(|e| e.to_string())?;
    let with_lyrics = first
        .tracks
        .iter()
        .find(|t| t.has_lyrics)
        .map(|t| t.source_item_id.clone());
    let without = first
        .tracks
        .iter()
        .find(|t| !t.has_lyrics)
        .map(|t| t.source_item_id.clone());
    for id in [&with_lyrics, &without].into_iter().flatten() {
        client.lyrics(id).await.map_err(|e| e.to_string())?;
    }
    client.sign_out().await.map_err(|e| e.to_string())?;
    println!("Signed out; the recorder's token is gone from the server.");

    // Anonymize, check, write.
    let mut anon = Anonymizer::new(key);
    let log = recorder.log.lock().map(|l| l.clone()).unwrap_or_default();
    let exchanges = Value::Array(log.iter().map(|x| to_fixture(&mut anon, x)).collect());
    if let Some(path) = anon.find_leak(&exchanges) {
        return Err(format!(
            "an original value survived at {path}; nothing was written"
        ));
    }
    // find_leak checks every value, including request paths and queries. JSON keys
    // are Jellyfin's schema names, so a plain-text scan of the file would flag a
    // replaced word like "Default" that is also part of a key.
    let published = serde_json::to_string_pretty(&exchanges).map_err(|e| e.to_string())?;
    for secret in [
        &session.token,
        &code.secret,
        &session.user_id,
        &info.server_id,
        &session.user_name,
    ] {
        if secret.len() >= 3 && published.contains(secret.as_str()) {
            return Err("an identifying value survived anonymization; nothing was written".into());
        }
    }
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    std::fs::write(a.out.join("recorded.json"), format!("{published}\n"))
        .map_err(|e| e.to_string())?;
    let notes = format!(
        "# Jellyfin fixtures\n\n\
         `recorded.json` is a recording of the Jellyfin adapter talking to a real Jellyfin {version} server,\n\
         made with `tools/fixture-recorder`. The adapter tests replay it with no server\n\
         (`core/encore-sources/src/jellyfin/recorded_tests.rs`).\n\n\
         - Recorded through the adapter's own code: the probe, Quick Connect, a first library sync,\n\
           a second (delta) sync, and lyrics for one song with lyrics and one without.\n\
         - Trimmed: each item type cut to the first {limit} items, with `TotalRecordCount` cut to match.\n\
         - The ID listing that finds removals was answered from the recorded items, not the library.\n\
         - Anonymized before writing: names, titles, genres, IDs, image tags, provider IDs and the\n\
           server's address are replaced with consistent stand-ins of the same shape and script; file\n\
           paths, BlurHash previews, media stream details and tokens are removed. The key was random\n\
           and was not kept. A leak check refused to write if any original value was still present.\n\n\
         Recorded items: {artists} artists, {albums} albums, {tracks} songs, {playlists} playlists;\n\
         the delta sync found {changed} changed songs and {removed} removals.\n",
        version = info.version,
        limit = a.limit,
        artists = first.artists.len(),
        albums = first.albums.len(),
        tracks = first.tracks.len(),
        playlists = first.playlists.len(),
        changed = second.tracks.len(),
        removed = second.removed_item_ids.len(),
    );
    std::fs::write(a.out.join("RECORDING.md"), notes).map_err(|e| e.to_string())?;
    println!(
        "Wrote {} exchanges to {} ({} songs, {} albums, {} artists, {} playlists).",
        log.len(),
        a.out.display(),
        first.tracks.len(),
        first.albums.len(),
        first.artists.len(),
        first.playlists.len()
    );
    Ok(())
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\n{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
