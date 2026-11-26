//! Adapter tests on fixture replies, with no server.
//!
//! STAND-INS: tools/fixtures/subsonic/*.json were written by hand from the
//! OpenSubsonic documentation's response shapes (opensubsonic.github.io/docs/), not
//! recorded, with made-up artists and songs. The small error cases are built inline.
//! A real Navidrome replay in CI (ADR core-0021 point 10) is still to come.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use url::Url;

use super::client::{map_code, resolve_entry};
use super::*;
use crate::error::SourceError;
use crate::jellyfin::{StreamCodec, StreamQuality, StreamRequest, SyncProgress};
use crate::model::{Cursor, EditOutcome, ServerEdit};
use crate::source::{MediaSource, SubsonicSource};
use crate::transport::{FixtureExchange, FixtureTransport, Request};

const SERVER: &str = include_str!("../../../../tools/fixtures/subsonic/server.json");
const LIBRARY: &str = include_str!("../../../../tools/fixtures/subsonic/library.json");
const LYRICS: &str = include_str!("../../../../tools/fixtures/subsonic/lyrics.json");
const EDITS: &str = include_str!("../../../../tools/fixtures/subsonic/playlist-edits.json");

const PASSWORD: &str = "correct horse battery";

fn base() -> Url {
    Url::parse("https://music.example/").unwrap()
}

fn fixture(json: &str) -> Arc<FixtureTransport> {
    Arc::new(FixtureTransport::from_json(json).unwrap())
}

fn client(t: &Arc<FixtureTransport>) -> SubsonicClient {
    SubsonicClient::new(t.clone(), base(), "Encore".into(), Duration::from_secs(5))
}

fn creds() -> SubsonicCredentials {
    SubsonicCredentials::from_token_string(
        "subsonic1:u=listener&t=26719a1196d2a940705a59634eb18eab&s=c19b2d",
    )
    .unwrap()
}

fn signed_in(t: &Arc<FixtureTransport>) -> SubsonicClient {
    client(t).signed_in(creds())
}

/// An inline exchange answering `endpoint` with `body`.
fn x(endpoint: &str, query: &[(&str, &str)], body: serde_json::Value) -> FixtureExchange {
    FixtureExchange {
        origin: None,
        method: "GET".into(),
        path: format!("/rest/{endpoint}.view"),
        query: query
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        status: 200,
        body: Some(body),
        body_text: None,
        body_hex: None,
        error: None,
        date: None,
    }
}

fn failed(code: i64, message: &str) -> serde_json::Value {
    json!({"subsonic-response": {"status": "failed", "version": "1.15.0",
           "error": {"code": code, "message": message}}})
}

fn ok() -> serde_json::Value {
    json!({"subsonic-response": {"status": "ok", "version": "1.15.0"}})
}

/// Every value of `key` in a request's query.
fn values(req: &Request, key: &str) -> Vec<String> {
    req.url
        .query_pairs()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .collect()
}

fn value(req: &Request, key: &str) -> Option<String> {
    values(req, key).into_iter().next()
}

fn endpoint(req: &Request) -> String {
    req.url
        .path()
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".view")
        .to_owned()
}

fn sent_to(t: &FixtureTransport, name: &str) -> Vec<Request> {
    t.sent()
        .into_iter()
        .filter(|r| endpoint(r) == name)
        .collect()
}

#[tokio::test]
async fn an_opensubsonic_server_is_found_without_signing_in() {
    let t = fixture(SERVER);
    let info = client(&t).server_info().await.unwrap();
    assert_eq!(info.name, "navidrome");
    assert_eq!(info.version, "0.58.0 (fixture)");
    assert!(info.open_subsonic);
    assert!(info.extensions.iter().any(|e| e == "songLyrics"));
    assert!(info.extensions.iter().any(|e| e == "apiKeyAuthentication"));
    assert_eq!(info.server_id, server_id_for(&base()));
    assert_eq!(info.base_url, base());
    assert_eq!(info.warning, None);
    let sent = t.sent();
    assert_eq!(sent[0].url.path(), "/rest/ping.view");
    assert_eq!(value(&sent[0], "v").as_deref(), Some(API_VERSION));
    assert_eq!(value(&sent[0], "c").as_deref(), Some("Encore"));
    assert_eq!(value(&sent[0], "f").as_deref(), Some("json"));
    for r in &sent {
        for k in ["u", "t", "s", "p", "apiKey"] {
            assert!(value(r, k).is_none(), "{k} sent before signing in");
        }
    }
}

