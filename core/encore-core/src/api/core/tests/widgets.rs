//! Widget snapshots and `play_mix` (Phase 9, WP2) through the API, on the
//! Jellyfin recording, with a stand-in that serves covers (the recording holds
//! none) and, once told to, refuses every other request.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use encore_sources::transport::{BoxFuture, Request, Response};
use encore_sources::{SourceError, Unreachable};
use serde_json::Value;

use super::*;

/// What every cover answers with: a JPEG's first bytes and a marker.
const COVER: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, b'c', b'o', b'v', b'e', b'r'];

fn is_cover(req: &Request) -> bool {
    let path = req.url.path();
    path.starts_with("/Items/")
        && path.ends_with("/Images/Primary")
        && req.url.query().is_some_and(|q| q.contains("fillWidth"))
}

/// What every palette request answers with: the board's Golden Hour Static cover as
/// PNG, so every album's palette is the one `album_palettes_are_made_once_and_shown_offline`
/// pins: base 3D0000, stops CF4630 1B1030 C02D56 986E00, text FFFFFF.
const PALETTE_PNG: &[u8] =
    include_bytes!("../../../../../../tools/fixtures/artwork/golden-hour-static.png");

/// The small PNG the palette is sampled from (`cover_png`).
fn is_palette(req: &Request) -> bool {
    let path = req.url.path();
    path.starts_with("/Items/")
        && path.ends_with("/Images/Primary")
        && req.url.query().is_some_and(|q| q.contains("format=Png"))
}

/// The recording (swappable, for a second user) until `strict`; from then on
/// covers and palette images only, and anything else is refused and noted.
#[derive(Debug)]
struct Guard {
    inner: Mutex<Arc<dyn Transport>>,
    strict: AtomicBool,
    refused: Mutex<Vec<String>>,
    covers: AtomicU32,
    palettes: AtomicU32,
}

impl Transport for Guard {
    fn send(&self, req: Request) -> BoxFuture<'_, encore_sources::Result<Response>> {
        if is_cover(&req) {
            self.covers.fetch_add(1, Ordering::SeqCst);
            return Box::pin(async {
                Ok(Response {
                    status: 200,
                    body: COVER.to_vec(),
                    server_date_utc_ms: None,
                    content_range_total: None,
                })
            });
        }
        if is_palette(&req) {
            self.palettes.fetch_add(1, Ordering::SeqCst);
            return Box::pin(async {
                Ok(Response {
                    status: 200,
                    body: PALETTE_PNG.to_vec(),
                    server_date_utc_ms: None,
                    content_range_total: None,
                })
            });
        }
        if self.strict.load(Ordering::SeqCst) {
            self.refused.lock().unwrap().push(req.url.path().to_owned());
            return Box::pin(async { Err(SourceError::Unreachable(Unreachable::TimedOut)) });
        }
        let t = self.inner.lock().unwrap().clone();
        Box::pin(async move { t.send(req).await })
    }
}

fn guarded(dir: &TempDir) -> (Arc<EncoreCore>, Arc<Events>, Arc<Guard>) {
    let guard = Arc::new(Guard {
        inner: Mutex::new(Arc::new(FixtureTransport::from_json(RECORDED).unwrap())),
        strict: AtomicBool::new(false),
        refused: Mutex::new(Vec::new()),
        covers: AtomicU32::new(0),
        palettes: AtomicU32::new(0),
    });
    let config = CoreConfig {
        data_dir: dir.0.to_string_lossy().into_owned(),
        device_id: "device-test".into(),
        device_name: "Test".into(),
        platform: "test".into(),
    };
    let core = EncoreCore::build(config, Duration::ZERO, 50, Some(guard.clone())).unwrap();
    let events = Arc::new(Events::default());
    core.set_listener(events.clone());
    (core, events, guard)
}

/// One debounced write now (it fetches the covers it lacks first).
fn flush(core: &Arc<EncoreCore>) {
    let inner = core.inner.clone();
    block(core.spawn(async move {
        inner.widgets_flush().await;
        Ok(())
    }))
    .unwrap();
}

