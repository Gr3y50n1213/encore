//! Library edits: playlists and likes (plan section 26, board 43).
//!
//! Every edit is **one call**: whatever the number of songs, it writes the local
//! change, **one event** (the log Phase 4 syncs) and **one undo entry** together,
//! in one transaction, plus **one outbox entry** with the server writes it needs.
//! So it works offline: the library shows the change at once, and the outbox sends
//! it when the server can be reached, in order, surviving a closed app.
//!
//! Sending (`flush`): an entry whose server can't be reached waits for the next
//! chance; one the server refuses (it no longer has the playlist, say) is dropped,
//! and the next sync brings back the server's version. A partly sent entry
//! remembers what is left, so nothing is sent twice.
//!
//! Duplicates are found by song key, so a song on two albums counts once.

use std::collections::HashSet;
use std::sync::Mutex;

use encore_sources::{EditOutcome, MediaSource, PlaylistEntry, ServerEdit, SourceError};
use encore_store::{EventRecord, LOCAL_ITEM_PREFIX, PlaylistItemRow, Store, StoreError, TrackRow};
use encore_sync::HlcClock;
use serde::{Deserialize, Serialize};

/// Where dropped or chosen songs go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Playlist(i64),
    /// A new playlist, named after the album or the first artist.
    NewPlaylist,
    LikedSongs,
    UpNext,
    PlayNext,
    /// Albums never take songs: they come from the server's tags and folders.
    Album(i64),
}

/// Why a drop is refused (board 43). The UI shows `sentence()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    GuestCannotSave,
    AlbumsTakeNoSongs,
    PlaylistReadOnly,
    NothingToAdd,
}

impl Refusal {
    pub fn sentence(self) -> &'static str {
        match self {
            Refusal::GuestCannotSave => "Guest cannot save playlists",
            Refusal::AlbumsTakeNoSongs => {
                "Albums come from your server's tags and folders, so songs can't be added to them. Make a playlist instead."
            }
            Refusal::PlaylistReadOnly => "This playlist is read-only",
            Refusal::NothingToAdd => "None of these songs can be added here",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Songs that would be added (duplicates included).
    pub count: u32,
    /// Of those, already there (by song).
    pub already_there: u32,
    /// Over `gestures.dragAndDrop.confirmOverSongs`: ask first.
    pub needs_confirm: bool,
    /// For a new playlist: the album's name, or the first artist's.
    pub suggested_name: Option<String>,
    pub refusal: Option<Refusal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// None when nothing changed (every song was already there).
    pub edit_id: Option<String>,
    pub added: u32,
    pub already_there: u32,
    pub playlist_id: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("{}", .0.sentence())]
    Refused(Refusal),
    #[error("that {0} isn't in your library")]
    NotFound(&'static str),
    #[error("too late to undo that")]
    UndoExpired,
    #[error("that was already undone")]
    AlreadyUndone,
    #[error("the profile store is unavailable")]
    Poisoned,
}

/// A playlist row an edit refers to, as it was when the edit was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowRef {
    pub position: u32,
    pub item_id: String,
    pub entry_id: Option<String>,
}

/// One server write as the outbox keeps it. Playlists are named by their row ID
/// here, so a playlist made offline can be written to before it has a server ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum QueuedOp {
    Create {
        playlist: i64,
        name: String,
        item_ids: Vec<String>,
    },
    Add {
        playlist: i64,
        item_ids: Vec<String>,
        position: Option<u32>,
    },
    Remove {
        playlist: i64,
        rows: Vec<RowRef>,
    },
    Move {
        playlist: i64,
        row: RowRef,
        to: u32,
    },
    Rename {
        playlist: i64,
        name: String,
    },
    Delete {
        item_id: String,
    },
    Favorite {
        item_ids: Vec<String>,
        favorite: bool,
    },
}

impl QueuedOp {
    fn playlist(&self) -> Option<i64> {
        match self {
            QueuedOp::Create { playlist, .. }
            | QueuedOp::Add { playlist, .. }
            | QueuedOp::Remove { playlist, .. }
            | QueuedOp::Move { playlist, .. }
            | QueuedOp::Rename { playlist, .. } => Some(*playlist),
            QueuedOp::Delete { .. } | QueuedOp::Favorite { .. } => None,
        }
    }
}

/// What undoing an edit does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Inverse {
    RemoveEditRows {
        playlist: i64,
        edit_id: String,
    },
    DeletePlaylist {
        playlist: i64,
    },
    Reinsert {
        playlist: i64,
        rows: Vec<RowRef>,
    },
    MoveBack {
        playlist: i64,
        from: u32,
        to: u32,
    },
    Rename {
        playlist: i64,
        name: String,
    },
    Unlike {
        keys: Vec<String>,
    },
    Like {
        keys: Vec<String>,
        items: Vec<(i64, String)>,
    },
    /// The queue's own undo (Up Next and Play Next).
    Queue,
}

/// What `undo` leaves to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undone {
    Done,
    /// Undo the queue change (the queue lives outside the store).
    Queue,
}

/// The context every edit needs.
pub struct Editor<'a> {
    pub store: &'a Store,
    pub clock: &'a mut HlcClock,
    pub device_id: &'a str,
    pub now_utc_ms: i64,
    /// `gestures.dragAndDrop.shortcutUndoMs`: the longest any Undo is offered.
    pub undo_ms: i64,
}

