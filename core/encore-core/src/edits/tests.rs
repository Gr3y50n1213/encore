//! Edits against a small model of a Jellyfin server that keeps playlists the way
//! Jellyfin does (entry IDs, positions), and can be unreachable or refuse.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use encore_sources::{
    Album, ArtistCredit, Capabilities, Cursor, LibraryDelta, PlaylistEntry, SourceKind, Track,
    Unreachable,
};
use encore_store::{NewSource, TrackRow};
use encore_sync::HlcClock;

use super::*;

// ---------------------------------------------------------------- a fake server

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Up,
    Down,
    Refuses,
}

#[derive(Debug)]
struct FakeServer {
    mode: Mutex<Mode>,
    sent: Mutex<Vec<ServerEdit>>,
    playlists: Mutex<HashMap<String, Vec<PlaylistEntry>>>,
    favorites: Mutex<HashSet<String>>,
    next: Mutex<u32>,
}

impl FakeServer {
    fn new() -> FakeServer {
        FakeServer {
            mode: Mutex::new(Mode::Up),
            sent: Mutex::new(Vec::new()),
            playlists: Mutex::new(HashMap::new()),
            favorites: Mutex::new(HashSet::new()),
            next: Mutex::new(0),
        }
    }
    fn set(&self, m: Mode) {
        *self.mode.lock().unwrap() = m;
    }
    fn id(&self, prefix: &str) -> String {
        let mut n = self.next.lock().unwrap();
        *n += 1;
        format!("{prefix}{n}")
    }
    fn items(&self, playlist: &str) -> Vec<String> {
        self.playlists.lock().unwrap()[playlist]
            .iter()
            .map(|e| e.track_item_id.clone())
            .collect()
    }
    fn sent(&self) -> Vec<ServerEdit> {
        self.sent.lock().unwrap().clone()
    }
    fn with_playlist(&self, id: &str, items: &[&str]) {
        let entries = items
            .iter()
            .map(|i| PlaylistEntry {
                entry_id: Some(self.id("e")),
                track_item_id: (*i).to_owned(),
            })
            .collect();
        self.playlists
            .lock()
            .unwrap()
            .insert(id.to_owned(), entries);
    }
}

type Fut<'a, T> = encore_sources::transport::BoxFuture<'a, encore_sources::Result<T>>;

