//! What smart picks reads and writes (migration 7; docs/adr/core-0012): the
//! library and play log as the engine wants them, Not interested, feedback, the
//! media server's play counts, and Clear listening history.

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// One song for the engine (one per song key: the first row with that key).
#[derive(Debug, Clone, PartialEq)]
pub struct RecsSongRow {
    pub track_id: i64,
    pub song_key: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: String,
    pub genres: Vec<String>,
    pub year: Option<i32>,
    pub duration_ms: u64,
    pub added_utc_ms: Option<i64>,
    pub server_play_count: u32,
    pub server_last_played_utc_ms: Option<i64>,
    pub favorite: bool,
}

/// One play for the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecsPlayRow {
    pub song_key: String,
    pub started_utc_ms: i64,
    pub utc_offset_minutes: i32,
    pub listened_ms: u64,
    pub signal: Option<String>,
    pub chosen_by: String,
    pub started_from: String,
}

/// A Not interested item (song key or artist name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotInterestedRow {
    /// "song" or "artist".
    pub scope: String,
    pub value: String,
    /// What the Settings list shows (the song's title, the artist's name).
    pub label: String,
    /// "notMyTaste", "heardTooMuch" or "wrongMoment".
    pub reason: String,
    pub bucket: u8,
    pub at_utc_ms: i64,
    pub hlc: String,
    pub device_id: String,
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackRow {
    pub id: String,
    pub song_key: String,
    pub more: bool,
    pub bucket: u8,
    pub at_utc_ms: i64,
}

impl Store {
    /// The library for the engine.
    pub fn recs_songs(&self) -> Result<Vec<RecsSongRow>> {
        let mut st = self.conn.prepare(
            "SELECT t.id, t.song_key, t.title, t.album_title, t.album_artist_display, t.year,
                    t.duration_ms, t.date_added_utc_ms, t.server_play_count,
                    t.server_last_played_utc_ms, t.favorite, t.artist_display
             FROM tracks t
             WHERE t.id = (SELECT min(id) FROM tracks u WHERE u.song_key = t.song_key)
             ORDER BY t.song_key",
        )?;
        let mut rows: Vec<RecsSongRow> = st
            .query_map([], |r| {
                Ok(RecsSongRow {
                    track_id: r.get(0)?,
                    song_key: r.get(1)?,
                    title: r.get(2)?,
                    album: r.get(3)?,
                    album_artist: r.get(4)?,
                    year: r.get(5)?,
                    duration_ms: r.get::<_, i64>(6)? as u64,
                    added_utc_ms: r.get(7)?,
                    server_play_count: r.get(8)?,
                    server_last_played_utc_ms: r.get(9)?,
                    favorite: r.get(10)?,
                    artists: vec![r.get::<_, String>(11)?],
                    genres: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        // Credited artists and genres, in two passes over their tables.
        let mut by_id: std::collections::HashMap<i64, usize> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.track_id, i))
            .collect();
        let mut st = self
            .conn
            .prepare("SELECT track_id, name FROM track_artists ORDER BY track_id, position")?;
        let mut credited: std::collections::HashMap<i64, Vec<String>> = Default::default();
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            if by_id.contains_key(&id) {
                credited.entry(id).or_default().push(name);
            }
        }
        let mut st = self
            .conn
            .prepare("SELECT track_id, name FROM track_genres ORDER BY track_id, name")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            if let Some(i) = by_id.get(&id) {
                rows[*i].genres.push(name);
            }
        }
        for (id, names) in credited {
            if let Some(i) = by_id.remove(&id) {
                rows[i].artists = names;
            }
        }
        Ok(rows)
    }