impl std::fmt::Debug for Editor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("now_utc_ms", &self.now_utc_ms)
            .finish_non_exhaustive()
    }
}

fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

/// The name a new playlist gets (board 43): the album's, when every song is from
/// one album; otherwise the first song's artist.
pub fn suggested_name(songs: &[TrackRow]) -> String {
    let first = songs.first();
    let album = first.and_then(|s| s.album_title.clone());
    if let Some(a) = &album
        && songs.iter().all(|s| s.album_title.as_ref() == Some(a))
    {
        return a.clone();
    }
    first
        .map(|s| s.artist.clone())
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| "New Playlist".to_owned())
}

/// The songs not already there by song key, each song once.
fn new_by_key<'s>(songs: &'s [TrackRow], there: &HashSet<String>) -> (Vec<&'s TrackRow>, u32) {
    let mut seen = HashSet::new();
    let mut fresh = Vec::new();
    let mut dup = 0;
    for s in songs {
        if there.contains(&s.song_key) || !seen.insert(s.song_key.clone()) {
            dup += 1;
        } else {
            fresh.push(s);
        }
    }
    (fresh, dup)
}

/// What a drop would do, before it is done (board 43: the tip, the count, the
/// "Added 2 · 1 already there" toast, the confirm over 100, the refusals).
pub fn preview(
    store: &Store,
    target: Target,
    songs: &[TrackRow],
    guest: bool,
    confirm_over: u32,
) -> Result<Preview, EditError> {
    let mut p = Preview {
        count: songs.len() as u32,
        already_there: 0,
        needs_confirm: songs.len() as u32 > confirm_over,
        suggested_name: None,
        refusal: None,
    };
    let saves = matches!(
        target,
        Target::Playlist(_) | Target::NewPlaylist | Target::LikedSongs
    );
    p.refusal = if guest && saves {
        Some(Refusal::GuestCannotSave)
    } else if matches!(target, Target::Album(_)) {
        Some(Refusal::AlbumsTakeNoSongs)
    } else if songs.is_empty() {
        Some(Refusal::NothingToAdd)
    } else {
        None
    };
    if p.refusal.is_some() {
        return Ok(p);
    }
    match target {
        Target::Playlist(id) => {
            let rec = store
                .playlist_record(id)?
                .ok_or(EditError::NotFound("playlist"))?;
            if !rec.can_edit {
                p.refusal = Some(Refusal::PlaylistReadOnly);
                return Ok(p);
            }
            let same_source: Vec<TrackRow> = songs
                .iter()
                .filter(|s| s.source_id == rec.source_id)
                .cloned()
                .collect();
            if same_source.is_empty() {
                p.refusal = Some(Refusal::NothingToAdd);
            }
            p.already_there = new_by_key(&same_source, &store.playlist_song_keys(id)?).1;
        }
        Target::NewPlaylist => p.suggested_name = Some(suggested_name(songs)),
        Target::LikedSongs => p.already_there = new_by_key(songs, &store.liked_keys()?).1,
        _ => {}
    }
    Ok(p)
}

