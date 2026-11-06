//! Adapter tests on fixture replies, with no server.
//!
//! STAND-INS: these replies were written by hand from the Jellyfin 12.0.0 OpenAPI
//! document, not recorded. The normal paths are also tested on a real recording in
//! recorded_tests.rs; these stay for what a recording cannot safely or reliably
//! contain: a wrong password, Quick Connect turned off or expired, unreachable
//! addresses, an untrusted certificate, an expired session, an old server and
//! paging edge cases.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use url::Url;

use super::*;
use crate::error::{SourceError, Unreachable};
use crate::model::Cursor;
use crate::transport::{FixtureExchange, FixtureTransport, Transport};

fn device() -> DeviceInfo {
    DeviceInfo {
        client: "Encore".into(),
        device_name: "Test Phone".into(),
        device_id: "dev-1".into(),
        version: "0.1.0".into(),
    }
}

fn x(method: &str, path: &str, query: &[(&str, &str)], body: serde_json::Value) -> FixtureExchange {
    FixtureExchange {
        origin: None,
        method: method.into(),
        path: path.into(),
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

fn dated(mut e: FixtureExchange, http_date: &str) -> FixtureExchange {
    e.date = Some(http_date.into());
    e
}

fn status(mut e: FixtureExchange, s: u16) -> FixtureExchange {
    e.status = s;
    e
}

fn at(mut e: FixtureExchange, origin: &str) -> FixtureExchange {
    e.origin = Some(origin.into());
    e
}

fn fails(origin: &str, path: &str, error: &str) -> FixtureExchange {
    FixtureExchange {
        origin: Some(origin.into()),
        method: "GET".into(),
        path: path.into(),
        query: Default::default(),
        status: 0,
        body: None,
        body_text: None,
        body_hex: None,
        error: Some(error.into()),
        date: None,
    }
}

fn public_info(version: &str, product: &str) -> serde_json::Value {
    json!({"LocalAddress": "http://192.168.1.20:8096", "ServerName": "Living Room", "Version": version,
           "ProductName": product, "OperatingSystem": "", "Id": "f1e2d3c4b5a6", "StartupWizardCompleted": true})
}

fn client(t: Arc<FixtureTransport>) -> JellyfinClient {
    JellyfinClient::new(
        t,
        Url::parse("https://music.example").unwrap(),
        device(),
        Duration::from_secs(5),
    )
}

fn signed_in(t: Arc<FixtureTransport>) -> JellyfinClient {
    client(t).signed_in("secret-token".into(), "user-1".into())
}

#[tokio::test]
async fn probe_walks_the_plan_and_warns_about_plain_http_on_the_lan() {
    let t = Arc::new(FixtureTransport::new(vec![
        fails("https://192.168.1.20", "/System/Info/Public", "refused"),
        fails(
            "https://192.168.1.20:8920",
            "/System/Info/Public",
            "timedOut",
        ),
        at(
            x(
                "GET",
                "/System/Info/Public",
                &[],
                public_info("12.0.0", "Jellyfin Server"),
            ),
            "http://192.168.1.20:8096",
        ),
    ]));
    let plan = plan_address("192.168.1.20", &[443, 8920], 8096).unwrap();
    let info = probe(
        t.clone() as Arc<dyn Transport>,
        &plan,
        &device(),
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(info.base_url.as_str(), "http://192.168.1.20:8096/");
    assert_eq!(
        (
            info.name.as_str(),
            info.version.as_str(),
            info.server_id.as_str()
        ),
        ("Living Room", "12.0.0", "f1e2d3c4b5a6")
    );
    assert_eq!(info.warning, Some(AddressWarning::PlainHttpOnLocalNetwork));
    assert_eq!(t.sent().len(), 3);
}

#[tokio::test]
async fn an_untrusted_certificate_is_the_reason_given() {
    let t = Arc::new(FixtureTransport::new(vec![
        fails(
            "https://nas.local",
            "/System/Info/Public",
            "certificateUntrusted:ab12",
        ),
        fails("https://nas.local:8920", "/System/Info/Public", "refused"),
        fails("http://nas.local:8096", "/System/Info/Public", "refused"),
    ]));
    let plan = plan_address("nas.local", &[443, 8920], 8096).unwrap();
    let err = probe(
        t as Arc<dyn Transport>,
        &plan,
        &device(),
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err,
        SourceError::CertificateUntrusted {
            sha256: "ab12".into()
        }
    );
}

#[tokio::test]
async fn not_jellyfin_too_old_and_unreachable() {
    let plan = plan_address("https://music.example", &[443], 8096).unwrap();
    for (reply, want) in [
        (
            public_info("12.0.0", "Emby Server"),
            SourceError::NotJellyfin,
        ),
        (
            public_info("10.9.11", "Jellyfin Server"),
            SourceError::UnsupportedVersion {
                found: "10.9.11".into(),
                minimum: "10.10.0".into(),
            },
        ),
    ] {
        let t = Arc::new(FixtureTransport::new(vec![x(
            "GET",
            "/System/Info/Public",
            &[],
            reply,
        )]));
        assert_eq!(
            probe(
                t as Arc<dyn Transport>,
                &plan,
                &device(),
                Duration::from_secs(5)
            )
            .await
            .unwrap_err(),
            want
        );
    }
    let t = Arc::new(FixtureTransport::new(vec![fails(
        "https://music.example",
        "/System/Info/Public",
        "nameNotFound",
    )]));
    assert_eq!(
        probe(
            t as Arc<dyn Transport>,
            &plan,
            &device(),
            Duration::from_secs(5)
        )
        .await
        .unwrap_err(),
        SourceError::Unreachable(Unreachable::NameNotFound)
    );
}

#[tokio::test]
async fn sign_in_sends_the_password_once_and_returns_a_session() {
    let t = Arc::new(FixtureTransport::new(vec![x(
        "POST",
        "/Users/AuthenticateByName",
        &[],
        json!({"User": {"Name": "listener", "Id": "user-1"}, "AccessToken": "secret-token", "ServerId": "f1e2d3c4b5a6"}),
    )]));
    let s = client(t.clone())
        .sign_in("listener", "pa ss\"word")
        .await
        .unwrap();
    assert_eq!(
        (
            s.user_id.as_str(),
            s.user_name.as_str(),
            s.server_id.as_str()
        ),
        ("user-1", "listener", "f1e2d3c4b5a6")
    );
    assert_eq!(s.token, "secret-token");
    let sent = &t.sent()[0];
    let body: serde_json::Value = serde_json::from_slice(sent.body.as_ref().unwrap()).unwrap();
    assert_eq!(body, json!({"Username": "listener", "Pw": "pa ss\"word"}));
    let auth = &sent
        .headers
        .iter()
        .find(|(k, _)| k == "Authorization")
        .unwrap()
        .1;
    assert!(auth.starts_with("MediaBrowser Client=\"Encore\""));
    assert!(!auth.contains("Token="));
    // The session never prints its token.
    assert!(!format!("{s:?}").contains("secret-token"));
}

#[tokio::test]
async fn a_wrong_password_says_so() {
    let t = Arc::new(FixtureTransport::new(vec![status(
        x(
            "POST",
            "/Users/AuthenticateByName",
            &[],
            json!("Error processing request."),
        ),
        401,
    )]));
    assert_eq!(
        client(t).sign_in("listener", "nope").await.unwrap_err(),
        SourceError::WrongCredentials
    );
}

#[tokio::test]
async fn quick_connect_from_code_to_session() {
    let t = Arc::new(FixtureTransport::new(vec![
        x("GET", "/QuickConnect/Enabled", &[], json!(true)),
        x(
            "POST",
            "/QuickConnect/Initiate",
            &[],
            json!({"Authenticated": false, "Secret": "s3cr3t", "Code": "123456", "DateAdded": "2026-11-06T12:00:00Z"}),
        ),
        x(
            "GET",
            "/QuickConnect/Connect",
            &[("secret", "s3cr3t")],
            json!({"Authenticated": true, "Secret": "s3cr3t", "Code": "123456"}),
        ),
        x(
            "POST",
            "/Users/AuthenticateWithQuickConnect",
            &[],
            json!({"User": {"Name": "listener", "Id": "user-1"}, "AccessToken": "qc-token", "ServerId": "f1e2d3c4b5a6"}),
        ),
    ]));
    let c = client(t.clone());
    assert!(c.quick_connect_enabled().await.unwrap());
    let code = c.quick_connect_start().await.unwrap();
    assert_eq!(code.code, "123456");
    assert!(c.quick_connect_poll(&code.secret).await.unwrap());
    let s = c.quick_connect_finish(&code.secret).await.unwrap();
    assert_eq!(s.token, "qc-token");
    let body: serde_json::Value =
        serde_json::from_slice(t.sent()[3].body.as_ref().unwrap()).unwrap();
    assert_eq!(body, json!({"Secret": "s3cr3t"}));
}

#[tokio::test]
async fn quick_connect_disabled_and_expired() {
    let t = Arc::new(FixtureTransport::new(vec![
        status(
            x(
                "POST",
                "/QuickConnect/Initiate",
                &[],
                json!("Quick connect is disabled"),
            ),
            401,
        ),
        status(x("GET", "/QuickConnect/Connect", &[], json!(null)), 404),
    ]));
    let c = client(t);
    assert_eq!(
        c.quick_connect_start().await.unwrap_err(),
        SourceError::QuickConnectDisabled
    );
    assert_eq!(
        c.quick_connect_poll("old").await.unwrap_err(),
        SourceError::QuickConnectExpired
    );
}

fn track_json(id: &str, name: &str, n: i64, saved: &str) -> serde_json::Value {
    json!({"Id": id, "Name": name, "SortName": name, "Type": "Audio", "Album": "Golden Hour Static", "AlbumId": "al1",
           "AlbumArtist": "Ember Cove", "AlbumArtists": [{"Name": "Ember Cove", "Id": "ar1"}],
           "ArtistItems": [{"Name": "Ember Cove", "Id": "ar1"}], "Artists": ["Ember Cove"],
           "IndexNumber": n, "ParentIndexNumber": 1, "RunTimeTicks": 2_000_000_000i64 + n * 10_000_000,
           "ProductionYear": 2024, "Container": "flac", "Genres": ["Ambient"], "ProviderIds": {},
           "DateCreated": "2026-01-01T00:00:00.0000000Z", "DateLastSaved": saved, "HasLyrics": n == 1,
           "NormalizationGain": -6.0, "ImageTags": {"Primary": "t"}})
}

fn library(delta_since: Option<&str>) -> Vec<FixtureExchange> {
    let mut v = vec![
        // The first reply carries the server's clock; the cursor comes from it.
        dated(
            x(
                "GET",
                "/Items",
                &[("includeItemTypes", "MusicArtist")],
                json!({"Items": [
                {"Id": "ar1", "Name": "Ember Cove", "SortName": "Ember Cove", "Type": "MusicArtist"}
            ], "TotalRecordCount": 1, "StartIndex": 0}),
            ),
            "Fri, 06 Mar 2026 05:06:07 GMT",
        ),
        x(
            "GET",
            "/Items",
            &[("includeItemTypes", "MusicAlbum")],
            json!({"Items": [
            {"Id": "al1", "Name": "Golden Hour Static", "Type": "MusicAlbum", "AlbumArtist": "Ember Cove",
             "AlbumArtists": [{"Name": "Ember Cove", "Id": "ar1"}], "ProductionYear": 2024,
             "DateCreated": "2026-01-01T00:00:00Z", "DateLastSaved": "2026-01-02T00:00:00Z", "ImageTags": {"Primary": "a"}}
        ], "TotalRecordCount": 1, "StartIndex": 0}),
        ),
        // Songs in two pages of two, then one.
        x(
            "GET",
            "/Items",
            &[("includeItemTypes", "Audio"), ("startIndex", "0")],
            json!({"Items": [
            track_json("t1", "Slow Tide", 1, "2026-01-03T00:00:00Z"), track_json("t2", "Paper Lanterns", 2, "2026-03-04T05:06:07.5Z")
        ], "TotalRecordCount": 3, "StartIndex": 0}),
        ),
        x(
            "GET",
            "/Items",
            &[("includeItemTypes", "Audio"), ("startIndex", "2")],
            json!({"Items": [
            track_json("t3", "Count the Hours", 3, "2026-01-05T00:00:00Z")
        ], "TotalRecordCount": 3, "StartIndex": 2}),
        ),
        x(
            "GET",
            "/Items",
            &[("includeItemTypes", "Playlist")],
            json!({"Items": [
            {"Id": "pl1", "Name": "Evening Drive", "Type": "Playlist"}
        ], "TotalRecordCount": 1, "StartIndex": 0}),
        ),
        x(
            "GET",
            "/Playlists/pl1/Items",
            &[],
            json!({"Items": [
            {"Id": "t3", "PlaylistItemId": "e1"}, {"Id": "t1", "PlaylistItemId": "e2"}
        ], "TotalRecordCount": 2}),
        ),
        x(
            "GET",
            "/Items",
            &[("includeItemTypes", "Audio,MusicAlbum,MusicArtist,Playlist")],
            json!({"Items": [
            {"Id": "ar1"}, {"Id": "al1"}, {"Id": "t1"}, {"Id": "t2"}, {"Id": "t3"}, {"Id": "pl1"}
        ], "TotalRecordCount": 6}),
        ),
    ];
    if let Some(s) = delta_since {
        for e in v.iter_mut().filter(|e| {
            e.path == "/Items"
                && !e
                    .query
                    .get("includeItemTypes")
                    .is_some_and(|t| t.contains(',') || t == "Playlist")
        }) {
            e.query.insert("minDateLastSaved".into(), s.into());
        }
    }
    v
}

#[tokio::test]
async fn a_first_sync_reads_everything_page_by_page() {
    let t = Arc::new(FixtureTransport::new(library(None)));
    let c = signed_in(t.clone());
    let seen = Mutex::new(Vec::new());
    let progress = |p: SyncProgress| seen.lock().unwrap().push(p);
    let d = sync_library(&c, None, &HashSet::new(), 2, &progress)
        .await
        .unwrap();
    assert!(d.full);
    assert_eq!(
        (
            d.artists.len(),
            d.albums.len(),
            d.tracks.len(),
            d.playlists.len()
        ),
        (1, 1, 3, 1)
    );
    assert_eq!(d.tracks[1].duration_ms, 202_000);
    assert_eq!(d.playlists[0].entries[0].track_item_id, "t3");
    assert!(d.removed_item_ids.is_empty());
    // The cursor is the server's clock at the start, a minute early.
    assert_eq!(d.cursor, Some(Cursor("2026-03-06T05:05:07Z".into())));
    let seen = seen.lock().unwrap();
    // 1 artist + 1 album + 3 songs + 1 playlist.
    assert_eq!(*seen.last().unwrap(), SyncProgress { done: 6, total: 6 });
    // Every request is signed with the session, and asks for this user's view.
    for req in t.sent() {
        let auth = &req
            .headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .unwrap()
            .1;
        assert!(auth.ends_with("Token=\"secret-token\""));
        assert!(
            req.url
                .query_pairs()
                .any(|(k, v)| k == "userId" && v == "user-1")
        );
    }
}

#[tokio::test]
async fn a_later_sync_asks_for_changes_and_finds_removals() {
    let since = Cursor("2026-02-01T00:00:00Z".into());
    let t = Arc::new(FixtureTransport::new(library(Some(&since.0))));
    let c = signed_in(t.clone());
    let known: HashSet<String> = ["ar1", "al1", "t1", "t2", "t3", "pl1", "t-gone", "al-gone"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let d = sync_library(&c, Some(&since), &known, 2, &|_| {})
        .await
        .unwrap();
    assert!(!d.full);
    assert_eq!(d.removed_item_ids, ["al-gone", "t-gone"]);
    let asked_for_changes = t
        .sent()
        .iter()
        .filter(|r| {
            r.url
                .query_pairs()
                .any(|(k, v)| k == "minDateLastSaved" && v == since.0)
        })
        .count();
    assert_eq!(asked_for_changes, 4); // artists, albums and two pages of songs
}

#[tokio::test]
async fn without_a_server_clock_the_next_sync_is_a_full_one() {
    let mut lib = library(None);
    lib[0].date = None;
    let t = Arc::new(FixtureTransport::new(lib));
    let d = sync_library(&signed_in(t), None, &HashSet::new(), 2, &|_| {})
        .await
        .unwrap();
    assert_eq!(d.cursor, None);
}

#[tokio::test]
async fn repeated_requests_replay_in_recorded_order() {
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "GET",
            "/QuickConnect/Connect",
            &[],
            json!({"Authenticated": false}),
        ),
        x(
            "GET",
            "/QuickConnect/Connect",
            &[],
            json!({"Authenticated": false}),
        ),
        x(
            "GET",
            "/QuickConnect/Connect",
            &[],
            json!({"Authenticated": true}),
        ),
    ]));
    let c = client(t);
    let mut answers = Vec::new();
    for _ in 0..4 {
        answers.push(c.quick_connect_poll("s").await.unwrap());
    }
    assert_eq!(answers, [false, false, true, true]);
}

