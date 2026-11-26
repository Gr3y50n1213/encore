//! The public API end to end on the real Jellyfin 12.0.0 recording: connect, sign
//! in with Quick Connect, sync, browse, search, play an album, report playback.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use encore_sources::{FixtureTransport, Transport};

use super::*;

// Phase 10.
mod analysis;
mod downloads;
// Phase 9, WP1.
mod sleep;
// Phase 9, WP2.
mod widgets;

const RECORDED: &str = include_str!("../../../../../tools/fixtures/jellyfin/recorded.json");

#[derive(Default)]
struct Events(Mutex<Vec<CoreEvent>>);

impl CoreListener for Events {
    fn on_event(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Events {
    fn kinds(&self) -> Vec<EventKind> {
        self.0.lock().unwrap().iter().map(|e| e.kind).collect()
    }
    fn take(&self) -> Vec<CoreEvent> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(name: &str) -> TempDir {
    let d = std::env::temp_dir().join(format!(
        "encore-api-{name}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir_all(&d).unwrap();
    TempDir(d)
}

fn core_on_recording(dir: &TempDir) -> (Arc<EncoreCore>, Arc<Events>) {
    let t: Arc<dyn Transport> = Arc::new(FixtureTransport::from_json(RECORDED).unwrap());
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    };
    // The recording was made with pages of 50.
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(t)).unwrap();
    let events = Arc::new(Events::default());
    core.set_listener(events.clone());
    (core, events)
}

fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

/// Connect and sign in with Quick Connect, as the recording did.
fn signed_in(core: &EncoreCore) -> SignedIn {
    let check = block(core.check_address("http://jellyfin.local:8096".into())).unwrap();
    assert_eq!(check.version, "12.0.0");
    assert!(check.plain_http_on_local_network);
    let start = block(core.quick_connect_start(check.base_url.clone())).unwrap();
    assert_eq!(start.code.chars().count(), 6);
    assert_eq!(start.expires_in_seconds, 300);
    block(core.quick_connect_wait()).unwrap()
}

/// The production path (`EncoreCore::new`: real HTTP, the real page size and poll
/// interval) against tools/ci/replay_jellyfin.py. Ignored because it needs that
/// server: CI starts it and runs this with `--ignored` (tools/ci/replay.sh).
#[test]
#[ignore = "needs ENCORE_REPLAY_URL: tools/ci/replay.sh"]
fn library_flow_over_http_against_the_replay_server() {
    let url = std::env::var("ENCORE_REPLAY_URL").expect("set ENCORE_REPLAY_URL");
    let dir = temp_dir("http");
    let core = EncoreCore::new(CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    })
    .unwrap();
    let check = block(core.check_address(url)).unwrap();
    assert_eq!(check.version, "12.0.0");
    assert!(check.plain_http_on_local_network);
    block(core.quick_connect_start(check.base_url)).unwrap();
    let signed_in = block(core.quick_connect_wait()).unwrap();
    assert_eq!(core.current_profile(), Some(signed_in.profile_id));
    let synced = block(core.sync_library()).unwrap();
    assert_eq!(
        (synced[0].artists, synced[0].albums, synced[0].songs),
        (26, 38, 150)
    );
    // A second sync is a delta, cursored by the replayed Date header.
    let again = block(core.sync_library()).unwrap();
    assert_eq!(again[0].removed, 0);
    let all = core.songs(SongSort::Title, 0, 1000).unwrap();
    assert_eq!(all.len(), 150);

    // Phase 3 over real HTTP, as the smoke tests do it.
    let lyrics = all
        .iter()
        .filter(|s| s.has_lyrics)
        .find_map(|s| block(core.lyrics(s.id)).unwrap())
        .expect("recorded lyrics");
    assert_eq!(lyrics.sync_level, LyricsSync::Line);
    let art = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.has_artwork)
        .unwrap();
    let palette = block(core.album_palette(art.id)).unwrap().unwrap();
    assert_eq!(
        palette.base, "3D0000",
        "board 04's cover, as the recording serves it"
    );
    let list = core.playlists(0, 1).unwrap()[0].id;
    let before = core.playlist(list).unwrap().entries.len();
    let in_list: Vec<i64> = core
        .playlist(list)
        .unwrap()
        .entries
        .iter()
        .filter_map(|e| e.song.as_ref().map(|s| s.id))
        .collect();
    let fresh = all.iter().find(|s| !in_list.contains(&s.id)).unwrap().id;
    let edit = core
        .add_to(
            DropTarget {
                kind: DropTargetKind::Playlist,
                id: Some(list),
            },
            vec![fresh],
            None,
            false,
        )
        .unwrap();
    assert_eq!(edit.added, 1);
    core.undo(edit.edit_id.unwrap()).unwrap();
    assert_eq!(core.playlist(list).unwrap().entries.len(), before);
}

/// Phase 12, stage 1 (ADR core-0021): the production path against a real Navidrome, which
/// CI starts in Docker on a small library of made-up tone files (core.yml, "navidrome").
/// Recognise it, sign in, sync, read lyrics, edit a playlist end to end, like a song.
#[test]
#[ignore = "needs a Navidrome: NAVIDROME_URL, NAVIDROME_USER, NAVIDROME_PASSWORD, NAVIDROME_SONGS"]
fn navidrome_flow_over_http() {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let songs: usize = env("NAVIDROME_SONGS").parse().unwrap();
    let dir = temp_dir("navidrome");
    let core = EncoreCore::new(CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-navidrome".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    })
    .unwrap();

    let check = block(core.check_address(env("NAVIDROME_URL"))).unwrap();
    assert_eq!(check.kind, ServerKind::Subsonic);
    assert!(check.name.to_lowercase().contains("navidrome"), "{check:?}");
    let signed = block(core.sign_in(
        check.base_url.clone(),
        env("NAVIDROME_USER"),
        env("NAVIDROME_PASSWORD"),
    ))
    .unwrap();
    assert!(signed.token.starts_with("subsonic1:"));
    assert!(
        !signed.token.contains(&env("NAVIDROME_PASSWORD")),
        "the password is never kept"
    );

    let synced = block(core.sync_library()).unwrap();
    assert_eq!(synced[0].songs as usize, songs, "{synced:?}");
    let all = core.songs(SongSort::Title, 0, 1000).unwrap();
    assert_eq!(all.len(), songs);

    // Nothing changed on the server: the next sync is a light one (playlists and
    // likes, no songs), and the library stays whole. The edits below sync this way.
    let again = block(core.sync_library()).unwrap();
    assert_eq!(again[0].songs, 0, "{again:?}");
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), songs);

    // The one song with a sidecar .lrc has timed lyrics.
    let lyrics = all
        .iter()
        .find_map(|s| block(core.lyrics(s.id)).ok().flatten())
        .expect("a song with lyrics");
    assert_eq!(lyrics.sync_level, LyricsSync::Line);

    // The album with a cover (flat teal, 1E6E8C) gets its colours from Navidrome's JPEG.
    let covered = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.has_artwork)
        .expect("the album with cover.jpg");
    let palette = block(core.album_palette(covered.id))
        .unwrap()
        .expect("a palette from a JPEG cover");
    println!("palette of a Navidrome JPEG cover: {palette:?}");
    let lead = u32::from_str_radix(&palette.sampled[0].colour, 16).unwrap();
    let [r, g, b] = [lead >> 16, (lead >> 8) & 0xFF, lead & 0xFF].map(|c| c as i32);
    assert!(
        (r - 0x1E).abs() < 16 && (g - 0x6E).abs() < 16 && (b - 0x8C).abs() < 16,
        "{palette:?}"
    );

    // A new playlist made here reaches the server, then a rename, a removal and a delete.
    let made = core
        .add_to(
            DropTarget {
                kind: DropTargetKind::NewPlaylist,
                id: None,
            },
            vec![all[0].id, all[1].id, all[2].id],
            None,
            false,
        )
        .unwrap();
    let list = made.playlist_id.unwrap();
    core.rename_playlist(list, "From CI".into()).unwrap();
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    let on_server = core
        .playlists(0, 50)
        .unwrap()
        .into_iter()
        .find(|p| p.name == "From CI")
        .expect("the playlist came back from the server");
    assert_eq!(on_server.song_count, 3);
    let titles = |id: i64| -> Vec<String> {
        core.playlist(id)
            .unwrap()
            .entries
            .iter()
            .map(|e| e.song.as_ref().map(|s| s.title.clone()).unwrap_or_default())
            .collect()
    };
    println!("before the removal: {:?}", titles(on_server.id));
    core.remove_from_playlist(on_server.id, vec![1]).unwrap();
    println!("removed here: {:?}", titles(on_server.id));
    let sent = block(core.send_edits()).unwrap();
    println!("sent: {sent:?}; after sending: {:?}", titles(on_server.id));
    block(core.sync_library()).unwrap();
    println!("after a sync: {:?}", titles(on_server.id));
    assert_eq!(core.playlist(on_server.id).unwrap().entries.len(), 2);
    core.delete_playlist(on_server.id).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(
        core.playlists(0, 50)
            .unwrap()
            .iter()
            .all(|p| p.name != "From CI")
    );

    // A like is a star on the server, and comes back from it.
    core.like(vec![all[0].id]).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(
        core.liked_songs(0, 10)
            .unwrap()
            .iter()
            .any(|s| s.id == all[0].id)
    );
}