impl Editor<'_> {
    /// Write the event and undo entry and queue the server writes, in the open
    /// transaction.
    fn record(
        &mut self,
        kind: &str,
        edit_id: &str,
        payload: serde_json::Value,
        inverse: Option<&Inverse>,
        queued: Option<(i64, Option<i64>, Vec<QueuedOp>)>,
    ) -> Result<(), EditError> {
        let hlc = self.clock.tick(self.now_utc_ms);
        let mut payload = payload;
        payload["editId"] = edit_id.into();
        self.store.insert_event(&EventRecord {
            id: new_id(),
            device_id: self.device_id.to_owned(),
            hlc: hlc.to_string(),
            kind: kind.to_owned(),
            payload: payload.to_string(),
            created_utc_ms: self.now_utc_ms,
        })?;
        if let Some(inv) = inverse {
            self.store.save_undo(
                edit_id,
                kind,
                &serde_json::to_string(inv).map_err(|e| StoreError::Edit(e.to_string()))?,
                self.now_utc_ms + self.undo_ms,
            )?;
        }
        if let Some((source, playlist, ops)) = queued
            && !ops.is_empty()
        {
            let json = serde_json::to_string(&ops).map_err(|e| StoreError::Edit(e.to_string()))?;
            self.store.enqueue(source, edit_id, playlist, &json)?;
        }
        Ok(())
    }

    /// Add songs to a playlist (at `position`, or the end), a new playlist, or
    /// Liked Songs. One call, however many songs.
    pub fn add(
        &mut self,
        target: Target,
        songs: &[TrackRow],
        position: Option<u32>,
        allow_duplicates: bool,
    ) -> Result<Applied, EditError> {
        match target {
            Target::Playlist(id) => self.add_to_playlist(id, songs, position, allow_duplicates),
            Target::NewPlaylist => self.new_playlist(songs),
            Target::LikedSongs => self.like(songs),
            Target::Album(_) => Err(EditError::Refused(Refusal::AlbumsTakeNoSongs)),
            Target::UpNext | Target::PlayNext => Err(EditError::NotFound("playlist")),
        }
    }

    fn add_to_playlist(
        &mut self,
        id: i64,
        songs: &[TrackRow],
        position: Option<u32>,
        allow_duplicates: bool,
    ) -> Result<Applied, EditError> {
        let rec = self
            .store
            .playlist_record(id)?
            .ok_or(EditError::NotFound("playlist"))?;
        if !rec.can_edit {
            return Err(EditError::Refused(Refusal::PlaylistReadOnly));
        }
        let songs: Vec<TrackRow> = songs
            .iter()
            .filter(|s| s.source_id == rec.source_id)
            .cloned()
            .collect();
        if songs.is_empty() {
            return Err(EditError::Refused(Refusal::NothingToAdd));
        }
        let there = self.store.playlist_song_keys(id)?;
        let (fresh, dup) = new_by_key(&songs, &there);
        let adding: Vec<&TrackRow> = if allow_duplicates {
            songs.iter().collect()
        } else {
            fresh
        };
        if adding.is_empty() {
            return Ok(Applied {
                edit_id: None,
                added: 0,
                already_there: dup,
                playlist_id: Some(id),
            });
        }
        let edit_id = new_id();
        self.store.in_transaction(|store| {
            let mut rows = store.playlist_items(id)?;
            let at = position.map_or(rows.len(), |p| (p as usize).min(rows.len()));
            let new_rows = adding.iter().map(|s| PlaylistItemRow {
                track_item_id: s.source_item_id.clone(),
                entry_id: None,
                edit_id: Some(edit_id.clone()),
            });
            let tail = rows.split_off(at);
            rows.extend(new_rows);
            rows.extend(tail);
            store.set_playlist_items(id, &rows)?;
            let item_ids: Vec<String> = adding.iter().map(|s| s.source_item_id.clone()).collect();
            let end = at == rows.len() - adding.len();
            self.record(
                "playlistAdd",
                &edit_id,
                serde_json::json!({
                    "source": rec.source_id,
                    "playlist": rec.source_item_id,
                    "songKeys": adding.iter().map(|s| &s.song_key).collect::<Vec<_>>(),
                    "itemIds": item_ids,
                    "position": if end { None } else { Some(at) },
                }),
                Some(&Inverse::RemoveEditRows {
                    playlist: id,
                    edit_id: edit_id.clone(),
                }),
                Some((
                    rec.source_id,
                    Some(id),
                    vec![QueuedOp::Add {
                        playlist: id,
                        item_ids,
                        position: if end { None } else { Some(at as u32) },
                    }],
                )),
            )
            .map_err(|e| match e {
                EditError::Store(s) => s,
                other => StoreError::Edit(other.to_string()),
            })
        })?;
        Ok(Applied {
            edit_id: Some(edit_id),
            added: adding.len() as u32,
            already_there: dup,
            playlist_id: Some(id),
        })
    }

    fn new_playlist(&mut self, songs: &[TrackRow]) -> Result<Applied, EditError> {
        let Some(first) = songs.first() else {
            return Err(EditError::Refused(Refusal::NothingToAdd));
        };
        let source = first.source_id;
        let songs: Vec<TrackRow> = songs
            .iter()
            .filter(|s| s.source_id == source)
            .cloned()
            .collect();
        let (fresh, dup) = new_by_key(&songs, &HashSet::new());
        let name = suggested_name(&songs);
        let edit_id = new_id();
        let id = self.store.in_transaction(|store| {
            let id = store.create_playlist_row(
                source,
                &format!("{LOCAL_ITEM_PREFIX}{edit_id}"),
                &name,
                self.now_utc_ms,
            )?;
            let rows: Vec<PlaylistItemRow> = fresh
                .iter()
                .map(|s| PlaylistItemRow {
                    track_item_id: s.source_item_id.clone(),
                    entry_id: None,
                    edit_id: Some(edit_id.clone()),
                })
                .collect();
            store.set_playlist_items(id, &rows)?;
            let item_ids: Vec<String> = fresh.iter().map(|s| s.source_item_id.clone()).collect();
            self.record(
                "playlistCreate",
                &edit_id,
                serde_json::json!({
                    "source": source,
                    "name": name,
                    "songKeys": fresh.iter().map(|s| &s.song_key).collect::<Vec<_>>(),
                    "itemIds": item_ids,
                }),
                Some(&Inverse::DeletePlaylist { playlist: id }),
                Some((
                    source,
                    Some(id),
                    vec![QueuedOp::Create {
                        playlist: id,
                        name: name.clone(),
                        item_ids,
                    }],
                )),
            )
            .map_err(as_store)?;
            Ok(id)
        })?;
        Ok(Applied {
            edit_id: Some(edit_id),
            added: fresh.len() as u32,
            already_there: dup,
            playlist_id: Some(id),
        })
    }

    /// Like songs (Liked Songs, and each one a favourite on its server).
    pub fn like(&mut self, songs: &[TrackRow]) -> Result<Applied, EditError> {
        let liked = self.store.liked_keys()?;
        let (fresh, dup) = new_by_key(songs, &liked);
        if fresh.is_empty() {
            return Ok(Applied {
                edit_id: None,
                added: 0,
                already_there: dup,
                playlist_id: None,
            });
        }
        let edit_id = new_id();
        self.store.in_transaction(|store| {
            let mut by_source: Vec<(i64, Vec<String>)> = Vec::new();
            for s in &fresh {
                store.like(&s.song_key, self.now_utc_ms)?;
                store.set_favorite(s.source_id, &s.source_item_id, true)?;
                match by_source.iter_mut().find(|(src, _)| *src == s.source_id) {
                    Some((_, items)) => items.push(s.source_item_id.clone()),
                    None => by_source.push((s.source_id, vec![s.source_item_id.clone()])),
                }
            }
            let keys: Vec<String> = fresh.iter().map(|s| s.song_key.clone()).collect();
            // One outbox entry per server (normally one).
            let mut queued = by_source.into_iter();
            let first = queued.next();
            self.record(
                "like",
                &edit_id,
                serde_json::json!({ "songKeys": keys }),
                Some(&Inverse::Unlike { keys: keys.clone() }),
                first.map(|(src, items)| {
                    (
                        src,
                        None,
                        vec![QueuedOp::Favorite {
                            item_ids: items,
                            favorite: true,
                        }],
                    )
                }),
            )
            .map_err(as_store)?;
            for (src, items) in queued {
                let json = serde_json::to_string(&vec![QueuedOp::Favorite {
                    item_ids: items,
                    favorite: true,
                }])
                .map_err(|e| StoreError::Edit(e.to_string()))?;
                store.enqueue(src, &edit_id, None, &json)?;
            }
            Ok(())
        })?;
        Ok(Applied {
            edit_id: Some(edit_id),
            added: fresh.len() as u32,
            already_there: dup,
            playlist_id: None,
        })
    }

    /// Unlike songs: every copy of each song (by song key) stops being a favourite.
    pub fn unlike(&mut self, songs: &[TrackRow]) -> Result<Applied, EditError> {
        let liked = self.store.liked_keys()?;
        let mut keys: Vec<String> = Vec::new();
        for s in songs {
            if liked.contains(&s.song_key) && !keys.contains(&s.song_key) {
                keys.push(s.song_key.clone());
            }
        }
        if keys.is_empty() {
            return Ok(Applied {
                edit_id: None,
                added: 0,
                already_there: 0,
                playlist_id: None,
            });
        }
        let edit_id = new_id();
        let items = self.store.items_for_keys(&keys)?;
        let n = keys.len() as u32;
        self.store.in_transaction(|store| {
            for k in &keys {
                store.unlike(k)?;
            }
            for (src, item, _) in &items {
                store.set_favorite(*src, item, false)?;
            }
            let pairs: Vec<(i64, String)> = items.iter().map(|(s, i, _)| (*s, i.clone())).collect();
            self.record(
                "unlike",
                &edit_id,
                serde_json::json!({ "songKeys": keys }),
                Some(&Inverse::Like {
                    keys: keys.clone(),
                    items: pairs.clone(),
                }),
                None,
            )
            .map_err(as_store)?;
            queue_favorites(store, &edit_id, &pairs, false)?;
            Ok(())
        })?;
        Ok(Applied {
            edit_id: Some(edit_id),
            added: n,
            already_there: 0,
            playlist_id: None,
        })
    }

    /// Remove rows of a playlist (Remove on a row swipe, a move's source).
    pub fn remove(&mut self, id: i64, positions: &[u32]) -> Result<String, EditError> {
        let rec = self
            .store
            .playlist_record(id)?
            .ok_or(EditError::NotFound("playlist"))?;
        if !rec.can_edit {
            return Err(EditError::Refused(Refusal::PlaylistReadOnly));
        }
        let edit_id = new_id();
        self.store.in_transaction(|store| {
            let rows = store.playlist_items(id)?;
            let mut wanted: Vec<u32> = positions
                .iter()
                .copied()
                .filter(|p| (*p as usize) < rows.len())
                .collect();
            wanted.sort_unstable();
            wanted.dedup();
            let removed: Vec<RowRef> = wanted
                .iter()
                .map(|p| {
                    let r = &rows[*p as usize];
                    RowRef {
                        position: *p,
                        item_id: r.track_item_id.clone(),
                        entry_id: r.entry_id.clone(),
                    }
                })
                .collect();
            let kept: Vec<PlaylistItemRow> = rows
                .into_iter()
                .enumerate()
                .filter(|(i, _)| !wanted.contains(&(*i as u32)))
                .map(|(_, r)| r)
                .collect();
            store.set_playlist_items(id, &kept)?;
            self.record(
                "playlistRemove",
                &edit_id,
                serde_json::json!({
                    "source": rec.source_id,
                    "playlist": rec.source_item_id,
                    "positions": wanted,
                    "itemIds": removed.iter().map(|r| &r.item_id).collect::<Vec<_>>(),
                }),
                Some(&Inverse::Reinsert {
                    playlist: id,
                    rows: removed.clone(),
                }),
                Some((
                    rec.source_id,
                    Some(id),
                    vec![QueuedOp::Remove {
                        playlist: id,
                        rows: removed,
                    }],
                )),
            )
            .map_err(as_store)
        })?;
        Ok(edit_id)
    }

    /// Move one row within a playlist (reordering is always a move).
    pub fn move_row(&mut self, id: i64, from: u32, to: u32) -> Result<String, EditError> {
        let rec = self
            .store
            .playlist_record(id)?
            .ok_or(EditError::NotFound("playlist"))?;
        if !rec.can_edit {
            return Err(EditError::Refused(Refusal::PlaylistReadOnly));
        }
        let edit_id = new_id();
        self.store.in_transaction(|store| {
            let mut rows = store.playlist_items(id)?;
            if from as usize >= rows.len() {
                return Err(StoreError::NotFound("playlist row".into()));
            }
            let row = rows.remove(from as usize);
            let to = (to as usize).min(rows.len());
            let r = RowRef {
                position: from,
                item_id: row.track_item_id.clone(),
                entry_id: row.entry_id.clone(),
            };
            rows.insert(to, row);
            store.set_playlist_items(id, &rows)?;
            self.record(
                "playlistMove",
                &edit_id,
                serde_json::json!({
                    "source": rec.source_id,
                    "playlist": rec.source_item_id,
                    "from": from,
                    "to": to,
                }),
                Some(&Inverse::MoveBack {
                    playlist: id,
                    from: to as u32,
                    to: from,
                }),
                Some((
                    rec.source_id,
                    Some(id),
                    vec![QueuedOp::Move {
                        playlist: id,
                        row: r,
                        to: to as u32,
                    }],
                )),
            )
            .map_err(as_store)
        })?;
        Ok(edit_id)
    }

    pub fn rename(&mut self, id: i64, name: &str) -> Result<String, EditError> {
        let rec = self
            .store
            .playlist_record(id)?
            .ok_or(EditError::NotFound("playlist"))?;
        if !rec.can_edit {
            return Err(EditError::Refused(Refusal::PlaylistReadOnly));
        }
        let name = name.trim();
        let name = if name.is_empty() {
            rec.name.as_str()
        } else {
            name
        };
        let edit_id = new_id();
        self.store.in_transaction(|store| {
            store.rename_playlist_row(id, name)?;
            self.record(
                "playlistRename",
                &edit_id,
                serde_json::json!({
                    "source": rec.source_id,
                    "playlist": rec.source_item_id,
                    "name": name,
                }),
                Some(&Inverse::Rename {
                    playlist: id,
                    name: rec.name.clone(),
                }),
                Some((
                    rec.source_id,
                    Some(id),
                    vec![QueuedOp::Rename {
                        playlist: id,
                        name: name.to_owned(),
                    }],
                )),
            )
            .map_err(as_store)
        })?;
        Ok(edit_id)
    }

    /// Remember how to undo a queue change (Up Next, Play Next): the queue keeps the
    /// state itself; this gives it an edit ID like every other edit.
    pub fn queue_edit(&mut self) -> Result<String, EditError> {
        let edit_id = new_id();
        self.store.save_undo(
            &edit_id,
            "queue",
            &serde_json::to_string(&Inverse::Queue).map_err(|e| StoreError::Edit(e.to_string()))?,
            self.now_utc_ms + self.undo_ms,
        )?;
        Ok(edit_id)
    }

    /// Undo an edit, within the Undo window. The undo is itself one event and one
    /// outbox entry; it can't be undone.
    pub fn undo(&mut self, edit_id: &str) -> Result<Undone, EditError> {
        let entry = self
            .store
            .undo_entry(edit_id)?
            .ok_or(EditError::NotFound("edit"))?;
        if entry.undone {
            return Err(EditError::AlreadyUndone);
        }
        if self.now_utc_ms > entry.expires_utc_ms {
            return Err(EditError::UndoExpired);
        }
        let inverse: Inverse = serde_json::from_str(&entry.inverse)
            .map_err(|e| StoreError::Edit(format!("undo entry: {e}")))?;
        let undo_id = new_id();
        let result = self.store.in_transaction(|store| {
            let done = self
                .apply_inverse(store, &inverse, &undo_id)
                .map_err(as_store)?;
            store.mark_undone(edit_id)?;
            Ok(done)
        })?;
        Ok(result)
    }

    fn apply_inverse(
        &mut self,
        store: &Store,
        inverse: &Inverse,
        undo_id: &str,
    ) -> Result<Undone, EditError> {
        let undo_event = |kind: &str| serde_json::json!({ "undoes": kind });
        match inverse {
            Inverse::Queue => return Ok(Undone::Queue),
            Inverse::RemoveEditRows { playlist, edit_id } => {
                let Some(rec) = store.playlist_record(*playlist)? else {
                    return Ok(Undone::Done);
                };
                let rows = store.playlist_items(*playlist)?;
                let removed: Vec<RowRef> = rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.edit_id.as_deref() == Some(edit_id))
                    .map(|(i, r)| RowRef {
                        position: i as u32,
                        item_id: r.track_item_id.clone(),
                        entry_id: r.entry_id.clone(),
                    })
                    .collect();
                let kept: Vec<PlaylistItemRow> = rows
                    .into_iter()
                    .filter(|r| r.edit_id.as_deref() != Some(edit_id))
                    .collect();
                store.set_playlist_items(*playlist, &kept)?;
                self.record(
                    "undo",
                    undo_id,
                    undo_event("playlistAdd"),
                    None,
                    Some((
                        rec.source_id,
                        Some(*playlist),
                        vec![QueuedOp::Remove {
                            playlist: *playlist,
                            rows: removed,
                        }],
                    )),
                )?;
            }
            Inverse::DeletePlaylist { playlist } => {
                let Some(rec) = store.playlist_record(*playlist)? else {
                    return Ok(Undone::Done);
                };
                let never_sent = rec.source_item_id.starts_with(LOCAL_ITEM_PREFIX);
                if never_sent {
                    // The server never saw it: forget its writes too.
                    store.cancel_pending_for_playlist(*playlist)?;
                }
                store.delete_playlist_row(*playlist)?;
                self.record(
                    "undo",
                    undo_id,
                    undo_event("playlistCreate"),
                    None,
                    (!never_sent).then(|| {
                        (
                            rec.source_id,
                            None,
                            vec![QueuedOp::Delete {
                                item_id: rec.source_item_id.clone(),
                            }],
                        )
                    }),
                )?;
            }
            Inverse::Reinsert { playlist, rows } => {
                let Some(rec) = store.playlist_record(*playlist)? else {
                    return Ok(Undone::Done);
                };
                let mut current = store.playlist_items(*playlist)?;
                let mut ops = Vec::new();
                for r in rows {
                    let at = (r.position as usize).min(current.len());
                    current.insert(
                        at,
                        PlaylistItemRow {
                            track_item_id: r.item_id.clone(),
                            entry_id: None,
                            edit_id: Some(undo_id.to_owned()),
                        },
                    );
                    ops.push(QueuedOp::Add {
                        playlist: *playlist,
                        item_ids: vec![r.item_id.clone()],
                        position: Some(at as u32),
                    });
                }
                store.set_playlist_items(*playlist, &current)?;
                self.record(
                    "undo",
                    undo_id,
                    undo_event("playlistRemove"),
                    None,
                    Some((rec.source_id, Some(*playlist), ops)),
                )?;
            }
            Inverse::MoveBack { playlist, from, to } => {
                let Some(rec) = store.playlist_record(*playlist)? else {
                    return Ok(Undone::Done);
                };
                let mut rows = store.playlist_items(*playlist)?;
                if (*from as usize) < rows.len() {
                    let row = rows.remove(*from as usize);
                    let r = RowRef {
                        position: *from,
                        item_id: row.track_item_id.clone(),
                        entry_id: row.entry_id.clone(),
                    };
                    let at = (*to as usize).min(rows.len());
                    rows.insert(at, row);
                    store.set_playlist_items(*playlist, &rows)?;
                    self.record(
                        "undo",
                        undo_id,
                        undo_event("playlistMove"),
                        None,
                        Some((
                            rec.source_id,
                            Some(*playlist),
                            vec![QueuedOp::Move {
                                playlist: *playlist,
                                row: r,
                                to: at as u32,
                            }],
                        )),
                    )?;
                }
            }
            Inverse::Rename { playlist, name } => {
                let Some(rec) = store.playlist_record(*playlist)? else {
                    return Ok(Undone::Done);
                };
                store.rename_playlist_row(*playlist, name)?;
                self.record(
                    "undo",
                    undo_id,
                    undo_event("playlistRename"),
                    None,
                    Some((
                        rec.source_id,
                        Some(*playlist),
                        vec![QueuedOp::Rename {
                            playlist: *playlist,
                            name: name.clone(),
                        }],
                    )),
                )?;
            }
            Inverse::Unlike { keys } => {
                let items = store.items_for_keys(keys)?;
                for k in keys {
                    store.unlike(k)?;
                }
                let pairs: Vec<(i64, String)> =
                    items.iter().map(|(s, i, _)| (*s, i.clone())).collect();
                for (src, item) in &pairs {
                    store.set_favorite(*src, item, false)?;
                }
                self.record("undo", undo_id, undo_event("like"), None, None)?;
                queue_favorites(store, undo_id, &pairs, false)?;
            }
            Inverse::Like { keys, items } => {
                for k in keys {
                    store.like(k, self.now_utc_ms)?;
                }
                for (src, item) in items {
                    store.set_favorite(*src, item, true)?;
                }
                self.record("undo", undo_id, undo_event("unlike"), None, None)?;
                queue_favorites(store, undo_id, items, true)?;
            }
        }
        Ok(Undone::Done)
    }
}