#[tokio::test]
async fn an_expired_session_is_reported_as_such() {
    let t = Arc::new(FixtureTransport::new(vec![status(
        x("GET", "/Items", &[], json!(null)),
        401,
    )]));
    let err = sync_library(&signed_in(t), None, &HashSet::new(), 100, &|_| {})
        .await
        .unwrap_err();
    assert_eq!(err, SourceError::SessionExpired);
}

#[tokio::test]
async fn lyrics_are_fetched_raw_or_absent() {
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "GET",
            "/Audio/t1/Lyrics",
            &[],
            json!({"Metadata": {"IsSynced": true}, "Lyrics": [{"Text": "Slow tide", "Start": 10_000_000}]}),
        ),
        status(x("GET", "/Audio/t2/Lyrics", &[], json!(null)), 404),
    ]));
    let c = signed_in(t);
    let l = c.lyrics("t1").await.unwrap().unwrap();
    assert_eq!(l.format, "jellyfin-json");
    assert!(l.body.contains("Slow tide"));
    assert_eq!(c.lyrics("t2").await.unwrap(), None);
}

#[test]
fn stream_urls_for_original_and_capped_quality() {
    let c = signed_in(Arc::new(FixtureTransport::new(vec![])));
    let original = c
        .stream(
            "t1",
            &StreamRequest {
                quality: StreamQuality::Original,
                codec: StreamCodec::Aac,
                direct_play_containers: vec![],
            },
        )
        .unwrap();
    assert_eq!(
        original.url,
        "https://music.example/Audio/t1/stream?static=true"
    );
    assert_eq!(
        original.url_with_api_key,
        "https://music.example/Audio/t1/stream?static=true&ApiKey=secret-token"
    );
    assert!(!original.may_transcode);
    assert!(!format!("{original:?}").contains("secret-token"));

    let capped = c
        .stream(
            "t1",
            &StreamRequest {
                quality: StreamQuality::Capped { kbps: 160 },
                codec: StreamCodec::Opus,
                direct_play_containers: vec!["flac".into(), "mp3".into()],
            },
        )
        .unwrap();
    let u = Url::parse(&capped.url).unwrap();
    assert_eq!(u.path(), "/Audio/t1/universal");
    let q: std::collections::HashMap<String, String> = u.query_pairs().into_owned().collect();
    assert_eq!(q["maxStreamingBitrate"], "160000");
    assert_eq!(q["audioCodec"], "opus");
    assert_eq!(q["transcodingContainer"], "ogg");
    assert_eq!(q["container"], "flac,mp3");
    assert_eq!(q["deviceId"], "dev-1");
    assert!(!q.contains_key("ApiKey"));
    assert!(capped.headers[0].1.contains("Token=\"secret-token\""));
}

#[test]
fn artwork_at_the_exact_size_and_behind_a_reverse_proxy_path() {
    let c = JellyfinClient::new(
        Arc::new(FixtureTransport::new(vec![])),
        Url::parse("https://example.org/jellyfin").unwrap(),
        device(),
        Duration::from_secs(5),
    )
    .signed_in("t".into(), "u".into());
    let a = c.artwork("al1", 600).unwrap();
    assert_eq!(
        a.url,
        "https://example.org/jellyfin/Items/al1/Images/Primary?fillWidth=600&fillHeight=600&quality=90"
    );
}
