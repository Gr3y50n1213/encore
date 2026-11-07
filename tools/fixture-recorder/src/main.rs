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
mod phase3;
mod phase4;
mod phase5;

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

/// Board 04's Golden Hour Static cover: the project's own art.
const STAND_IN_COVER: &[u8] = include_bytes!("../../fixtures/artwork/golden-hour-static.png");

struct Args {
    server: String,
    limit: u64,
    page_size: u32,
    out: PathBuf,
    /// Real covers, for a local check only; never committed (git-ignored).
    covers_dir: PathBuf,
    /// Phase 4 only: sign in, `/Users/Me`, the user image, sign out, `/Users/Me`
    /// again; read-only, written to recorded-phase4.json.
    phase4: bool,
    /// Phase 5 only: the whole library with play counts, the Playback Reporting
    /// log if installed, similar items; read-only. The history goes to
    /// `private_dir` only; recorded-phase5.json gets three scrubbed rows of each.
    phase5: bool,
    private_dir: PathBuf,
}

fn args() -> std::result::Result<Args, String> {
    let mut a = Args {
        server: "http://localhost:8096".into(),
        limit: 150,
        page_size: 50,
        out: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/jellyfin")),
        covers_dir: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/private-covers"
        )),
        phase4: false,
        phase5: false,
        private_dir: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/private-history"
        )),
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
            "--covers-dir" => a.covers_dir = PathBuf::from(value()?),
            "--phase4" => a.phase4 = true,
            "--phase5" => a.phase5 = true,
            "--private-dir" => a.private_dir = PathBuf::from(value()?),
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
                // A real cover is copyrighted: the board's own original art stands
                // in for it, so replays still decode a PNG.
                Err(_) if r.body.starts_with(b"\x89PNG") => {
                    out["bodyHex"] = json!(hex(STAND_IN_COVER));
                    out["contentType"] = json!("image/png");
                }
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

/// Anonymize the log and check nothing original survived; the JSON to write.
fn publish(
    recorder: &Recorder,
    key: [u8; 32],
    session: &jellyfin::Session,
    secret: &str,
    server_id: &str,
) -> std::result::Result<String, String> {
    let log = recorder.log.lock().map(|l| l.clone()).unwrap_or_default();
    publish_log(&log, key, session, secret, server_id)
}