fn widget_dir(dir: &TempDir, profile: &str) -> PathBuf {
    dir.0.join("widgets").join(profile)
}

fn snapshot(dir: &TempDir, profile: &str) -> Value {
    let text = std::fs::read_to_string(widget_dir(dir, profile).join("snapshot.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn art_files(dir: &TempDir, profile: &str) -> HashSet<String> {
    std::fs::read_dir(widget_dir(dir, profile).join("art"))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The covers a snapshot file names.
fn named_covers(v: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut add = |c: &Value| {
        if let Some(s) = c.as_str() {
            out.insert(s.to_owned());
        }
    };
    add(&v["song"]["cover"]);
    add(&v["nextUp"]["cover"]);
    for m in v["pinnedMixes"].as_array().into_iter().flatten() {
        add(&m["cover"]);
    }
    for a in v["recentAlbums"].as_array().into_iter().flatten() {
        add(&a["cover"]);
    }
    out
}

/// Every title, artist and album title a snapshot file holds.
fn texts_of(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                match (k.as_str(), x.as_str()) {
                    ("title" | "artist" | "albumTitle", Some(s)) => out.push(s.to_owned()),
                    _ => texts_of(x, out),
                }
            }
        }
        Value::Array(a) => {
            for x in a {
                texts_of(x, out);
            }
        }
        _ => {}
    }
}

/// Every file under a profile's widget folder, as bytes.
fn widget_bytes(dir: &TempDir, profile: &str) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut folders = vec![widget_dir(dir, profile)];
    while let Some(d) = folders.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let path = e.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                out.push(std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn holds(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// None of `texts` is in any file of the profile's widget folder, as written or
/// JSON-escaped. Texts too short or too plain to tell apart from the file's own
/// keys, numbers and hex IDs are left out.
fn assert_no_titles(dir: &TempDir, profile: &str, texts: &[String]) {
    let keys = "schema profileId privacy song isPlaying positionMs positionAtUtcMs \
                pinnedMixes nextUp recentAlbums reachable writtenAtUtcMs recap \
                locked guest normal true false null";
    let files = widget_bytes(dir, profile);
    let mut checked = 0;
    for t in texts {
        let plain = t.chars().count() < 4
            || !t.chars().any(char::is_alphabetic)
            || t.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
            || keys.contains(t.as_str());
        if plain {
            continue;
        }
        checked += 1;
        let escaped = serde_json::to_string(t).unwrap();
        let escaped = &escaped[1..escaped.len() - 1];
        for f in &files {
            assert!(
                !holds(f, t.as_bytes()) && !holds(f, escaped.as_bytes()),
                "{t:?} is still in the widget files"
            );
        }
    }
    assert!(checked > 0, "nothing to look for");
}

/// Every colour (base and stops) of every palette a snapshot file holds.
fn palette_colours(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "palette" && x.is_object() {
                    out.extend(x["base"].as_str().map(str::to_owned));
                    out.extend(
                        x["stops"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|s| s.as_str().map(str::to_owned)),
                    );
                } else {
                    palette_colours(x, out);
                }
            }
        }
        Value::Array(a) => {
            for x in a {
                palette_colours(x, out);
            }
        }
        _ => {}
    }
}

/// No palette is in any file of the profile's widget folder: not its key, and none
/// of `colours` (a palette's base and stops, as they were written before).
fn assert_no_palettes(dir: &TempDir, profile: &str, colours: &[String]) {
    assert!(!colours.is_empty(), "nothing to look for");
    for f in widget_bytes(dir, profile) {
        for key in ["palette", "stops"] {
            assert!(!holds(&f, key.as_bytes()), "{key:?} is in the widget files");
        }
        for c in colours {
            assert!(!holds(&f, c.as_bytes()), "{c} is in the widget files");
        }
    }
}

/// The mix the current song was started from.
fn playing_context(core: &EncoreCore) -> Option<String> {
    let p = core.inner.profile().unwrap();
    let pb = p.playback.lock().unwrap();
    pb.queue.current().and_then(|c| c.context_id.clone())
}

fn song_ids(core: &EncoreCore) -> Vec<i64> {
    core.songs(SongSort::Title, 0, 1000)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect()
}

