//! Adapter tests on fixture replies, with no server and no plex.tv.
//!
//! STAND-INS: tools/fixtures/plex/*.json were written by hand, with made-up artists,
//! songs, codes and tokens: `server.json` in the shapes a real Plex Media Server 1.43.5
//! answered in CI (core.yml, job `plex`), `plex-tv.json` from plex.tv's documented
//! PIN and resources replies. No test here or anywhere contacts the real plex.tv.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use url::Url;

use super::*;
use crate::error::SourceError;
use crate::jellyfin::{StreamCodec, StreamQuality, StreamRequest, SyncProgress};
use crate::model::{EditOutcome, ServerEdit};
use crate::source::{MediaSource, PlexSource};
use crate::transport::{FixtureExchange, FixtureTransport, Method, Request};

const SERVER: &str = include_str!("../../../../tools/fixtures/plex/server.json");
const PLEX_TV: &str = include_str!("../../../../tools/fixtures/plex/plex-tv.json");

const TOKEN: &str = "fixture-server-token";

fn fixture(json: &str) -> Arc<FixtureTransport> {
    Arc::new(FixtureTransport::from_json(json).unwrap())
}

fn identity() -> PlexIdentity {
    PlexIdentity {
        product: "Encore".into(),
        version: "0.1.0".into(),
        platform: "test".into(),
        client_id: client_identifier("device-test"),
    }
}

fn base() -> Url {
    Url::parse("http://192.168.7.20:32400").unwrap()
}

fn creds() -> PlexCredentials {
    PlexCredentials::new(TOKEN.into(), "fixture-machine-0001".into(), vec![base()])
}

fn client(t: &Arc<FixtureTransport>) -> PlexClient {
    PlexClient::new(t.clone(), base(), identity(), Duration::from_secs(5))
}