#[tokio::test]
async fn a_plain_subsonic_server_and_things_that_are_not_one() {
    let t = Arc::new(FixtureTransport::new(vec![x(
        "ping",
        &[],
        failed(40, "Wrong username or password"),
    )]));
    let info = client(&t).server_info().await.unwrap();
    assert_eq!(
        (info.name.as_str(), info.version.as_str()),
        ("Subsonic", "1.15.0")
    );
    assert!(!info.open_subsonic);
    assert!(info.extensions.is_empty());
    assert_eq!(t.sent().len(), 1, "no extensions asked of a plain server");

    let mut html = x("ping", &[], json!(null));
    html.body = None;
    html.body_text = Some("<html><body>A router page</body></html>".into());
    let t = Arc::new(FixtureTransport::new(vec![html]));
    assert!(matches!(
        client(&t).server_info().await,
        Err(SourceError::Protocol(_))
    ));

    let mut missing = x("ping", &[], json!(null));
    missing.body = None;
    missing.status = 404;
    let t = Arc::new(FixtureTransport::new(vec![missing]));
    assert!(matches!(
        client(&t).server_info().await,
        Err(SourceError::Protocol(_))
    ));

    let t = Arc::new(FixtureTransport::new(vec![x(
        "ping",
        &[],
        json!({"subsonic-response": {"status": "failed", "version": "1.12.0",
               "error": {"code": 30, "message": "Incompatible Subsonic REST protocol version. Server must upgrade."}}}),
    )]));
    assert_eq!(
        client(&t).server_info().await,
        Err(SourceError::UnsupportedVersion {
            found: "1.12.0".into(),
            minimum: API_VERSION.into()
        })
    );
}

#[tokio::test]
async fn plain_http_is_refused_outside_the_home_network() {
    let t = fixture(SERVER);
    let c = SubsonicClient::new(
        t.clone(),
        Url::parse("http://music.example/").unwrap(),
        "Encore".into(),
        Duration::from_secs(5),
    );
    assert_eq!(c.server_info().await, Err(SourceError::InsecureAddress));
}

#[tokio::test]
async fn signing_in_sends_an_md5_token_and_keeps_no_password() {
    let t = fixture(SERVER);
    let creds = client(&t).sign_in("listener", PASSWORD).await.unwrap();
    assert_eq!(creds.user, "listener");
    assert!(!creds.uses_api_key());
    let ping = &sent_to(&t, "ping")[0];
    let salt = value(ping, "s").unwrap();
    assert!(salt.len() >= 6, "the spec asks for six characters or more");
    assert_eq!(
        value(ping, "t").unwrap(),
        super::md5::md5_hex(format!("{PASSWORD}{salt}").as_bytes())
    );
    assert_eq!(value(ping, "u").as_deref(), Some("listener"));
    assert!(value(ping, "p").is_none(), "the password is never sent");
    let saved = creds.to_token_string();
    assert!(!saved.contains(PASSWORD) && !saved.contains("correct"));
    assert!(!ping.url.as_str().contains("correct"));
    assert!(!format!("{creds:?}").contains(&salt));
    // The saved string signs the same requests in.
    assert_eq!(
        SubsonicCredentials::from_token_string(&saved),
        Some(creds.clone())
    );
}

#[tokio::test]
async fn a_wrong_password_and_an_unsupported_sign_in() {
    let t = Arc::new(FixtureTransport::new(vec![x(
        "ping",
        &[("u", "listener")],
        failed(40, "Wrong username or password"),
    )]));
    assert_eq!(
        client(&t).sign_in("listener", "nope").await,
        Err(SourceError::WrongCredentials)
    );
    let t = Arc::new(FixtureTransport::new(vec![x(
        "ping",
        &[("u", "listener")],
        failed(41, "Token authentication not supported for LDAP users."),
    )]));
    assert!(matches!(
        client(&t).sign_in("listener", "pw").await,
        Err(SourceError::Server { status: 401, message }) if message.contains("41")
    ));
    let t = Arc::new(FixtureTransport::new(vec![x(
        "ping",
        &[],
        failed(44, "Invalid API key"),
    )]));
    assert_eq!(
        client(&t).sign_in_with_key("bad-key").await,
        Err(SourceError::WrongCredentials)
    );
}

#[tokio::test]
async fn signing_in_with_an_api_key() {
    let t = fixture(SERVER);
    let creds = client(&t)
        .sign_in_with_key("fixture-api-key-0001")
        .await
        .unwrap();
    assert!(creds.uses_api_key());
    assert_eq!(creds.user, "listener");
    let ping = &sent_to(&t, "ping")[0];
    assert_eq!(
        value(ping, "apiKey").as_deref(),
        Some("fixture-api-key-0001")
    );
    assert!(value(ping, "u").is_none(), "u is an error beside apiKey");
    let back = SubsonicCredentials::from_token_string(&creds.to_token_string()).unwrap();
    assert_eq!(back, creds);
    assert!(!format!("{back:?}").contains("fixture-api-key"));
}

#[tokio::test]
async fn me_names_the_user_and_finds_the_avatar() {
    let t = fixture(SERVER);
    let me = signed_in(&t).me().await.unwrap();
    assert_eq!(me.user_name, "listener");
    let avatar = me.avatar_url.clone().unwrap();
    assert_eq!(avatar.path(), "/rest/getAvatar.view");
    assert!(!format!("{me:?}").contains("26719a"));

    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "getUser",
            &[],
            json!({"subsonic-response": {"status": "ok", "version": "1.15.0", "user": {"username": "listener"}}}),
        ),
        x("getAvatar", &[], failed(70, "Not found")),
    ]));
    assert_eq!(signed_in(&t).me().await.unwrap().avatar_url, None);
}