fn as_store(e: EditError) -> StoreError {
    match e {
        EditError::Store(s) => s,
        other => StoreError::Edit(other.to_string()),
    }
}

/// One outbox entry per server for favourite changes.
fn queue_favorites(
    store: &Store,
    edit_id: &str,
    items: &[(i64, String)],
    favorite: bool,
) -> Result<(), StoreError> {
    let mut sources: Vec<i64> = items.iter().map(|(s, _)| *s).collect();
    sources.sort_unstable();
    sources.dedup();
    for src in sources {
        let ids: Vec<String> = items
            .iter()
            .filter(|(s, _)| *s == src)
            .map(|(_, i)| i.clone())
            .collect();
        let json = serde_json::to_string(&vec![QueuedOp::Favorite {
            item_ids: ids,
            favorite,
        }])
        .map_err(|e| StoreError::Edit(e.to_string()))?;
        store.enqueue(src, edit_id, None, &json)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- sending

/// What one `flush` did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flushed {
    pub sent: u32,
    pub refused: u32,
    /// Still waiting (the server couldn't be reached).
    pub waiting: u32,
}

/// Whether the server said no (drop the write) rather than "not now" (keep it).
fn refused(e: &SourceError) -> bool {
    match e {
        SourceError::NotFound | SourceError::Protocol(_) => true,
        SourceError::Server { status, .. } => {
            (400..500).contains(status) && !matches!(status, 401 | 403 | 408 | 429)
        }
        _ => false,
    }
}

/// The server's entry ID for each row: the one recorded, else the entry at the
/// same position if it is the same song, else the first unused entry of the song.
fn resolve_entries(rows: &[RowRef], entries: &[PlaylistEntry]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for r in rows {
        let id = r.entry_id.clone().or_else(|| {
            entries
                .get(r.position as usize)
                .filter(|e| e.track_item_id == r.item_id)
                .and_then(|e| e.entry_id.clone())
                .filter(|id| !used.contains(id))
                .or_else(|| {
                    entries
                        .iter()
                        .filter(|e| e.track_item_id == r.item_id)
                        .filter_map(|e| e.entry_id.clone())
                        .find(|id| !used.contains(id))
                })
        });
        if let Some(id) = id {
            used.insert(id.clone());
            out.push(id);
        }
    }
    out
}

fn lock(store: &Mutex<Store>) -> Result<std::sync::MutexGuard<'_, Store>, EditError> {
    store.lock().map_err(|_| EditError::Poisoned)
}

