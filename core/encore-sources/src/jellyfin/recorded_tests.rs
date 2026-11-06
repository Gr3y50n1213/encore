//! Adapter tests on a recording from a real Jellyfin 12.0.0 server
//! (tools/fixtures/jellyfin/recorded.json, made by tools/fixture-recorder; how it
//! was made is in RECORDING.md beside it). No server is needed to run them.
//!
//! The recording is anonymized, so the tests check structure and behaviour, not
//! titles. What a recording cannot safely contain (a wrong password, an unreachable
//! address, an untrusted certificate) stays in fixture_tests.rs as stand-ins.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::transport::{FixtureTransport, Transport};

const RECORDED: &str = include_str!("../../../../tools/fixtures/jellyfin/recorded.json");

fn transport() -> Arc<FixtureTransport> {
    Arc::new(FixtureTransport::from_json(RECORDED).expect("the recording parses"))
}

fn device() -> DeviceInfo {
    DeviceInfo {
        client: "Encore".into(),
        device_name: "Replay".into(),
        device_id: "replay".into(),
        version: "0.1.0".into(),
    }
}

/// Probe, then Quick Connect, exactly as the recording did. Returns a signed-in
/// client on the same transport.
async fn signed_in(t: &Arc<FixtureTransport>) -> JellyfinClient {
    let plan = plan_address("http://jellyfin.local:8096", &[443, 8920], 8096).unwrap();
    let info = probe(
        t.clone() as Arc<dyn Transport>,
        &plan,
        &device(),
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    let c = JellyfinClient::new(t.clone(), info.base_url, device(), Duration::from_secs(5));
    assert!(c.quick_connect_enabled().await.unwrap());
    let code = c.quick_connect_start().await.unwrap();
    assert_eq!(code.code.chars().count(), 6);
    let mut polls = 0;
    while !c.quick_connect_poll(&code.secret).await.unwrap() {
        polls += 1;
        assert!(polls < 200, "the recording never approved the code");
    }
    let s = c.quick_connect_finish(&code.secret).await.unwrap();
    assert_eq!(s.token, "anonymized-token");
    c.signed_in(s.token, s.user_id)
}

fn all_ids(d: &crate::model::LibraryDelta) -> HashSet<String> {
    d.artists
        .iter()
        .map(|x| x.source_item_id.clone())
        .chain(d.albums.iter().map(|x| x.source_item_id.clone()))
        .chain(d.tracks.iter().map(|x| x.source_item_id.clone()))
        .chain(d.playlists.iter().map(|x| x.source_item_id.clone()))
        .collect()
}

#[tokio::test]
async fn the_probe_finds_jellyfin_12_on_the_local_network() {
    let t = transport();
    let plan = plan_address("http://jellyfin.local:8096", &[443, 8920], 8096).unwrap();
    let info = probe(
        t as Arc<dyn Transport>,
        &plan,
        &device(),
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(info.version, "12.0.0");
    assert_eq!(info.warning, Some(AddressWarning::PlainHttpOnLocalNetwork));
    assert!(!info.server_id.is_empty());
}

#[tokio::test]
async fn quick_connect_signs_in_after_the_code_is_approved() {
    let t = transport();
    signed_in(&t).await;
    // Every request after sign-in carries the token in the MediaBrowser header,
    // never in a legacy header or parameter (Jellyfin 12 refuses those).
    for r in t.sent() {
        assert!(r.headers.iter().all(|(k, _)| !k.starts_with("X-Emby")));
        assert!(r.url.query_pairs().all(|(k, _)| k != "api_key"));
    }
}

#[tokio::test]
async fn a_first_sync_maps_the_whole_recorded_library() {
    let t = transport();
    let c = signed_in(&t).await;
    let seen = Mutex::new(Vec::new());
    let progress = |p: SyncProgress| seen.lock().unwrap().push(p);
    let d = sync_library(&c, None, &HashSet::new(), 50, &progress)
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
        (26, 38, 150, 1)
    );
    let last = *seen.lock().unwrap().last().unwrap();
    assert_eq!(
        last,
        SyncProgress {
            done: 215,
            total: 215
        }
    );

    let albums: HashSet<&str> = d.albums.iter().map(|a| a.source_item_id.as_str()).collect();
    for track in &d.tracks {
        assert!(!track.title.trim().is_empty());
        assert!(track.duration_ms > 0, "every recorded song has a length");
        let album = track
            .album_item_id
            .as_deref()
            .expect("every recorded song is on an album");
        assert!(albums.contains(album), "a song's album is in the same sync");
        assert!(!track.song_key().is_empty());
    }
    // Real tags came through: some songs carry a MusicBrainz recording ID, and
    // those get an `mb:` song key.
    assert!(d.tracks.iter().any(|t| t.mbid_recording.is_some()));
    assert!(
        d.tracks
            .iter()
            .filter(|t| t.mbid_recording.is_some())
            .all(|t| t.song_key().starts_with("mb:"))
    );
    assert!(d.tracks.iter().any(|t| t.has_lyrics));
    assert!(!d.playlists[0].entries.is_empty());
    // The cursor is the server's clock at the start, a minute early.
    assert!(d.cursor.as_ref().is_some_and(|c| c.0.ends_with('Z')));
}

#[tokio::test]
async fn a_second_sync_asks_only_for_changes_and_finds_no_removals() {
    let t = transport();
    let c = signed_in(&t).await;
    let first = sync_library(&c, None, &HashSet::new(), 50, &|_| {})
        .await
        .unwrap();
    let cursor = first.cursor.clone().unwrap();
    let second = sync_library(&c, Some(&cursor), &all_ids(&first), 50, &|_| {})
        .await
        .unwrap();
    assert!(!second.full);
    assert!(second.tracks.is_empty() && second.albums.is_empty() && second.artists.is_empty());
    assert!(second.removed_item_ids.is_empty());
    let asked = t
        .sent()
        .iter()
        .filter(|r| {
            r.url
                .query_pairs()
                .any(|(k, v)| k == "minDateLastSaved" && v == cursor.0)
        })
        .count();
    assert_eq!(asked, 3, "artists, albums and songs since the cursor");
}

#[tokio::test]
async fn lyrics_for_a_song_with_them_and_none_for_one_without() {
    let t = transport();
    let c = signed_in(&t).await;
    let d = sync_library(&c, None, &HashSet::new(), 50, &|_| {})
        .await
        .unwrap();
    let with = d.tracks.iter().find(|t| t.has_lyrics).unwrap();
    let without = d.tracks.iter().find(|t| !t.has_lyrics).unwrap();
    let lyrics = c.lyrics(&with.source_item_id).await.unwrap().unwrap();
    assert_eq!(lyrics.format, "jellyfin-json");
    let parsed: serde_json::Value = serde_json::from_str(&lyrics.body).unwrap();
    assert!(parsed["Lyrics"].as_array().is_some_and(|l| !l.is_empty()));
    assert_eq!(c.lyrics(&without.source_item_id).await.unwrap(), None);
}

#[tokio::test]
async fn sign_out_is_accepted() {
    let t = transport();
    let c = signed_in(&t).await;
    c.sign_out().await.unwrap();
}

/// The recording is published with the code, so it must stay anonymous.
#[test]
fn the_recording_carries_no_paths_previews_tokens_or_real_addresses() {
    let v: serde_json::Value = serde_json::from_str(RECORDED).unwrap();
    let text = RECORDED;
    for forbidden in [
        "\"Path\"",
        "ImageBlurHashes",
        "\"LocalAddress\"",
        "X-Emby",
        "api_key",
        "Token=",
        "localhost",
    ] {
        assert!(
            !text.contains(forbidden),
            "the recording contains {forbidden}"
        );
    }
    for exchange in v.as_array().unwrap() {
        let origin = exchange["origin"].as_str().unwrap();
        assert!(
            origin.contains("://jellyfin.local") || origin.contains("://jellyfin.example"),
            "a real address is in the recording"
        );
        if let Some(token) = exchange["body"].get("AccessToken") {
            assert_eq!(token, "anonymized-token");
        }
    }
}
