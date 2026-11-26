//! The Emby flavour on invented replies (tools/fixtures/emby/, README there), with no
//! server. Their shapes follow what a real Emby 4.10.2.0 answered in CI; the real
//! server itself is driven by `emby_flow_over_http` (encore-core) in the `emby` job.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use url::Url;

use super::*;
use crate::error::SourceError;
use crate::model::{EditOutcome, ServerEdit, SourceKind};
use crate::source::{JellyfinSource, MediaSource};
use crate::transport::{FixtureExchange, FixtureTransport, Method, Request};

const SERVER: &str = include_str!("../../../../tools/fixtures/emby/server.json");
const LIBRARY: &str = include_str!("../../../../tools/fixtures/emby/library.json");
const LYRICS: &str = include_str!("../../../../tools/fixtures/emby/lyrics.json");

const USER: &str = "5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0";
const TOKEN: &str = "invented-emby-token";

fn device(id: &str, client: &str) -> DeviceInfo {
    DeviceInfo {
        client: client.into(),
        device_name: "Test Phone".into(),
        device_id: id.into(),
        version: "0.1.0".into(),
    }
}

fn fixture(json: &str) -> Arc<FixtureTransport> {
    Arc::new(FixtureTransport::from_json(json).unwrap())
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

fn emby_client(t: Arc<FixtureTransport>, base: &str) -> JellyfinClient {
    JellyfinClient::new(
        t,
        Url::parse(base).unwrap(),
        device("dev-1", "Encore"),
        Duration::from_secs(5),
    )
    .with_flavor(Flavor::Emby)
}

fn signed_in(t: Arc<FixtureTransport>) -> JellyfinClient {
    emby_client(t, "http://192.168.1.20:8096").signed_in(TOKEN.into(), USER.into())
}

fn value(req: &Request, key: &str) -> Option<String> {
    req.url
        .query_pairs()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.into_owned())
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[tokio::test]
async fn emby_is_told_from_jellyfin_by_its_public_info() {
    let t = fixture(SERVER);
    let plain = JellyfinClient::new(
        t.clone(),
        Url::parse("http://192.168.1.20:8096").unwrap(),
        device("dev-1", "Encore"),
        Duration::from_secs(5),
    );
    let info = plain.public_info().await.unwrap();
    assert_eq!(info.flavor, Flavor::Emby);
    assert_eq!(info.name, "Den Server");
    assert_eq!(info.version, "4.10.2.0");
    assert_eq!(info.server_id, "0b1c2d3e4f5a46b7a8c9d0e1f2a3b4c5");
    // Asked at the root, where both servers answer.
    assert_eq!(t.sent()[0].url.path(), "/System/Info/Public");

    let reply = |product: Option<&str>, version: &str| {
        let mut b = json!({"ServerName": "Den", "Version": version, "Id": "abc"});
        if let Some(p) = product {
            b["ProductName"] = json!(p);
        }
        Arc::new(FixtureTransport::new(vec![x(
            "GET",
            "/System/Info/Public",
            &[],
            b,
        )]))
    };
    let ask = |t: Arc<FixtureTransport>| async move {
        JellyfinClient::new(
            t,
            Url::parse("https://music.example").unwrap(),
            device("dev-1", "Encore"),
            Duration::from_secs(5),
        )
        .public_info()
        .await
    };
    assert_eq!(
        ask(reply(Some("Jellyfin Server"), "10.11.0"))
            .await
            .unwrap()
            .flavor,
        Flavor::Jellyfin
    );
    assert_eq!(
        ask(reply(Some("Emby Server"), "4.8.11.0"))
            .await
            .unwrap()
            .flavor,
        Flavor::Emby
    );
    assert_eq!(
        ask(reply(Some("Some Other Server"), "4.8.0")).await,
        Err(SourceError::NotJellyfin)
    );
    assert_eq!(
        ask(reply(None, "4.7.14.0")).await,
        Err(SourceError::UnsupportedVersion {
            found: "4.7.14.0".into(),
            minimum: "4.8.0".into()
        })
    );
    // No product, no server name: not enough to call it Emby.
    let bare = Arc::new(FixtureTransport::new(vec![x(
        "GET",
        "/System/Info/Public",
        &[],
        json!({"Version": "4.9.0", "Id": "abc"}),
    )]));
    assert_eq!(ask(bare).await, Err(SourceError::NotJellyfin));
}

#[tokio::test]
async fn emby_paths_headers_and_keys() {
    let t = fixture(SERVER);
    let c = emby_client(t.clone(), "http://192.168.1.20:8096");
    let s = c.sign_in("listener", "made-up password").await.unwrap();
    assert_eq!(s.user_id, USER);
    assert_eq!(s.token, TOKEN);
    let sent = &t.sent()[0];
    assert_eq!(sent.url.path(), "/emby/Users/AuthenticateByName");
    let auth = header(sent, "X-Emby-Authorization").unwrap();
    assert!(auth.starts_with("MediaBrowser Client=\"Encore\""), "{auth}");
    assert!(header(sent, "Authorization").is_none());
    assert!(!format!("{c:?}").contains(TOKEN));

    // No Quick Connect on Emby, and nothing asked to learn that.
    let before = t.sent().len();
    assert!(!c.quick_connect_enabled().await.unwrap());
    assert_eq!(t.sent().len(), before);

    let c = c.signed_in(s.token, s.user_id);
    let req = StreamRequest {
        quality: StreamQuality::Original,
        codec: StreamCodec::Aac,
        direct_play_containers: vec!["flac".into()],
    };
    let st = c.stream("31", &req).unwrap();
    assert!(
        st.url
            .starts_with("http://192.168.1.20:8096/emby/Audio/31/stream?"),
        "{}",
        st.url
    );
    let keyed = Url::parse(&st.url_with_api_key).unwrap();
    assert_eq!(
        keyed.query_pairs().find(|(k, _)| k == "api_key").unwrap().1,
        TOKEN
    );
    assert!(!st.url.contains(TOKEN));
    assert_eq!(st.headers[0].0, "X-Emby-Authorization");
    let capped = c
        .stream(
            "31",
            &StreamRequest {
                quality: StreamQuality::Capped { kbps: 128 },
                ..req
            },
        )
        .unwrap();
    assert!(capped.url.contains("/emby/Audio/31/universal?"));
    assert!(capped.may_transcode);

    // An address that already ends in /emby (a reverse proxy) isn't doubled.
    let proxied = emby_client(fixture("[]"), "https://music.example/emby")
        .signed_in(TOKEN.into(), USER.into());
    assert_eq!(
        proxied.artwork("22", 300).unwrap().url.split('?').next(),
        Some("https://music.example/emby/Items/22/Images/Primary")
    );
}

#[tokio::test]
async fn who_a_token_belongs_to_without_users_me() {
    // A client given only a token (the companion) learns its user from its own session.
    let t = fixture(SERVER);
    let companion = JellyfinClient::new(
        t.clone(),
        Url::parse("http://192.168.1.20:8096").unwrap(),
        device("encore-server", "Encore companion"),
        Duration::from_secs(5),
    )
    .with_flavor(Flavor::Emby)
    .signed_in(TOKEN.into(), String::new());
    let me = companion.me().await.unwrap();
    assert_eq!(me.user_id, USER);
    assert_eq!(me.user_name, "listener");
    assert_eq!(me.server_id, "0b1c2d3e4f5a46b7a8c9d0e1f2a3b4c5");
    assert_eq!(me.image_tag.as_deref(), Some("a1b2c3"));
    assert!(t.sent().iter().all(|r| r.url.path() != "/emby/Users/Me"));

    // A signed-in device knows its user ID: straight to the user record.
    let t = fixture(SERVER);
    let me = signed_in(t.clone()).me().await.unwrap();
    assert_eq!(me.user_id, USER);
    assert_eq!(t.sent().len(), 1);

    // Two users under the one device name (two checks at once): not guessed.
    let t = Arc::new(FixtureTransport::new(vec![x(
        "GET",
        "/emby/Sessions",
        &[],
        json!([{"UserId": "u1", "DeviceId": "encore-server", "Client": "Encore companion"},
               {"UserId": "u2", "DeviceId": "encore-server", "Client": "Encore companion"}]),
    )]));
    let ambiguous = JellyfinClient::new(
        t,
        Url::parse("http://192.168.1.20:8096").unwrap(),
        device("encore-server", "Encore companion"),
        Duration::from_secs(5),
    )
    .with_flavor(Flavor::Emby)
    .signed_in(TOKEN.into(), String::new());
    assert!(matches!(
        ambiguous.me().await,
        Err(SourceError::Protocol(_))
    ));

    // An ended token: 401, as for Jellyfin.
    let mut refused = x("GET", "/emby/Sessions", &[], json!({}));
    refused.status = 401;
    let gone = JellyfinClient::new(
        Arc::new(FixtureTransport::new(vec![refused])),
        Url::parse("http://192.168.1.20:8096").unwrap(),
        device("encore-server", "Encore companion"),
        Duration::from_secs(5),
    )
    .with_flavor(Flavor::Emby)
    .signed_in(TOKEN.into(), String::new());
    assert_eq!(gone.me().await, Err(SourceError::SessionExpired));
}

#[tokio::test]
async fn a_full_emby_sync() {
    let t = fixture(LIBRARY);
    let source = JellyfinSource {
        client: signed_in(t.clone()),
        quick_connect: false,
    };
    assert_eq!(source.kind(), SourceKind::Emby);
    let caps = source.capabilities();
    assert!(!caps.quick_connect);
    assert!(caps.lyrics && caps.transcoding && caps.playlists_writable);
    let delta = source
        .sync_library(None, &HashSet::new(), 100, &|_| {})
        .await
        .unwrap();
    assert!(delta.full);
    assert_eq!(
        (delta.artists.len(), delta.albums.len(), delta.tracks.len()),
        (2, 2, 2)
    );
    let tide = delta
        .tracks
        .iter()
        .find(|t| t.title == "Slow Tide")
        .unwrap();
    // Emby has no HasLyrics: the lyrics stream says so.
    assert!(tide.has_lyrics);
    assert_eq!(tide.duration_ms, 222_400);
    assert_eq!(tide.album_item_id.as_deref(), Some("22"));
    assert!(
        !delta
            .tracks
            .iter()
            .find(|t| t.title == "Low Lantern")
            .unwrap()
            .has_lyrics
    );
    let golden = delta
        .albums
        .iter()
        .find(|a| a.title == "Golden Hour Static")
        .unwrap();
    assert!(golden.has_artwork);
    // Entries by position and song: Emby's slot IDs move.
    let pl = &delta.playlists[0];
    let ids: Vec<&str> = pl
        .entries
        .iter()
        .map(|e| e.entry_id.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["0:31", "1:32", "2:31"]);
    assert_eq!(delta.favorite_item_ids, Some(vec!["31".to_owned()]));
    // Emby's lists say PlayCount 0 beside Played: true; it counts once.
    assert_eq!(delta.play_counts, Some(vec![("32".to_owned(), 1, None)]));
    assert!(delta.cursor.is_some());
    // The songs were asked for with their streams, under /emby.
    let songs = t
        .sent()
        .into_iter()
        .find(|r| {
            value(r, "includeItemTypes").as_deref() == Some("Audio")
                && value(r, "filters").is_none()
        })
        .unwrap();
    assert_eq!(songs.url.path(), "/emby/Items");
    assert!(value(&songs, "fields").unwrap().contains("MediaStreams"));
}

#[tokio::test]
async fn emby_lyrics_come_from_the_songs_lyrics_stream() {
    let t = fixture(LYRICS);
    let c = signed_in(t.clone());
    let raw = c.lyrics("31").await.unwrap().unwrap();
    assert_eq!(
        raw.body,
        "[00:01.20]Salt on the window\n[00:06.10]The tide comes slow\n"
    );
    // Never Jellyfin's /Audio/{id}/Lyrics, which Emby reads as a container name.
    assert!(t.sent().iter().all(|r| !r.url.path().ends_with("/Lyrics")));
    assert_eq!(c.lyrics("32").await.unwrap(), None);
}

/// A playlist whose slots are `slots`, holding `songs`, as Emby lists it.
fn listing(songs: &[&str], slots: &[&str]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = songs
        .iter()
        .zip(slots)
        .map(|(s, p)| json!({"Id": s, "PlaylistItemId": p, "Type": "Audio"}))
        .collect();
    json!({"Items": items, "TotalRecordCount": songs.len()})
}

fn done(method: &str, path: &str, query: &[(&str, &str)]) -> FixtureExchange {
    let mut e = x(method, path, query, json!({}));
    e.status = 204;
    e.body = None;
    e
}

#[tokio::test]
async fn emby_playlist_writes() {
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "POST",
            "/emby/Playlists",
            &[
                ("name", "Road Trip"),
                ("ids", "31,32"),
                ("mediaType", "Audio"),
            ],
            json!({"Id": "41", "Name": "Road Trip", "ItemAddedCount": 2}),
        ),
        // Just after a move: the new order (32, 31, 33), each song still with its old
        // slot. The slots in rising order are the truth: position 1 is slot 2.
        x(
            "GET",
            "/emby/Playlists/40/Items",
            &[],
            listing(&["32", "31", "33"], &["2", "1", "3"]),
        ),
        done("DELETE", "/emby/Playlists/40/Items", &[("entryIds", "2")]),
        // A removal not yet renumbered (5, 7), then renumbered (5, 6).
        x(
            "GET",
            "/emby/Playlists/42/Items",
            &[],
            listing(&["31", "33"], &["5", "7"]),
        ),
        x(
            "GET",
            "/emby/Playlists/42/Items",
            &[],
            listing(&["31", "33"], &["5", "6"]),
        ),
        done("POST", "/emby/Playlists/42/Items/6/Move/0", &[]),
        x(
            "GET",
            "/emby/Users/5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0/Items/40",
            &[],
            json!({"Id": "40", "Name": "Evening Drive", "Type": "Playlist", "Overview": "kept"}),
        ),
        done("POST", "/emby/Items/40", &[]),
        done("DELETE", "/emby/Items/40", &[]),
        x(
            "POST",
            "/emby/Users/5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0/FavoriteItems/31",
            &[],
            json!({"IsFavorite": true}),
        ),
        x(
            "DELETE",
            "/emby/Users/5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0/FavoriteItems/31",
            &[],
            json!({"IsFavorite": false}),
        ),
    ]));
    let c = signed_in(t.clone());
    assert_eq!(
        c.edit(&ServerEdit::CreatePlaylist {
            name: "Road Trip".into(),
            item_ids: vec!["31".into(), "32".into()],
        })
        .await
        .unwrap(),
        EditOutcome::Created("41".into())
    );
    // The mirror's "1:31" (Slow Tide, second) is slot 2 now, not the 1 listed with it.
    c.edit(&ServerEdit::RemoveFromPlaylist {
        playlist_id: "40".into(),
        entry_ids: vec!["1:31".into()],
    })
    .await
    .unwrap();
    // A move waits for the renumbering, then goes by the slot of the song's position.
    c.edit(&ServerEdit::MoveInPlaylist {
        playlist_id: "42".into(),
        entry_id: "1:33".into(),
        new_index: 0,
    })
    .await
    .unwrap();
    c.edit(&ServerEdit::RenamePlaylist {
        playlist_id: "40".into(),
        name: "Night Drive".into(),
    })
    .await
    .unwrap();
    c.edit(&ServerEdit::DeletePlaylist {
        playlist_id: "40".into(),
    })
    .await
    .unwrap();
    for favorite in [true, false] {
        c.edit(&ServerEdit::SetFavorite {
            item_id: "31".into(),
            favorite,
        })
        .await
        .unwrap();
    }
    let sent = t.sent();
    let calls: Vec<String> = sent
        .iter()
        .map(|r| format!("{} {}", r.method.as_str(), r.url.path()))
        .collect();
    assert_eq!(
        calls,
        [
            "POST /emby/Playlists",
            "GET /emby/Playlists/40/Items",
            "DELETE /emby/Playlists/40/Items",
            "GET /emby/Playlists/42/Items",
            "GET /emby/Playlists/42/Items",
            "POST /emby/Playlists/42/Items/6/Move/0",
            "GET /emby/Users/5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0/Items/40",
            "POST /emby/Items/40",
            "DELETE /emby/Items/40",
            "POST /emby/Users/5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0/FavoriteItems/31",
            "DELETE /emby/Users/5e6f7a8b9c0d41e2b3c4d5e6f7a8b9c0/FavoriteItems/31",
        ]
    );
    // A playlist is made from the query (Emby answers a JSON body with a 500).
    assert!(sent[0].body.is_none());
    assert_eq!(value(&sent[2], "entryIds").as_deref(), Some("2"));
    // The rename saves the item back whole, with only its name changed.
    let saved: serde_json::Value = serde_json::from_slice(sent[7].body.as_ref().unwrap()).unwrap();
    assert_eq!(saved["Name"], "Night Drive");
    assert_eq!(saved["Overview"], "kept");
}