impl MediaSource for FakeServer {
    fn kind(&self) -> SourceKind {
        SourceKind::Jellyfin
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn sync_library<'a>(
        &'a self,
        _: Option<&'a Cursor>,
        _: &'a HashSet<String>,
        _: u32,
        _: &'a (dyn Fn(encore_sources::jellyfin::SyncProgress) + Send + Sync),
    ) -> Fut<'a, LibraryDelta> {
        Box::pin(async { Err(SourceError::NotFound) })
    }
    fn stream_info(
        &self,
        _: &str,
        _: &encore_sources::jellyfin::StreamRequest,
    ) -> encore_sources::Result<encore_sources::jellyfin::StreamInfo> {
        Err(SourceError::NotFound)
    }
    fn artwork(
        &self,
        _: &str,
        _: u32,
    ) -> encore_sources::Result<encore_sources::jellyfin::ArtworkInfo> {
        Err(SourceError::NotFound)
    }
    fn lyrics<'a>(&'a self, _: &'a str) -> Fut<'a, Option<encore_sources::jellyfin::RawLyrics>> {
        Box::pin(async { Ok(None) })
    }
    fn cover_png<'a>(&'a self, _: &'a str, _: u32, _: Option<&'a str>) -> Fut<'a, Option<Vec<u8>>> {
        Box::pin(async { Ok(None) })
    }
    fn edit<'a>(&'a self, edit: &'a ServerEdit) -> Fut<'a, EditOutcome> {
        Box::pin(async move {
            match *self.mode.lock().unwrap() {
                Mode::Down => return Err(SourceError::Unreachable(Unreachable::Refused)),
                Mode::Refuses => return Err(SourceError::NotFound),
                Mode::Up => {}
            }
            self.sent.lock().unwrap().push(edit.clone());
            let mut lists = self.playlists.lock().unwrap();
            Ok(match edit {
                ServerEdit::CreatePlaylist { item_ids, .. } => {
                    let id = self.id("pl");
                    let entries = item_ids
                        .iter()
                        .map(|i| PlaylistEntry {
                            entry_id: Some(self.id("e")),
                            track_item_id: i.clone(),
                        })
                        .collect();
                    lists.insert(id.clone(), entries);
                    EditOutcome::Created(id)
                }
                ServerEdit::AddToPlaylist {
                    playlist_id,
                    item_ids,
                    position,
                } => {
                    let list = lists.get_mut(playlist_id).ok_or(SourceError::NotFound)?;
                    let at = position.map_or(list.len(), |p| (p as usize).min(list.len()));
                    for (k, i) in item_ids.iter().enumerate() {
                        list.insert(
                            at + k,
                            PlaylistEntry {
                                entry_id: Some(self.id("e")),
                                track_item_id: i.clone(),
                            },
                        );
                    }
                    EditOutcome::Done
                }
                ServerEdit::RemoveFromPlaylist {
                    playlist_id,
                    entry_ids,
                } => {
                    let list = lists.get_mut(playlist_id).ok_or(SourceError::NotFound)?;
                    list.retain(|e| !entry_ids.contains(e.entry_id.as_ref().unwrap()));
                    EditOutcome::Done
                }
                ServerEdit::MoveInPlaylist {
                    playlist_id,
                    entry_id,
                    new_index,
                } => {
                    let list = lists.get_mut(playlist_id).ok_or(SourceError::NotFound)?;
                    let from = list
                        .iter()
                        .position(|e| e.entry_id.as_ref() == Some(entry_id))
                        .ok_or(SourceError::NotFound)?;
                    let e = list.remove(from);
                    let to = (*new_index as usize).min(list.len());
                    list.insert(to, e);
                    EditOutcome::Done
                }
                ServerEdit::RenamePlaylist { .. } => EditOutcome::Done,
                ServerEdit::DeletePlaylist { playlist_id } => {
                    lists.remove(playlist_id);
                    EditOutcome::Done
                }
                ServerEdit::SetFavorite { item_id, favorite } => {
                    let mut f = self.favorites.lock().unwrap();
                    if *favorite {
                        f.insert(item_id.clone());
                    } else {
                        f.remove(item_id);
                    }
                    EditOutcome::Done
                }
            })
        })
    }
    fn playlist_entries<'a>(&'a self, playlist_id: &'a str) -> Fut<'a, Vec<PlaylistEntry>> {
        Box::pin(async move {
            if *self.mode.lock().unwrap() == Mode::Down {
                return Err(SourceError::Unreachable(Unreachable::Refused));
            }
            self.playlists
                .lock()
                .unwrap()
                .get(playlist_id)
                .cloned()
                .ok_or(SourceError::NotFound)
        })
    }
}

// ---------------------------------------------------------------- a library

const T0: i64 = 1_762_400_000_000;

fn credit(n: &str) -> ArtistCredit {
    ArtistCredit {
        source_item_id: None,
        name: n.into(),
    }
}

/// 600 songs on 60 albums by 6 artists; "dup" is s0 again on album 59 (same
/// artist, title and length: the same song).
fn library() -> LibraryDelta {
    let albums = (0..60)
        .map(|a| Album {
            source_item_id: format!("al{a}"),
            title: format!("Album {a}"),
            sort_title: format!("Album {a:02}"),
            album_artists: vec![credit(&format!("Artist {}", a % 6))],
            year: None,
            mbid: None,
            genres: vec![],
            date_added_utc_ms: None,
            has_artwork: false,
            artwork_tag: None,
        })
        .collect();
    let mut tracks: Vec<Track> = (0..600)
        .map(|i| Track {
            source_item_id: format!("s{i}"),
            title: format!("Song {i}"),
            sort_title: format!("Song {i:03}"),
            album_item_id: Some(format!("al{}", i / 10)),
            album_title: Some(format!("Album {}", i / 10)),
            artists: vec![credit(&format!("Artist {}", (i / 10) % 6))],
            album_artists: vec![],
            disc_number: Some(1),
            track_number: Some((i % 10) as u32 + 1),
            duration_ms: 200_000,
            year: None,
            container: None,
            bitrate: None,
            mbid_recording: None,
            genres: vec![],
            date_added_utc_ms: None,
            has_lyrics: false,
            track_gain_db: None,
            album_gain_db: None,
        })
        .collect();
    let mut dup = tracks[0].clone();
    dup.source_item_id = "dup".into();
    dup.album_item_id = Some("al59".into());
    dup.album_title = Some("Album 59".into());
    tracks.push(dup);
    LibraryDelta {
        full: true,
        artists: vec![],
        albums,
        tracks,
        playlists: vec![encore_sources::Playlist {
            source_item_id: "srv-road".into(),
            name: "Road".into(),
            date_added_utc_ms: None,
            entries: vec![
                PlaylistEntry {
                    entry_id: Some("road-e1".into()),
                    track_item_id: "s0".into(),
                },
                PlaylistEntry {
                    entry_id: Some("road-e2".into()),
                    track_item_id: "s1".into(),
                },
            ],
            artwork_tag: None,
            can_edit: true,
        }],
        removed_item_ids: vec![],
        favorite_item_ids: None,
        cursor: None,
    }
}