/// After a playlist's writes are all accepted, take the server's rows (with their
/// entry IDs), keeping which edit put each row there where the song matches.
async fn refresh_playlist(
    store: &Mutex<Store>,
    source: &dyn MediaSource,
    playlist: i64,
) -> Result<(), EditError> {
    let item_id = {
        let s = lock(store)?;
        if s.has_pending_for_playlist(playlist)? {
            return Ok(());
        }
        match s.playlist_record(playlist)? {
            Some(rec) if !rec.source_item_id.starts_with(LOCAL_ITEM_PREFIX) => rec.source_item_id,
            _ => return Ok(()),
        }
    };
    let Ok(entries) = source.playlist_entries(&item_id).await else {
        return Ok(()); // the next sync brings them
    };
    let s = lock(store)?;
    if s.has_pending_for_playlist(playlist)? {
        return Ok(());
    }
    let old = s.playlist_items(playlist)?;
    let rows: Vec<PlaylistItemRow> = entries
        .into_iter()
        .enumerate()
        .map(|(i, e)| PlaylistItemRow {
            edit_id: old
                .get(i)
                .filter(|o| o.track_item_id == e.track_item_id)
                .and_then(|o| o.edit_id.clone()),
            track_item_id: e.track_item_id,
            entry_id: e.entry_id,
        })
        .collect();
    s.set_playlist_items(playlist, &rows)?;
    Ok(())
}