/// Albums with a cover, in title order.
fn covered_albums(core: &EncoreCore) -> Vec<AlbumDetail> {
    core.albums(AlbumSort::Title, 0, 200)
        .unwrap()
        .into_iter()
        .filter(|a| a.has_artwork && a.song_count >= 1)
        .map(|a| core.album(a.id).unwrap())
        .collect()
}

/// Board 34, item 10: the file's shape, written whole, with only saved covers.
#[test]
fn the_snapshot_is_schema_1_written_atomically_with_its_covers() {
    let dir = temp_dir("widgets-file");
    let (core, events, _) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let album = covered_albums(&core).remove(0);
    let ids: Vec<i64> = album.songs.iter().map(|s| s.id).collect();
    let q = core
        .play(
            ids.clone(),
            0,
            StartedFrom::Album,
            Some(album.album.id.to_string()),
        )
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 60).unwrap();
    core.playback_paused(42_000).unwrap();
    flush(&core);

    let v = snapshot(&dir, &me);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["profileId"], me.as_str());
    assert_eq!(v["privacy"], "normal");
    assert!(v.as_object().unwrap().contains_key("recap") && v["recap"].is_null());
    assert_eq!(v["reachable"], true);
    assert!(v["writtenAtUtcMs"].as_i64().unwrap() > 0);
    assert_eq!(v["song"]["songId"], ids[0]);
    assert_eq!(v["song"]["albumId"], album.album.id);
    assert_eq!(v["song"]["nowPlaying"], true);
    assert_eq!(v["isPlaying"], false);
    assert_eq!(v["positionMs"], 42_000);
    assert_eq!(v["recentAlbums"][0]["albumId"], album.album.id);
    assert!(v["pinnedMixes"].as_array().unwrap().is_empty());
    // Every song, mix and album carries a palette key (null until made).
    assert!(v["song"].as_object().unwrap().contains_key("palette"));
    assert!(
        v["recentAlbums"][0]
            .as_object()
            .unwrap()
            .contains_key("palette")
    );

    // The covers it names are saved beside it, exactly those, as the server sent them.
    let named = named_covers(&v);
    assert!(!named.is_empty(), "{v}");
    assert_eq!(art_files(&dir, &me), named);
    for name in &named {
        let bytes = std::fs::read(widget_dir(&dir, &me).join("art").join(name)).unwrap();
        assert_eq!(bytes, COVER);
    }
    // Written whole: no temporary file is left anywhere.
    let mut folders = vec![widget_dir(&dir, &me)];
    while let Some(d) = folders.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            assert!(!name.ends_with(".tmp"), "{name} left behind");
            if e.path().is_dir() {
                folders.push(e.path());
            }
        }
    }
    // The app hears which profile's file changed.
    assert!(events.take().iter().any(|e| {
        e.kind == EventKind::WidgetSnapshotChanged
            && e.widget_profile_id.as_deref() == Some(me.as_str())
    }));
    // The read call agrees.
    let s = core.widget_snapshot().unwrap();
    assert_eq!((s.schema, s.privacy), (1, WidgetPrivacy::Normal));
    assert_eq!(s.song.as_ref().and_then(|x| x.song_id), Some(ids[0]));
    assert_eq!(s.recent_albums[0].album_id, album.album.id);

    // Resumed: playing, from where it was.
    core.playback_resumed(42_000).unwrap();
    flush(&core);
    let v = snapshot(&dir, &me);
    assert_eq!(v["isPlaying"], true);
    assert_eq!(v["positionMs"], 42_000);
}