#[tokio::test]
async fn a_full_sync_maps_the_whole_library() {
    let t = fixture(LIBRARY);
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let seen: Arc<Mutex<Vec<SyncProgress>>> = Arc::default();
    let s2 = seen.clone();
    let delta = source
        .sync_library(None, &HashSet::new(), 2, &move |p| {
            s2.lock().unwrap().push(p)
        })
        .await
        .unwrap();
    assert!(delta.full, "with no cursor, a full sync");
    assert!(delta.playlists_complete);
    assert!(delta.removed_item_ids.is_empty());
    let cursor = delta.cursor.clone().unwrap();
    assert!(
        cursor.0.starts_with("subsonic1:lm=1762400000000;full="),
        "{cursor:?}"
    );

    let mut artists: Vec<_> = delta.artists.iter().map(|a| a.name.as_str()).collect();
    artists.sort_unstable();
    assert_eq!(
        artists,
        ["Paper Harbour", "The Glass Orchard", "Wren Alder"],
        "a single artist object counts as a list of one"
    );
    let orchard = delta
        .artists
        .iter()
        .find(|a| a.source_item_id == "ar3")
        .unwrap();
    assert_eq!(orchard.sort_name, "Glass Orchard, The");

    assert_eq!(delta.albums.len(), 3);
    let quiet = delta
        .albums
        .iter()
        .find(|a| a.source_item_id == "al1")
        .unwrap();
    assert_eq!(quiet.title, "Quiet Rooms");
    assert_eq!(quiet.album_artists[0].name, "Paper Harbour");
    assert_eq!(quiet.artwork_tag.as_deref(), Some("al-al1_6740a1b2"));
    assert!(quiet.has_artwork);
    assert_eq!(quiet.date_added_utc_ms, Some(1_762_400_000_000));
    assert!(
        !delta
            .albums
            .iter()
            .find(|a| a.source_item_id == "al3")
            .unwrap()
            .has_artwork
    );

    assert_eq!(delta.tracks.len(), 5);
    let s1 = delta
        .tracks
        .iter()
        .find(|t| t.source_item_id == "s1")
        .unwrap();
    assert_eq!(s1.title, "Low Lantern");
    assert_eq!(s1.sort_title, "low lantern");
    assert_eq!(s1.duration_ms, 201_000);
    assert_eq!(s1.artists.len(), 2);
    assert_eq!(s1.artists[1].source_item_id.as_deref(), Some("ar2"));
    assert_eq!(s1.album_artists[0].name, "Paper Harbour");
    assert_eq!(s1.album_item_id.as_deref(), Some("al1"));
    assert_eq!((s1.disc_number, s1.track_number), (Some(1), Some(1)));
    assert_eq!(s1.genres, ["Folk", "Ambient"]);
    assert_eq!(s1.container.as_deref(), Some("flac"));
    assert_eq!(s1.bitrate, Some(912));
    assert_eq!(
        (s1.track_gain_db, s1.album_gain_db),
        (Some(-6.5), Some(-5.9))
    );
    assert!(s1.song_key().starts_with("mb:5b11f4ce"));
    assert!(s1.has_lyrics);
    let s3 = delta
        .tracks
        .iter()
        .find(|t| t.source_item_id == "s3")
        .unwrap();
    assert_eq!(s3.artists[0].name, "Wren Alder");
    assert_eq!(s3.album_artists[0].name, "Wren Alder");
    assert_eq!(s3.year, Some(2019));
    assert!(s3.song_key().starts_with("t1:"));

    assert_eq!(delta.playlists.len(), 2);
    let drive = &delta.playlists[0];
    assert_eq!(drive.name, "Evening Drive");
    assert!(drive.can_edit);
    assert_eq!(drive.artwork_tag.as_deref(), Some("pl-pl1_6740a1b2"));
    let ids: Vec<_> = drive
        .entries
        .iter()
        .map(|e| (e.entry_id.as_deref().unwrap(), e.track_item_id.as_str()))
        .collect();
    assert_eq!(ids, [("0:s5", "s5"), ("1:s1", "s1"), ("2:s5", "s5")]);
    assert!(!delta.playlists[1].can_edit, "someone else's, read-only");

    let mut favs = delta.favorite_item_ids.clone().unwrap();
    favs.sort_unstable();
    assert_eq!(favs, ["s1", "s4"]);
    let counts = delta.play_counts.clone().unwrap();
    assert!(counts.contains(&("s1".into(), 12, Some(1_793_913_300_000))));
    assert!(counts.contains(&("s2".into(), 3, None)));
    assert_eq!(counts.len(), 2);

    let seen = seen.lock().unwrap();
    let last = seen.last().unwrap();
    assert_eq!(last.done, last.total);
    assert_eq!(last.done, 3 + 3 + 5 + 2);
    assert!(seen.windows(2).all(|w| w[0].done <= w[1].done));
    let pages = sent_to(&t, "getAlbumList2");
    assert_eq!(pages.len(), 2);
    assert_eq!(value(&pages[1], "offset").as_deref(), Some("2"));

    // Covers learned in the sync are asked for by their cover art ID.
    let art = source.artwork("al1", 300).unwrap();
    let art = Url::parse(&art.url).unwrap();
    let pairs: Vec<(String, String)> = art.query_pairs().into_owned().collect();
    assert!(pairs.contains(&("id".into(), "al-al1_6740a1b2".into())));
    assert!(pairs.contains(&("size".into(), "300".into())));
}

