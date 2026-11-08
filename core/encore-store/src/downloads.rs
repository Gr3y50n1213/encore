//! Downloads (Phase 8; docs/adr/core-0017): what a profile asked to keep, what
//! each of those holds now, and each song's state. Migration 8 explains the tables.
//! The rules (what to fetch next, when to let a file go) are encore-core's; this
//! file only reads and writes rows.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// One thing this profile asked to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadCollectionRow {
    /// "song", "album", "playlist", "mix", "liked" or "smart".
    pub kind: String,
    /// The item's ID as text ('' for Liked songs and Smart downloads).
    pub item: String,
    pub title: String,
    pub added_utc_ms: i64,
    /// Kept by the app itself (Liked songs, Smart downloads), not asked for.
    pub auto: bool,
    pub paused: bool,
}

/// A wanted song and where its download stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadSongRow {
    pub track_id: i64,
    pub file_key: String,
    /// "queued", "saved" or "failed".
    pub state: String,
    pub bytes: i64,
    pub attempts: u32,
    pub error: Option<String>,
    pub queued_utc_ms: i64,
    pub saved_utc_ms: Option<i64>,
}

/// How far one collection has got.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadCounts {
    pub total: u32,
    pub saved: u32,
    pub failed: u32,
    pub queued: u32,
    pub bytes: i64,
}

impl Store {
    /// Add a collection, or rename one already kept. True when it is new.
    pub fn upsert_download_collection(
        &self,
        kind: &str,
        item: &str,
        title: &str,
        auto: bool,
        now_utc_ms: i64,
    ) -> Result<bool> {
        let added = self.conn.execute(
            "INSERT OR IGNORE INTO download_collections (kind, item, title, added_utc_ms, auto)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind, item, title, now_utc_ms, auto],
        )?;
        if added == 0 {
            self.conn.execute(
                "UPDATE download_collections SET title = ?3 WHERE kind = ?1 AND item = ?2",
                params![kind, item, title],
            )?;
        }
        Ok(added > 0)
    }

    /// Forget a collection and its members (its songs are let go by the caller).
    pub fn remove_download_collection(&self, kind: &str, item: &str) -> Result<bool> {
        Ok(self.conn.execute(
            "DELETE FROM download_collections WHERE kind = ?1 AND item = ?2",
            params![kind, item],
        )? > 0)
    }

    pub fn download_collection(
        &self,
        kind: &str,
        item: &str,
    ) -> Result<Option<DownloadCollectionRow>> {
        Ok(self
            .conn
            .query_row(
                &format!("{COLLECTION_SELECT} WHERE kind = ?1 AND item = ?2"),
                params![kind, item],
                collection_row,
            )
            .optional()?)
    }