/// The privacy rule's hardest case: from the moment a PIN is set, the file and
/// the covers hold nothing of the profile's listening, byte for byte.
#[test]
fn a_locked_profile_keeps_no_titles_or_covers() {
    let dir = temp_dir("widgets-pin");
    let (core, _, _) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let ids = song_ids(&core);
    listen(&core, &ids[..60], 60);
    let home = block(core.regenerate_mixes()).unwrap();
    let first = home.made_for_you.mixes[0].mix_id.clone();
    core.pin_mix(first, true).unwrap();
    // Something playing now, too.
    let q = core
        .play(vec![ids[70]], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 0).unwrap();
    flush(&core);
    let before = snapshot(&dir, &me);
    let mut texts = Vec::new();
    texts_of(&before, &mut texts);
    assert!(!before["song"].is_null() && !before["pinnedMixes"].as_array().unwrap().is_empty());
    assert!(!art_files(&dir, &me).is_empty());
    // The colours too are part of what a PIN must hide.
    let mut colours = Vec::new();
    palette_colours(&before, &mut colours);
    assert!(!colours.is_empty(), "the snapshot had palettes: {before}");

    core.set_profile_pin(me.clone(), "4821".into()).unwrap();
    let check = |why: &str| {
        let v = snapshot(&dir, &me);
        assert_eq!(v["privacy"], "locked", "{why}");
        // Nothing but the profile, the state and the time.
        for (k, x) in v.as_object().unwrap() {
            match x {
                Value::String(s) => {
                    assert!(s == me.as_str() || s == "locked", "{why}: {k} = {s}")
                }
                Value::Array(a) => assert!(a.is_empty(), "{why}: {k}"),
                Value::Object(_) => panic!("{why}: {k} holds {x}"),
                _ => {}
            }
        }
        assert!(art_files(&dir, &me).is_empty(), "{why}: covers kept");
        assert_no_titles(&dir, &me, &texts);
        assert_no_palettes(&dir, &me, &colours);
    };
    check("at once");
    // Still locked while the owner keeps listening, and after the debounced writer.
    core.playback_paused(1_000).unwrap();
    core.playback_resumed(1_000).unwrap();
    core.next().unwrap();
    flush(&core);
    check("after playing on");
    std::thread::sleep(Duration::from_millis(1_500));
    check("after the debounced writer");
    let s = core.widget_snapshot().unwrap();
    assert_eq!(s.privacy, WidgetPrivacy::Locked);
    assert!(s.song.is_none() && s.pinned_mixes.is_empty() && s.next_up.is_none());

    // Without the PIN the widgets show the profile again.
    core.remove_profile_pin(me.clone()).unwrap();
    flush(&core);
    let v = snapshot(&dir, &me);
    assert_eq!(v["privacy"], "normal");
    assert!(!v["song"].is_null());
}

/// Guest shows nothing; Private listening and Learning off record no plays, so
/// last played and the recent albums stay as they were.
#[test]
fn guest_private_and_learning_off_leave_last_played_alone() {
    let dir = temp_dir("widgets-modes");
    let (core, _, _) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    // A song from each of four albums with a cover.
    let ids: Vec<i64> = covered_albums(&core)
        .iter()
        .take(4)
        .map(|a| a.songs[0].id)
        .collect();
    listen(&core, &ids[..1], 1);
    flush(&core);
    let v = snapshot(&dir, &me);
    assert_eq!(v["song"]["songId"], ids[0]);
    assert_eq!(v["song"]["nowPlaying"], false);
    let recent = v["recentAlbums"].clone();
    assert!(!recent.as_array().unwrap().is_empty());

    for (mode, other) in [
        (ListeningMode::Private, ids[1]),
        (ListeningMode::LearningOff, ids[2]),
    ] {
        core.set_listening_mode(mode).unwrap();
        let q = core.play(vec![other], 0, StartedFrom::Songs, None).unwrap();
        core.playback_started(q.current.unwrap().uid, 0).unwrap();
        core.playback_resumed(5_000).unwrap();
        flush(&core);
        let v = snapshot(&dir, &me);
        assert_eq!(v["song"]["songId"], ids[0], "{mode:?}: now playing stays");
        assert_eq!(v["song"]["nowPlaying"], false, "{mode:?}");
        assert_eq!(v["isPlaying"], false, "{mode:?}");
        assert_eq!(v["recentAlbums"], recent, "{mode:?}");
        core.playback_ended(EndReason::Finished, 200_000).unwrap();
        flush(&core);
        let v = snapshot(&dir, &me);
        assert_eq!(v["song"]["songId"], ids[0], "{mode:?}: last played stays");
        assert_eq!(v["recentAlbums"], recent, "{mode:?}");
        core.set_listening_mode(ListeningMode::Normal).unwrap();
    }

    // Guest: the neutral card at once, whatever plays.
    core.start_guest().unwrap();
    let guest = |why: &str| {
        let v = snapshot(&dir, &me);
        assert_eq!(v["privacy"], "guest", "{why}");
        assert!(v["song"].is_null(), "{why}");
        assert!(v["recentAlbums"].as_array().unwrap().is_empty(), "{why}");
        assert!(v["pinnedMixes"].as_array().unwrap().is_empty(), "{why}");
        assert!(art_files(&dir, &me).is_empty(), "{why}");
    };
    guest("at once");
    let q = core
        .play(vec![ids[3]], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 0).unwrap();
    flush(&core);
    guest("while a guest plays");
    core.end_guest().unwrap();
    flush(&core);
    let v = snapshot(&dir, &me);
    assert_eq!(v["privacy"], "normal");
    assert_eq!(v["song"]["songId"], ids[0], "the guest's song never shows");
    assert_eq!(v["recentAlbums"], recent);
}