#[tokio::test]
async fn an_unchanged_library_syncs_only_its_playlists_and_favourites() {
    let t = fixture(LIBRARY);
    let client = signed_in(&t);
    let now: i64 = 1_762_500_000_000;
    let none = HashSet::new();
    let quiet = |_: SyncProgress| {};
    let fresh = Cursor(format!("subsonic1:lm=1762400000000;full={}", now - 60_000));
    let delta = sync::sync_library_at(&client, Some(&fresh), &none, 2, &quiet, now)
        .await
        .unwrap();
    assert!(!delta.full, "a light sync never removes songs");
    assert!(delta.playlists_complete);
    assert!(delta.artists.is_empty() && delta.albums.is_empty() && delta.tracks.is_empty());
    assert_eq!(delta.playlists.len(), 2);
    assert_eq!(delta.playlists[0].entries.len(), 3);
    let mut favs = delta.favorite_item_ids.clone().unwrap();
    favs.sort_unstable();
    assert_eq!(favs, ["s1", "s4"], "the same set the full sync reads");
    assert_eq!(
        delta.play_counts, None,
        "play counts wait for the full sync"
    );
    assert_eq!(
        delta.cursor,
        Some(fresh.clone()),
        "the full sync's time is kept"
    );
    assert!(sent_to(&t, "getAlbumList2").is_empty());
    assert!(sent_to(&t, "getAlbum").is_empty());
    assert!(sent_to(&t, "getArtists").is_empty());

    // A day after the last full sync, a cursor from before light syncs, or a moved
    // lastModified: a full sync, with a new full sync time.
    let stale = [
        Cursor(format!(
            "subsonic1:lm=1762400000000;full={}",
            now - sync::FULL_SYNC_EVERY_MS
        )),
        Cursor("subsonic1:lm=1762400000000".into()),
        Cursor(format!("subsonic1:lm=1762300000000;full={}", now - 60_000)),
        // A clock that went back: not trusted.
        Cursor(format!("subsonic1:lm=1762400000000;full={}", now + 60_000)),
    ];
    for since in stale {
        let d = sync::sync_library_at(&client, Some(&since), &none, 2, &quiet, now)
            .await
            .unwrap();
        assert!(d.full, "{since:?}");
        assert_eq!(d.tracks.len(), 5);
        assert_eq!(
            d.cursor,
            Some(Cursor(format!("subsonic1:lm=1762400000000;full={now}")))
        );
    }
}

#[tokio::test]
async fn the_cursor_tells_whether_the_library_changed() {
    let t = fixture(LIBRARY);
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let same = Cursor("subsonic1:lm=1762400000000".into());
    let older = Cursor("subsonic1:lm=1762300000000".into());
    assert!(!source.library_changed(Some(&same)).await.unwrap());
    assert!(source.library_changed(Some(&older)).await.unwrap());
    assert!(source.library_changed(None).await.unwrap());
    assert!(
        source
            .library_changed(Some(&Cursor("2026-11-06T00:00:00Z".into())))
            .await
            .unwrap()
    );
    let ask = &sent_to(&t, "getIndexes")[0];
    assert_eq!(
        value(ask, "ifModifiedSince").as_deref(),
        Some("99999999999999"),
        "asked so the server sends only the time"
    );
}