/// Send the waiting writes for one server, in order. Stops at the first write the
/// server can't take now (so order is kept); drops writes it refuses.
pub async fn flush(
    store: &Mutex<Store>,
    source_id: i64,
    source: &dyn MediaSource,
    now_utc_ms: i64,
) -> Result<Flushed, EditError> {
    let mut report = Flushed::default();
    let pending = lock(store)?.pending_outbox(source_id)?;
    let mut touched: Vec<i64> = Vec::new();
    for (index, row) in pending.iter().enumerate() {
        let mut ops: Vec<QueuedOp> = match serde_json::from_str(&row.op) {
            Ok(ops) => ops,
            Err(e) => {
                lock(store)?.outbox_refused(row.seq, &format!("unreadable: {e}"))?;
                report.refused += 1;
                continue;
            }
        };
        let mut outcome: Result<(), SourceError> = Ok(());
        while !ops.is_empty() {
            match send_one(store, source, &mut ops[0]).await {
                Ok(SendStep::Done) => {
                    if let Some(p) = ops[0].playlist() {
                        touched.push(p);
                    }
                    ops.remove(0);
                    // Remember what is left, so a retry never sends twice.
                    let left = serde_json::to_string(&ops).unwrap_or_else(|_| "[]".into());
                    lock(store)?.update_outbox_op(row.seq, &left)?;
                }
                Ok(SendStep::Partial) => {
                    let left = serde_json::to_string(&ops).unwrap_or_else(|_| "[]".into());
                    lock(store)?.update_outbox_op(row.seq, &left)?;
                }
                Err(e) => {
                    outcome = Err(e);
                    break;
                }
            }
        }
        match outcome {
            Ok(()) => {
                lock(store)?.outbox_done(row.seq)?;
                report.sent += 1;
            }
            Err(e) if refused(&e) => {
                lock(store)?.outbox_refused(row.seq, &e.to_string())?;
                report.refused += 1;
            }
            Err(e) => {
                let backoff = 5_000i64 << row.attempts.min(10);
                lock(store)?.outbox_retry(row.seq, &e.to_string(), now_utc_ms + backoff)?;
                report.waiting = (pending.len() - index) as u32;
                break;
            }
        }
    }
    touched.sort_unstable();
    touched.dedup();
    for p in touched {
        refresh_playlist(store, source, p).await?;
    }
    Ok(report)
}