/// Clear listening history: the widgets lose it at once, and the covers only it
/// named are deleted.
#[test]
fn clearing_history_empties_the_widgets_and_prunes_the_covers() {
    let dir = temp_dir("widgets-clear");
    let (core, _, _) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let albums = covered_albums(&core);
    let ids: Vec<i64> = albums.iter().take(3).map(|a| a.songs[0].id).collect();
    listen(&core, &ids, ids.len());
    flush(&core);
    let v = snapshot(&dir, &me);
    assert!(!v["song"].is_null());
    assert_eq!(v["recentAlbums"].as_array().unwrap().len(), ids.len());
    assert_eq!(art_files(&dir, &me).len(), ids.len());

    // A file no snapshot names goes at the next write.
    let stray = widget_dir(&dir, &me).join("art").join("stray.img");
    std::fs::write(&stray, COVER).unwrap();
    flush(&core);
    assert!(!stray.exists(), "an unnamed cover stays");

    core.clear_listening_history().unwrap();
    // No flush: the write is immediate.
    let v = snapshot(&dir, &me);
    assert!(v["song"].is_null(), "{v}");
    assert!(v["recentAlbums"].as_array().unwrap().is_empty());
    assert!(
        art_files(&dir, &me).is_empty(),
        "covers only the history named"
    );
}

/// Two profiles on one device: each has its own folder, and one's listening
/// never reaches the other's file. A profile that closes stops "playing".
#[test]
fn two_profiles_never_mix() {
    let dir = temp_dir("widgets-two");
    let (core, _, guard) = guarded(&dir);
    let sam = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let sam_ids = song_ids(&core);
    listen(&core, &sam_ids[..1], 1);
    let q = core
        .play(vec![sam_ids[1]], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 0).unwrap();
    flush(&core);
    assert_eq!(snapshot(&dir, &sam)["isPlaying"], true);

    // Alex signs in on the same device: Sam's profile closes.
    *guard.inner.lock().unwrap() = second_user();
    let alex = signed_in(&core).profile_id;
    assert_ne!(alex, sam);
    let v = snapshot(&dir, &sam);
    assert_eq!(v["isPlaying"], false, "Sam's closed profile isn't playing");
    assert_eq!(v["song"]["songId"], sam_ids[1]);
    assert_eq!(v["song"]["nowPlaying"], false);
    let sam_file = std::fs::read(widget_dir(&dir, &sam).join("snapshot.json")).unwrap();
    let sam_art = art_files(&dir, &sam);

    block(core.sync_library()).unwrap();
    flush(&core);
    let v = snapshot(&dir, &alex);
    assert_eq!(v["profileId"], alex.as_str());
    assert!(v["song"].is_null(), "nothing of Sam's: {v}");
    assert!(v["recentAlbums"].as_array().unwrap().is_empty());

    let alex_song = covered_albums(&core)[0].songs[0].id;
    listen(&core, &[alex_song], 1);
    flush(&core);
    let v = snapshot(&dir, &alex);
    assert_eq!(v["song"]["songId"], alex_song);
    assert_eq!(v["recentAlbums"].as_array().unwrap().len(), 1);
    // Sam's file and covers didn't move.
    assert_eq!(
        std::fs::read(widget_dir(&dir, &sam).join("snapshot.json")).unwrap(),
        sam_file
    );
    assert_eq!(art_files(&dir, &sam), sam_art);
    assert_eq!(snapshot(&dir, &sam)["profileId"], sam.as_str());
}

