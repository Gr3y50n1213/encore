//! Storage for library edits (plan sections 5 and 26): playlist rows, likes, the
//! event log, the outbox of server writes and undo entries. The rules (what one
//! call writes, when undo is allowed) live in encore-core.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store, TrackRow};

/// One row of a playlist as this device has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistItemRow {
    pub track_item_id: String,
    /// The server's ID for this entry; None until the server has it.
    pub entry_id: Option<String>,
    /// The edit on this device that put it there, if any.
    pub edit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistRecord {
    pub id: i64,
    pub source_id: i64,
    pub source_item_id: String,
    pub name: String,
    pub can_edit: bool,
}

/// One user action, for the event log Phase 4 syncs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub id: String,
    pub device_id: String,
    pub hlc: String,
    pub kind: String,
    pub payload: String,
    pub created_utc_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRow {
    pub seq: i64,
    pub source_id: i64,
    pub edit_id: String,
    pub playlist_id: Option<i64>,
    pub op: String,
    pub attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoRow {
    pub edit_id: String,
    pub kind: String,
    pub inverse: String,
    pub expires_utc_ms: i64,
    pub undone: bool,
}

impl Store {
    // ------------------------------------------------------------ playlists

    pub fn playlist_record(&self, id: i64) -> Result<Option<PlaylistRecord>> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT id, source_id, source_item_id, name, can_edit FROM playlists WHERE id = ?1",
            )?
            .query_row([id], |r| {
                Ok(PlaylistRecord {
                    id: r.get(0)?,
                    source_id: r.get(1)?,
                    source_item_id: r.get(2)?,
                    name: r.get(3)?,
                    can_edit: r.get(4)?,
                })
            })
            .optional()?)
    }

    pub fn playlist_items(&self, playlist_id: i64) -> Result<Vec<PlaylistItemRow>> {
        let mut st = self.conn.prepare_cached(
            "SELECT track_item_id, entry_id, edit_id FROM playlist_items
             WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let rows = st.query_map([playlist_id], |r| {
            Ok(PlaylistItemRow {
                track_item_id: r.get(0)?,
                entry_id: r.get(1)?,
                edit_id: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Replace a playlist's rows, in this order.
    pub fn set_playlist_items(&self, playlist_id: i64, rows: &[PlaylistItemRow]) -> Result<()> {
        self.in_transaction(|s| {
            s.conn
                .execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [playlist_id])?;
            let mut ins = s.conn.prepare_cached(
                "INSERT INTO playlist_items (playlist_id, position, track_item_id, entry_id, edit_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (i, r) in rows.iter().enumerate() {
                ins.execute(params![playlist_id, i as i64, r.track_item_id, r.entry_id, r.edit_id])?;
            }
            Ok(())
        })
    }

    /// A playlist made on this device; `item_id` is its local ID until the server
    /// gives it one (`LOCAL_ITEM_PREFIX`).
    pub fn create_playlist_row(
        &self,
        source_id: i64,
        item_id: &str,
        name: &str,
        now_utc_ms: i64,
    ) -> Result<i64> {
        let id: i64 = self.conn.query_row(
            "INSERT INTO playlists (source_id, source_item_id, name, search_name, date_added_utc_ms)
             VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
            params![source_id, item_id, name, crate::fold_for_search(name), now_utc_ms],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO search_index (rowid, text) VALUES (?1, ?2)",
            params![
                crate::mirror::index_rowid(id, crate::mirror::KIND_PLAYLIST),
                name
            ],
        )?;
        Ok(id)
    }

    /// The server gave a playlist made here its own ID.
    pub fn set_playlist_item_id(&self, id: i64, item_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE playlists SET source_item_id = ?2 WHERE id = ?1",
            params![id, item_id],
        )?;
        Ok(())
    }

    pub fn rename_playlist_row(&self, id: i64, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE playlists SET name = ?2, search_name = ?3 WHERE id = ?1",
            params![id, name, crate::fold_for_search(name)],
        )?;
        let rowid = crate::mirror::index_rowid(id, crate::mirror::KIND_PLAYLIST);
        self.conn
            .execute("DELETE FROM search_index WHERE rowid = ?1", [rowid])?;
        self.conn.execute(
            "INSERT INTO search_index (rowid, text) VALUES (?1, ?2)",
            params![rowid, name],
        )?;
        Ok(())
    }

    pub fn delete_playlist_row(&self, id: i64) -> Result<()> {
        let rowid = crate::mirror::index_rowid(id, crate::mirror::KIND_PLAYLIST);
        self.conn
            .execute("DELETE FROM search_index WHERE rowid = ?1", [rowid])?;
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// The song keys of a playlist's songs that are in the library.
    pub fn playlist_song_keys(&self, playlist_id: i64) -> Result<HashSet<String>> {
        let mut st = self.conn.prepare_cached(
            "SELECT t.song_key FROM playlist_items i JOIN playlists p ON p.id = i.playlist_id
             JOIN tracks t ON t.source_id = p.source_id AND t.source_item_id = i.track_item_id
             WHERE i.playlist_id = ?1",
        )?;
        let keys = st.query_map([playlist_id], |r| r.get(0))?;
        Ok(keys.collect::<rusqlite::Result<_>>()?)
    }

    // ------------------------------------------------------------ likes

    /// Returns true when it wasn't liked before.
    pub fn like(&self, song_key: &str, now_utc_ms: i64) -> Result<bool> {
        Ok(self.conn.execute(
            "INSERT OR IGNORE INTO likes (song_key, liked_utc_ms) VALUES (?1, ?2)",
            params![song_key, now_utc_ms],
        )? == 1)
    }

    /// Returns true when it was liked.
    pub fn unlike(&self, song_key: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM likes WHERE song_key = ?1", [song_key])?
            == 1)
    }

    pub fn liked_keys(&self) -> Result<HashSet<String>> {
        let mut st = self.conn.prepare_cached("SELECT song_key FROM likes")?;
        let keys = st.query_map([], |r| r.get(0))?;
        Ok(keys.collect::<rusqlite::Result<_>>()?)
    }

    /// Liked songs, newest like first; a song on several albums appears once.
    pub fn liked_songs(&self, offset: u32, limit: u32) -> Result<Vec<TrackRow>> {
        let ids: Vec<i64> = {
            let mut st = self.conn.prepare_cached(
                "SELECT (SELECT min(t.id) FROM tracks t WHERE t.song_key = l.song_key)
                 FROM likes l
                 WHERE EXISTS (SELECT 1 FROM tracks t WHERE t.song_key = l.song_key)
                 ORDER BY l.liked_utc_ms DESC, l.song_key LIMIT ?1 OFFSET ?2",
            )?;
            let rows = st.query_map(params![limit, offset], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        ids.into_iter().map(|id| self.track(id)).collect()
    }

    /// Every mirrored song with one of these keys, as (source, item ID, favourite).
    pub fn items_for_keys(&self, keys: &[String]) -> Result<Vec<(i64, String, bool)>> {
        let mut out = Vec::new();
        let mut st = self.conn.prepare_cached(
            "SELECT source_id, source_item_id, favorite FROM tracks WHERE song_key = ?1 ORDER BY id",
        )?;
        for k in keys {
            let rows = st.query_map([k], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// Make likes match the server's favourites after a sync: a favourite set
    /// elsewhere becomes a like, a favourite cleared elsewhere clears the like.
    /// Songs with a write still waiting (`pending`) are left alone, and so are
    /// likes of songs this source doesn't have.
    pub fn reconcile_likes(
        &self,
        source_id: i64,
        pending: &HashSet<String>,
        now_utc_ms: i64,
    ) -> Result<()> {
        self.in_transaction(|s| s.reconcile_likes_in(source_id, pending, now_utc_ms))
    }

    fn reconcile_likes_in(
        &self,
        source_id: i64,
        pending: &HashSet<String>,
        now_utc_ms: i64,
    ) -> Result<()> {
        let tx = &self.conn;
        let fav: Vec<String> = {
            let mut st = tx.prepare(
                "SELECT DISTINCT song_key FROM tracks WHERE source_id = ?1 AND favorite = 1",
            )?;
            let rows = st.query_map([source_id], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for k in &fav {
            if !pending.contains(k) {
                tx.execute(
                    "INSERT OR IGNORE INTO likes (song_key, liked_utc_ms) VALUES (?1, ?2)",
                    params![k, now_utc_ms],
                )?;
            }
        }
        let gone: Vec<String> = {
            let mut st = tx.prepare(
                "SELECT l.song_key FROM likes l
                 WHERE EXISTS (SELECT 1 FROM tracks t WHERE t.song_key = l.song_key AND t.source_id = ?1)
                   AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.song_key = l.song_key
                                   AND t.source_id = ?1 AND t.favorite = 1)",
            )?;
            let rows = st.query_map([source_id], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for k in &gone {
            if !pending.contains(k) {
                tx.execute("DELETE FROM likes WHERE song_key = ?1", [k])?;
            }
        }
        Ok(())
    }

    /// Mark one song as a favourite or not in the mirror (after a write was sent).
    pub fn set_favorite(&self, source_id: i64, item_id: &str, favorite: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET favorite = ?3 WHERE source_id = ?1 AND source_item_id = ?2",
            params![source_id, item_id, favorite],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------ events

    pub fn insert_event(&self, e: &EventRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events (id, device_id, hlc, kind, payload, created_utc_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                e.id,
                e.device_id,
                e.hlc,
                e.kind,
                e.payload,
                e.created_utc_ms
            ],
        )?;
        // Who last decided a song's like, for merging with other devices.
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&e.payload)
            && let Some((keys, liked)) = crate::sync::like_effect(&e.kind, &payload)
        {
            self.note_like_decision(&keys, liked, &e.hlc, &e.device_id, true)?;
        }
        Ok(())
    }

    /// Events not yet synced, oldest first.
    pub fn unsynced_events(&self) -> Result<Vec<EventRecord>> {
        let mut st = self.conn.prepare(
            "SELECT id, device_id, hlc, kind, payload, created_utc_ms FROM events
             WHERE synced = 0 ORDER BY hlc",
        )?;
        let rows = st.query_map([], |r| {
            Ok(EventRecord {
                id: r.get(0)?,
                device_id: r.get(1)?,
                hlc: r.get(2)?,
                kind: r.get(3)?,
                payload: r.get(4)?,
                created_utc_ms: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The newest clock value in the event log, so a restarted clock never goes back.
    pub fn newest_event_hlc(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT max(hlc) FROM events", [], |r| r.get(0))?)
    }

    // ------------------------------------------------------------ outbox

    pub fn enqueue(
        &self,
        source_id: i64,
        edit_id: &str,
        playlist_id: Option<i64>,
        op: &str,
    ) -> Result<i64> {
        Ok(self.conn.query_row(
            "INSERT INTO outbox (source_id, edit_id, playlist_id, op) VALUES (?1, ?2, ?3, ?4)
             RETURNING seq",
            params![source_id, edit_id, playlist_id, op],
            |r| r.get(0),
        )?)
    }

    /// The writes waiting for one source, in order.
    pub fn pending_outbox(&self, source_id: i64) -> Result<Vec<OutboxRow>> {
        let mut st = self.conn.prepare_cached(
            "SELECT seq, source_id, edit_id, playlist_id, op, attempts FROM outbox
             WHERE source_id = ?1 AND state = 'pending' ORDER BY seq",
        )?;
        let rows = st.query_map([source_id], |r| {
            Ok(OutboxRow {
                seq: r.get(0)?,
                source_id: r.get(1)?,
                edit_id: r.get(2)?,
                playlist_id: r.get(3)?,
                op: r.get(4)?,
                attempts: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn pending_outbox_count(&self) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM outbox WHERE state = 'pending'",
            [],
            |r| r.get(0),
        )?)
    }

    /// Drop the waiting writes for a playlist the server never saw (its creation
    /// was undone before it was sent).
    pub fn cancel_pending_for_playlist(&self, playlist_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE outbox SET state = 'cancelled' WHERE playlist_id = ?1 AND state = 'pending'",
            [playlist_id],
        )?;
        Ok(())
    }

    pub fn has_pending_for_playlist(&self, playlist_id: i64) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM outbox WHERE playlist_id = ?1 AND state = 'pending')",
            [playlist_id],
            |r| r.get(0),
        )?)
    }

    /// Rewrite a waiting write (its playlist got its server ID, say).
    pub fn update_outbox_op(&self, seq: i64, op: &str) -> Result<()> {
        self.conn
            .execute("UPDATE outbox SET op = ?2 WHERE seq = ?1", params![seq, op])?;
        Ok(())
    }

    pub fn outbox_done(&self, seq: i64) -> Result<()> {
        self.conn
            .execute("UPDATE outbox SET state = 'done' WHERE seq = ?1", [seq])?;
        Ok(())
    }

    /// Keep it for later (the server couldn't be reached).
    pub fn outbox_retry(&self, seq: i64, error: &str, next_try_utc_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE outbox SET attempts = attempts + 1, last_error = ?2, next_try_utc_ms = ?3
             WHERE seq = ?1",
            params![seq, error, next_try_utc_ms],
        )?;
        Ok(())
    }

    /// The server refused it: stop trying; the next sync brings the server's version.
    pub fn outbox_refused(&self, seq: i64, error: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE outbox SET state = 'refused', attempts = attempts + 1, last_error = ?2
             WHERE seq = ?1",
            params![seq, error],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------ undo

    pub fn save_undo(
        &self,
        edit_id: &str,
        kind: &str,
        inverse: &str,
        expires_utc_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO undo_entries (edit_id, kind, inverse, expires_utc_ms) VALUES (?1, ?2, ?3, ?4)",
            params![edit_id, kind, inverse, expires_utc_ms],
        )?;
        Ok(())
    }

    pub fn undo_entry(&self, edit_id: &str) -> Result<Option<UndoRow>> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT edit_id, kind, inverse, expires_utc_ms, undone FROM undo_entries WHERE edit_id = ?1",
            )?
            .query_row([edit_id], |r| {
                Ok(UndoRow {
                    edit_id: r.get(0)?,
                    kind: r.get(1)?,
                    inverse: r.get(2)?,
                    expires_utc_ms: r.get(3)?,
                    undone: r.get(4)?,
                })
            })
            .optional()?)
    }

    pub fn undo_entries_count(&self) -> Result<u32> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM undo_entries", [], |r| r.get(0))?)
    }

    pub fn mark_undone(&self, edit_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE undo_entries SET undone = 1 WHERE edit_id = ?1",
            [edit_id],
        )?;
        Ok(())
    }

    /// Forget undo entries that expired before `before_utc_ms`.
    pub fn prune_undo(&self, before_utc_ms: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM undo_entries WHERE expires_utc_ms < ?1",
            [before_utc_ms],
        )?;
        Ok(())
    }

    /// Run several writes as one: all of them or none.
    pub fn in_transaction<T>(&self, f: impl FnOnce(&Store) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("SAVEPOINT edit")?;
        match f(self) {
            Ok(v) => {
                self.conn.execute_batch("RELEASE edit")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK TO edit; RELEASE edit");
                Err(e)
            }
        }
    }
}