#[tokio::test]
async fn an_emby_insert_is_an_add_then_moves() {
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "GET",
            "/emby/Playlists/40/Items",
            &[],
            listing(&["31", "32"], &["1", "2"]),
        ),
        done("POST", "/emby/Playlists/40/Items", &[("ids", "33,34")]),
        x(
            "GET",
            "/emby/Playlists/40/Items",
            &[],
            listing(&["31", "32", "33", "34"], &["1", "2", "3", "4"]),
        ),
        done("POST", "/emby/Playlists/40/Items/3/Move/1", &[]),
        x(
            "GET",
            "/emby/Playlists/40/Items",
            &[],
            listing(&["31", "33", "32", "34"], &["1", "2", "3", "4"]),
        ),
        done("POST", "/emby/Playlists/40/Items/4/Move/2", &[]),
    ]));
    signed_in(t.clone())
        .edit(&ServerEdit::AddToPlaylist {
            playlist_id: "40".into(),
            item_ids: vec!["33".into(), "34".into()],
            position: Some(1),
        })
        .await
        .unwrap();
    let moves: Vec<String> = t
        .sent()
        .iter()
        .filter(|r| r.url.path().contains("/Move/"))
        .map(|r| r.url.path().to_owned())
        .collect();
    assert_eq!(
        moves,
        [
            "/emby/Playlists/40/Items/3/Move/1",
            "/emby/Playlists/40/Items/4/Move/2"
        ]
    );
    // At the end: no moves at all.
    let t = Arc::new(FixtureTransport::new(vec![done(
        "POST",
        "/emby/Playlists/40/Items",
        &[("ids", "33")],
    )]));
    signed_in(t.clone())
        .edit(&ServerEdit::AddToPlaylist {
            playlist_id: "40".into(),
            item_ids: vec!["33".into()],
            position: None,
        })
        .await
        .unwrap();
    assert_eq!(t.sent().len(), 1);
    assert_eq!(t.sent()[0].method, Method::Post);
}

#[tokio::test]
async fn a_jellyfin_client_is_unchanged() {
    // The flavour is Jellyfin unless set: no /emby prefix, `Authorization`, `ApiKey`.
    let c = JellyfinClient::new(
        fixture("[]"),
        Url::parse("https://music.example").unwrap(),
        device("dev-1", "Encore"),
        Duration::from_secs(5),
    )
    .signed_in("t".into(), "u".into());
    assert_eq!(c.flavor(), Flavor::Jellyfin);
    let st = c
        .stream(
            "a1",
            &StreamRequest {
                quality: StreamQuality::Original,
                codec: StreamCodec::Aac,
                direct_play_containers: vec![],
            },
        )
        .unwrap();
    assert!(st.url.starts_with("https://music.example/Audio/a1/stream?"));
    assert!(st.url_with_api_key.contains("ApiKey=t"));
    assert_eq!(st.headers[0].0, "Authorization");
}