/// Phase 12, stage 3 (ADR core-0022): the production path against a real Emby, which CI
/// starts in Docker on the same made-up tone files (core.yml, "emby"), signed in as a user
/// who is not an administrator. Recognise it, sign in, sync, read lyrics and cover colours,
/// stream, like and unlike, read play counts, and edit a playlist end to end, moves too.
#[test]
#[ignore = "needs an Emby: EMBY_URL, EMBY_USER, EMBY_PASSWORD, EMBY_SONGS, EMBY_PLAYED"]
fn emby_flow_over_http() {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let songs: usize = env("EMBY_SONGS").parse().unwrap();
    let dir = temp_dir("emby");
    let core = EncoreCore::new(CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-emby".into(),
        device_name: "Test Phone".into(),
        platform: "test".into(),
    })
    .unwrap();

    // 1. Found as Emby, not Jellyfin or Subsonic; no Quick Connect.
    let check = block(core.check_address(env("EMBY_URL"))).unwrap();
    println!("check: {check:?}");
    assert_eq!(check.kind, ServerKind::Emby);
    assert!(!check.server_id.is_empty());
    assert!(matches!(
        block(core.quick_connect_start(check.base_url.clone())),
        Err(EncoreError::QuickConnectDisabled { .. })
    ));

    // 2. A wrong password is refused as one; the right one signs in.
    assert!(matches!(
        block(core.sign_in(
            check.base_url.clone(),
            env("EMBY_USER"),
            "not the password".into()
        )),
        Err(EncoreError::WrongCredentials { .. })
    ));
    let signed = block(core.sign_in(
        check.base_url.clone(),
        env("EMBY_USER"),
        env("EMBY_PASSWORD"),
    ))
    .unwrap();
    assert!(!signed.token.contains(&env("EMBY_PASSWORD")));
    assert!(!format!("{signed:?}").contains(&signed.token));
    let servers = core.servers().unwrap();
    assert_eq!(servers[0].kind, "emby", "{servers:?}");

    // 3. A full sync: every song, album and artist.
    let synced = block(core.sync_library()).unwrap();
    println!("first sync: {synced:?}");
    assert_eq!(synced[0].songs as usize, songs, "{synced:?}");
    assert_eq!((synced[0].albums, synced[0].artists), (2, 2), "{synced:?}");
    let all = core.songs(SongSort::Title, 0, 1000).unwrap();
    assert_eq!(all.len(), songs);
    assert_eq!(core.albums(AlbumSort::Title, 0, 100).unwrap().len(), 2);
    assert_eq!(core.artists(0, 100).unwrap().len(), 2);
    let by_title = |t: &str| all.iter().find(|s| s.title == t).unwrap().id;

    // 4. A second sync is a delta (what was saved since the first, by Emby's clock),
    // and the library stays whole.
    let again = block(core.sync_library()).unwrap();
    println!("second sync: {again:?}");
    assert_eq!(again[0].removed, 0);
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), songs);

    // 5. The song with a sidecar .lrc has timed lyrics, from Emby's lyrics stream.
    let lyrics = block(core.lyrics(by_title("Opening")))
        .unwrap()
        .expect("Opening has lyrics");
    println!("lyrics: {lyrics:?}");
    assert_eq!(lyrics.sync_level, LyricsSync::Line);
    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.lines[0].text, "A line to sing");
    assert_eq!(block(core.lyrics(by_title("Middle"))).unwrap(), None);

    // 6. The album with the cover (flat teal, 1E6E8C) gets its colours from Emby's PNG.
    let covered = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.has_artwork)
        .expect("the album with a cover");
    let palette = block(core.album_palette(covered.id))
        .unwrap()
        .expect("a palette");
    println!("palette of an Emby cover: {palette:?}");
    let lead = u32::from_str_radix(&palette.sampled[0].colour, 16).unwrap();
    let [r, g, b] = [lead >> 16, (lead >> 8) & 0xFF, lead & 0xFF].map(|c| c as i32);
    assert!(
        (r - 0x1E).abs() < 16 && (g - 0x6E).abs() < 16 && (b - 0x8C).abs() < 16,
        "{palette:?}"
    );

    // 7. The stream address really streams: with the headers, and with the key in it.
    let http = encore_sources::http::HttpTransport::new(None, Duration::from_secs(10)).unwrap();
    for network in [Network::Wifi, Network::Cellular] {
        let s = core.stream_for(by_title("Hum"), network).unwrap();
        for (url, headers) in [
            (s.url.clone(), s.headers.clone()),
            (s.url_with_api_key.clone(), vec![]),
        ] {
            let got = block(http.send(encore_sources::transport::Request {
                method: encore_sources::transport::Method::Get,
                url: url::Url::parse(&url).unwrap(),
                headers: headers.into_iter().map(|h| (h.name, h.value)).collect(),
                body: None,
                timeout: Duration::from_secs(60),
            }))
            .unwrap();
            println!(
                "{network:?} stream (may transcode: {}): status {}, {} bytes",
                s.may_transcode,
                got.status,
                got.body.len()
            );
            assert_eq!(got.status, 200);
            assert!(got.body.len() > 10_000, "{} bytes", got.body.len());
        }
    }

    // 8. Like and unlike: a favourite on the server, and back from it.
    let liked = by_title("Middle");
    core.like(vec![liked]).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(
        core.liked_songs(0, 10)
            .unwrap()
            .iter()
            .any(|s| s.id == liked)
    );
    core.unlike(vec![liked]).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(
        core.liked_songs(0, 10)
            .unwrap()
            .iter()
            .all(|s| s.id != liked)
    );

    // 9. Play counts from other apps: CI marked one song played for this user.
    let played = env("EMBY_PLAYED");
    let rows = core
        .inner
        .profile()
        .unwrap()
        .store
        .lock()
        .unwrap()
        .recs_songs()
        .unwrap();
    let row = rows.iter().find(|r| r.title == played).unwrap();
    println!("{played}: played {} times elsewhere", row.server_play_count);
    assert!(row.server_play_count >= 1);
    assert!(
        rows.iter()
            .filter(|r| r.title != played)
            .all(|r| r.server_play_count == 0)
    );

    // 10. A playlist made here reaches Emby, then a rename.
    let titles = |id: i64| -> Vec<String> {
        core.playlist(id)
            .unwrap()
            .entries
            .iter()
            .map(|e| e.song.as_ref().map(|s| s.title.clone()).unwrap_or_default())
            .collect()
    };
    let find_list = |name: &str| {
        core.playlists(0, 50)
            .unwrap()
            .into_iter()
            .find(|p| p.name == name)
    };
    let made = core
        .add_to(
            DropTarget {
                kind: DropTargetKind::NewPlaylist,
                id: None,
            },
            vec![by_title("Opening"), by_title("Middle"), by_title("Closing")],
            None,
            false,
        )
        .unwrap();
    core.rename_playlist(made.playlist_id.unwrap(), "From CI".into())
        .unwrap();
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    let list = find_list("From CI")
        .expect("the playlist came back from Emby")
        .id;
    assert_eq!(titles(list), ["Opening", "Middle", "Closing"]);

    // 11. A move and, at once, a removal: Emby may renumber its entry IDs after a move
    // and list the old ones for a moment, so both are done on the songs, not the IDs.
    core.move_in_playlist(list, 2, 0).unwrap();
    core.remove_from_playlist(list, vec![1]).unwrap();
    assert_eq!(titles(list), ["Closing", "Middle"]);
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    println!("after a move and a removal: {:?}", titles(list));
    assert_eq!(titles(list), ["Closing", "Middle"]);

    // 12. An insert in the middle (Emby adds at the end only).
    core.add_to(
        DropTarget {
            kind: DropTargetKind::Playlist,
            id: Some(list),
        },
        vec![by_title("Hum")],
        Some(1),
        false,
    )
    .unwrap();
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    println!("after an insert at 1: {:?}", titles(list));
    assert_eq!(titles(list), ["Closing", "Hum", "Middle"]);

    // 13. A removal right after the insert, then the delete.
    core.remove_from_playlist(list, vec![2]).unwrap();
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    assert_eq!(titles(list), ["Closing", "Hum"]);
    core.delete_playlist(list).unwrap();
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    assert!(find_list("From CI").is_none());
}

/// ADR core-0022 on the invented Emby replies (tools/fixtures/emby/): found as Emby,
/// signed in as kind "emby", synced and read under `/emby`, and opened again from the
/// stored kind after a restart, still as Emby.
#[test]
fn an_emby_server_is_found_signed_in_and_restored_by_its_kind() {
    let fixtures = || -> Arc<FixtureTransport> {
        let mut all: Vec<serde_json::Value> = Vec::new();
        for f in [
            include_str!("../../../../../tools/fixtures/emby/server.json"),
            include_str!("../../../../../tools/fixtures/emby/library.json"),
            include_str!("../../../../../tools/fixtures/emby/lyrics.json"),
        ] {
            all.extend(serde_json::from_str::<Vec<serde_json::Value>>(f).unwrap());
        }
        Arc::new(FixtureTransport::from_json(&serde_json::to_string(&all).unwrap()).unwrap())
    };
    let dir = temp_dir("emby-unit");
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "dev-1".into(),
        device_name: "Test Phone".into(),
        platform: "test".into(),
    };
    let t = fixtures();
    let core = EncoreCore::build(
        config.clone(),
        Duration::ZERO,
        50,
        Some(t.clone() as Arc<dyn Transport>),
    )
    .unwrap();
    let check = block(core.check_address("http://192.168.1.20:8096".into())).unwrap();
    assert_eq!(check.kind, ServerKind::Emby);
    assert_eq!(check.name, "Den Server");
    let signed = block(core.sign_in(
        check.base_url.clone(),
        "listener".into(),
        "made-up password".into(),
    ))
    .unwrap();
    assert_eq!(signed.token, "invented-emby-token");
    assert_eq!(core.servers().unwrap()[0].kind, "emby");
    let synced = block(core.sync_library()).unwrap();
    assert_eq!((synced[0].songs, synced[0].albums), (2, 2));
    let tide = core
        .songs(SongSort::Title, 0, 10)
        .unwrap()
        .into_iter()
        .find(|s| s.title == "Slow Tide")
        .unwrap();
    let lyrics = block(core.lyrics(tide.id)).unwrap().unwrap();
    assert_eq!(lyrics.sync_level, LyricsSync::Line);
    assert_eq!(lyrics.lines[1].text, "The tide comes slow");
    // Every request but the public check (asked at the root, where both servers answer)
    // went under /emby, with Emby's header; none to /Users/Me.
    let sent: Vec<_> = t
        .sent()
        .into_iter()
        .filter(|r| r.url.path() != "/System/Info/Public")
        .collect();
    assert!(
        sent.iter().all(|r| r.url.path().starts_with("/emby/")
            && r.headers.iter().any(|(k, _)| k == "X-Emby-Authorization")),
        "{:?}",
        sent.iter()
            .map(|r| r.url.path().to_owned())
            .collect::<Vec<_>>()
    );
    assert!(sent.iter().all(|r| !r.url.path().ends_with("/Users/Me")));
    drop(core);

    // The app starts again: the stored kind brings the Emby client back.
    let t = fixtures();
    let core = EncoreCore::build(
        config,
        Duration::ZERO,
        50,
        Some(t.clone() as Arc<dyn Transport>),
    )
    .unwrap();
    core.open_profile(
        signed.profile_id.clone(),
        vec![SourceToken {
            source_id: signed.source_id,
            token: signed.token.clone(),
        }],
    )
    .unwrap();
    block(core.sync_library()).unwrap();
    assert!(!t.sent().is_empty());
    assert!(t.sent().iter().all(|r| r.url.path().starts_with("/emby/")));
}