/// Board 34, item 10: a removed profile's widgets say "Profile removed": its
/// folder is gone, and the app hears of it.
#[test]
fn removing_a_profile_deletes_its_widget_folder() {
    let dir = temp_dir("widgets-remove");
    let (core, events, _) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    listen(&core, &song_ids(&core)[..2], 2);
    flush(&core);
    assert!(widget_dir(&dir, &me).join("snapshot.json").is_file());
    events.take();

    core.remove_profile_from_device(me.clone()).unwrap();
    assert!(!widget_dir(&dir, &me).exists());
    assert!(events.take().iter().any(|e| {
        e.kind == EventKind::WidgetSnapshotChanged
            && e.widget_profile_id.as_deref() == Some(me.as_str())
    }));
    // No profile is open: nothing writes it back.
    std::thread::sleep(Duration::from_millis(1_500));
    assert!(!widget_dir(&dir, &me).exists());
}

/// A burst of play-state reports is one write, about a second later.
#[test]
fn writes_are_debounced() {
    let dir = temp_dir("widgets-debounce");
    let (core, events, _) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let ids = song_ids(&core);
    let q = core
        .play(vec![ids[0]], 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 0).unwrap();
    // Let the writer settle.
    std::thread::sleep(Duration::from_millis(2_500));
    events.take();

    for i in 0..20 {
        core.playback_paused(i * 100).unwrap();
        core.playback_resumed(i * 100).unwrap();
    }
    std::thread::sleep(Duration::from_millis(2_600));
    let writes = events
        .take()
        .iter()
        .filter(|e| e.kind == EventKind::WidgetSnapshotChanged)
        .count();
    assert_eq!(writes, 1, "40 reports, one write");
    let v = snapshot(&dir, &me);
    assert_eq!(v["isPlaying"], true);
    assert_eq!(v["positionMs"], 1_900);
}

/// `play_mix`: a mix by ID, Next up, and the fallbacks.
#[test]
fn play_mix_falls_back_to_next_up() {
    let dir = temp_dir("widgets-mix");
    let (core, _, _) = guarded(&dir);
    signed_in(&core);
    block(core.sync_library()).unwrap();
    listen(&core, &song_ids(&core)[..60], 60);
    let home = block(core.regenerate_mixes()).unwrap();
    let set: Vec<String> = home
        .made_for_you
        .mixes
        .iter()
        .map(|m| m.mix_id.clone())
        .collect();
    assert!(!set.is_empty());

    // Nothing played from the set yet: Next up is its first mix.
    let next = core.widget_snapshot().unwrap().next_up.unwrap();
    assert_eq!(next.mix_id, set[0]);
    assert!(!next.pinned);

    // A mix that's gone (the set rotated) plays Next up instead, shuffle off.
    core.set_shuffle(true).unwrap();
    let q = core
        .play_mix(MixRef {
            kind: MixRefKind::Id,
            mix_id: Some("mix-gone".into()),
        })
        .unwrap();
    assert!(!q.shuffle);
    assert_eq!(playing_context(&core).as_deref(), Some(set[0].as_str()));
    let page = core.mix(set[0].clone()).unwrap();
    assert_eq!(q.current.as_ref().unwrap().song.id, page.songs[0].song.id);

    // Once it plays, Next up moves on (back to the first when all were played).
    core.playback_started(q.current.unwrap().uid, 0).unwrap();
    let expected = set.get(1).unwrap_or(&set[0]).clone();
    let next = core.widget_snapshot().unwrap().next_up.unwrap();
    assert_eq!(next.mix_id, expected);
    let q = core
        .play_mix(MixRef {
            kind: MixRefKind::NextUp,
            mix_id: None,
        })
        .unwrap();
    assert!(q.current.is_some());
    assert_eq!(playing_context(&core), Some(expected));

    // A mix by ID plays that mix.
    let last = set.last().unwrap().clone();
    core.play_mix(MixRef {
        kind: MixRefKind::Id,
        mix_id: Some(last.clone()),
    })
    .unwrap();
    assert_eq!(playing_context(&core), Some(last));

    // Learning off has no set, so no Next up: the first pinned mix plays.
    core.pin_mix(set[0].clone(), true).unwrap();
    core.set_listening_mode(ListeningMode::LearningOff).unwrap();
    assert!(core.widget_snapshot().unwrap().next_up.is_none());
    core.play_mix(MixRef {
        kind: MixRefKind::NextUp,
        mix_id: None,
    })
    .unwrap();
    assert_eq!(playing_context(&core).as_deref(), Some(set[0].as_str()));
    // With no pin either, there is nothing to play.
    core.pin_mix(set[0].clone(), false).unwrap();
    assert!(matches!(
        core.play_mix(MixRef {
            kind: MixRefKind::NextUp,
            mix_id: None,
        }),
        Err(EncoreError::NotFound { .. })
    ));
}