/// `publish` for a chosen part of the log.
fn publish_log(
    log: &[Exchange],
    key: [u8; 32],
    session: &jellyfin::Session,
    secret: &str,
    server_id: &str,
) -> std::result::Result<String, String> {
    let mut anon = Anonymizer::new(key);
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
    for (what, value) in [
        ("the access token", session.token.as_str()),
        ("the Quick Connect secret", secret),
        ("the user ID", session.user_id.as_str()),
        ("the server ID", server_id),
        ("the user name", session.user_name.as_str()),
    ] {
        if value.len() >= 3 && published.contains(value) {
            // Name the kind of value, never the value.
            return Err(format!(
                "{what} survived anonymization; nothing was written"
            ));
        }
    }
    Ok(published)
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
        limit: if a.phase5 { u64::MAX / 2 } else { a.limit },
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

    if a.phase5 {
        // Read-only: nothing on the server changes.
        let progress =
            |p: jellyfin::SyncProgress| print!("\rLibrary: {} of {}   ", p.done, p.total);
        let library =
            jellyfin::sync_library(&client, None, &HashSet::new(), a.page_size, &progress)
                .await
                .map_err(|e| e.to_string())?;
        println!();
        let p5 = phase5::run(&client, &session.user_id, &library).await;
        client.sign_out().await.map_err(|e| e.to_string())?;
        println!("Signed out; the recorder's token is gone from the server.");
        phase5::write_private(&a.private_dir, key, &library, &p5).map_err(|e| e.to_string())?;
        println!(
            "Your history is in {} (never committed).",
            a.private_dir.display()
        );
        // The committed fixture: one of each reply, three rows, values scrubbed.
        let log = recorder.log.lock().map(|l| l.clone()).unwrap_or_default();
        let mut seen = HashSet::new();
        let mut keep = Vec::new();
        for mut x in log {
            let path = x.url.path().to_owned();
            let kind = if path.ends_with("/Plugins") {
                "plugins"
            } else if path.ends_with("submit_custom_query") {
                "query"
            } else if path.ends_with("/Similar") {
                "similar"
            } else if path.ends_with("/Items")
                && query(&x.url, "filters").as_deref() == Some("IsPlayed")
            {
                "userData"
            } else {
                continue;
            };
            if !seen.insert(kind) {
                continue;
            }
            if let Ok(r) = &mut x.outcome
                && let Ok(mut v) = serde_json::from_slice::<Value>(&r.body)
            {
                phase5::scrub(&path, &mut v);
                r.body = serde_json::to_vec(&v).unwrap_or_default();
            }
            keep.push(x);
        }
        let published = publish_log(&keep, key, &session, &code.secret, &info.server_id)?;
        std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
        std::fs::write(a.out.join("recorded-phase5.json"), format!("{published}\n"))
            .map_err(|e| e.to_string())?;
        let notes = format!(
            "# Jellyfin fixtures: Phase 5\n\n\
             `recorded-phase5.json` holds the shapes of what smart picks reads from a real Jellyfin {version}\n\
             server (`tools/fixture-recorder --phase5`, read-only): one page of Jellyfin's own user data, the\n\
             plugin list, the Playback Reporting query when the plugin is there, and one similar-items reply;\n\
             three rows each, with play counts and dates replaced, anonymized like the other recordings.\n\
             The owner's history itself stays on the owner's PC (`tools/target/private-history`, git-ignored).\n\n{lines}\n",
            version = info.version,
            lines = p5
                .lines
                .iter()
                .map(|l| format!("- {l}"))
                .chain(p5.warnings.iter().map(|w| format!("- Warning: {w}")))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        std::fs::write(a.out.join("RECORDING-PHASE5.md"), notes).map_err(|e| e.to_string())?;
        println!("Wrote {} exchanges to recorded-phase5.json.", keep.len());
        for l in &p5.lines {
            println!("{l}");
        }
        for w in &p5.warnings {
            println!("Warning: {w}");
        }
        return Ok(());
    }

    if a.phase4 {
        // Read-only: nothing on the server changes.
        let mut p4 = phase4::run(&client, transport.as_ref()).await;
        client.sign_out().await.map_err(|e| e.to_string())?;
        println!("Signed out; the recorder's token is gone from the server.");
        phase4::after_sign_out(&client, &mut p4).await;
        let published = publish(&recorder, key, &session, &code.secret, &info.server_id)?;
        std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
        std::fs::write(a.out.join("recorded-phase4.json"), format!("{published}\n"))
            .map_err(|e| e.to_string())?;
        let notes = format!(
            "# Jellyfin fixtures: Phase 4\n\n\
             `recorded-phase4.json` is a read-only recording against a real Jellyfin {version} server,\n\
             made with `tools/fixture-recorder --phase4`, for the companion server's sign-in check and\n\
             the profile switcher's avatars. Anonymized like `recorded.json` (with its own random key,\n\
             not kept), and a real user image is replaced by the project's own art.\n\n{lines}\n",
            version = info.version,
            lines = p4
                .lines
                .iter()
                .map(|l| format!("- {l}"))
                .chain(p4.warnings.iter().map(|w| format!("- Warning: {w}")))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        std::fs::write(a.out.join("RECORDING-PHASE4.md"), notes).map_err(|e| e.to_string())?;
        let n = recorder.log.lock().map(|l| l.len()).unwrap_or(0);
        println!(
            "Wrote {n} exchanges to {}.",
            a.out.join("recorded-phase4.json").display()
        );
        for w in &p4.warnings {
            println!("Warning: {w}");
        }
        return Ok(());
    }

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
    let without = first
        .tracks
        .iter()
        .find(|t| !t.has_lyrics)
        .map(|t| t.source_item_id.clone());
    if let Some(id) = &without {
        client.lyrics(id).await.map_err(|e| e.to_string())?;
    }
    // Phase 3. Nothing here stops the run: every step reports what happened, and
    // the writes are always undone.
    let p3 = phase3::run(&client, &first, &a.covers_dir).await;
    client.sign_out().await.map_err(|e| e.to_string())?;
    println!("Signed out; the recorder's token is gone from the server.");

    // Anonymize, check, write.
    let published = publish(&recorder, key, &session, &code.secret, &info.server_id)?;
    let log = recorder.log.lock().map(|l| l.clone()).unwrap_or_default();
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
         the delta sync found {changed} changed songs and {removed} removals.\n\n\
         ## Phase 3: lyrics, covers and writes\n\n\
         Real covers were replaced in the fixture with board 04's own Golden Hour Static art. The\n\
         writes were made on a playlist named \"{test}\" and on one favourite, and were undone in the\n\
         same session.\n\n\
         {p3}\n",
        version = info.version,
        limit = a.limit,
        artists = first.artists.len(),
        albums = first.albums.len(),
        tracks = first.tracks.len(),
        playlists = first.playlists.len(),
        changed = second.tracks.len(),
        removed = second.removed_item_ids.len(),
        test = phase3::TEST_PLAYLIST,
        p3 = p3
            .lines
            .iter()
            .map(|l| format!("- {l}"))
            .chain(p3.warnings.iter().map(|w| format!("- Warning: {w}")))
            .collect::<Vec<_>>()
            .join("\n"),
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