/// Phase 12, stage 2 (ADR core-0023): the production path against a real Plex Media Server,
/// which CI starts in Docker, unclaimed and with no way out to the internet, on the same
/// made-up tone files (core.yml, "plex"). plex.tv can't vouch for an unclaimed server, so
/// the sign-in starts from the server's address and the token it takes (any, from CI's
/// allowed network); everything after it is the app's own path.
#[test]
#[ignore = "needs a Plex Media Server: PLEX_URL, PLEX_SONGS"]
fn plex_flow_over_http() {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let songs: usize = env("PLEX_SONGS").parse().unwrap();
    let dir = temp_dir("plex");
    let core = EncoreCore::new(CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-plex".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    })
    .unwrap();

    // Found by its address, and by its host alone (Plex's own port is tried last).
    let check = block(core.check_address(env("PLEX_URL"))).unwrap();
    println!("found: {check:?}");
    assert_eq!(check.kind, ServerKind::Plex);
    assert!(!check.server_id.is_empty());
    assert!(check.plain_http_on_local_network);
    let host = url::Url::parse(&env("PLEX_URL"))
        .unwrap()
        .host_str()
        .unwrap()
        .to_owned();
    let by_host = block(core.check_address(host)).unwrap();
    assert_eq!(by_host.kind, ServerKind::Plex);
    assert_eq!(by_host.server_id, check.server_id);

    let signed = block(
        core.inner
            .plex_sign_in_direct(&check.base_url, "ci-unclaimed-server"),
    )
    .unwrap();
    assert!(signed.token.starts_with("plex1:"));
    let source_id = signed.source_id;
    let plex = || -> Arc<crate::source_any::AnySource> {
        let p = core.inner.profile().unwrap();
        lock(&p.sources).unwrap().get(&source_id).cloned().unwrap()
    };
    let client = |src: &crate::source_any::AnySource| match src {
        crate::source_any::AnySource::Plex(s) => s.client.credentials().is_some(),
        _ => false,
    };
    assert!(client(&plex()));

    let synced = block(core.sync_library()).unwrap();
    println!("first sync: {synced:?}");
    assert_eq!(synced[0].songs as usize, songs, "{synced:?}");
    assert_eq!((synced[0].artists, synced[0].albums), (2, 2), "{synced:?}");
    let all = core.songs(SongSort::Title, 0, 1000).unwrap();
    assert_eq!(all.len(), songs);

    // Nothing changed on the server: the next sync is a light one (playlists and likes).
    let again = block(core.sync_library()).unwrap();
    assert_eq!(again[0].songs, 0, "{again:?}");
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), songs);

    // The song with a sidecar .lrc has timed lyrics, from its lyric stream.
    let lyrics = all
        .iter()
        .find_map(|s| match block(core.lyrics(s.id)) {
            Ok(Some(l)) => Some(l),
            other => {
                let raw = block(core.lyrics_raw(s.id));
                println!("lyrics of {}: {other:?}; raw {raw:?}", s.title);
                None
            }
        })
        .expect("a song with lyrics");
    assert_eq!(lyrics.sync_level, LyricsSync::Line);
    assert_eq!(lyrics.lines[0].text, "A line to sing");

    // The album with cover.jpg (flat teal, 1E6E8C) gets its colours from the photo
    // transcoder's PNG.
    let covered = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.has_artwork)
        .expect("the album with cover.jpg");
    let palette = block(core.album_palette(covered.id))
        .unwrap()
        .expect("a palette from the cover");
    println!("palette of a Plex cover: {palette:?}");
    let lead = u32::from_str_radix(&palette.sampled[0].colour, 16).unwrap();
    let [r, g, b] = [lead >> 16, (lead >> 8) & 0xFF, lead & 0xFF].map(|c| c as i32);
    assert!(
        (r - 0x1E).abs() < 16 && (g - 0x6E).abs() < 16 && (b - 0x8C).abs() < 16,
        "{palette:?}"
    );

    // The stream really plays: the original file, with the headers and with the token in
    // the address, and a ranged read for sound analysis.
    core.set_stream_quality(Network::Wifi, StreamQuality::Original)
        .unwrap();
    let original = core.stream_for(all[0].id, Network::Wifi).unwrap();
    assert!(!original.may_transcode);
    let headers: Vec<(&str, &str)> = original
        .headers
        .iter()
        .map(|h| (h.name.as_str(), h.value.as_str()))
        .collect();
    let with_headers = block(get(&original.url, &headers));
    assert_eq!(with_headers.status, 200);
    assert!(
        with_headers.body.starts_with(b"ID3") || with_headers.body.first() == Some(&0xFF),
        "an MP3"
    );
    let keyed = block(get(&original.url_with_api_key, &[]));
    assert_eq!(
        (keyed.status, keyed.body.len()),
        (200, with_headers.body.len())
    );
    let mut ranged = headers.clone();
    ranged.push(("Range", "bytes=0-99"));
    let part = block(get(&original.url, &ranged));
    assert_eq!((part.status, part.body.len()), (206, 100));
    // The bitrate cap goes to the transcoder with a profile it accepts (the transcode
    // itself needs codecs the server downloads from plex.tv, which CI never reaches).
    core.set_stream_quality(Network::Cellular, StreamQuality::DataSaver)
        .unwrap();
    let capped = core.stream_for(all[0].id, Network::Cellular).unwrap();
    assert!(capped.may_transcode);
    let decision = capped.url_with_api_key.replace(
        "/transcode/universal/start.mp3",
        "/transcode/universal/decision",
    );
    let answer = block(get(&decision, &[("Accept", "application/json")])).text();
    println!("transcode decision: {}", &answer[..answer.len().min(400)]);
    assert!(answer.contains("\"generalDecisionCode\":1001"), "{answer}");

    // Playlists: create with three songs, rename, move, remove, add at a position, delete.
    let made = core
        .add_to(
            DropTarget {
                kind: DropTargetKind::NewPlaylist,
                id: None,
            },
            vec![all[0].id, all[1].id, all[2].id],
            None,
            false,
        )
        .unwrap();
    let list = made.playlist_id.unwrap();
    core.rename_playlist(list, "From CI".into()).unwrap();
    let sent = block(core.send_edits()).unwrap();
    assert_eq!((sent.refused, sent.waiting), (0, 0), "{sent:?}");
    block(core.sync_library()).unwrap();
    let on_server = core
        .playlists(0, 50)
        .unwrap()
        .into_iter()
        .find(|p| p.name == "From CI")
        .expect("the playlist came back from the server");
    assert_eq!(on_server.song_count, 3);
    let titles = |id: i64| -> Vec<String> {
        core.playlist(id)
            .unwrap()
            .entries
            .iter()
            .map(|e| e.song.as_ref().map(|s| s.title.clone()).unwrap_or_default())
            .collect()
    };
    let before = titles(on_server.id);
    println!("made: {before:?}");
    core.move_in_playlist(on_server.id, 0, 2).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    let moved = titles(on_server.id);
    println!("moved the first to the end: {moved:?}");
    assert_eq!(
        moved,
        [before[1].clone(), before[2].clone(), before[0].clone()]
    );
    core.remove_from_playlist(on_server.id, vec![1]).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    let removed = titles(on_server.id);
    println!("removed the middle one: {removed:?}");
    assert_eq!(removed, [moved[0].clone(), moved[2].clone()]);
    core.add_to(
        DropTarget {
            kind: DropTargetKind::Playlist,
            id: Some(on_server.id),
        },
        vec![all[3].id],
        Some(1),
        false,
    )
    .unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    let added = titles(on_server.id);
    println!("added at position 1: {added:?}");
    assert_eq!(added.len(), 3);
    assert_eq!(added[1], all[3].title);
    core.delete_playlist(on_server.id).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(
        core.playlists(0, 50)
            .unwrap()
            .iter()
            .all(|p| p.name != "From CI")
    );

    // A like is five stars on the server, and comes back from it (light sync); an unlike
    // clears it.
    core.like(vec![all[0].id]).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(
        core.liked_songs(0, 10)
            .unwrap()
            .iter()
            .any(|s| s.id == all[0].id)
    );
    core.unlike(vec![all[0].id]).unwrap();
    block(core.send_edits()).unwrap();
    block(core.sync_library()).unwrap();
    assert!(core.liked_songs(0, 10).unwrap().is_empty());

    // Plays counted by another Plex app come in with the next full sync.
    let item = {
        let p = core.inner.profile().unwrap();
        lock(&p.store)
            .unwrap()
            .track(all[1].id)
            .unwrap()
            .source_item_id
    };
    if let crate::source_any::AnySource::Plex(s) = &*plex() {
        block(s.client.scrobble(&item)).unwrap();
        block(s.client.scrobble(&item)).unwrap();
    }
    {
        let p = core.inner.profile().unwrap();
        lock(&p.store)
            .unwrap()
            .set_sync_state(source_id, None, now_ms())
            .unwrap();
    }
    block(core.sync_library()).unwrap();
    let plays = {
        let p = core.inner.profile().unwrap();
        lock(&p.store)
            .unwrap()
            .recs_songs()
            .unwrap()
            .into_iter()
            .find(|r| r.track_id == all[1].id)
            .unwrap()
    };
    println!(
        "plays from the server: {} (last {:?})",
        plays.server_play_count, plays.server_last_played_utc_ms
    );
    assert_eq!(plays.server_play_count, 2);
    assert!(plays.server_last_played_utc_ms.is_some());

    // The saved sign-in opens the profile again on another start.
    let profile = signed.profile_id.clone();
    core.close_profile().unwrap();
    core.open_profile(
        profile,
        vec![SourceToken {
            source_id,
            token: signed.token.clone(),
        }],
    )
    .unwrap();
    let restored = block(core.sync_library()).unwrap();
    assert_eq!(restored.len(), 1, "{restored:?}");
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), songs);
}

/// GET a URL with the real transport, for the Plex flow's checks.
async fn get(url: &str, headers: &[(&str, &str)]) -> encore_sources::transport::Response {
    let t = encore_sources::http::HttpTransport::new(None, Duration::from_secs(20)).unwrap();
    t.send(encore_sources::transport::Request {
        method: encore_sources::transport::Method::Get,
        url: url::Url::parse(url).unwrap(),
        headers: headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        body: None,
        timeout: Duration::from_secs(20),
    })
    .await
    .unwrap()
}