    /// Every collection, oldest first.
    pub fn download_collections(&self) -> Result<Vec<DownloadCollectionRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            "{COLLECTION_SELECT} ORDER BY added_utc_ms, kind, item"
        ))?;
        let rows = st.query_map([], collection_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_download_paused(&self, kind: &str, item: &str, paused: bool) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE download_collections SET paused = ?3 WHERE kind = ?1 AND item = ?2",
            params![kind, item, paused],
        )? > 0)
    }

    /// Replace what a collection holds, in its order.
    pub fn set_download_members(&self, kind: &str, item: &str, track_ids: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM download_members WHERE kind = ?1 AND item = ?2",
            params![kind, item],
        )?;
        {
            let mut st = tx.prepare_cached(
                "INSERT OR IGNORE INTO download_members (kind, item, track_id, position)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (i, id) in track_ids.iter().enumerate() {
                st.execute(params![kind, item, id, i as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// A collection's songs, in its order.
    pub fn download_members(&self, kind: &str, item: &str) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare_cached(
            "SELECT track_id FROM download_members WHERE kind = ?1 AND item = ?2 ORDER BY position",
        )?;
        let rows = st.query_map(params![kind, item], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every song some collection holds.
    pub fn download_wanted(&self) -> Result<HashSet<i64>> {
        let mut st = self
            .conn
            .prepare_cached("SELECT DISTINCT track_id FROM download_members")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn download_songs(&self) -> Result<Vec<DownloadSongRow>> {
        let mut st = self
            .conn
            .prepare_cached(&format!("{SONG_SELECT} ORDER BY queued_utc_ms, track_id"))?;
        let rows = st.query_map([], song_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn download_song(&self, track_id: i64) -> Result<Option<DownloadSongRow>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SONG_SELECT} WHERE track_id = ?1"),
                [track_id],
                song_row,
            )
            .optional()?)
    }

    /// The songs that are saved on this device.
    pub fn saved_download_ids(&self) -> Result<HashSet<i64>> {
        let mut st = self
            .conn
            .prepare_cached("SELECT track_id FROM download_songs WHERE state = 'saved'")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Want a song (nothing when it already is).
    pub fn queue_download_song(
        &self,
        track_id: i64,
        file_key: &str,
        now_utc_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO download_songs (track_id, file_key, state, queued_utc_ms)
             VALUES (?1, ?2, 'queued', ?3)",
            params![track_id, file_key, now_utc_ms],
        )?;
        Ok(())
    }

    pub fn set_download_saved(&self, track_id: i64, bytes: i64, now_utc_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE download_songs SET state = 'saved', bytes = ?2, error = NULL, saved_utc_ms = ?3
             WHERE track_id = ?1",
            params![track_id, bytes, now_utc_ms],
        )?;
        Ok(())
    }

    /// A try failed: queued again until `give_up`, then failed with the reason.
    pub fn set_download_attempt_failed(
        &self,
        track_id: i64,
        error: &str,
        give_up: bool,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE download_songs SET attempts = attempts + 1, error = ?2,
                 state = CASE WHEN ?3 THEN 'failed' ELSE 'queued' END
             WHERE track_id = ?1",
            params![track_id, error, give_up],
        )?;
        Ok(())
    }

    /// A saved file went missing: fetch it again.
    pub fn requeue_download(&self, track_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE download_songs SET state = 'queued', bytes = 0, attempts = 0, error = NULL,
                 saved_utc_ms = NULL
             WHERE track_id = ?1",
            [track_id],
        )?;
        Ok(())
    }

    /// Try the failed ones again from the start.
    pub fn retry_failed_downloads(&self) -> Result<u32> {
        Ok(self.conn.execute(
            "UPDATE download_songs SET state = 'queued', attempts = 0, error = NULL
             WHERE state = 'failed'",
            [],
        )? as u32)
    }

    pub fn remove_download_song(&self, track_id: i64) -> Result<Option<DownloadSongRow>> {
        let row = self.download_song(track_id)?;
        self.conn
            .execute("DELETE FROM download_songs WHERE track_id = ?1", [track_id])?;
        Ok(row)
    }

    /// The queued songs to fetch, in the order they were asked for: the oldest
    /// collection first, each in its own order. Songs only in paused collections
    /// wait.
    pub fn download_candidates(&self, limit: u32) -> Result<Vec<(i64, String)>> {
        let mut st = self.conn.prepare_cached(
            "SELECT s.track_id, s.file_key FROM download_songs s
             JOIN download_members m ON m.track_id = s.track_id
             JOIN download_collections c ON c.kind = m.kind AND c.item = m.item
             WHERE s.state = 'queued' AND c.paused = 0
             GROUP BY s.track_id
             -- The earliest collection's own position: (added, position) as one number.
             ORDER BY min(c.added_utc_ms * 1000000 + m.position), s.track_id
             LIMIT ?1",
        )?;
        let rows = st.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The songs waiting to be fetched (paused collections left out).
    pub fn download_queued_count(&self) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(DISTINCT s.track_id) FROM download_songs s
             JOIN download_members m ON m.track_id = s.track_id
             JOIN download_collections c ON c.kind = m.kind AND c.item = m.item
             WHERE s.state = 'queued' AND c.paused = 0",
            [],
            |r| r.get(0),
        )?)
    }

    /// How far each collection has got, keyed by (kind, item).
    pub fn download_counts(&self) -> Result<Vec<((String, String), DownloadCounts)>> {
        let mut st = self.conn.prepare_cached(
            "SELECT m.kind, m.item, count(*),
                    sum(s.state = 'saved'), sum(s.state = 'failed'), sum(s.state = 'queued'),
                    coalesce(sum(CASE WHEN s.state = 'saved' THEN s.bytes END), 0)
             FROM download_members m LEFT JOIN download_songs s ON s.track_id = m.track_id
             GROUP BY m.kind, m.item",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                (r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                DownloadCounts {
                    total: r.get(2)?,
                    saved: r.get::<_, Option<u32>>(3)?.unwrap_or(0),
                    failed: r.get::<_, Option<u32>>(4)?.unwrap_or(0),
                    queued: r.get::<_, Option<u32>>(5)?.unwrap_or(0),
                    bytes: r.get(6)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Bytes this profile's saved songs and covers take (a file shared with
    /// another profile counts in full for each).
    pub fn download_bytes(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT coalesce((SELECT sum(bytes) FROM download_songs WHERE state = 'saved'), 0)
                  + coalesce((SELECT sum(bytes) FROM download_covers), 0)",
            [],
            |r| r.get(0),
        )?)
    }

    /// The last time any song of a collection was played (UTC ms), if ever.
    pub fn download_last_played(&self, kind: &str, item: &str) -> Result<Option<i64>> {
        Ok(self.conn.query_row(
            "SELECT max(p.started_utc_ms) FROM download_members m
             JOIN tracks t ON t.id = m.track_id
             JOIN play_events p ON p.song_key = t.song_key
             WHERE m.kind = ?1 AND m.item = ?2",
            params![kind, item],
            |r| r.get(0),
        )?)
    }

    pub fn download_cover(&self, album_id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT file_key FROM download_covers WHERE album_id = ?1",
                [album_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_download_cover(&self, album_id: i64, file_key: &str, bytes: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO download_covers (album_id, file_key, bytes) VALUES (?1, ?2, ?3)
             ON CONFLICT (album_id) DO UPDATE SET file_key = excluded.file_key, bytes = excluded.bytes",
            params![album_id, file_key, bytes],
        )?;
        Ok(())
    }

    pub fn remove_download_cover(&self, album_id: i64) -> Result<Option<String>> {
        let key = self.download_cover(album_id)?;
        self.conn.execute(
            "DELETE FROM download_covers WHERE album_id = ?1",
            [album_id],
        )?;
        Ok(key)
    }

    /// Albums with a kept cover.
    pub fn download_cover_albums(&self) -> Result<Vec<i64>> {
        let mut st = self
            .conn
            .prepare_cached("SELECT album_id FROM download_covers ORDER BY album_id")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every file this profile holds: saved songs' and covers' keys.
    pub fn download_file_keys(&self) -> Result<Vec<String>> {
        let mut st = self.conn.prepare_cached(
            "SELECT file_key FROM download_songs WHERE state = 'saved'
             UNION SELECT file_key FROM download_covers",
        )?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Songs the person chose to download, for smart picks' "added" signal.
    pub fn add_download_signals(&self, song_keys: &[String], at_utc_ms: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut st = tx.prepare_cached(
                "INSERT INTO download_signals (song_key, at_utc_ms) VALUES (?1, ?2)",
            )?;
            for k in song_keys {
                st.execute(params![k, at_utc_ms])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn download_signals(&self) -> Result<Vec<(String, i64)>> {
        let mut st = self.conn.prepare_cached(
            "SELECT song_key, at_utc_ms FROM download_signals ORDER BY at_utc_ms",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

const COLLECTION_SELECT: &str =
    "SELECT kind, item, title, added_utc_ms, auto, paused FROM download_collections";

fn collection_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadCollectionRow> {
    Ok(DownloadCollectionRow {
        kind: r.get(0)?,
        item: r.get(1)?,
        title: r.get(2)?,
        added_utc_ms: r.get(3)?,
        auto: r.get(4)?,
        paused: r.get(5)?,
    })
}

const SONG_SELECT: &str = "SELECT track_id, file_key, state, bytes, attempts, error, queued_utc_ms,
                                  saved_utc_ms FROM download_songs";

fn song_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadSongRow> {
    Ok(DownloadSongRow {
        track_id: r.get(0)?,
        file_key: r.get(1)?,
        state: r.get(2)?,
        bytes: r.get(3)?,
        attempts: r.get(4)?,
        error: r.get(5)?,
        queued_utc_ms: r.get(6)?,
        saved_utc_ms: r.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collections_members_and_songs_keep_their_state() {
        let s = Store::open_in_memory().unwrap();
        assert!(
            s.upsert_download_collection("album", "7", "Road", false, 10)
                .unwrap()
        );
        assert!(
            !s.upsert_download_collection("album", "7", "Road trip", false, 20)
                .unwrap()
        );
        assert!(
            s.upsert_download_collection("song", "3", "Kiln", false, 30)
                .unwrap()
        );
        let c = s.download_collection("album", "7").unwrap().unwrap();
        assert_eq!((c.title.as_str(), c.added_utc_ms), ("Road trip", 10));

        s.set_download_members("album", "7", &[1, 2, 3]).unwrap();
        s.set_download_members("song", "3", &[3]).unwrap();
        assert_eq!(s.download_members("album", "7").unwrap(), [1, 2, 3]);
        assert_eq!(s.download_wanted().unwrap(), HashSet::from([1, 2, 3]));
        for id in [1, 2, 3] {
            s.queue_download_song(id, &format!("k{id}"), 40).unwrap();
        }
        // The album came first, so its songs go first, in its order.
        assert_eq!(
            s.download_candidates(10).unwrap(),
            [(1, "k1".into()), (2, "k2".into()), (3, "k3".into())]
        );
        s.set_download_saved(1, 100, 50).unwrap();
        s.set_download_attempt_failed(2, "server error", false)
            .unwrap();
        s.set_download_attempt_failed(2, "server error", true)
            .unwrap();
        let two = s.download_song(2).unwrap().unwrap();
        assert_eq!((two.state.as_str(), two.attempts), ("failed", 2));
        assert_eq!(s.saved_download_ids().unwrap(), HashSet::from([1]));
        assert_eq!(s.download_bytes().unwrap(), 100);

        let counts: std::collections::HashMap<_, _> =
            s.download_counts().unwrap().into_iter().collect();
        let album = &counts[&("album".to_owned(), "7".to_owned())];
        assert_eq!(
            (album.total, album.saved, album.failed, album.queued),
            (3, 1, 1, 1)
        );

        // Paused collections wait; a song another collection holds still goes.
        s.set_download_paused("album", "7", true).unwrap();
        assert_eq!(s.download_candidates(10).unwrap(), [(3, "k3".into())]);
        assert_eq!(s.download_queued_count().unwrap(), 1);
        assert_eq!(s.retry_failed_downloads().unwrap(), 1);

        assert!(s.remove_download_collection("album", "7").unwrap());
        assert!(
            s.download_members("album", "7").unwrap().is_empty(),
            "members go with it"
        );
        assert_eq!(s.download_wanted().unwrap(), HashSet::from([3]));
    }

    #[test]
    fn covers_and_signals_are_kept() {
        let s = Store::open_in_memory().unwrap();
        s.set_download_cover(4, "c4", 20).unwrap();
        assert_eq!(s.download_cover(4).unwrap().as_deref(), Some("c4"));
        assert_eq!(s.download_cover_albums().unwrap(), [4]);
        assert_eq!(s.download_file_keys().unwrap(), ["c4"]);
        assert_eq!(s.download_bytes().unwrap(), 20);
        assert_eq!(s.remove_download_cover(4).unwrap().as_deref(), Some("c4"));
        s.add_download_signals(&["a".into(), "b".into()], 9)
            .unwrap();
        assert_eq!(
            s.download_signals().unwrap(),
            [("a".into(), 9), ("b".into(), 9)]
        );
    }
}