/// "A widget never calls the network", core side: building and writing the
/// snapshot asks the server for covers and palette images and nothing else, whatever
/// happens; `widget_snapshot()` alone asks for nothing.
#[test]
fn building_the_snapshot_makes_no_network_call_but_covers_and_palettes() {
    let dir = temp_dir("widgets-offline");
    let (core, _, guard) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let ids = song_ids(&core);
    listen(&core, &ids[..60], 60);
    let home = block(core.regenerate_mixes()).unwrap();
    let mixes: Vec<String> = home
        .made_for_you
        .mixes
        .iter()
        .map(|m| m.mix_id.clone())
        .collect();
    core.pin_mix(mixes[0].clone(), true).unwrap();
    flush(&core);
    std::thread::sleep(Duration::from_millis(1_500));
    // Every cover must be fetched again from here on.
    let _ = std::fs::remove_dir_all(widget_dir(&dir, &me).join("art"));
    let covers_before = guard.covers.load(Ordering::SeqCst);
    guard.strict.store(true, Ordering::SeqCst);

    let q = core
        .play(ids[60..64].to_vec(), 0, StartedFrom::Songs, None)
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 60).unwrap();
    core.playback_paused(10_000).unwrap();
    core.playback_seeked(10_000, 30_000).unwrap();
    core.playback_resumed(30_000).unwrap();
    let q = core.next().unwrap();
    core.playback_started(q.current.unwrap().uid, 60).unwrap();
    core.playback_ended(EndReason::Finished, 200_000).unwrap();
    core.pin_mix(mixes[0].clone(), false).unwrap();
    core.pin_mix(mixes[0].clone(), true).unwrap();
    if let Some(other) = mixes.get(1) {
        core.hide_mix(other.clone()).unwrap();
    }
    core.set_listening_mode(ListeningMode::Private).unwrap();
    core.set_listening_mode(ListeningMode::Normal).unwrap();
    core.start_guest().unwrap();
    core.end_guest().unwrap();
    core.set_profile_pin(me.clone(), "4821".into()).unwrap();
    core.remove_profile_pin(me.clone()).unwrap();
    core.set_connectivity(false, Network::Wifi, Vec::new())
        .unwrap();
    core.set_connectivity(true, Network::Wifi, Vec::new())
        .unwrap();
    core.play_mix(MixRef {
        kind: MixRefKind::NextUp,
        mix_id: None,
    })
    .unwrap();
    flush(&core);
    std::thread::sleep(Duration::from_millis(1_500));
    // The read call builds from local state and asks for nothing, not even a cover.
    let before_read = (
        guard.covers.load(Ordering::SeqCst),
        guard.palettes.load(Ordering::SeqCst),
    );
    let _ = core.widget_snapshot().unwrap();
    assert_eq!(
        (
            guard.covers.load(Ordering::SeqCst),
            guard.palettes.load(Ordering::SeqCst)
        ),
        before_read,
        "widget_snapshot() made a request"
    );

    let refused = guard.refused.lock().unwrap().clone();
    assert!(
        refused.is_empty(),
        "requests other than covers and palettes: {refused:?}"
    );
    assert!(
        guard.covers.load(Ordering::SeqCst) > covers_before,
        "covers came through the core's client"
    );
    let v = snapshot(&dir, &me);
    assert_eq!(art_files(&dir, &me), named_covers(&v));
    assert!(!named_covers(&v).is_empty());
}