/// ADR core-0023: the plex.tv link end to end on fixtures (no plex.tv, no server): the
/// address is recognised, the code is approved, the account's servers are listed, the
/// first address that answers is kept, and only that server's token is handed to the app.
#[test]
fn signing_in_with_plex_keeps_only_the_server_token() {
    let mut exchanges: Vec<encore_sources::FixtureExchange> = serde_json::from_str(include_str!(
        "../../../../../tools/fixtures/plex/plex-tv.json"
    ))
    .unwrap();
    // The https address on the local network doesn't answer; the plain http one does.
    exchanges.push(
        serde_json::from_value(serde_json::json!({
            "origin": "https://192-168-7-20.fixturehash.plex.direct:32400",
            "method": "GET", "path": "/identity", "error": "refused"
        }))
        .unwrap(),
    );
    exchanges.extend(
        serde_json::from_str::<Vec<encore_sources::FixtureExchange>>(include_str!(
            "../../../../../tools/fixtures/plex/server.json"
        ))
        .unwrap(),
    );
    let t = Arc::new(FixtureTransport::new(exchanges));
    let dir = temp_dir("plex-link");
    let core = EncoreCore::build(
        CoreConfig {
            data_dir: dir.0.to_string_lossy().into_owned(),
            device_id: "device-test".into(),
            device_name: "Sam's phone".into(),
            platform: "test".into(),
        },
        Duration::ZERO,
        50,
        Some(t.clone()),
    )
    .unwrap();

    let check = block(core.check_address("http://192.168.7.20:32400".into())).unwrap();
    assert_eq!(check.kind, ServerKind::Plex);
    assert_eq!(check.server_id, "fixture-machine-0001");

    let start = block(core.plex_link_start()).unwrap();
    assert_eq!(start.code, "K7QZ");
    assert_eq!(start.link_url, "https://plex.tv/link");
    assert!(start.auth_url.contains("code=K7QZ"));
    let servers = block(core.plex_link_wait()).unwrap();
    assert_eq!(servers.len(), 2);
    assert_eq!(
        servers[0].server_id, check.server_id,
        "the typed server can be picked"
    );
    assert!(servers[0].owned && !servers[1].owned);

    let signed = block(core.plex_sign_in(check.server_id.clone())).unwrap();
    assert!(signed.token.starts_with("plex1:t=fixture-server-token"));
    assert!(!signed.token.contains("fixture-account-token"));
    assert_eq!(signed.user_name, "tone_listener");
    let rows = core.servers().unwrap();
    assert_eq!(rows[0].kind, "plex");
    assert_eq!(rows[0].name, "Attic Server");
    assert_eq!(rows[0].base_url, "http://192.168.7.20:32400/");

    // The account token went with the sign-in.
    assert!(matches!(
        block(core.plex_sign_in(check.server_id.clone())),
        Err(EncoreError::QuickConnectExpired { .. })
    ));
    // Nothing on disk holds a token.
    let db = std::fs::read(
        dir.0
            .join("profiles")
            .join(&signed.profile_id)
            .join("library.sqlite"),
    )
    .unwrap();
    for secret in [&b"fixture-server-token"[..], b"fixture-account-token"] {
        assert!(!db.windows(secret.len()).any(|w| w == secret));
    }
    // Neither plex.tv nor the Plex server was told the device's name (the Jellyfin
    // probe before them names the device, as Jellyfin's Devices page shows it).
    let plex_requests: Vec<_> = t
        .sent()
        .into_iter()
        .filter(|r| r.headers.iter().any(|(k, _)| k == "X-Plex-Product"))
        .collect();
    assert!(plex_requests.len() > 5);
    for r in plex_requests {
        assert!(r.headers.iter().all(|(_, v)| !v.contains("Sam")), "{r:?}");
    }

    let synced = block(core.sync_library()).unwrap();
    assert_eq!(
        (synced[0].artists, synced[0].albums, synced[0].songs),
        (2, 2, 3)
    );

    // Cancelling a link forgets an approved account.
    block(core.plex_link_start()).unwrap();
    core.plex_link_cancel();
    assert!(block(core.plex_link_wait()).is_err());
}

/// Dart can release the last reference on another runtime's worker; dropping a
/// tokio runtime there panics unless it is shut down in the background.
#[test]
fn dropping_the_core_inside_another_runtime_does_not_panic() {
    let dir = temp_dir("drop");
    let (core, _) = core_on_recording(&dir);
    block(core.check_address("http://music.example.org".into())).unwrap_err();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap()
        .block_on(async move { drop(core) });
}

#[test]
fn quick_connect_creates_a_profile_and_hands_the_token_to_the_app() {
    let dir = temp_dir("signin");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    assert!(s.profile_id.starts_with("jf-"));
    assert_eq!(
        s.credential_key,
        format!("encore/{}/{}", s.profile_id, s.source_id)
    );
    assert_eq!(s.token, "anonymized-token");
    assert!(
        !format!("{s:?}").contains("anonymized-token"),
        "the token never reaches a log"
    );
    assert_eq!(core.current_profile(), Some(s.profile_id.clone()));
    // The token is not on disk: only the app's keychain holds it.
    let db = std::fs::read(
        dir.0
            .join("profiles")
            .join(&s.profile_id)
            .join("library.sqlite"),
    )
    .unwrap();
    assert!(
        !db.windows(b"anonymized-token".len())
            .any(|w| w == b"anonymized-token")
    );
}

#[test]
fn sync_then_browse_search_and_list_servers_offline() {
    let dir = temp_dir("browse");
    let (core, events) = core_on_recording(&dir);
    signed_in(&core);
    let results = block(core.sync_library()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        (
            results[0].artists,
            results[0].albums,
            results[0].songs,
            results[0].playlists
        ),
        (26, 38, 150, 1)
    );
    assert!(events.kinds().contains(&EventKind::SyncProgress));

    let servers = core.servers().unwrap();
    assert_eq!(servers[0].song_count, 150);
    assert!(servers[0].signed_in && servers[0].last_sync_utc_ms.is_some());

    let albums = core.albums(AlbumSort::Title, 0, 100).unwrap();
    assert_eq!(albums.len(), 38);
    let detail = core.album(albums[0].id).unwrap();
    assert_eq!(detail.songs.len() as u32, detail.album.song_count);
    assert!(!core.artists(0, 100).unwrap().is_empty());
    let playlist = &core.playlists(0, 10).unwrap()[0];
    assert!(!core.playlist(playlist.id).unwrap().entries.is_empty());
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);

    let title = detail.songs[0].title.clone();
    let found = core.search(title.clone()).unwrap();
    assert!(found.songs.iter().any(|s| s.title == title));
    core.add_recent_search(title.clone()).unwrap();
    assert_eq!(core.recent_searches().unwrap(), vec![title]);
    assert!(matches!(
        core.album(999_999),
        Err(EncoreError::NotFound { .. })
    ));
}

#[test]
fn play_an_album_report_it_and_preload_the_next_song() {
    let dir = temp_dir("play");
    let (core, events) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let album = core
        .albums(AlbumSort::Title, 0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.song_count >= 2)
        .unwrap();
    let songs = core.album(album.id).unwrap().songs;
    let ids: Vec<i64> = songs.iter().map(|s| s.id).collect();
    events.take();

    let state = core
        .play(ids.clone(), 0, StartedFrom::Album, Some("al".into()))
        .unwrap();
    let current = state.current.clone().unwrap();
    assert_eq!(current.song.id, ids[0]);
    assert_eq!(state.up_next.len(), ids.len() - 1);
    assert_eq!(
        events.kinds(),
        [EventKind::QueueChanged, EventKind::NowPlaying]
    );

    core.playback_started(current.uid, -300).unwrap();
    // Near the end the core asks the player to preload the next song, once.
    let near_end = current.song.duration_ms - 5_000;
    core.playback_progress(near_end, near_end).unwrap();
    core.playback_progress(near_end + 1_000, near_end + 1_000)
        .unwrap();
    let preloads: Vec<CoreEvent> = events
        .take()
        .into_iter()
        .filter(|e| e.kind == EventKind::Preload)
        .collect();
    assert_eq!(preloads.len(), 1);
    assert_eq!(preloads[0].preload.as_ref().unwrap().song.id, ids[1]);

    let next = core
        .playback_ended(EndReason::Finished, current.song.duration_ms)
        .unwrap()
        .unwrap();
    assert_eq!(next.song.id, ids[1]);
    // The finished play is in the log with its section 5 fields.
    let p = core.inner.profile().unwrap();
    let log = p.store.lock().unwrap().recent_play_events(5).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(
        (
            log[0].end_reason.as_str(),
            log[0].started_from.as_str(),
            log[0].chosen_by.as_str()
        ),
        ("finished", "album", "user")
    );
    assert_eq!(log[0].utc_offset_minutes, -300);
    assert_eq!(log[0].listened_ms as i64, current.song.duration_ms);
    assert_eq!(log[0].device_id, "device-test");
}

#[test]
fn private_listening_records_nothing() {
    let dir = temp_dir("private");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let id = core.songs(SongSort::Title, 0, 1).unwrap()[0].id;
    core.set_listening_mode(ListeningMode::Private).unwrap();
    let state = core.play(vec![id], 0, StartedFrom::Songs, None).unwrap();
    core.playback_started(state.current.unwrap().uid, 0)
        .unwrap();
    core.lyric_line_tapped(1, 1000).unwrap();
    core.playback_ended(EndReason::Finished, 10_000).unwrap();
    let p = core.inner.profile().unwrap();
    assert_eq!(
        p.store.lock().unwrap().listening_row_counts().unwrap(),
        (0, 0)
    );
}

#[test]
fn queue_controls_from_the_api() {
    let dir = temp_dir("queue");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 5)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    core.play(ids.clone(), 0, StartedFrom::Songs, None).unwrap();
    let s = core.next().unwrap();
    assert_eq!(s.current.as_ref().unwrap().song.id, ids[1]);
    assert_eq!(
        core.previous(10_000, false).unwrap(),
        PreviousResult::Restarted
    );
    assert_eq!(core.previous(10_000, true).unwrap(), PreviousResult::Moved);
    let s = core.set_repeat(RepeatMode::One).unwrap();
    assert_eq!(s.repeat, RepeatMode::One);
    let last = s.up_next.last().unwrap().uid;
    let s = core.remove_from_queue(last).unwrap();
    assert!(s.can_undo);
    let s = core.undo_queue().unwrap();
    assert_eq!(s.up_next.len(), 4);
    let s = core.clear_up_next().unwrap();
    assert!(s.up_next.is_empty());
}

#[test]
fn streams_use_the_quality_setting_and_never_log_the_token() {
    let dir = temp_dir("stream");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let id = core.songs(SongSort::Title, 0, 1).unwrap()[0].id;
    let wifi = core.stream_for(id, Network::Wifi).unwrap();
    assert!(
        wifi.url.contains("/stream?static=true"),
        "Wi-Fi defaults to the original file"
    );
    assert!(wifi.url_with_api_key.contains("ApiKey="));
    assert!(!format!("{wifi:?}").contains("ApiKey"));
    let cell = core.stream_for(id, Network::Cellular).unwrap();
    assert!(cell.url.contains("/universal") && cell.url.contains("maxStreamingBitrate=320000"));
    core.set_stream_quality(Network::Cellular, StreamQuality::DataSaver)
        .unwrap();
    assert!(
        core.stream_for(id, Network::Cellular)
            .unwrap()
            .url
            .contains("maxStreamingBitrate=96000")
    );
    let art = core.artwork(ArtworkOf::Song, id, 300).unwrap();
    assert!(art.url.contains("fillWidth=300"));
}