enum SendStep {
    /// The op is fully done.
    Done,
    /// Part of it was sent (favourites go one song at a time); it changed in place.
    Partial,
}

/// Send one queued write. Favourites are sent one song at a time, and the op
/// shrinks as they go, so a retry continues where it stopped.
async fn send_one(
    store: &Mutex<Store>,
    source: &dyn MediaSource,
    op: &mut QueuedOp,
) -> Result<SendStep, SourceError> {
    let store_err = |e: EditError| SourceError::Protocol(e.to_string());
    // The playlist's server ID now (it may have been created since the op was queued).
    let server_id = |playlist: i64| -> Result<Option<String>, SourceError> {
        let s = lock(store).map_err(store_err)?;
        Ok(s.playlist_record(playlist)
            .map_err(|e| SourceError::Protocol(e.to_string()))?
            .map(|r| r.source_item_id))
    };
    let not_created = || SourceError::Protocol("the playlist was never made on the server".into());
    match op {
        QueuedOp::Create {
            playlist,
            name,
            item_ids,
        } => {
            let Some(current) = server_id(*playlist)? else {
                return Ok(SendStep::Done); // removed here before it was sent
            };
            if !current.starts_with(LOCAL_ITEM_PREFIX) {
                return Ok(SendStep::Done); // already made
            }
            let outcome = source
                .edit(&ServerEdit::CreatePlaylist {
                    name: name.clone(),
                    item_ids: item_ids.clone(),
                })
                .await?;
            if let EditOutcome::Created(id) = outcome {
                lock(store)
                    .map_err(store_err)?
                    .set_playlist_item_id(*playlist, &id)
                    .map_err(|e| SourceError::Protocol(e.to_string()))?;
            }
            Ok(SendStep::Done)
        }
        QueuedOp::Add {
            playlist,
            item_ids,
            position,
        } => {
            let Some(id) = server_id(*playlist)? else {
                return Ok(SendStep::Done);
            };
            if id.starts_with(LOCAL_ITEM_PREFIX) {
                return Err(not_created());
            }
            source
                .edit(&ServerEdit::AddToPlaylist {
                    playlist_id: id,
                    item_ids: item_ids.clone(),
                    position: *position,
                })
                .await?;
            Ok(SendStep::Done)
        }
        QueuedOp::Remove { playlist, rows } => {
            let Some(id) = server_id(*playlist)? else {
                return Ok(SendStep::Done);
            };
            if id.starts_with(LOCAL_ITEM_PREFIX) {
                return Err(not_created());
            }
            let entries = if rows.iter().any(|r| r.entry_id.is_none()) {
                source.playlist_entries(&id).await?
            } else {
                Vec::new()
            };
            let entry_ids = resolve_entries(rows, &entries);
            if !entry_ids.is_empty() {
                source
                    .edit(&ServerEdit::RemoveFromPlaylist {
                        playlist_id: id,
                        entry_ids,
                    })
                    .await?;
            }
            Ok(SendStep::Done)
        }
        QueuedOp::Move { playlist, row, to } => {
            let Some(id) = server_id(*playlist)? else {
                return Ok(SendStep::Done);
            };
            if id.starts_with(LOCAL_ITEM_PREFIX) {
                return Err(not_created());
            }
            let entries = if row.entry_id.is_none() {
                source.playlist_entries(&id).await?
            } else {
                Vec::new()
            };
            if let Some(entry_id) = resolve_entries(std::slice::from_ref(row), &entries).pop() {
                source
                    .edit(&ServerEdit::MoveInPlaylist {
                        playlist_id: id,
                        entry_id,
                        new_index: *to,
                    })
                    .await?;
            }
            Ok(SendStep::Done)
        }
        QueuedOp::Rename { playlist, name } => {
            let Some(id) = server_id(*playlist)? else {
                return Ok(SendStep::Done);
            };
            if id.starts_with(LOCAL_ITEM_PREFIX) {
                return Err(not_created());
            }
            source
                .edit(&ServerEdit::RenamePlaylist {
                    playlist_id: id,
                    name: name.clone(),
                })
                .await?;
            Ok(SendStep::Done)
        }
        QueuedOp::Delete { item_id } => {
            if !item_id.starts_with(LOCAL_ITEM_PREFIX) {
                source
                    .edit(&ServerEdit::DeletePlaylist {
                        playlist_id: item_id.clone(),
                    })
                    .await?;
            }
            Ok(SendStep::Done)
        }
        QueuedOp::Favorite { item_ids, favorite } => {
            let Some(first) = item_ids.first().cloned() else {
                return Ok(SendStep::Done);
            };
            source
                .edit(&ServerEdit::SetFavorite {
                    item_id: first,
                    favorite: *favorite,
                })
                .await?;
            item_ids.remove(0);
            Ok(if item_ids.is_empty() {
                SendStep::Done
            } else {
                SendStep::Partial
            })
        }
    }
}

/// Song keys with a favourite write still waiting: a sync leaves their likes alone.
pub fn pending_favorite_keys(store: &Store, source_id: i64) -> Result<HashSet<String>, EditError> {
    let mut items: Vec<String> = Vec::new();
    for row in store.pending_outbox(source_id)? {
        if let Ok(ops) = serde_json::from_str::<Vec<QueuedOp>>(&row.op) {
            for op in ops {
                if let QueuedOp::Favorite { item_ids, .. } = op {
                    items.extend(item_ids);
                }
            }
        }
    }
    let mut keys = HashSet::new();
    for item in items {
        if let Some(t) = store.track_by_item(source_id, &item)? {
            keys.insert(t.song_key);
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests;