struct Fixture {
    store: Mutex<Store>,
    clock: HlcClock,
    source: i64,
    road: i64,
    server: FakeServer,
}

fn fixture() -> Fixture {
    let store = Store::open_in_memory().unwrap();
    let source = store
        .add_source(&NewSource {
            kind: "jellyfin",
            name: "Home",
            base_url: "http://h",
            server_id: "srv",
            server_version: None,
            user_id: "u",
            user_name: "Ana",
            pinned_cert_sha256: None,
            created_utc_ms: 0,
        })
        .unwrap();
    store.apply_delta(source, &library()).unwrap();
    let road = store
        .playlists(encore_store::Page {
            offset: 0,
            limit: 5,
        })
        .unwrap()[0]
        .id;
    let server = FakeServer::new();
    server.with_playlist("srv-road", &["s0", "s1"]);
    // The server's entry IDs for Road are the ones the mirror has.
    {
        let mut lists = server.playlists.lock().unwrap();
        let road_list = lists.get_mut("srv-road").unwrap();
        road_list[0].entry_id = Some("road-e1".into());
        road_list[1].entry_id = Some("road-e2".into());
    }
    Fixture {
        store: Mutex::new(store),
        clock: HlcClock::default(),
        source,
        road,
        server,
    }
}

impl Fixture {
    fn songs(&self, ids: &[&str]) -> Vec<TrackRow> {
        let s = self.store.lock().unwrap();
        ids.iter()
            .map(|i| s.track_by_item(self.source, i).unwrap().unwrap())
            .collect()
    }
    fn edit<T>(&mut self, at: i64, f: impl FnOnce(&mut Editor<'_>) -> T) -> T {
        let s = self.store.lock().unwrap();
        let mut ed = Editor {
            store: &s,
            clock: &mut self.clock,
            device_id: "dev",
            now_utc_ms: at,
            undo_ms: 10_000,
        };
        f(&mut ed)
    }
    fn rows(&self, playlist: i64) -> Vec<String> {
        self.store
            .lock()
            .unwrap()
            .playlist_items(playlist)
            .unwrap()
            .into_iter()
            .map(|r| r.track_item_id)
            .collect()
    }
    /// (events, undo entries, outbox entries waiting)
    fn counts(&self) -> (usize, u32, u32) {
        let s = self.store.lock().unwrap();
        (
            s.unsynced_events().unwrap().len(),
            s.undo_entries_count().unwrap(),
            s.pending_outbox_count().unwrap(),
        )
    }
    fn flush(&self) -> Flushed {
        block(flush(&self.store, self.source, &self.server, T0)).unwrap()
    }
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

// ---------------------------------------------------------------- the rules

#[test]
fn five_hundred_songs_are_one_event_one_undo_and_one_outbox_entry() {
    let mut f = fixture();
    let ids: Vec<String> = (100..600).map(|i| format!("s{i}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let songs = f.songs(&refs);
    let road = f.road;
    let applied = f
        .edit(T0, |ed| ed.add(Target::Playlist(road), &songs, None, false))
        .unwrap();
    assert_eq!((applied.added, applied.already_there), (500, 0));
    assert_eq!(f.counts(), (1, 1, 1));
    assert_eq!(f.rows(road).len(), 502);
    // Sent as one write, in the order given.
    let sent = f.flush();
    assert_eq!(
        sent,
        Flushed {
            sent: 1,
            refused: 0,
            waiting: 0
        }
    );
    assert_eq!(f.server.sent().len(), 1);
    assert_eq!(f.server.items("srv-road"), f.rows(road));
}

#[test]
fn duplicates_are_found_by_song_not_file() {
    let mut f = fixture();
    let road = f.road;
    // "dup" is s0 on another album: the playlist already has it.
    let songs = f.songs(&["dup", "s5", "s5"]);
    let p = preview(
        &f.store.lock().unwrap(),
        Target::Playlist(road),
        &songs,
        false,
        100,
    )
    .unwrap();
    assert_eq!((p.count, p.already_there, p.refusal), (3, 2, None));
    let a = f
        .edit(T0, |ed| ed.add(Target::Playlist(road), &songs, None, false))
        .unwrap();
    assert_eq!(
        (a.added, a.already_there),
        (1, 2),
        "Added 1 · 2 already there"
    );
    // Add anyway.
    let a = f
        .edit(T0, |ed| ed.add(Target::Playlist(road), &songs, None, true))
        .unwrap();
    assert_eq!(a.added, 3);
    assert_eq!(f.rows(road), ["s0", "s1", "s5", "dup", "s5", "s5"]);
    // Nothing new at all: no edit, no event.
    let before = f.counts();
    let again = f.songs(&["s0"]);
    let a = f
        .edit(T0, |ed| ed.add(Target::Playlist(road), &again, None, false))
        .unwrap();
    assert_eq!((a.edit_id, a.added), (None, 0));
    assert_eq!(f.counts(), before);
}

#[test]
fn songs_go_in_the_order_shown_at_the_line() {
    let mut f = fixture();
    let road = f.road;
    let songs = f.songs(&["s9", "s3", "s7"]);
    f.edit(T0, |ed| {
        ed.add(Target::Playlist(road), &songs, Some(1), false)
    })
    .unwrap();
    assert_eq!(f.rows(road), ["s0", "s9", "s3", "s7", "s1"]);
    f.flush();
    assert_eq!(f.server.items("srv-road"), f.rows(road));
}

#[test]
fn refusals_confirmations_and_names() {
    let f = fixture();
    let s = f.store.lock().unwrap();
    let songs: Vec<TrackRow> = (0..150)
        .map(|i| {
            s.track_by_item(f.source, &format!("s{i}"))
                .unwrap()
                .unwrap()
        })
        .collect();
    let p = preview(&s, Target::Album(1), &songs[..2], false, 100).unwrap();
    assert_eq!(p.refusal, Some(Refusal::AlbumsTakeNoSongs));
    let p = preview(&s, Target::Playlist(f.road), &songs[..2], true, 100).unwrap();
    assert_eq!(p.refusal, Some(Refusal::GuestCannotSave));
    let p = preview(&s, Target::UpNext, &songs[..2], true, 100).unwrap();
    assert_eq!(p.refusal, None, "a guest may still queue songs");
    let p = preview(&s, Target::Playlist(f.road), &songs, false, 100).unwrap();
    assert!(p.needs_confirm, "over 100 songs asks first");
    let p = preview(&s, Target::Playlist(f.road), &songs[..100], false, 100).unwrap();
    assert!(!p.needs_confirm);
    // Named after the album when all songs share it, else the first artist.
    let p = preview(&s, Target::NewPlaylist, &songs[..10], false, 100).unwrap();
    assert_eq!(p.suggested_name.as_deref(), Some("Album 0"));
    let p = preview(&s, Target::NewPlaylist, &songs[5..15], false, 100).unwrap();
    assert_eq!(p.suggested_name.as_deref(), Some("Artist 0"));
    let p = preview(&s, Target::LikedSongs, &[], false, 100).unwrap();
    assert_eq!(p.refusal, Some(Refusal::NothingToAdd));
    assert!(matches!(
        preview(&s, Target::Playlist(999), &songs[..1], false, 100),
        Err(EditError::NotFound(_))
    ));
}

#[test]
fn undo_removes_exactly_that_edit_within_its_window() {
    let mut f = fixture();
    let road = f.road;
    let first = f.songs(&["s2", "s3"]);
    let second = f.songs(&["s4"]);
    let a = f
        .edit(T0, |ed| ed.add(Target::Playlist(road), &first, None, false))
        .unwrap();
    f.edit(T0 + 1_000, |ed| {
        ed.add(Target::Playlist(road), &second, Some(0), false)
    })
    .unwrap();
    // Another edit moved the first edit's rows; undo still finds them.
    assert_eq!(f.rows(road), ["s4", "s0", "s1", "s2", "s3"]);
    let id = a.edit_id.unwrap();
    assert_eq!(f.edit(T0 + 9_000, |ed| ed.undo(&id)).unwrap(), Undone::Done);
    assert_eq!(f.rows(road), ["s4", "s0", "s1"]);
    assert!(matches!(
        f.edit(T0 + 9_500, |ed| ed.undo(&id)),
        Err(EditError::AlreadyUndone)
    ));
    // Too late.
    let more = f.songs(&["s8"]);
    let late = f
        .edit(T0, |ed| ed.add(Target::Playlist(road), &more, None, false))
        .unwrap();
    assert!(matches!(
        f.edit(T0 + 10_001, |ed| ed.undo(late.edit_id.as_deref().unwrap())),
        Err(EditError::UndoExpired)
    ));
    assert!(matches!(
        f.edit(T0, |ed| ed.undo("no-such-edit")),
        Err(EditError::NotFound(_))
    ));
    // Sent in order, the server ends where this device is.
    f.flush();
    assert_eq!(f.server.items("srv-road"), f.rows(road));
}

#[test]
fn offline_edits_wait_survive_a_pull_and_go_when_the_server_is_back() {
    let mut f = fixture();
    let road = f.road;
    f.server.set(Mode::Down);
    let songs = f.songs(&["s20", "s21"]);
    f.edit(T0, |ed| ed.add(Target::Playlist(road), &songs, None, false))
        .unwrap();
    f.edit(T0, |ed| ed.remove(road, &[0])).unwrap();
    f.edit(T0, |ed| ed.move_row(road, 2, 0)).unwrap();
    // The library shows the change at once, offline.
    assert_eq!(f.rows(road), ["s21", "s1", "s20"]);
    let r = f.flush();
    assert_eq!((r.sent, r.waiting), (0, 3));
    assert_eq!(f.server.sent(), []);
    // A pull now must not undo the edits waiting to be sent.
    f.store
        .lock()
        .unwrap()
        .apply_delta(f.source, &library())
        .unwrap();
    assert_eq!(f.rows(road), ["s21", "s1", "s20"]);
    // Back online: sent in order, and the server ends up the same.
    f.server.set(Mode::Up);
    let r = f.flush();
    assert_eq!((r.sent, r.refused, r.waiting), (3, 0, 0));
    assert_eq!(f.server.items("srv-road"), ["s21", "s1", "s20"]);
    assert_eq!(f.rows(road), f.server.items("srv-road"));
    // Every row now has its server entry ID (read back after the writes).
    let s = f.store.lock().unwrap();
    assert!(
        s.playlist_items(road)
            .unwrap()
            .iter()
            .all(|r| r.entry_id.is_some())
    );
}

#[test]
fn a_playlist_made_offline_gets_its_server_id_before_later_writes() {
    let mut f = fixture();
    f.server.set(Mode::Down);
    let songs = f.songs(&["s30", "s31", "s32"]);
    let made = f
        .edit(T0, |ed| ed.add(Target::NewPlaylist, &songs, None, false))
        .unwrap();
    let pl = made.playlist_id.unwrap();
    let more = f.songs(&["s40"]);
    f.edit(T0, |ed| ed.add(Target::Playlist(pl), &more, None, false))
        .unwrap();
    f.edit(T0, |ed| ed.rename(pl, "Night drive")).unwrap();
    f.server.set(Mode::Up);
    f.flush();
    let sent = f.server.sent();
    assert!(matches!(&sent[0], ServerEdit::CreatePlaylist { name, .. } if name == "Album 3"));
    let server_id = f
        .store
        .lock()
        .unwrap()
        .playlist_record(pl)
        .unwrap()
        .unwrap()
        .source_item_id;
    assert!(!server_id.starts_with(LOCAL_ITEM_PREFIX));
    assert!(
        matches!(&sent[1], ServerEdit::AddToPlaylist { playlist_id, .. } if *playlist_id == server_id)
    );
    assert!(matches!(&sent[2], ServerEdit::RenamePlaylist { name, .. } if name == "Night drive"));
    assert_eq!(f.server.items(&server_id), ["s30", "s31", "s32", "s40"]);
}

#[test]
fn undoing_a_new_playlist_before_and_after_it_was_sent() {
    let mut f = fixture();
    // Before: the server never hears of it.
    f.server.set(Mode::Down);
    let songs = f.songs(&["s50"]);
    let made = f
        .edit(T0, |ed| ed.add(Target::NewPlaylist, &songs, None, false))
        .unwrap();
    f.edit(T0 + 1, |ed| ed.undo(made.edit_id.as_deref().unwrap()))
        .unwrap();
    f.server.set(Mode::Up);
    f.flush();
    assert_eq!(f.server.sent(), []);
    assert!(
        f.store
            .lock()
            .unwrap()
            .playlist_record(made.playlist_id.unwrap())
            .unwrap()
            .is_none()
    );
    // After: it is deleted there too.
    let songs = f.songs(&["s51"]);
    let made = f
        .edit(T0, |ed| ed.add(Target::NewPlaylist, &songs, None, false))
        .unwrap();
    f.flush();
    f.edit(T0 + 1, |ed| ed.undo(made.edit_id.as_deref().unwrap()))
        .unwrap();
    f.flush();
    assert!(matches!(
        f.server.sent().last(),
        Some(ServerEdit::DeletePlaylist { .. })
    ));
    assert_eq!(
        f.server.playlists.lock().unwrap().len(),
        1,
        "only Road is left"
    );
}

#[test]
fn likes_are_favourites_and_one_song_on_two_albums_is_liked_once() {
    let mut f = fixture();
    let songs = f.songs(&["s0", "dup", "s7"]);
    let a = f
        .edit(T0, |ed| ed.add(Target::LikedSongs, &songs, None, false))
        .unwrap();
    assert_eq!((a.added, a.already_there), (2, 1));
    assert_eq!(
        f.counts(),
        (1, 1, 1),
        "one event, one undo, one outbox entry"
    );
    f.flush();
    let fav = f.server.favorites.lock().unwrap().clone();
    assert_eq!(fav, HashSet::from(["s0".to_owned(), "s7".to_owned()]));
    assert_eq!(f.store.lock().unwrap().liked_songs(0, 10).unwrap().len(), 2);
    // Unlike through the other copy of the song: every copy stops being a favourite.
    let other = f.songs(&["dup"]);
    f.edit(T0, |ed| ed.unlike(&other)).unwrap();
    f.flush();
    assert_eq!(
        f.server.favorites.lock().unwrap().clone(),
        HashSet::from(["s7".to_owned()])
    );
    // A favourite set elsewhere becomes a like at the next sync; one cleared
    // elsewhere clears it.
    let s = f.store.lock().unwrap();
    let mut d = library();
    d.full = false;
    d.favorite_item_ids = Some(vec!["s9".into()]);
    s.apply_delta(f.source, &d).unwrap();
    s.reconcile_likes(f.source, &HashSet::new(), T0).unwrap();
    let s9 = s.track_by_item(f.source, "s9").unwrap().unwrap().song_key;
    assert_eq!(s.liked_keys().unwrap(), HashSet::from([s9.clone()]));
    // A song with a favourite write still waiting keeps its like.
    let s20 = s.track_by_item(f.source, "s20").unwrap().unwrap().song_key;
    s.like(&s20, T0).unwrap();
    s.reconcile_likes(f.source, &HashSet::from([s20.clone()]), T0)
        .unwrap();
    assert!(s.liked_keys().unwrap().contains(&s20));
}

#[test]
fn a_refused_write_is_dropped_and_the_next_sync_restores_the_server_version() {
    let mut f = fixture();
    let road = f.road;
    let a = f.songs(&["s60"]);
    let b = f.songs(&["s61"]);
    f.edit(T0, |ed| ed.add(Target::Playlist(road), &a, None, false))
        .unwrap();
    f.edit(T0, |ed| ed.add(Target::LikedSongs, &b, None, false))
        .unwrap();
    f.server.set(Mode::Refuses);
    let r = f.flush();
    assert_eq!((r.sent, r.refused), (0, 2));
    assert_eq!(f.store.lock().unwrap().pending_outbox_count().unwrap(), 0);
    f.store
        .lock()
        .unwrap()
        .apply_delta(f.source, &library())
        .unwrap();
    assert_eq!(f.rows(road), ["s0", "s1"]);
}

#[test]
fn read_only_playlists_refuse_every_edit() {
    let mut f = fixture();
    let mut d = library();
    d.playlists[0].can_edit = false;
    f.store.lock().unwrap().apply_delta(f.source, &d).unwrap();
    let road = f.road;
    let songs = f.songs(&["s70"]);
    assert!(matches!(
        f.edit(T0, |ed| ed.add(Target::Playlist(road), &songs, None, false)),
        Err(EditError::Refused(Refusal::PlaylistReadOnly))
    ));
    assert!(matches!(
        f.edit(T0, |ed| ed.remove(road, &[0])),
        Err(EditError::Refused(Refusal::PlaylistReadOnly))
    ));
    assert_eq!(f.counts(), (0, 0, 0));
}

#[test]
fn entry_ids_are_found_by_position_then_by_song() {
    let entries = vec![
        PlaylistEntry {
            entry_id: Some("a".into()),
            track_item_id: "x".into(),
        },
        PlaylistEntry {
            entry_id: Some("b".into()),
            track_item_id: "y".into(),
        },
        PlaylistEntry {
            entry_id: Some("c".into()),
            track_item_id: "x".into(),
        },
    ];
    let row = |p: u32, i: &str| RowRef {
        position: p,
        item_id: i.into(),
        entry_id: None,
    };
    assert_eq!(resolve_entries(&[row(2, "x")], &entries), ["c"]);
    assert_eq!(
        resolve_entries(&[row(0, "x"), row(0, "x")], &entries),
        ["a", "c"]
    );
    assert_eq!(resolve_entries(&[row(9, "y")], &entries), ["b"]);
    assert!(resolve_entries(&[row(0, "z")], &entries).is_empty());
}

#[test]
fn the_outbox_survives_closing_the_app() {
    let dir = std::env::temp_dir().join(format!("encore-outbox-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.sqlite");
    let _ = std::fs::remove_file(&path);
    {
        let f = fixture();
        let s = f.store.lock().unwrap();
        s.backup_to(&path).unwrap();
    }
    let store = Store::open(&path).unwrap();
    let source = store.sources().unwrap()[0].id;
    let road = store
        .playlists(encore_store::Page {
            offset: 0,
            limit: 5,
        })
        .unwrap()[0]
        .id;
    let song = store.track_by_item(source, "s90").unwrap().unwrap();
    let mut clock = HlcClock::default();
    Editor {
        store: &store,
        clock: &mut clock,
        device_id: "dev",
        now_utc_ms: T0,
        undo_ms: 10_000,
    }
    .add(Target::Playlist(road), &[song], None, false)
    .unwrap();
    drop(store); // the app is closed before anything was sent
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.pending_outbox_count().unwrap(), 1);
    assert_eq!(reopened.unsynced_events().unwrap().len(), 1);
    drop(reopened);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 24, "two devices editing a playlist at the same time while offline":
/// both edits land on the media server, and each device ends up showing the
/// server's list (the phone after the sync event names the playlist).
#[test]
fn two_devices_edit_one_playlist_offline_and_both_edits_land() {
    let mut phone = fixture();
    let mut mac = fixture();
    // One media server for both: the phone fixture's.
    phone.server.set(Mode::Down);
    let s5 = phone.songs(&["s5"]);
    let phone_road = phone.road;
    phone
        .edit(T0, |ed| {
            ed.add(Target::Playlist(phone_road), &s5, None, false)
        })
        .unwrap();
    let s7 = mac.songs(&["s7"]);
    let road = mac.road;
    mac.edit(T0, |ed| ed.remove(road, &[0])).unwrap();
    mac.edit(T0, |ed| ed.add(Target::Playlist(road), &s7, None, false))
        .unwrap();
    assert_eq!(phone.rows(phone.road), ["s0", "s1", "s5"]);
    assert_eq!(mac.rows(road), ["s1", "s7"]);

    // Back online: the phone sends first, then the Mac.
    phone.server.set(Mode::Up);
    block(flush(&phone.store, phone.source, &phone.server, T0)).unwrap();
    block(flush(&mac.store, mac.source, &phone.server, T0)).unwrap();
    assert_eq!(
        phone.server.items("srv-road"),
        ["s1", "s5", "s7"],
        "both edits landed"
    );
    assert_eq!(
        mac.rows(road),
        ["s1", "s5", "s7"],
        "the Mac re-read it after sending"
    );
    // The Mac's sync event names the playlist; the phone re-reads it.
    block(refresh_playlist(&phone.store, &phone.server, phone.road)).unwrap();
    assert_eq!(phone.rows(phone.road), ["s1", "s5", "s7"]);
}