/// Board 19, item 5: one open file with the play log, likes, playlists and
/// settings; never a token; nothing in Guest mode.
#[test]
fn an_export_holds_the_history_likes_and_playlists_in_open_json() {
    let dir = temp_dir("export");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 3)
        .unwrap()
        .iter()
        .map(|x| x.id)
        .collect();
    let state = core.play(ids.clone(), 0, StartedFrom::Songs, None).unwrap();
    core.playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    core.playback_ended(EndReason::Finished, 200_000).unwrap();
    let likes_before = core.liked_songs(0, 1000).unwrap().len() as u64;
    core.like(vec![ids[2]]).unwrap();

    let file = dir.0.join("encore-export.json");
    let summary = core
        .export_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        !text.contains("anonymized-token"),
        "an export never holds the token"
    );
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["format"], "encore-export");
    assert_eq!(v["profile"]["id"], s.profile_id.as_str());
    assert_eq!(summary.bytes, text.len() as u64);
    assert_eq!(summary.plays, 1);
    assert_eq!(v["plays"][0]["end_reason"], "finished");
    assert_eq!(summary.likes, likes_before + 1);
    assert_eq!(
        summary.playlists,
        core.playlists(0, 1000).unwrap().len() as u64
    );
    assert_eq!(v["servers"][0]["kind"], "jellyfin");

    // Guest mode saves nothing, so there is nothing to export.
    core.set_listening_mode(ListeningMode::Guest).unwrap();
    assert!(matches!(
        core.export_profile(s.profile_id, file.to_string_lossy().into_owned()),
        Err(EncoreError::NotAllowed { .. })
    ));
}

#[test]
fn a_backup_taken_while_playing_restores_on_another_device() {
    let dir = temp_dir("backup");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 3)
        .unwrap()
        .iter()
        .map(|x| x.id)
        .collect();
    let state = core.play(ids, 0, StartedFrom::Songs, None).unwrap();
    core.playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    core.playback_ended(EndReason::Finished, 200_000).unwrap();
    // The second song is still playing while the backup is taken.
    core.playback_started(core.queue().unwrap().current.unwrap().uid, 60)
        .unwrap();
    let file = dir.0.join("encore-backup.sqlite");
    core.backup_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    let db = std::fs::read(&file).unwrap();
    assert!(
        !db.windows(b"anonymized-token".len())
            .any(|w| w == b"anonymized-token"),
        "a backup never holds the token"
    );

    // A new device: a fresh data folder, restored from the file, opened with the
    // token from the keychain.
    let other = temp_dir("restored");
    let (core2, _) = core_on_recording(&other);
    core2
        .restore_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    core2
        .open_profile(
            s.profile_id.clone(),
            vec![SourceToken {
                source_id: s.source_id,
                token: s.token.clone(),
            }],
        )
        .unwrap();
    assert_eq!(core2.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);
    let p = core2.inner.profile().unwrap();
    let log = p.store.lock().unwrap().recent_play_events(5).unwrap();
    // The finished play, and the one that was playing, closed as ended by the system.
    assert_eq!(log.len(), 2, "{log:?}");
    assert!(log.iter().any(|e| e.end_reason == "finished"));
    assert!(log.iter().any(|e| e.end_reason == "endedBySystem"));

    // A bad file changes nothing, and an ID can't point outside the profiles folder.
    let junk = dir.0.join("junk.sqlite");
    std::fs::write(&junk, b"junk").unwrap();
    assert!(matches!(
        core2.restore_profile(s.profile_id.clone(), junk.to_string_lossy().into_owned()),
        Err(EncoreError::Storage { .. })
    ));
    assert_eq!(core2.current_profile(), Some(s.profile_id.clone()));
    for id in ["../escape", "a/b", "", "..", "x\\y"] {
        assert!(
            core2
                .backup_profile(id.into(), file.to_string_lossy().into_owned())
                .is_err()
        );
        assert!(core2.open_profile(id.into(), vec![]).is_err());
    }
}

#[test]
fn lyrics_are_read_cached_and_shown_offline() {
    let dir = temp_dir("lyrics");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let songs = core.songs(SongSort::Title, 0, 1000).unwrap();
    let with: Vec<i64> = songs
        .iter()
        .filter(|x| x.has_lyrics)
        .map(|x| x.id)
        .collect();
    let without = songs.iter().find(|x| !x.has_lyrics).unwrap().id;
    assert!(!with.is_empty());

    // The recording holds lyrics for one song: line-synced, every line timed and
    // ended, in order. Songs whose lyrics weren't recorded fail to fetch.
    let mut found = None;
    for id in &with {
        if let Ok(Some(l)) = block(core.lyrics(*id)) {
            found = Some(l);
            break;
        }
    }
    let l = found.expect("the recorded song's lyrics");
    assert_eq!(l.sync_level, LyricsSync::Line);
    assert_eq!(l.format, LyricsFormat::Jellyfin);
    assert_eq!(l.origin, LyricsOrigin::MediaServer);
    assert!(l.lines.len() > 10);
    assert!(
        l.lines
            .iter()
            .all(|x| x.start_ms.is_some() && x.end_ms >= x.start_ms)
    );
    assert!(l.lines.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));

    // A song the server says has no lyrics: None, without asking.
    assert_eq!(block(core.lyrics(without)).unwrap(), None);

    // Offline (reopened without a token, so the server can't be asked): the kept
    // copy still shows.
    core.close_profile().unwrap();
    core.open_profile(s.profile_id, vec![]).unwrap();
    assert_eq!(block(core.lyrics(l.song_id)).unwrap(), Some(l.clone()));
    assert!(matches!(
        block(core.lyrics(999_999)),
        Err(EncoreError::NotFound { .. })
    ));
}

/// Board 07: "lyrics included when the server has them". Fetching ahead keeps
/// every recorded song's lyrics, then search finds a song by a line of them, with
/// the line and its time to open the lyrics view at.
#[test]
fn songs_are_found_by_their_lyrics_after_fetching_ahead() {
    let dir = temp_dir("lyrics-search");
    let (core, _) = core_on_recording(&dir);
    let _s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let with_lyrics = core
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .iter()
        .filter(|x| x.has_lyrics)
        .count() as u32;

    // Batches of 10 until nothing is left to try; a failed song isn't tried again.
    let mut tried = 0;
    loop {
        let n = block(core.fetch_lyrics_ahead(10)).unwrap();
        assert!(n <= 10);
        if n == 0 {
            break;
        }
        tried += n;
    }
    assert_eq!(tried, with_lyrics, "every song with lyrics tried once");

    // A word-start of a lyric line from a song whose title doesn't contain it.
    let mut probe = None;
    for song in core.songs(SongSort::Title, 0, 1000).unwrap() {
        let Ok(Some(l)) = block(core.lyrics(song.id)) else {
            continue;
        };
        let title = song.title.to_lowercase();
        for (i, line) in l.lines.iter().enumerate() {
            let words: Vec<&str> = line.text.split_whitespace().collect();
            if words.len() >= 3 && !title.contains(&words[1].to_lowercase()) {
                let q = format!("{} {}", words[0], words[1]);
                probe = Some((song.id, q, i as u32, line.start_ms));
                break;
            }
        }
        if probe.is_some() {
            break;
        }
    }
    let (song_id, q, line_index, start_ms) = probe.expect("a lyric line to search for");
    let r = core.search(q.clone()).unwrap();
    let hit = r
        .lyrics
        .iter()
        .find(|h| h.song.id == song_id)
        .unwrap_or_else(|| panic!("{q:?} finds its song by its lyrics"));
    assert!(hit.line.to_lowercase().contains(&q.to_lowercase()));
    assert!(hit.line_index <= line_index, "the first line that matches");
    if hit.line_index == line_index {
        assert_eq!(hit.start_ms, start_ms);
    }
    assert!(r.lyrics.len() as u32 <= tokens().search_lyrics);
}

/// The recording, plus every album cover answered with the board's Golden Hour
/// Static cover as PNG.
fn core_with_covers(dir: &TempDir) -> (Arc<EncoreCore>, Arc<FixtureTransport>) {
    use encore_sources::FixtureExchange;
    let mut exchanges: Vec<FixtureExchange> = serde_json::from_str(RECORDED).unwrap();
    let png = include_bytes!("../../../../../tools/fixtures/artwork/golden-hour-static.png");
    let hex: String = png.iter().map(|b| format!("{b:02x}")).collect();
    let recording: serde_json::Value = serde_json::from_str(RECORDED).unwrap();
    for x in recording.as_array().unwrap() {
        for item in x["body"]["Items"].as_array().into_iter().flatten() {
            if item["Type"] == "MusicAlbum" && item["ImageTags"]["Primary"].is_string() {
                exchanges.push(FixtureExchange {
                    origin: None,
                    method: "GET".into(),
                    path: format!("/Items/{}/Images/Primary", item["Id"].as_str().unwrap()),
                    query: [("format".to_owned(), "Png".to_owned())].into(),
                    status: 200,
                    body: None,
                    body_text: None,
                    body_hex: Some(hex.clone()),
                    error: None,
                    date: None,
                });
            }
        }
    }
    let t = Arc::new(FixtureTransport::new(exchanges));
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    };
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(t.clone())).unwrap();
    (core, t)
}