    /// The play log for the engine, oldest first.
    pub fn recs_plays(&self) -> Result<Vec<RecsPlayRow>> {
        let mut st = self.conn.prepare(
            "SELECT song_key, started_utc_ms, utc_offset_minutes, listened_ms, signal, chosen_by,
                    started_from
             FROM play_events ORDER BY started_utc_ms, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(RecsPlayRow {
                song_key: r.get(0)?,
                started_utc_ms: r.get(1)?,
                utc_offset_minutes: r.get(2)?,
                listened_ms: r.get::<_, i64>(3)? as u64,
                signal: r.get(4)?,
                chosen_by: r.get(5)?,
                started_from: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Liked songs with when they were liked.
    pub fn likes_with_times(&self) -> Result<Vec<(String, i64)>> {
        let mut st = self
            .conn
            .prepare("SELECT song_key, liked_utc_ms FROM likes ORDER BY liked_utc_ms, song_key")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Songs added to playlists (song key, when), from the edit events: removing
    /// them later doesn't undo the signal (section 21).
    pub fn playlist_adds(&self) -> Result<Vec<(String, i64)>> {
        let mut st = self.conn.prepare(
            "SELECT payload, created_utc_ms FROM events
             WHERE kind IN ('playlistAdd', 'playlistCreate') ORDER BY created_utc_ms",
        )?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (payload, at) = row?;
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&payload) {
                for k in v["songKeys"].as_array().into_iter().flatten() {
                    if let Some(k) = k.as_str() {
                        out.push((k.to_owned(), at));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The media server's play counts (from other apps) for one source's songs.
    pub fn set_server_play_counts(
        &self,
        source_id: i64,
        counts: &[(String, u32, Option<i64>)],
    ) -> Result<()> {
        self.in_transaction(|s| {
            let mut st = s.conn.prepare_cached(
                "UPDATE tracks SET server_play_count = ?3, server_last_played_utc_ms = ?4
                 WHERE source_id = ?1 AND source_item_id = ?2",
            )?;
            for (item, count, last) in counts {
                st.execute(params![source_id, item, count, last])?;
            }
            Ok(())
        })
    }

    /// Add, change or remove a Not interested item, if this change is the latest
    /// for it by (clock, device). Returns whether it applied.
    pub fn put_not_interested(&self, row: &NotInterestedRow) -> Result<bool> {
        let held: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT hlc, device_id FROM not_interested WHERE scope = ?1 AND value = ?2",
                params![row.scope, row.value],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((hlc, device)) = held
            && (hlc.as_str(), device.as_str()) >= (row.hlc.as_str(), row.device_id.as_str())
        {
            return Ok(false);
        }
        self.conn.execute(
            "INSERT INTO not_interested (scope, value, label, reason, bucket, at_utc_ms, hlc, device_id, removed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (scope, value) DO UPDATE SET label = excluded.label,
                 reason = excluded.reason, bucket = excluded.bucket, at_utc_ms = excluded.at_utc_ms,
                 hlc = excluded.hlc, device_id = excluded.device_id, removed = excluded.removed",
            params![
                row.scope,
                row.value,
                row.label,
                row.reason,
                row.bucket,
                row.at_utc_ms,
                row.hlc,
                row.device_id,
                row.removed
            ],
        )?;
        Ok(true)
    }

    /// Every Not interested row, removed ones included (the engine skips those).
    pub fn not_interested_rows(&self) -> Result<Vec<NotInterestedRow>> {
        let mut st = self.conn.prepare(
            "SELECT scope, value, label, reason, bucket, at_utc_ms, hlc, device_id, removed
             FROM not_interested ORDER BY at_utc_ms DESC, scope, value",
        )?;
        let rows = st.query_map([], |r| {
            Ok(NotInterestedRow {
                scope: r.get(0)?,
                value: r.get(1)?,
                label: r.get(2)?,
                reason: r.get(3)?,
                bucket: r.get(4)?,
                at_utc_ms: r.get(5)?,
                hlc: r.get(6)?,
                device_id: r.get(7)?,
                removed: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn add_feedback(&self, f: &FeedbackRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO feedback (id, song_key, more, bucket, at_utc_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![f.id, f.song_key, f.more, f.bucket, f.at_utc_ms],
        )?;
        Ok(())
    }

    pub fn feedback_rows(&self) -> Result<Vec<FeedbackRow>> {
        let mut st = self.conn.prepare(
            "SELECT id, song_key, more, bucket, at_utc_ms FROM feedback ORDER BY at_utc_ms, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(FeedbackRow {
                id: r.get(0)?,
                song_key: r.get(1)?,
                more: r.get(2)?,
                bucket: r.get(3)?,
                at_utc_ms: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn recs_state(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM recs_state WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_recs_state(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.conn.execute(
                "INSERT INTO recs_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?,
            None => self
                .conn
                .execute("DELETE FROM recs_state WHERE key = ?1", [key])?,
        };
        Ok(())
    }

    /// Clear listening history (section 21): every play and lyric jump that started
    /// before `before_utc_ms` goes, and the media server's counts are no longer
    /// used as a prior (they are history too). Returns how many plays went.
    pub fn clear_history_before(&self, before_utc_ms: i64) -> Result<u32> {
        self.in_transaction(|s| {
            s.conn.execute(
                "DELETE FROM lyric_jumps WHERE play_event_id IN
                     (SELECT id FROM play_events WHERE started_utc_ms < ?1)",
                [before_utc_ms],
            )?;
            let n = s.conn.execute(
                "DELETE FROM play_events WHERE started_utc_ms < ?1",
                [before_utc_ms],
            )?;
            s.conn.execute(
                "UPDATE tracks SET server_play_count = 0, server_last_played_utc_ms = NULL",
                [],
            )?;
            Ok(n as u32)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::tests::sample_delta;
    use crate::test_support::store_with_source;

    fn ni(value: &str, hlc: &str, device: &str, removed: bool) -> NotInterestedRow {
        NotInterestedRow {
            scope: "artist".into(),
            value: value.into(),
            label: value.into(),
            reason: "notMyTaste".into(),
            bucket: 2,
            at_utc_ms: 5,
            hlc: hlc.into(),
            device_id: device.into(),
            removed,
        }
    }

    #[test]
    fn the_engine_reads_one_song_per_key_with_artists_and_genres() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let songs = store.recs_songs().unwrap();
        assert!(!songs.is_empty());
        let mut keys: Vec<&str> = songs.iter().map(|s| s.song_key.as_str()).collect();
        keys.dedup();
        assert_eq!(keys.len(), songs.len());
        assert!(songs.iter().all(|s| !s.artists.is_empty()));
        let item: String = store
            .conn
            .query_row(
                "SELECT source_item_id FROM tracks ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        store
            .set_server_play_counts(src, &[(item, 7, Some(99))])
            .unwrap();
        assert!(
            store
                .recs_songs()
                .unwrap()
                .iter()
                .any(|s| s.server_play_count == 7)
        );
        assert_eq!(store.clear_history_before(i64::MAX).unwrap(), 0);
        assert!(
            store
                .recs_songs()
                .unwrap()
                .iter()
                .all(|s| s.server_play_count == 0)
        );
    }

    #[test]
    fn not_interested_is_a_set_where_the_latest_change_wins() {
        let (store, _) = store_with_source();
        assert!(
            store
                .put_not_interested(&ni("Ember Cove", "h5", "phone", false))
                .unwrap()
        );
        // An older add from the Mac doesn't overwrite; a later removal does.
        assert!(
            !store
                .put_not_interested(&ni("Ember Cove", "h3", "mac", false))
                .unwrap()
        );
        assert!(
            store
                .put_not_interested(&ni("Ember Cove", "h6", "mac", true))
                .unwrap()
        );
        // And an older add can't bring it back.
        assert!(
            !store
                .put_not_interested(&ni("Ember Cove", "h5", "phone", false))
                .unwrap()
        );
        let rows = store.not_interested_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].removed);
        store
            .add_feedback(&FeedbackRow {
                id: "f1".into(),
                song_key: "k".into(),
                more: true,
                bucket: 1,
                at_utc_ms: 3,
            })
            .unwrap();
        assert_eq!(store.feedback_rows().unwrap().len(), 1);
        store.set_recs_state("epoch", Some("10")).unwrap();
        assert_eq!(store.recs_state("epoch").unwrap().as_deref(), Some("10"));
    }
}