#[tokio::test]
async fn stream_and_cover_urls() {
    let t = fixture(SERVER);
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let original = source
        .stream_info(
            "s1",
            &StreamRequest {
                quality: StreamQuality::Original,
                codec: StreamCodec::Aac,
                direct_play_containers: vec!["flac".into()],
            },
        )
        .unwrap();
    let u = Url::parse(&original.url).unwrap();
    assert_eq!(u.path(), "/rest/stream.view");
    let r = Request {
        method: crate::transport::Method::Get,
        url: u,
        headers: vec![],
        body: None,
        timeout: Duration::from_secs(1),
    };
    assert_eq!(value(&r, "id").as_deref(), Some("s1"));
    assert_eq!(value(&r, "format").as_deref(), Some("raw"));
    assert_eq!(value(&r, "u").as_deref(), Some("listener"));
    assert!(value(&r, "t").is_some() && value(&r, "s").is_some());
    assert!(!original.may_transcode);
    assert!(original.headers.is_empty());
    assert_eq!(original.url, original.url_with_api_key);
    assert!(!format!("{original:?}").contains("26719a"));
    assert!(
        !format!("{r:?}").contains("26719a"),
        "a request logs no token"
    );

    let capped = source
        .stream_info(
            "s1",
            &StreamRequest {
                quality: StreamQuality::Capped { kbps: 192 },
                codec: StreamCodec::Opus,
                direct_play_containers: vec![],
            },
        )
        .unwrap();
    let r = Request {
        url: Url::parse(&capped.url).unwrap(),
        ..r
    };
    assert_eq!(value(&r, "maxBitRate").as_deref(), Some("192"));
    assert_eq!(value(&r, "format").as_deref(), Some("opus"));
    assert!(capped.may_transcode);

    // A cover not learned in a sync is asked for by the item's own ID.
    let art = source.artwork("al7", 9000).unwrap();
    let r = Request {
        url: Url::parse(&art.url).unwrap(),
        ..r
    };
    assert_eq!(endpoint(&r), "getCoverArt");
    assert_eq!(value(&r, "id").as_deref(), Some("al7"));
    assert_eq!(value(&r, "size").as_deref(), Some("4096"));
    assert!(!format!("{art:?}").contains("26719a"));

    // A server under a path keeps it.
    let under = SubsonicClient::new(
        t.clone(),
        Url::parse("https://music.example/navidrome/").unwrap(),
        "Encore".into(),
        Duration::from_secs(5),
    )
    .signed_in(creds());
    let u = Url::parse(&under.artwork("al1", 64).unwrap().url).unwrap();
    assert_eq!(u.path(), "/navidrome/rest/getCoverArt.view");

    // Signed out: no URL at all.
    assert_eq!(
        client(&t).artwork("al7", 300).unwrap_err(),
        SourceError::SessionExpired
    );
}

#[tokio::test]
async fn cover_bytes_and_a_missing_cover() {
    let mut img = x("getCoverArt", &[("id", "al-al1_6740a1b2")], json!(null));
    img.body = None;
    img.body_hex = Some("ffd8ffe000104a464946".into());
    let mut missing = x("getCoverArt", &[("id", "al9")], json!(null));
    missing.body = None;
    missing.body_text = Some(
        r#"<?xml version="1.0" encoding="UTF-8"?><subsonic-response xmlns="http://subsonic.org/restapi" status="failed" version="1.15.0"><error code="70" message="Artwork not found"/></subsonic-response>"#.into(),
    );
    let t = Arc::new(FixtureTransport::new(vec![img, missing]));
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let bytes = source
        .cover_png("al1", 64, Some("al-al1_6740a1b2"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes[..3], &[0xff, 0xd8, 0xff]);
    // Eight times the sample size: the JPEG is read at one eighth (palette/jpeg.rs).
    assert_eq!(value(&t.sent()[0], "size").as_deref(), Some("512"));
    assert_eq!(source.artwork_bytes("al9", 300).await.unwrap(), None);
}

#[tokio::test]
async fn synced_lyrics_come_as_lrc() {
    let t = fixture(LYRICS);
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let l = source.lyrics("s1").await.unwrap().unwrap();
    assert_eq!(l.format, "lrc");
    assert_eq!(
        l.body,
        "[la:eng]\n[offset:120]\n[00:01.500]A lamp left on\n[00:04.250]for no one\n[01:01.010]\n",
        "the synced set wins over the unsynced one"
    );
    assert_eq!(source.lyrics("s2").await.unwrap(), None);
    assert_eq!(
        sent_to(&t, "getOpenSubsonicExtensions").len(),
        1,
        "the extensions are asked once"
    );
    assert!(sent_to(&t, "getLyrics").is_empty());
}

#[tokio::test]
async fn plain_lyrics_without_the_extension() {
    let t = fixture(LYRICS);
    let source = SubsonicSource {
        client: signed_in(&t).with_extensions(Vec::new()),
    };
    let l = source.lyrics("s3").await.unwrap().unwrap();
    assert_eq!(l.format, "plain");
    assert_eq!(l.body, "Smoke on the ridge\nwe keep the fire low");
    let ask = &sent_to(&t, "getLyrics")[0];
    assert_eq!(value(ask, "artist").as_deref(), Some("Wren Alder"));
    assert_eq!(value(ask, "title").as_deref(), Some("Signal Fires"));
    assert_eq!(source.lyrics("s4").await.unwrap(), None);
    assert!(sent_to(&t, "getLyricsBySongId").is_empty());

    let t = Arc::new(FixtureTransport::new(vec![x(
        "getSong",
        &[],
        failed(70, "Song not found"),
    )]));
    let c = signed_in(&t).with_extensions(Vec::new());
    assert_eq!(c.lyrics("gone").await.unwrap(), None);

    // A plain Subsonic server refuses the extensions call: asked once, then known.
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "getOpenSubsonicExtensions",
            &[],
            failed(0, "Unknown method"),
        ),
        x("getSong", &[], failed(70, "Song not found")),
    ]));
    let c = signed_in(&t);
    assert_eq!(c.lyrics("a").await.unwrap(), None);
    assert_eq!(c.lyrics("b").await.unwrap(), None);
    assert_eq!(sent_to(&t, "getOpenSubsonicExtensions").len(), 1);
}