#[test]
fn album_palettes_are_made_once_and_shown_offline() {
    let dir = temp_dir("palette");
    let (core, transport) = core_with_covers(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let albums = core.albums(AlbumSort::Title, 0, 100).unwrap();
    let with_art = albums.iter().find(|a| a.has_artwork).unwrap().id;
    let image_requests = || {
        transport
            .sent()
            .iter()
            .filter(|r| r.url.path().contains("/Images/"))
            .count()
    };

    let p = block(core.album_palette(with_art)).unwrap().unwrap();
    // The same cover gives the same palette as the pure function (palette tests).
    assert_eq!(p.scheme, PaletteScheme::Dark);
    assert_eq!(p.base, "3D0000");
    assert_eq!(p.stops, ["CF4630", "1B1030", "C02D56", "986E00"]);
    assert_eq!(p.text, "FFFFFF");
    assert!(p.lowest_contrast >= 4.5);
    assert_eq!(image_requests(), 1);
    let sent = transport.sent();
    let request = sent
        .iter()
        .find(|r| r.url.path().contains("/Images/"))
        .unwrap();
    let query: String = request.url.query().unwrap_or_default().to_owned();
    assert!(
        query.contains("format=Png") && query.contains("maxWidth=64"),
        "{query}"
    );

    // Made once: the second call reads it back.
    assert_eq!(
        block(core.album_palette(with_art)).unwrap(),
        Some(p.clone())
    );
    assert_eq!(image_requests(), 1);

    // Offline: still there.
    core.close_profile().unwrap();
    core.open_profile(s.profile_id, vec![]).unwrap();
    assert_eq!(block(core.album_palette(with_art)).unwrap(), Some(p));
    assert!(matches!(
        block(core.album_palette(999_999)),
        Err(EncoreError::NotFound { .. })
    ));
}

/// Board 42 through the API, with the real clock (so the waits are real).
#[test]
fn swipes_headsets_and_interruptions_through_the_api() {
    let dir = temp_dir("signal");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 4)
        .unwrap()
        .iter()
        .map(|x| x.id)
        .collect();
    // Autoplay off: this test reaches the end of the queue.
    core.set_autoplay(false).unwrap();
    let state = core.play(ids.clone(), 0, StartedFrom::Songs, None).unwrap();
    let wait = || std::thread::sleep(Duration::from_millis(700));
    let uid = |c: &EncoreCore| c.queue().unwrap().current.unwrap().uid;

    // Song 1 plays 5 s (reported), then a swipe.
    core.playback_started(state.current.unwrap().uid, 0)
        .unwrap();
    wait();
    core.playback_seeked(1_000, 60_000).unwrap();
    core.playback_progress(5_000, 5_000).unwrap();
    assert_eq!(core.next_target().unwrap().song.unwrap().song.id, ids[1]);
    core.skip_next(SkipOrigin::Swipe).unwrap();
    // Song 2 is swiped past at once: it leaves no trace.
    core.playback_started(uid(&core), 0).unwrap();
    core.skip_next(SkipOrigin::Swipe).unwrap();
    // Song 3: the headphones come out, and the headset's Next 0 s later.
    core.playback_started(uid(&core), 0).unwrap();
    wait();
    core.playback_progress(9_000, 9_000).unwrap();
    core.playback_interrupted(InterruptionKind::HeadphonesUnplugged)
        .unwrap();
    core.skip_next(SkipOrigin::Headset).unwrap();
    // Song 4, the last: plays to the end. No autoplay: the end of the queue.
    core.playback_started(uid(&core), 0).unwrap();
    let last = core.next_target().unwrap();
    assert!(last.end_of_queue && last.song.is_none());
    core.playback_ended(EndReason::Finished, 200_000).unwrap();

    let p = core.inner.profile().unwrap();
    let mut log = p.store.lock().unwrap().recent_play_events(10).unwrap();
    log.reverse();
    let rows: Vec<(String, Option<String>, Option<String>, u32)> = log
        .iter()
        .map(|e| {
            (
                e.end_reason.clone(),
                e.signal.clone(),
                e.origin.clone(),
                e.seeks,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                "skipped".into(),
                Some("skippedUserPick".into()),
                Some("swipe".into()),
                1
            ),
            (
                "skipped".into(),
                Some("interruption".into()),
                Some("headset".into()),
                0
            ),
            (
                "finished".into(),
                Some("finished".into()),
                Some("auto".into()),
                0
            ),
        ],
        "song 2 was passed over and is not there"
    );

    // Back means back: the swipe goes to song 3, the button restarts after 3 s.
    assert_eq!(
        core.skip_previous(SkipOrigin::Button, 10_000).unwrap(),
        PreviousResult::Restarted
    );
    assert_eq!(
        core.previous_target().unwrap().song.unwrap().song.id,
        ids[2]
    );
    assert_eq!(
        core.skip_previous(SkipOrigin::Swipe, 10_000).unwrap(),
        PreviousResult::Moved
    );
}

#[test]
fn playlist_drops_likes_and_undo_through_the_api() {
    let dir = temp_dir("edits");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let playlist = core.playlists(0, 1).unwrap()[0].clone();
    let in_playlist: Vec<i64> = core
        .playlist(playlist.id)
        .unwrap()
        .entries
        .iter()
        .filter_map(|e| e.song.as_ref().map(|s| s.id))
        .collect();
    let songs: Vec<i64> = core
        .songs(SongSort::Title, 0, 200)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .filter(|id| !in_playlist.contains(id))
        // Not already liked: the owner's favourites arrive as likes.
        .filter(|id| !core.are_liked(vec![*id]).unwrap()[0])
        .take(6)
        .collect();
    let before = core.playlist(playlist.id).unwrap().entries.len();
    let to_playlist = DropTarget {
        kind: DropTargetKind::Playlist,
        id: Some(playlist.id),
    };

    // Albums refuse, with the reason to show.
    let album = core.albums(AlbumSort::Title, 0, 1).unwrap()[0].id;
    let p = core
        .drop_preview(
            DropTarget {
                kind: DropTargetKind::Album,
                id: Some(album),
            },
            songs.clone(),
        )
        .unwrap();
    assert_eq!(p.refusal, Some(DropRefusal::AlbumsTakeNoSongs));
    assert!(p.refusal_reason.unwrap().contains("playlist"));

    // Two songs in one call, then the same two: already there.
    let r = core
        .add_to(to_playlist.clone(), songs[..2].to_vec(), None, false)
        .unwrap();
    assert_eq!((r.added, r.already_there), (2, 0));
    assert!(r.edit_id.is_some() && r.undo_until_utc_ms.is_some());
    assert_eq!(
        core.playlist(playlist.id).unwrap().entries.len(),
        before + 2
    );
    let again = core
        .add_to(to_playlist.clone(), songs[..2].to_vec(), None, false)
        .unwrap();
    assert_eq!(
        (again.edit_id, again.added, again.already_there),
        (None, 0, 2)
    );
    core.undo(r.edit_id.clone().unwrap()).unwrap();
    assert_eq!(core.playlist(playlist.id).unwrap().entries.len(), before);
    assert!(matches!(
        core.undo(r.edit_id.unwrap()),
        Err(EncoreError::NotAllowed { .. })
    ));

    // Likes. Favourites set on the server arrived as likes with the sync.
    let base = core.liked_songs(0, 1000).unwrap().len();
    assert!(base > 0, "the recording's favourites become likes");
    let liked = core.like(songs[2..4].to_vec()).unwrap();
    assert_eq!(
        core.are_liked(songs[1..5].to_vec()).unwrap(),
        [false, true, true, false]
    );
    assert_eq!(core.liked_songs(0, 1000).unwrap().len(), base + 2);
    core.undo(liked.edit_id.unwrap()).unwrap();
    assert_eq!(core.liked_songs(0, 1000).unwrap().len(), base);

    // Up Next at the line, and its Undo.
    core.play(songs[..3].to_vec(), 0, StartedFrom::Songs, None)
        .unwrap();
    let q = core
        .add_to(
            DropTarget {
                kind: DropTargetKind::UpNext,
                id: None,
            },
            vec![songs[5]],
            Some(1),
            false,
        )
        .unwrap();
    let up_next: Vec<i64> = core
        .queue()
        .unwrap()
        .up_next
        .iter()
        .map(|e| e.song.id)
        .collect();
    assert_eq!(up_next, [songs[1], songs[5], songs[2]]);
    core.undo(q.edit_id.unwrap()).unwrap();
    let up_next: Vec<i64> = core
        .queue()
        .unwrap()
        .up_next
        .iter()
        .map(|e| e.song.id)
        .collect();
    assert_eq!(up_next, [songs[1], songs[2]]);

    // A guest can queue, but not save.
    core.set_listening_mode(ListeningMode::Guest).unwrap();
    assert!(matches!(
        core.add_to(to_playlist, vec![songs[0]], None, false),
        Err(EncoreError::NotAllowed { .. })
    ));
}

/// A background send still holding the profile while it is restored: restore
/// waits for it, so the database file is closed before it is replaced (Windows
/// refuses to replace an open file).
#[test]
fn restoring_waits_for_a_background_task_to_let_go() {
    let dir = temp_dir("restore-wait");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let file = dir.0.join("backup.sqlite");
    core.backup_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    let held = core.inner.profile().unwrap();
    let holder = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    core.restore_profile(s.profile_id.clone(), file.to_string_lossy().into_owned())
        .unwrap();
    holder.join().unwrap();
    core.open_profile(s.profile_id, vec![]).unwrap();
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);
}

#[test]
fn a_profile_reopens_with_the_token_from_the_keychain() {
    let dir = temp_dir("reopen");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    core.close_profile().unwrap();
    assert!(matches!(core.servers(), Err(EncoreError::NoProfile { .. })));

    let profiles = core.profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].credentials[0].key, s.credential_key);
    // Without the token: the library is browsable offline, the server says sign in again.
    core.open_profile(s.profile_id.clone(), vec![]).unwrap();
    assert!(!core.servers().unwrap()[0].signed_in);
    assert_eq!(core.songs(SongSort::Title, 0, 1000).unwrap().len(), 150);
    let id = core.songs(SongSort::Title, 0, 1).unwrap()[0].id;
    assert!(matches!(
        core.stream_for(id, Network::Wifi),
        Err(EncoreError::SessionExpired { .. })
    ));
    // With it: streaming works again.
    core.open_profile(
        s.profile_id,
        vec![SourceToken {
            source_id: s.source_id,
            token: s.token,
        }],
    )
    .unwrap();
    assert!(core.stream_for(id, Network::Wifi).is_ok());
}

/// Hands each request to whichever recording is in place, so one core can sign in
/// as two people.
#[derive(Debug)]
struct Swap(Mutex<Arc<dyn Transport>>);

impl Transport for Swap {
    fn send(
        &self,
        req: encore_sources::transport::Request,
    ) -> encore_sources::transport::BoxFuture<
        '_,
        encore_sources::Result<encore_sources::transport::Response>,
    > {
        let t = self.0.lock().unwrap().clone();
        Box::pin(async move { t.send(req).await })
    }
}

/// The recording as a second Jellyfin user (a stand-in: the owner's server was
/// recorded with one user). Same library, another user ID, so another profile.
fn second_user() -> Arc<dyn Transport> {
    let alex = RECORDED.replace(
        "d988144769a4706d8c624e4061739e78",
        "a1e70000000000000000000000000002",
    );
    Arc::new(FixtureTransport::from_json(&alex).unwrap())
}

fn tokens_of(profile_id: &str) -> Vec<SourceToken> {
    let _ = profile_id;
    vec![SourceToken {
        source_id: 1,
        token: "anonymized-token".into(),
    }]
}