/// Colours: the song and the recent albums carry their album's palette, the same
/// one the app draws that album's gradient from (board 04), and a mix tile takes
/// its cover's album's.
#[test]
fn palettes_reach_the_snapshot_and_match_album_palette() {
    let dir = temp_dir("widgets-palette");
    let (core, _, guard) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let ids = song_ids(&core);
    listen(&core, &ids[..60], 60);
    let home = block(core.regenerate_mixes()).unwrap();
    core.pin_mix(home.made_for_you.mixes[0].mix_id.clone(), true)
        .unwrap();
    let album = covered_albums(&core).remove(0);
    let q = core
        .play(
            album.songs.iter().map(|s| s.id).collect(),
            0,
            StartedFrom::Album,
            Some(album.album.id.to_string()),
        )
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 60).unwrap();
    flush(&core);

    let golden = serde_json::json!({
        "scheme": "dark",
        "base": "3D0000",
        "stops": ["CF4630", "1B1030", "C02D56", "986E00"],
        "text": "FFFFFF",
    });
    let v = snapshot(&dir, &me);
    assert_eq!(v["song"]["palette"], golden, "{v}");
    assert_eq!(v["recentAlbums"][0]["palette"], golden);
    // A mix takes the colours of the album its cover comes from: there is one when
    // the tile has a cover, and none when it has not.
    let mix = &v["pinnedMixes"][0];
    if mix["cover"].is_null() {
        assert!(mix["palette"].is_null(), "{v}");
    } else {
        assert_eq!(mix["palette"], golden, "{v}");
    }

    // The record the read call returns is the app's palette, without its extras.
    let made = guard.palettes.load(Ordering::SeqCst);
    assert!(made >= 1);
    let app = block(core.album_palette(album.album.id)).unwrap().unwrap();
    let song = core.widget_snapshot().unwrap().song.unwrap();
    assert_eq!(song.palette, Some(WidgetPalette::from(app)));
    // The app got it from the same cache: no second image request.
    assert_eq!(guard.palettes.load(Ordering::SeqCst), made);
}

/// A palette is made once per cover, and not while the server can't be reached;
/// it comes when it can.
#[test]
fn a_palette_is_made_once_and_not_while_offline() {
    let dir = temp_dir("widgets-palette-once");
    let (core, _, guard) = guarded(&dir);
    let me = signed_in(&core).profile_id;
    block(core.sync_library()).unwrap();
    let album = covered_albums(&core).remove(0);
    let q = core
        .play(
            album.songs.iter().map(|s| s.id).collect(),
            0,
            StartedFrom::Album,
            Some(album.album.id.to_string()),
        )
        .unwrap();
    core.playback_started(q.current.unwrap().uid, 60).unwrap();
    // Offline before the first write (the debounced one is a second away): nothing
    // can be asked for.
    core.set_connectivity(false, Network::Wifi, Vec::new())
        .unwrap();
    flush(&core);
    assert_eq!(
        guard.palettes.load(Ordering::SeqCst),
        0,
        "asked while offline"
    );
    assert!(snapshot(&dir, &me)["song"]["palette"].is_null());

    // Reachable: made, written, and the app is told.
    core.set_connectivity(true, Network::Wifi, Vec::new())
        .unwrap();
    flush(&core);
    let made = guard.palettes.load(Ordering::SeqCst);
    assert!(made >= 1, "never made");
    assert_eq!(snapshot(&dir, &me)["song"]["palette"]["base"], "3D0000");

    // Once: more writes ask for nothing.
    flush(&core);
    flush(&core);
    assert_eq!(guard.palettes.load(Ordering::SeqCst), made);
}