async fn one_edit(edit: ServerEdit) -> (Result<EditOutcome, SourceError>, Arc<FixtureTransport>) {
    let t = fixture(EDITS);
    let got = signed_in(&t).edit(&edit).await;
    (got, t)
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[tokio::test]
async fn creating_a_playlist_returns_its_id() {
    let (got, t) = one_edit(ServerEdit::CreatePlaylist {
        name: "Road Trip".into(),
        item_ids: ids(&["s1", "s2"]),
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Created("pl9".into())));
    let sent = sent_to(&t, "createPlaylist");
    assert_eq!(value(&sent[0], "name").as_deref(), Some("Road Trip"));
    assert_eq!(values(&sent[0], "songId"), ["s1", "s2"]);
    assert!(sent_to(&t, "updatePlaylist").is_empty());

    // A server that returns no playlist (API before 1.14.0): found by name.
    let t = Arc::new(FixtureTransport::new(vec![
        x("createPlaylist", &[], ok()),
        x(
            "getPlaylists",
            &[],
            json!({"subsonic-response": {"status": "ok", "version": "1.13.0", "playlists": {"playlist": [
                {"id": "old", "name": "Road Trip", "owner": "listener", "created": "2025-01-01T00:00:00Z"},
                {"id": "new", "name": "Road Trip", "owner": "listener", "created": "2026-11-24T09:00:00Z"},
                {"id": "theirs", "name": "Road Trip", "owner": "housemate", "created": "2026-11-25T09:00:00Z"}]}}}),
        ),
    ]));
    let got = signed_in(&t)
        .edit(&ServerEdit::CreatePlaylist {
            name: "Road Trip".into(),
            item_ids: vec![],
        })
        .await;
    assert_eq!(got, Ok(EditOutcome::Created("new".into())));
}

#[tokio::test]
async fn a_long_playlist_is_sent_in_chunks() {
    let many: Vec<String> = (0..320).map(|i| format!("s{i}")).collect();
    let (got, t) = one_edit(ServerEdit::CreatePlaylist {
        name: "Road Trip".into(),
        item_ids: many,
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Created("pl9".into())));
    assert_eq!(
        values(&sent_to(&t, "createPlaylist")[0], "songId").len(),
        150
    );
    let adds = sent_to(&t, "updatePlaylist");
    assert_eq!(adds.len(), 2);
    assert_eq!(values(&adds[0], "songIdToAdd").len(), 150);
    assert_eq!(values(&adds[1], "songIdToAdd").len(), 20);
    assert_eq!(values(&adds[1], "songIdToAdd")[19], "s319");
    assert_eq!(value(&adds[0], "playlistId").as_deref(), Some("pl9"));
}

#[tokio::test]
async fn adding_at_the_end_and_at_a_position() {
    let (got, t) = one_edit(ServerEdit::AddToPlaylist {
        playlist_id: "pl1".into(),
        item_ids: ids(&["s3"]),
        position: None,
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    let up = &sent_to(&t, "updatePlaylist")[0];
    assert_eq!(values(up, "songIdToAdd"), ["s3"]);
    assert!(sent_to(&t, "getPlaylist").is_empty());

    // Subsonic can't insert: the list is written back in the new order.
    let (got, t) = one_edit(ServerEdit::AddToPlaylist {
        playlist_id: "pl1".into(),
        item_ids: ids(&["s3", "s4"]),
        position: Some(1),
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    let set = &sent_to(&t, "createPlaylist")[0];
    assert_eq!(value(set, "playlistId").as_deref(), Some("pl1"));
    assert!(value(set, "name").is_none(), "the name is kept");
    assert_eq!(values(set, "songId"), ["s5", "s3", "s4", "s1", "s5", "s2"]);

    // A position past the end is the end.
    let (_, t) = one_edit(ServerEdit::AddToPlaylist {
        playlist_id: "pl1".into(),
        item_ids: ids(&["s3"]),
        position: Some(4),
    })
    .await;
    assert!(sent_to(&t, "createPlaylist").is_empty());
    assert_eq!(
        values(&sent_to(&t, "updatePlaylist")[0], "songIdToAdd"),
        ["s3"]
    );
}

#[tokio::test]
async fn removing_maps_entries_to_positions() {
    let (got, t) = one_edit(ServerEdit::RemoveFromPlaylist {
        playlist_id: "pl1".into(),
        // "7:s2" is stale (an earlier edit moved it): s2 is at 3 now. "5:zz" is gone.
        entry_ids: ids(&["1:s1", "2:s5", "7:s2", "5:zz"]),
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    let up = sent_to(&t, "updatePlaylist");
    assert_eq!(up.len(), 1);
    assert_eq!(value(&up[0], "playlistId").as_deref(), Some("pl1"));
    assert_eq!(values(&up[0], "songIndexToRemove"), ["3", "2", "1"]);

    // Nothing left to remove: nothing is sent.
    let (got, t) = one_edit(ServerEdit::RemoveFromPlaylist {
        playlist_id: "pl1".into(),
        entry_ids: ids(&["0:zz"]),
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    assert!(sent_to(&t, "updatePlaylist").is_empty());
}

#[tokio::test]
async fn moving_writes_the_list_back_in_order() {
    let (got, t) = one_edit(ServerEdit::MoveInPlaylist {
        playlist_id: "pl1".into(),
        entry_id: "0:s5".into(),
        new_index: 3,
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    let set = &sent_to(&t, "createPlaylist")[0];
    assert_eq!(value(set, "playlistId").as_deref(), Some("pl1"));
    assert_eq!(values(set, "songId"), ["s1", "s5", "s2", "s5"]);

    // Already there: nothing is sent.
    let (got, t) = one_edit(ServerEdit::MoveInPlaylist {
        playlist_id: "pl1".into(),
        entry_id: "1:s1".into(),
        new_index: 1,
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    assert!(sent_to(&t, "createPlaylist").is_empty());

    // An entry that isn't there any more: refused, not retried.
    let (got, _) = one_edit(ServerEdit::MoveInPlaylist {
        playlist_id: "pl1".into(),
        entry_id: "0:zz".into(),
        new_index: 2,
    })
    .await;
    assert_eq!(got, Err(SourceError::NotFound));
}

#[tokio::test]
async fn rename_delete_star_and_unstar() {
    let (got, t) = one_edit(ServerEdit::RenamePlaylist {
        playlist_id: "pl1".into(),
        name: "Late Drive".into(),
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    let up = &sent_to(&t, "updatePlaylist")[0];
    assert_eq!(value(up, "name").as_deref(), Some("Late Drive"));

    let (got, t) = one_edit(ServerEdit::DeletePlaylist {
        playlist_id: "pl1".into(),
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    assert_eq!(
        value(&sent_to(&t, "deletePlaylist")[0], "id").as_deref(),
        Some("pl1")
    );

    let (got, t) = one_edit(ServerEdit::SetFavorite {
        item_id: "s3".into(),
        favorite: true,
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    assert_eq!(value(&sent_to(&t, "star")[0], "id").as_deref(), Some("s3"));

    let (got, t) = one_edit(ServerEdit::SetFavorite {
        item_id: "s1".into(),
        favorite: false,
    })
    .await;
    assert_eq!(got, Ok(EditOutcome::Done));
    assert_eq!(
        value(&sent_to(&t, "unstar")[0], "id").as_deref(),
        Some("s1")
    );
}

#[tokio::test]
async fn playlist_entries_have_position_and_song_ids() {
    let t = fixture(EDITS);
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let entries = source.playlist_entries("pl1").await.unwrap();
    let got: Vec<_> = entries
        .iter()
        .map(|e| e.entry_id.clone().unwrap())
        .collect();
    assert_eq!(got, ["0:s5", "1:s1", "2:s5", "3:s2"]);
    assert_eq!(resolve_entry("2:s5", &entries, &[]), Some(2));
}

#[tokio::test]
async fn errors_map_to_plain_reasons() {
    let t = Arc::new(FixtureTransport::new(vec![
        x("getAlbum", &[], failed(70, "Album not found")),
        x("updatePlaylist", &[], failed(50, "User not authorized")),
        x("star", &[], failed(40, "Wrong username or password")),
        x("unstar", &[], failed(44, "Invalid API key")),
        x("deletePlaylist", &[], failed(0, "A generic error")),
    ]));
    let c = signed_in(&t);
    assert_eq!(
        c.fetch::<serde_json::Value>("getAlbum", &[("id", "x")], "album")
            .await,
        Err(SourceError::NotFound)
    );
    let rename = ServerEdit::RenamePlaylist {
        playlist_id: "theirs".into(),
        name: "Mine now".into(),
    };
    assert!(matches!(
        c.edit(&rename).await,
        Err(SourceError::Server { status: 403, .. })
    ));
    let star = |favorite| ServerEdit::SetFavorite {
        item_id: "s1".into(),
        favorite,
    };
    assert_eq!(c.edit(&star(true)).await, Err(SourceError::SessionExpired));
    assert_eq!(c.edit(&star(false)).await, Err(SourceError::SessionExpired));
    assert!(matches!(
        c.edit(&ServerEdit::DeletePlaylist {
            playlist_id: "p".into()
        })
        .await,
        Err(SourceError::Server { status: 500, message }) if message.contains("generic")
    ));

    // No envelope at all: the HTTP status decides.
    let mut denied = x("getPlaylists", &[], json!(null));
    denied.body = None;
    denied.status = 401;
    let t = Arc::new(FixtureTransport::new(vec![denied]));
    assert_eq!(
        signed_in(&t).playlists().await.unwrap_err(),
        SourceError::SessionExpired
    );

    // Signed out: nothing is sent.
    let t = fixture(EDITS);
    assert_eq!(
        client(&t).edit(&star(true)).await,
        Err(SourceError::SessionExpired)
    );
    assert!(t.sent().is_empty());

    for (code, want) in [
        (10, 400),
        (20, 400),
        (41, 0),
        (42, 0),
        (43, 0),
        (50, 403),
        (60, 500),
    ] {
        let got = map_code(code, "m", "", &SourceError::SessionExpired);
        match got {
            SourceError::Server { status, .. } => assert_eq!(status, want, "code {code}"),
            SourceError::SessionExpired => assert_eq!(want, 0, "code {code}"),
            other => panic!("code {code}: {other:?}"),
        }
    }
}

fn temp_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("encore-subsonic-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&dir);
    dir
}

#[tokio::test]
async fn downloads_fall_back_to_the_raw_stream_and_never_keep_an_error() {
    let mut audio = x("stream", &[("format", "raw")], json!(null));
    audio.body = None;
    audio.body_hex = Some("664c614300000022".into());
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "download",
            &[("id", "s1")],
            failed(50, "Downloads are disabled"),
        ),
        audio,
    ]));
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let to = temp_file("fallback");
    let req = StreamRequest {
        quality: StreamQuality::Original,
        codec: StreamCodec::Aac,
        direct_play_containers: vec![],
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let got = source
        .download("s1", &req, &to, 0, &|_, _| {}, &cancel)
        .await
        .unwrap();
    assert_eq!(got.bytes, 8);
    assert_eq!(std::fs::read(&to).unwrap(), b"fLaC\0\0\0\"");
    assert_eq!(
        sent_to(&t, "download").len(),
        1,
        "the original file is asked for first"
    );

    let t = Arc::new(FixtureTransport::new(vec![x(
        "download",
        &[],
        failed(70, "Song not found"),
    )]));
    let source = SubsonicSource {
        client: signed_in(&t),
    };
    let to = temp_file("missing");
    assert_eq!(
        source
            .download("gone", &req, &to, 0, &|_, _| {}, &cancel)
            .await
            .unwrap_err(),
        SourceError::NotFound
    );
    assert_eq!(
        std::fs::metadata(&to).map(|m| m.len()).unwrap_or(0),
        0,
        "the error reply is not left as the song"
    );
    let _ = std::fs::remove_file(&to);
}

#[tokio::test]
async fn range_reader_needs_a_sign_in() {
    let t = fixture(SERVER);
    let rt = tokio::runtime::Handle::current();
    assert!(signed_in(&t).range_reader("s1", rt.clone()).is_ok());
    assert_eq!(
        client(&t).range_reader("s1", rt).unwrap_err(),
        SourceError::SessionExpired
    );
}

#[test]
fn credentials_round_trip_and_stay_versioned() {
    let c = creds();
    let s = c.to_token_string();
    assert!(s.starts_with("subsonic1:"));
    assert_eq!(SubsonicCredentials::from_token_string(&s), Some(c.clone()));
    assert_eq!(
        c.auth_params(),
        [
            ("u", "listener"),
            ("t", "26719a1196d2a940705a59634eb18eab"),
            ("s", "c19b2d")
        ]
    );

    // Odd user names survive.
    let odd = SubsonicCredentials::from_token_string(
        "subsonic1:u=a%3Ab%26c+d%3D&t=0123456789abcdef0123456789abcdef&s=salty1",
    )
    .unwrap();
    assert_eq!(odd.user, "a:b&c d=");
    assert_eq!(
        SubsonicCredentials::from_token_string(&odd.to_token_string()),
        Some(odd)
    );

    for bad in [
        "",
        "jellyfin-token",
        "subsonic2:u=a&t=b&s=c",
        "subsonic1:u=a&t=b",
        "subsonic1:t=b&s=c",
        "subsonic1:u=a&t=b&s=c&k=d",
        "subsonic1:u=a&k=",
    ] {
        assert_eq!(SubsonicCredentials::from_token_string(bad), None, "{bad}");
    }
}

#[test]
fn debug_output_hides_every_secret() {
    let t = fixture(SERVER);
    let token = creds();
    let key = SubsonicCredentials::from_token_string("subsonic1:u=listener&k=fixture-api-key-0001")
        .unwrap();
    let c = signed_in(&t);
    let shown = format!(
        "{token:?} {key:?} {c:?} {:?}",
        SubsonicSource {
            client: client(&t).signed_in(key.clone())
        }
    );
    for secret in [
        "26719a1196d2a940705a59634eb18eab",
        "c19b2d",
        "fixture-api-key",
    ] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
    assert!(shown.contains("listener"));
}

#[test]
fn capabilities_are_honest() {
    let caps = capabilities();
    assert!(caps.lyrics && caps.transcoding && caps.playlists_writable);
    assert!(!caps.quick_connect);
    assert!(!caps.playback_reporting);
    let t = fixture(SERVER);
    let s = SubsonicSource {
        client: signed_in(&t),
    };
    assert_eq!(s.kind(), crate::model::SourceKind::Subsonic);
    assert_eq!(s.kind().as_str(), "subsonic");
    assert_eq!(
        crate::model::SourceKind::parse("subsonic"),
        Some(crate::model::SourceKind::Subsonic)
    );
}