/// Phase 4's "done when", core part: two people share an iPad with fully
/// separate history. One core, one data folder, two profiles.
#[test]
fn two_people_share_an_ipad_with_fully_separate_history() {
    let dir = temp_dir("ipad");
    let swap = Arc::new(Swap(Mutex::new(
        Arc::new(FixtureTransport::from_json(RECORDED).unwrap()) as Arc<dyn Transport>,
    )));
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "ipad".into(),
        device_name: "iPad".into(),
        platform: "test".into(),
    };
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(swap.clone())).unwrap();

    // Sam signs in, plays a song, likes one, leaves a queue.
    let sam = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let songs = core.songs(SongSort::Title, 0, 1000).unwrap();
    let not_liked: Vec<i64> = songs
        .iter()
        .map(|s| s.id)
        .filter(|id| !core.are_liked(vec![*id]).unwrap()[0])
        .take(2)
        .collect();
    let state = core
        .play(
            songs[..5].iter().map(|s| s.id).collect(),
            2,
            StartedFrom::Songs,
            None,
        )
        .unwrap();
    core.playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    core.playback_ended(EndReason::Finished, 200_000).unwrap();
    core.like(vec![not_liked[0]]).unwrap();
    let sam_current = core.queue().unwrap().current.unwrap().song.id;

    // Alex signs in on the same iPad: a new profile, opened, with nothing of Sam's.
    *swap.0.lock().unwrap() = second_user();
    let alex = signed_in(&core).profile_id;
    assert_ne!(alex, sam);
    assert_eq!(core.current_profile().as_deref(), Some(alex.as_str()));
    block(core.sync_library()).unwrap();
    assert!(
        core.queue().unwrap().current.is_none(),
        "Alex's queue starts empty"
    );
    assert!(
        !core.are_liked(vec![not_liked[0]]).unwrap()[0],
        "Sam's like isn't Alex's"
    );
    let export = |name: &str, id: &str| -> serde_json::Value {
        let f = dir.0.join(name);
        core.export_profile(id.into(), f.to_string_lossy().into_owned())
            .unwrap();
        serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()
    };
    assert_eq!(
        export("alex.json", &alex)["plays"],
        serde_json::json!([]),
        "no history of Sam's"
    );

    // The switcher: active first, names, initials, no locks yet.
    core.rename_profile(alex.clone(), Some("Alex Kim".into()))
        .unwrap();
    let list = core.profiles().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        (list[0].profile_id.as_str(), list[0].active),
        (alex.as_str(), true)
    );
    assert_eq!(
        (list[0].display_name.as_str(), list[0].initials.as_str()),
        ("Alex Kim", "AK")
    );
    assert!(!list[1].active && !list[1].locked && !list[1].needs_sign_in);
    assert!(
        list.iter().all(|p| p.avatar_url.is_none()),
        "no /Users/Me in the recording yet"
    );

    // Back to Sam: Sam's queue, paused, and Sam's history and like.
    *swap.0.lock().unwrap() = Arc::new(FixtureTransport::from_json(RECORDED).unwrap());
    let q = block(core.switch_profile(sam.clone(), tokens_of(&sam), false)).unwrap();
    assert_eq!(
        q.current.map(|c| c.song.id),
        Some(sam_current),
        "the last queue loads"
    );
    assert!(core.are_liked(vec![not_liked[0]]).unwrap()[0]);
    assert_eq!(
        export("sam.json", &sam)["plays"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        core.last_active_profile().unwrap().as_deref(),
        Some(sam.as_str())
    );

    // "Keep playing": Sam's queue moves over to Alex.
    *swap.0.lock().unwrap() = second_user();
    let q = block(core.switch_profile(alex.clone(), tokens_of(&alex), true)).unwrap();
    assert_eq!(q.current.map(|c| c.song.id), Some(sam_current));
    assert!(
        export("alex2.json", &alex)["plays"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // Removing Alex from this device keeps Sam.
    *swap.0.lock().unwrap() = Arc::new(FixtureTransport::from_json(RECORDED).unwrap());
    block(core.switch_profile(sam.clone(), tokens_of(&sam), false)).unwrap();
    core.remove_profile_from_device(alex.clone()).unwrap();
    let list = core.profiles().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].profile_id, sam);
}

/// Board 14: a PIN (or biometrics) before a locked profile opens; five wrong tries
/// lock it; biometrics need the PIN once after they change.
#[test]
fn a_locked_profile_opens_only_after_its_pin() {
    let dir = temp_dir("pin");
    let (core, _) = core_on_recording(&dir);
    let sam = signed_in(&core).profile_id;
    assert!(matches!(
        core.set_profile_pin(sam.clone(), "12a4".into()),
        Err(EncoreError::NotAllowed { .. })
    ));
    core.set_profile_pin(sam.clone(), "4821".into()).unwrap();
    assert!(core.profiles().unwrap()[0].locked);
    core.close_profile().unwrap();
    let locked = core.open_profile(sam.clone(), tokens_of(&sam));
    assert!(
        matches!(locked, Err(EncoreError::NotAllowed { .. })),
        "locked"
    );

    // Four wrong tries say "Wrong PIN"; the fifth locks it for 30 s.
    for _ in 0..4 {
        let e = core
            .unlock_profile(sam.clone(), Some("0000".into()))
            .unwrap_err();
        assert_eq!(e.to_string(), "Wrong PIN");
    }
    let e = core
        .unlock_profile(sam.clone(), Some("0000".into()))
        .unwrap_err();
    assert!(e.to_string().contains("try again in 30 s"), "{e}");
    let e = core
        .unlock_profile(sam.clone(), Some("4821".into()))
        .unwrap_err();
    assert!(
        e.to_string().starts_with("Too many wrong tries"),
        "even the right PIN waits: {e}"
    );
    // (The 30 s pass.)
    lock(&core.inner.registry)
        .unwrap()
        .set_pin_state(&sam, 0, 0, 1, false)
        .unwrap();
    core.unlock_profile(sam.clone(), Some("4821".into()))
        .unwrap();
    core.open_profile(sam.clone(), tokens_of(&sam)).unwrap();
    // The unlock was used: closed again, it is locked again.
    core.close_profile().unwrap();
    assert!(core.open_profile(sam.clone(), tokens_of(&sam)).is_err());

    // Biometrics passed (the app checked): fine, until the device's biometrics change.
    core.unlock_profile(sam.clone(), None).unwrap();
    core.open_profile(sam.clone(), tokens_of(&sam)).unwrap();
    core.close_profile().unwrap();
    core.biometrics_changed().unwrap();
    assert!(core.unlock_profile(sam.clone(), None).is_err());
    core.unlock_profile(sam.clone(), Some("4821".into()))
        .unwrap();
    core.open_profile(sam.clone(), tokens_of(&sam)).unwrap();
    core.remove_profile_pin(sam.clone()).unwrap();
    core.close_profile().unwrap();
    core.open_profile(sam, tokens_of("")).unwrap();
}