fn signed_in(t: &Arc<FixtureTransport>) -> PlexClient {
    client(t).signed_in(creds())
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn value(req: &Request, key: &str) -> Option<String> {
    req.url
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

fn x(method: &str, path: &str, status: u16, body: Option<serde_json::Value>) -> FixtureExchange {
    FixtureExchange {
        origin: None,
        method: method.into(),
        path: path.into(),
        query: Default::default(),
        status,
        body,
        body_text: None,
        body_hex: None,
        error: None,
        date: None,
    }
}

fn no_progress() -> impl Fn(SyncProgress) + Send + Sync {
    |_| {}
}

#[tokio::test]
async fn a_plex_server_is_found_by_its_identity_without_signing_in() {
    let t = fixture(SERVER);
    let info = client(&t).server_info().await.unwrap();
    assert_eq!(info.server_id, "fixture-machine-0001");
    assert_eq!(info.version, "1.43.5.11079-fixture");
    assert!(info.claimed);
    assert!(info.warning.is_some(), "plain http on the local network");
    let sent = t.sent();
    assert_eq!(sent[0].url.path(), "/identity");
    assert_eq!(header(&sent[0], "Accept"), Some("application/json"));
    assert_eq!(header(&sent[0], "X-Plex-Product"), Some("Encore"));
    assert_eq!(header(&sent[0], "X-Plex-Device-Name"), Some("Encore"));
    assert!(header(&sent[0], "X-Plex-Token").is_none());

    // Jellyfin and Subsonic servers have no /identity: not Plex.
    let other = Arc::new(FixtureTransport::new(vec![x(
        "GET",
        "/identity",
        404,
        None,
    )]));
    assert!(matches!(
        client(&other).server_info().await,
        Err(SourceError::Protocol(_))
    ));
    let html = Arc::new(FixtureTransport::new(vec![FixtureExchange {
        body_text: Some("<html>a web page</html>".into()),
        ..x("GET", "/identity", 200, None)
    }]));
    assert!(matches!(
        client(&html).server_info().await,
        Err(SourceError::Protocol(_))
    ));
}

#[tokio::test]
async fn the_link_code_is_polled_until_approved_then_the_servers_are_listed() {
    let t = fixture(PLEX_TV);
    let tv = PlexTv::new(t.clone(), identity(), Duration::from_secs(5));
    let pin = tv.start_pin().await.unwrap();
    assert_eq!(pin.code, "K7QZ");
    assert_eq!(pin.expires_in_seconds, 900);
    assert!(
        pin.auth_url
            .starts_with("https://app.plex.tv/auth#?clientID=")
    );
    assert!(pin.auth_url.contains("code=K7QZ"));
    assert!(
        pin.auth_url
            .contains("context%5Bdevice%5D%5Bproduct%5D=Encore")
    );
    assert!(!format!("{pin:?}").contains("K7QZ"));

    assert_eq!(tv.poll_pin(&pin).await.unwrap(), None, "not approved yet");
    let account_token = tv.poll_pin(&pin).await.unwrap().expect("approved");
    assert!(!format!("{account_token:?}").contains("fixture-account-token"));

    let account = tv.account(&account_token).await.unwrap();
    assert_eq!(
        account,
        PlexAccount {
            id: "31415926".into(),
            user_name: "tone_listener".into()
        }
    );
    let servers = tv.servers(&account_token).await.unwrap();
    assert_eq!(servers.len(), 2, "the player is not a server");
    let attic = &servers[0];
    assert_eq!(attic.machine_id, "fixture-machine-0001");
    assert_eq!(attic.name, "Attic Server");
    assert!(attic.owned);
    assert_eq!(attic.access_token, "fixture-server-token");
    let uris: Vec<&str> = attic.connections.iter().map(|c| c.uri.as_str()).collect();
    assert_eq!(
        uris,
        [
            "https://192-168-7-20.fixturehash.plex.direct:32400/",
            "http://192.168.7.20:32400/",
            "https://203-0-113-9.fixturehash.plex.direct:32400/",
        ],
        "local first, https first, never plain http over the internet"
    );
    assert!(!servers[1].owned);
    assert!(!format!("{attic:?}").contains("fixture-server-token"));

    // Every request names the app and the install's client ID, and only the account
    // calls carry the account token.
    for r in t.sent() {
        assert_eq!(r.url.origin().ascii_serialization(), "https://plex.tv");
        assert_eq!(header(&r, "X-Plex-Product"), Some("Encore"));
        assert_eq!(
            header(&r, "X-Plex-Client-Identifier"),
            Some(identity().client_id.as_str())
        );
        let has_token = header(&r, "X-Plex-Token") == Some("fixture-account-token");
        assert_eq!(
            has_token,
            !r.url.path().starts_with("/api/v2/pins"),
            "{r:?}"
        );
    }
}

#[tokio::test]
async fn an_expired_code_and_a_refused_account_say_so() {
    let t = Arc::new(FixtureTransport::new(vec![
        x("GET", "/api/v2/pins/1", 404, None),
        x("GET", "/api/v2/resources", 401, None),
    ]));
    let tv = PlexTv::new(t.clone(), identity(), Duration::from_secs(5));
    let pin = PlexPin {
        id: "1".into(),
        code: "AAAA".into(),
        expires_in_seconds: 900,
        auth_url: String::new(),
    };
    assert_eq!(
        tv.poll_pin(&pin).await,
        Err(SourceError::QuickConnectExpired)
    );
    assert_eq!(
        tv.servers(&AccountToken::new("x".into())).await,
        Err(SourceError::WrongCredentials)
    );
}

#[tokio::test]
async fn a_full_sync_reads_every_music_section_in_pages_with_likes_and_plays() {
    let t = fixture(SERVER);
    let c = signed_in(&t);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let progress = move |p: SyncProgress| s2.lock().unwrap().push((p.done, p.total));
    let d = sync::sync_library_at(&c, None, &HashSet::new(), 2, &progress, 1_000)
        .await
        .unwrap();
    assert!(d.full && d.playlists_complete);
    assert_eq!(d.artists.len(), 2);
    assert_eq!(d.albums.len(), 2);
    assert_eq!(d.tracks.len(), 3, "two pages of tracks");
    let ids: Vec<&str> = d.tracks.iter().map(|t| t.source_item_id.as_str()).collect();
    assert_eq!(ids, ["111:501", "112:502", "211:601"]);
    let quiet = d.albums.iter().find(|a| a.title == "Quiet Rooms").unwrap();
    assert!(quiet.has_artwork);
    assert_eq!(
        quiet.artwork_tag.as_deref(),
        Some("/library/metadata/110/thumb/1795700011")
    );
    assert_eq!(quiet.genres, ["Ambient"]);
    assert!(!d.albums[1].has_artwork);
    assert_eq!(
        d.artists[1].mbid.as_deref(),
        Some("4f1c2d3e-0000-4000-8000-00000000a200")
    );
    assert_eq!(
        d.favorite_item_ids,
        Some(vec!["111:501".to_owned()]),
        "8 is 4 stars: not a like"
    );
    assert_eq!(
        d.play_counts,
        Some(vec![("111:501".to_owned(), 3, Some(1_795_710_000_000))])
    );
    assert_eq!(d.playlists.len(), 2);
    let night = &d.playlists[0];
    assert_eq!(night.name, "Night drive");
    assert!(night.can_edit);
    assert_eq!(night.entries[0].entry_id.as_deref(), Some("7001"));
    assert_eq!(night.entries[0].track_item_id, "211:601");
    assert!(!d.playlists[1].can_edit, "a smart playlist is read-only");
    assert_eq!(
        d.cursor.as_ref().map(|c| c.0.as_str()),
        Some("plex1:s=3.27;full=1000")
    );
    // The movie section is never listed.
    assert!(
        t.sent()
            .iter()
            .all(|r| !r.url.path().starts_with("/library/sections/1/"))
    );
    // Paging asked for two at a time, from 0 then 2.
    let starts: Vec<String> = t
        .sent()
        .iter()
        .filter(|r| value(r, "type").as_deref() == Some("10"))
        .filter_map(|r| value(r, "X-Plex-Container-Start"))
        .collect();
    assert_eq!(starts, ["0", "2"]);
    let last = *seen.lock().unwrap().last().unwrap();
    assert_eq!(last.0, last.1, "progress ends at its total");
    // Every library request carries the token as a header, never in the URL.
    for r in t.sent() {
        assert_eq!(header(&r, "X-Plex-Token"), Some(TOKEN));
        assert!(!r.url.as_str().contains(TOKEN));
    }
}

#[tokio::test]
async fn an_unchanged_library_syncs_only_playlists_and_likes() {
    let t = fixture(SERVER);
    let c = signed_in(&t);
    let since = crate::model::Cursor("plex1:s=3.27;full=1000".into());
    let d = sync::sync_library_at(&c, Some(&since), &HashSet::new(), 50, &no_progress(), 5_000)
        .await
        .unwrap();
    assert!(!d.full);
    assert!(d.tracks.is_empty() && d.albums.is_empty());
    assert!(d.playlists_complete);
    assert_eq!(d.playlists.len(), 2);
    assert_eq!(d.favorite_item_ids, Some(vec!["111:501".to_owned()]));
    assert_eq!(d.play_counts, None, "plays wait for the daily full sync");
    assert_eq!(d.cursor, Some(since.clone()));
    assert!(
        t.sent()
            .iter()
            .all(|r| value(r, "type").is_none_or(|t| t == "10")),
        "no artists or albums listed"
    );
    // A day later, or when the counter moved, the sync is full again.
    let t = fixture(SERVER);
    let c = signed_in(&t);
    let day = sync::FULL_SYNC_EVERY_MS + 2_000;
    let d = sync::sync_library_at(&c, Some(&since), &HashSet::new(), 50, &no_progress(), day)
        .await
        .unwrap();
    assert!(d.full);
    let moved = crate::model::Cursor("plex1:s=3.26;full=1000".into());
    assert!(library_changed(&c, Some(&moved)).await.unwrap());
    assert!(!library_changed(&c, Some(&since)).await.unwrap());
}

#[tokio::test]
async fn synced_lyrics_come_from_the_lrc_stream() {
    let t = fixture(SERVER);
    let c = signed_in(&t);
    let l = c.lyrics("111:501").await.unwrap().unwrap();
    assert_eq!(l.format, "lrc");
    assert!(l.body.starts_with("[00:01.00]First light"));
    assert_eq!(t.sent().last().unwrap().url.path(), "/library/streams/9003");
    assert_eq!(c.lyrics("112:502").await.unwrap(), None);
}

#[tokio::test]
async fn streams_play_the_file_or_the_transcoder_and_keep_the_token_out_of_the_url() {
    let t = fixture(SERVER);
    let c = signed_in(&t);
    let original = StreamRequest {
        quality: StreamQuality::Original,
        codec: StreamCodec::Aac,
        direct_play_containers: vec!["mp3".into()],
    };
    let s = c.stream("111:501", &original).unwrap();
    assert_eq!(s.url, "http://192.168.7.20:32400/library/parts/501/file");
    assert!(!s.may_transcode);
    assert!(!s.url.contains(TOKEN));
    assert!(
        s.url_with_api_key
            .ends_with(&format!("X-Plex-Token={TOKEN}"))
    );
    assert!(
        s.headers
            .iter()
            .any(|(k, v)| k == "X-Plex-Token" && v == TOKEN)
    );
    assert!(!format!("{s:?}").contains(TOKEN));

    let capped = StreamRequest {
        quality: StreamQuality::Capped { kbps: 128 },
        ..original.clone()
    };
    let s = c.stream("111:501", &capped).unwrap();
    let u = Url::parse(&s.url).unwrap();
    assert!(s.may_transcode);
    assert_eq!(u.path(), "/music/:/transcode/universal/start.mp3");
    let q = |k: &str| {
        u.query_pairs()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.into_owned())
    };
    assert_eq!(q("path").as_deref(), Some("/library/metadata/111"));
    assert_eq!(q("musicBitrate").as_deref(), Some("128"));
    assert_eq!(q("X-Plex-Product").as_deref(), Some("Encore"));
    assert!(
        q("X-Plex-Client-Profile-Extra")
            .unwrap()
            .contains("protocol=http&container=mp3")
    );
    assert!(q("X-Plex-Token").is_none());

    // Artwork: the photo transcoder with the token as a header.
    let a = c.artwork("110", 300).unwrap();
    let u = Url::parse(&a.url).unwrap();
    assert_eq!(u.path(), "/photo/:/transcode");
    assert!(u.as_str().contains("width=300"));
    assert!(!a.url.contains(TOKEN));
    assert!(a.headers.iter().any(|(k, _)| k == "X-Plex-Token"));
    // A track ID without its part can't be streamed.
    assert_eq!(c.stream("111", &original), Err(SourceError::NotFound));
}

#[tokio::test]
async fn cover_colours_ask_for_a_png_and_a_missing_cover_is_none() {
    let t = fixture(SERVER);
    let c = signed_in(&t);
    let png = c
        .cover_png("110", 64, Some("/library/metadata/110/thumb/1795700011"))
        .await
        .unwrap()
        .unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    assert_eq!(c.cover_png("210", 64, None).await.unwrap(), None);
}

#[tokio::test]
async fn playlist_writes_use_entries_moves_and_ratings() {
    let t = fixture(SERVER);
    let source = PlexSource {
        client: signed_in(&t),
    };
    let made = source
        .edit(&ServerEdit::CreatePlaylist {
            name: "Made here".into(),
            item_ids: vec!["111:501".into(), "211:601".into()],
        })
        .await
        .unwrap();
    assert_eq!(made, EditOutcome::Created("950".into()));
    let create = t.sent().pop().unwrap();
    assert_eq!(create.method, Method::Post);
    assert_eq!(
        value(&create, "uri").as_deref(),
        Some("server://fixture-machine-0001/com.plexapp.plugins.library/library/metadata/111,211")
    );
    assert_eq!(value(&create, "title").as_deref(), Some("Made here"));

    source
        .edit(&ServerEdit::AddToPlaylist {
            playlist_id: "900".into(),
            item_ids: vec!["112:502".into()],
            position: None,
        })
        .await
        .unwrap();
    let add = t.sent().pop().unwrap();
    assert_eq!(
        (add.method, add.url.path()),
        (Method::Put, "/playlists/900/items")
    );
    assert!(
        value(&add, "uri")
            .unwrap()
            .ends_with("/library/metadata/112")
    );

    // Move "Low Lantern" (7002) to the top: no `after`.
    source
        .edit(&ServerEdit::MoveInPlaylist {
            playlist_id: "900".into(),
            entry_id: "7002".into(),
            new_index: 0,
        })
        .await
        .unwrap();
    let mv = t.sent().pop().unwrap();
    assert_eq!(mv.url.path(), "/playlists/900/items/7002/move");
    assert_eq!(value(&mv, "after"), None);

    // Removing an entry that is already gone is not an error.
    source
        .edit(&ServerEdit::RemoveFromPlaylist {
            playlist_id: "900".into(),
            entry_ids: vec!["7001".into(), "7999".into()],
        })
        .await
        .unwrap();
    source
        .edit(&ServerEdit::RenamePlaylist {
            playlist_id: "900".into(),
            name: "Late drive".into(),
        })
        .await
        .unwrap();
    source
        .edit(&ServerEdit::DeletePlaylist {
            playlist_id: "900".into(),
        })
        .await
        .unwrap();

    // A like is five stars; an unlike clears the rating.
    for (favorite, rating) in [(true, "10"), (false, "-1")] {
        source
            .edit(&ServerEdit::SetFavorite {
                item_id: "111:501".into(),
                favorite,
            })
            .await
            .unwrap();
        let rate = t.sent().pop().unwrap();
        assert_eq!((rate.method, rate.url.path()), (Method::Put, "/:/rate"));
        assert_eq!(value(&rate, "key").as_deref(), Some("111"));
        assert_eq!(value(&rate, "rating").as_deref(), Some(rating));
    }
}

fn two_entries() -> FixtureExchange {
    x(
        "GET",
        "/playlists/900/items",
        200,
        Some(json!({"MediaContainer": {"size": 2, "Metadata": [
            {"ratingKey": "211", "type": "track", "playlistItemID": 7001, "Media": [{"Part": [{"id": 601}]}]},
            {"ratingKey": "111", "type": "track", "playlistItemID": 7002, "Media": [{"Part": [{"id": 501}]}]}
        ]}})),
    )
}

#[tokio::test]
async fn a_move_goes_after_the_entry_that_will_precede_it() {
    let t = Arc::new(FixtureTransport::new(vec![
        two_entries(),
        x("PUT", "/playlists/900/items/7001/move", 200, None),
    ]));
    let c = signed_in(&t);
    // [7001, 7002]: moving 7001 to index 1 puts it after 7002.
    c.edit(&ServerEdit::MoveInPlaylist {
        playlist_id: "900".into(),
        entry_id: "7001".into(),
        new_index: 1,
    })
    .await
    .unwrap();
    let mv = t.sent().pop().unwrap();
    assert_eq!(mv.url.path(), "/playlists/900/items/7001/move");
    assert_eq!(value(&mv, "after").as_deref(), Some("7002"));
}

#[tokio::test]
async fn adding_at_a_position_moves_the_new_entries_into_place() {
    let three = x(
        "GET",
        "/playlists/900/items",
        200,
        Some(json!({"MediaContainer": {"size": 3, "Metadata": [
            {"ratingKey": "211", "type": "track", "playlistItemID": 7001, "Media": [{"Part": [{"id": 601}]}]},
            {"ratingKey": "111", "type": "track", "playlistItemID": 7002, "Media": [{"Part": [{"id": 501}]}]},
            {"ratingKey": "112", "type": "track", "playlistItemID": 7003, "Media": [{"Part": [{"id": 502}]}]}
        ]}})),
    );
    let t = Arc::new(FixtureTransport::new(vec![
        two_entries(),
        three,
        x("PUT", "/playlists/900/items", 200, None),
        x("PUT", "/playlists/900/items/7003/move", 200, None),
    ]));
    let c = signed_in(&t);
    c.edit(&ServerEdit::AddToPlaylist {
        playlist_id: "900".into(),
        item_ids: vec!["112:502".into()],
        position: Some(1),
    })
    .await
    .unwrap();
    let mv = t.sent().pop().unwrap();
    assert_eq!(mv.url.path(), "/playlists/900/items/7003/move");
    assert_eq!(value(&mv, "after").as_deref(), Some("7001"));
}

#[tokio::test]
async fn a_refused_token_is_a_wrong_sign_in_then_an_expired_session() {
    let t = Arc::new(FixtureTransport::new(vec![
        x(
            "GET",
            "/identity",
            200,
            Some(
                json!({"MediaContainer": {"machineIdentifier": "fixture-machine-0001", "version": "1"}}),
            ),
        ),
        x("GET", "/library/sections", 401, None),
    ]));
    let c = signed_in(&t);
    assert_eq!(c.check_sign_in().await, Err(SourceError::WrongCredentials));
    assert_eq!(
        sync::sync_library(&c, None, &HashSet::new(), 50, &no_progress()).await,
        Err(SourceError::SessionExpired)
    );
    // A token saved for another server is refused too.
    let t = fixture(SERVER);
    let other = client(&t).signed_in(PlexCredentials::new(
        TOKEN.into(),
        "a-different-machine".into(),
        vec![],
    ));
    assert_eq!(
        other.check_sign_in().await,
        Err(SourceError::WrongCredentials)
    );
}

#[tokio::test]
async fn when_the_address_stops_answering_the_next_known_one_is_used() {
    let near = "http://192.168.7.20:32400";
    let far = "https://203-0-113-9.fixturehash.plex.direct:32400";
    let at = |origin: &str, mut e: FixtureExchange| {
        e.origin = Some(origin.to_owned());
        e
    };
    let t = Arc::new(FixtureTransport::new(vec![
        at(
            near,
            FixtureExchange {
                error: Some("timedOut".into()),
                ..x("GET", "/library/sections", 0, None)
            },
        ),
        at(
            far,
            x(
                "GET",
                "/identity",
                200,
                Some(json!({"MediaContainer": {"machineIdentifier": "fixture-machine-0001"}})),
            ),
        ),
        at(
            far,
            x(
                "GET",
                "/library/sections",
                200,
                Some(json!({"MediaContainer": {"size": 0}})),
            ),
        ),
    ]));
    let c = client(&t).signed_in(PlexCredentials::new(
        TOKEN.into(),
        "fixture-machine-0001".into(),
        vec![base(), Url::parse(far).unwrap()],
    ));
    let sections = c.music_sections().await.unwrap();
    assert!(sections.is_empty());
    assert_eq!(c.base_url().as_str(), format!("{far}/"));
    // plex.tv was never asked: the lookup is off unless the app turns it on.
    assert!(t.sent().iter().all(|r| r.url.host_str() != Some("plex.tv")));
}

#[tokio::test]
async fn with_the_lookup_on_plex_tv_gives_the_new_addresses() {
    let near = "http://192.168.7.20:32400";
    let far = "https://203-0-113-9.fixturehash.plex.direct:32400";
    let at = |origin: &str, mut e: FixtureExchange| {
        e.origin = Some(origin.to_owned());
        e
    };
    let mut exchanges: Vec<FixtureExchange> =
        serde_json::from_str::<Vec<FixtureExchange>>(PLEX_TV).unwrap();
    exchanges.extend([
        at(
            near,
            FixtureExchange {
                error: Some("refused".into()),
                ..x("GET", "/library/sections", 0, None)
            },
        ),
        at(
            "https://192-168-7-20.fixturehash.plex.direct:32400",
            FixtureExchange {
                error: Some("refused".into()),
                ..x("GET", "/identity", 0, None)
            },
        ),
        at(
            far,
            x(
                "GET",
                "/identity",
                200,
                Some(json!({"MediaContainer": {"machineIdentifier": "fixture-machine-0001"}})),
            ),
        ),
        at(
            far,
            x(
                "GET",
                "/library/sections",
                200,
                Some(json!({"MediaContainer": {"size": 0}})),
            ),
        ),
    ]);
    let t = Arc::new(FixtureTransport::new(exchanges));
    let c = client(&t).signed_in(creds()).with_address_lookup(true);
    c.music_sections().await.unwrap();
    assert_eq!(c.base_url().as_str(), format!("{far}/"));
    let asked = t
        .sent()
        .into_iter()
        .find(|r| r.url.path() == "/api/v2/resources")
        .expect("plex.tv asked for the addresses");
    assert_eq!(
        header(&asked, "X-Plex-Token"),
        Some(TOKEN),
        "with the server's token"
    );
}

#[tokio::test]
async fn a_server_that_lets_anyone_in_is_noticed() {
    let open = Arc::new(FixtureTransport::new(vec![x(
        "GET",
        "/library/sections",
        200,
        Some(json!({"MediaContainer": {"size": 0}})),
    )]));
    assert!(client(&open).lets_anyone_in().await.unwrap());
    let closed = Arc::new(FixtureTransport::new(vec![x(
        "GET",
        "/library/sections",
        401,
        None,
    )]));
    assert!(!client(&closed).lets_anyone_in().await.unwrap());
}