/// Board 14, item 5: Guest records nothing and saves nothing, keeps the profile's
/// queue aside, and ends on its own after the idle time.
#[test]
fn guest_saves_nothing_and_gives_the_queue_back() {
    let dir = temp_dir("guest");
    let (core, _) = core_on_recording(&dir);
    let sam = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let songs = core.songs(SongSort::Title, 0, 10).unwrap();
    core.play(vec![songs[0].id, songs[1].id], 0, StartedFrom::Songs, None)
        .unwrap();

    let q = core.start_guest().unwrap();
    assert!(q.current.is_none(), "Guest starts with an empty queue");
    assert!(core.guest_session().unwrap().is_some());
    let s = core
        .play(vec![songs[5].id], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(s.current.unwrap().uid, 0).unwrap();
    core.playback_ended(EndReason::Finished, 200_000).unwrap();
    assert!(matches!(
        core.like(vec![songs[5].id]),
        Err(EncoreError::NotAllowed { .. })
    ));
    let q = core.end_guest().unwrap();
    assert_eq!(
        q.current.map(|c| c.song.id),
        Some(songs[0].id),
        "Sam's queue is back"
    );
    assert!(core.guest_session().unwrap().is_none());
    let f = dir.0.join("sam.json");
    core.export_profile(sam, f.to_string_lossy().into_owned())
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
    assert_eq!(
        v["plays"],
        serde_json::json!([]),
        "Guest's play was never recorded"
    );

    // Idle past the limit: it ends by itself, and the queue comes back.
    core.start_guest().unwrap();
    lock(&core.inner.registry)
        .unwrap()
        .set_state("guest.activity", Some("0"))
        .unwrap();
    assert!(
        core.guest_session().unwrap().is_none(),
        "ended for being idle"
    );
    assert_eq!(
        core.queue().unwrap().current.map(|c| c.song.id),
        Some(songs[0].id)
    );
}

/// Owner, 26 November 2026: the app came back on an old song after being swiped away.
/// The queue is kept after every change, so a core that is never closed (the process
/// ended) still opens on the song that was playing.
#[test]
fn the_queue_is_kept_without_a_close() {
    let dir = temp_dir("kept-queue");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let songs = core.songs(SongSort::Title, 0, 10).unwrap();
    core.play(
        vec![songs[0].id, songs[1].id, songs[2].id],
        0,
        StartedFrom::Songs,
        None,
    )
    .unwrap();
    let q = core.skip_next(SkipOrigin::Button).unwrap();
    assert_eq!(q.current.map(|c| c.song.id), Some(songs[1].id));
    // The process ends: no close_profile.
    drop(core);

    let (again, _) = core_on_recording(&dir);
    again
        .open_profile(
            s.profile_id,
            vec![SourceToken {
                source_id: s.source_id,
                token: s.token,
            }],
        )
        .unwrap();
    let q = again.queue().unwrap();
    assert_eq!(
        q.current.map(|c| c.song.id),
        Some(songs[1].id),
        "opens on the song that was playing"
    );
    assert_eq!(q.up_next.first().map(|e| e.song.id), Some(songs[2].id));
}

/// Play `n` songs to the end (ids round robin), as the player would report them.
fn listen(core: &EncoreCore, ids: &[i64], n: usize) {
    for k in 0..n {
        let start = k % ids.len();
        let state = core
            .play(vec![ids[start]], 0, StartedFrom::Songs, None)
            .unwrap();
        core.playback_started(state.current.unwrap().uid, 60)
            .unwrap();
        core.playback_ended(EndReason::Finished, 200_000).unwrap();
    }
}

/// Phase 5 through the API on the recording: autoplay, radio, Smart shuffle, Why
/// this song, Not interested, feedback.
#[test]
fn smart_picks_fill_autoplay_and_explain_themselves() {
    let dir = temp_dir("recs");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let songs = core.songs(SongSort::Title, 0, 1000).unwrap();
    let ids: Vec<i64> = songs.iter().map(|s| s.id).collect();
    listen(&core, &ids[..20], 20);

    // Autoplay: 10 app-chosen songs after the current one, none of them queued.
    let state = core
        .play(vec![ids[0]], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    let q = core.refresh_autoplay().unwrap();
    assert_eq!(q.autoplay.len(), 10, "{:?}", q.autoplay.len());
    assert!(q.autoplay.iter().all(|e| e.chosen_by == ChosenBy::App));
    assert!(q.autoplay.iter().all(|e| e.song.id != ids[0]));
    let why = core.why_this_song(q.autoplay[0].song.id).unwrap();
    assert!(why.reasons.len() <= 2 && why.sounds_like.len() <= 2);

    // Not interested in an artist: none of their songs are suggested.
    let artist = songs
        .iter()
        .find(|s| s.id == q.autoplay[0].song.id)
        .unwrap()
        .artist
        .clone();
    let item = core
        .not_interested_artist(artist.clone(), NotInterestedReason::NotMyTaste)
        .unwrap();
    let q = core.refresh_autoplay().unwrap();
    assert!(
        q.autoplay.iter().all(|e| e.song.artist != artist),
        "{artist} still suggested"
    );
    assert_eq!(core.not_interested_list().unwrap().len(), 1);
    core.remove_not_interested(item).unwrap();
    assert!(core.not_interested_list().unwrap().is_empty(), "undone");

    // Radio holds its seed; Smart shuffle adds one suggestion per 4 songs.
    let q = core.start_radio(RadioSeed::Song, ids[5]).unwrap();
    assert_eq!(q.current.as_ref().map(|c| c.song.id), Some(ids[5]));
    assert_eq!(q.autoplay.len(), 10);
    let q = core
        .smart_shuffle(ids[30..42].to_vec(), StartedFrom::Songs, None)
        .unwrap();
    let all: Vec<&QueueEntry> = q.current.iter().chain(q.up_next.iter()).collect();
    assert_eq!(
        all.iter().filter(|e| e.chosen_by == ChosenBy::App).count(),
        3
    );
    assert_eq!(
        all.iter().filter(|e| e.chosen_by == ChosenBy::User).count(),
        12
    );

    // Feedback is recorded and synced as events; Guest can't give any.
    core.more_like_this(ids[1]).unwrap();
    core.fewer_like_this(ids[2]).unwrap();
    core.set_adventurousness(Adventurousness::Adventurous)
        .unwrap();
    assert_eq!(
        core.adventurousness().unwrap(),
        Adventurousness::Adventurous
    );
    core.set_listening_mode(ListeningMode::Guest).unwrap();
    assert!(matches!(
        core.more_like_this(ids[1]),
        Err(EncoreError::NotAllowed { .. })
    ));
}

/// The cold-start table through Home: under 50 plays "still learning" with
/// starter or family mixes; from 50, the full shelf; Learning off falls back.
#[test]
fn home_follows_the_cold_start_table_and_the_mix_actions_work() {
    let dir = temp_dir("home");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();

    let home = block(core.home()).unwrap();
    assert_eq!(home.made_for_you.state, MixState::NotEnoughPlays);
    assert_eq!(home.made_for_you.plays_so_far, 0);
    assert_eq!(home.made_for_you.plays_needed, 50);
    assert!(home.recently_played.is_empty());
    assert_eq!(home.recently_added.len(), 12);

    listen(&core, &ids[..60], 60);
    let home = block(core.regenerate_mixes()).unwrap();
    assert_eq!(
        home.made_for_you.state,
        MixState::Fresh,
        "{:?}",
        home.made_for_you
    );
    assert!(!home.made_for_you.mixes.is_empty());
    assert!(!home.recently_played.is_empty());
    let first = home.made_for_you.mixes[0].clone();
    assert!(first.cover_song_ids.len() <= 4 && !first.cover_song_ids.is_empty());
    let page = core.mix(first.mix_id.clone()).unwrap();
    assert_eq!(page.songs.len() as u32, first.song_count);

    // Pin keeps it; Not in this mix removes a song; Hide drops it from today's set.
    core.pin_mix(first.mix_id.clone(), true).unwrap();
    let gone = page.songs[0].song.id;
    core.not_in_this_mix(first.mix_id.clone(), gone).unwrap();
    assert!(
        core.mix(first.mix_id.clone())
            .unwrap()
            .songs
            .iter()
            .all(|s| s.song.id != gone)
    );
    let home = block(core.home()).unwrap();
    assert!(home.made_for_you.mixes[0].pinned);
    if let Some(other) = home.made_for_you.mixes.iter().find(|m| !m.pinned) {
        core.hide_mix(other.mix_id.clone()).unwrap();
        let home = block(core.home()).unwrap();
        assert!(
            home.made_for_you
                .mixes
                .iter()
                .all(|m| m.mix_id != other.mix_id)
        );
    }
    let saved = core.save_mix_as_playlist(first.mix_id.clone()).unwrap();
    assert!(saved.playlist_id.is_some());

    // Stats count where the app chose; Learning off falls back to plain shelves.
    let stats = core.suggestion_stats().unwrap();
    assert!(stats.shelves.iter().all(|s| s.plays > 0));
    core.set_listening_mode(ListeningMode::LearningOff).unwrap();
    let home = block(core.home()).unwrap();
    assert_eq!(home.made_for_you.state, MixState::LearningOff);
    assert!(
        home.made_for_you.mixes.iter().all(|m| m.pinned),
        "only pins, no new set"
    );
    assert!(!home.most_played.is_empty());
}

/// Reset forgets what was learned and keeps the history; Clear deletes the
/// history. Different buttons, different results (section 24, Smart picks).
#[test]
fn reset_and_clear_behave_differently() {
    let dir = temp_dir("reset");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    listen(&core, &ids[..55], 55);
    let plays = |core: &EncoreCore| -> usize {
        let f = dir.0.join("x.json");
        core.export_profile(s.profile_id.clone(), f.to_string_lossy().into_owned())
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        v["plays"].as_array().unwrap().len()
    };
    assert_eq!(block(core.home()).unwrap().made_for_you.plays_so_far, 50);
    core.reset_suggestions().unwrap();
    assert_eq!(plays(&core), 55, "Reset keeps the history");
    let home = block(core.home()).unwrap();
    assert_eq!(
        home.made_for_you.plays_so_far, 0,
        "but learns from none of it"
    );
    assert_eq!(home.made_for_you.state, MixState::NotEnoughPlays);
    assert_eq!(core.clear_listening_history().unwrap(), 55);
    assert_eq!(plays(&core), 0, "Clear deletes it");
    assert!(block(core.home()).unwrap().recently_played.is_empty());
}

/// Phase 10 (docs/adr/core-0020): the learned ranker trains only when due, keeps a note of it,
/// and Reset suggestions and Clear listening history start it over; Guest trains nothing.
#[test]
fn the_learned_ranker_trains_when_due_and_resets_with_the_history() {
    let dir = temp_dir("ranker");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 1000)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    // Two thirds of the library, so autoplay has songs not heard today (the 24-hour rule).
    let heard = &ids[..ids.len() * 2 / 3];
    listen(&core, heard, 120);
    assert!(!block(core.train_ranker_if_due()).unwrap(), "too few plays");
    listen(&core, heard, 200);
    assert!(
        block(core.train_ranker_if_due()).unwrap(),
        "320 plays: trained"
    );
    assert!(!block(core.train_ranker_if_due()).unwrap(), "not due again");
    // Picks still work with whatever it learned (or the hand-tuned score if it didn't pass).
    let state = core
        .play(vec![ids[0]], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(state.current.unwrap().uid, 60)
        .unwrap();
    assert_eq!(core.refresh_autoplay().unwrap().autoplay.len(), 10);

    core.reset_suggestions().unwrap();
    assert!(
        !block(core.train_ranker_if_due()).unwrap(),
        "a new epoch with no plays"
    );
    core.set_listening_mode(ListeningMode::Guest).unwrap();
    assert!(
        !block(core.train_ranker_if_due()).unwrap(),
        "Guest trains nothing"
    );
}

/// Phase 10, Browse by mood (board 07): nothing until enough songs are analysed; then the moods
/// the library has, each opening as a mix that can be pinned and is found again.
#[test]
fn browse_by_mood_follows_the_analysed_songs() {
    let dir = temp_dir("moods");
    let (core, _) = core_on_recording(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    assert!(core.moods().unwrap().is_empty(), "nothing analysed yet");

    // Analyse every song by hand: energy, brightness and dynamics spread over the library.
    {
        let p = core.inner.profile().unwrap();
        let store = p.store.lock().unwrap();
        let keys: Vec<String> = store
            .pending_sounds(
                u32::from(encore_analysis::VERSION),
                encore_analysis::MODEL_NAME,
                1000,
            )
            .unwrap()
            .into_iter()
            .map(|s| s.song_key)
            .collect();
        for (n, key) in keys.iter().enumerate() {
            let spread = |k: usize| ((n * k) % 101) as f32 / 100.0;
            let profile = encore_analysis::SoundProfile {
                bpm: 60.0 + (n % 13) as f32 * 10.0,
                beat_strength: 0.5,
                key: None,
                lufs: -12.0,
                energy: spread(37),
                brightness: spread(53),
                dynamics: spread(71),
                texture: [0; encore_analysis::TEXTURE_LEN],
            };
            store
                .put_sound(&encore_store::SoundRow {
                    song_key: key.clone(),
                    version: u32::from(encore_analysis::VERSION),
                    state: "ok".into(),
                    reason: None,
                    profile: Some(profile.encode()),
                    model: None,
                    embedding: None,
                    analysed_utc_ms: 1,
                })
                .unwrap();
        }
    }
    *core.inner.profile().unwrap().recs.lock().unwrap() = None;
    let moods = core.moods().unwrap();
    assert!(!moods.is_empty());
    assert!(
        moods
            .iter()
            .all(|m| m.song_count >= 25 && m.mix_id.starts_with("mood-"))
    );
    let page = core.mix(moods[0].mix_id.clone()).unwrap();
    assert_eq!(page.summary.kind, MixKind::Mood);
    assert_eq!(page.summary.title, moods[0].title);
    assert!(page.songs.len() >= 10, "{}", page.songs.len());
    core.pin_mix(moods[0].mix_id.clone(), true).unwrap();
    assert!(core.mix(moods[0].mix_id.clone()).unwrap().summary.pinned);
}

/// Board 38: Private listening ends after 2 hours of silence.
#[test]
fn private_listening_ends_after_two_hours_of_silence() {
    let dir = temp_dir("private");
    let (core, _) = core_on_recording(&dir);
    let s = signed_in(&core);
    block(core.sync_library()).unwrap();
    let ids: Vec<i64> = core
        .songs(SongSort::Title, 0, 10)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    core.set_listening_mode(ListeningMode::Private).unwrap();
    listen(&core, &ids[..2], 2);
    // (Two hours and a minute of silence.)
    lock(&core.inner.profile().unwrap().playback)
        .unwrap()
        .private_last_activity_utc_ms = Some(now_ms() - tokens().private_ends_after_ms - 60_000);
    listen(&core, &ids[2..3], 1);
    let f = dir.0.join("p.json");
    core.export_profile(s.profile_id, f.to_string_lossy().into_owned())
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
    assert_eq!(
        v["plays"].as_array().unwrap().len(),
        1,
        "only the play after Private ended"
    );
}
