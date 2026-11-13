//! Sound analysis: what each song sounds like (migration 9; Phase 10, docs/adr/core-0019).
//!
//! One row per song key (a song on two albums is analysed once). A row is about the song, never
//! the listener. `state` is `ok`, or `unavailable` with a reason when the audio could not be read
//! (an Opus file, a damaged file, a server that would not serve it): such a song is never guessed
//! at and is not tried again until the person asks.

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// One analysed song.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundRow {
    pub song_key: String,
    /// The basic profile's stored version (`encore_analysis::VERSION`).
    pub version: u32,
    /// "ok" or "unavailable".
    pub state: String,
    /// Why, when unavailable.
    pub reason: Option<String>,
    /// `encore_analysis::SoundProfile::encode()`.
    pub profile: Option<Vec<u8>>,
    /// The embedding model that made `embedding` (its name and version), when one has run.
    pub model: Option<String>,
    /// The model's embedding, quantised.
    pub embedding: Option<Vec<u8>>,
    pub analysed_utc_ms: i64,
}

/// A song that still needs analysing, with what the reader needs to fetch it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSound {
    pub track_id: i64,
    pub song_key: String,
    /// The media server the song is on, and its ID there.
    pub source_id: i64,
    pub source_item_id: String,
    /// The file's container as the media server names it ("mp3", "flac", "opus"), to hint the decoder.
    pub container: Option<String>,
    pub duration_ms: u64,
}

/// How far the library is analysed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoundCounts {
    /// Songs in the library (one per song key).
    pub songs: u32,
    pub analysed: u32,
    pub unavailable: u32,
}

impl Store {
    /// Save a song's analysis, replacing an older one.
    pub fn put_sound(&self, row: &SoundRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sound_profiles
                 (song_key, version, state, reason, profile, model, embedding, analysed_utc_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(song_key) DO UPDATE SET
                 version = excluded.version, state = excluded.state, reason = excluded.reason,
                 profile = excluded.profile, model = excluded.model,
                 embedding = excluded.embedding, analysed_utc_ms = excluded.analysed_utc_ms",
            params![
                row.song_key,
                row.version,
                row.state,
                row.reason,
                row.profile,
                row.model,
                row.embedding,
                row.analysed_utc_ms
            ],
        )?;
        Ok(())
    }

    pub fn sound(&self, song_key: &str) -> Result<Option<SoundRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT song_key, version, state, reason, profile, model, embedding, analysed_utc_ms
                 FROM sound_profiles WHERE song_key = ?1",
                params![song_key],
                row_of,
            )
            .optional()?)
    }

    /// Every analysed song (`ok`), for the engine, by song key.
    pub fn sounds_ok(&self) -> Result<Vec<SoundRow>> {
        let mut st = self.conn.prepare(
            "SELECT song_key, version, state, reason, profile, model, embedding, analysed_utc_ms
             FROM sound_profiles WHERE state = 'ok' ORDER BY song_key",
        )?;
        let rows = st.query_map([], row_of)?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Songs in the library, analysed and unreadable (one per song key).
    pub fn sound_counts(&self) -> Result<SoundCounts> {
        let (songs, analysed, unavailable) = self.conn.query_row(
            "SELECT
                 (SELECT count(DISTINCT song_key) FROM tracks),
                 (SELECT count(*) FROM sound_profiles s
                   WHERE s.state = 'ok' AND EXISTS (SELECT 1 FROM tracks t WHERE t.song_key = s.song_key)),
                 (SELECT count(*) FROM sound_profiles s
                   WHERE s.state = 'unavailable' AND EXISTS (SELECT 1 FROM tracks t WHERE t.song_key = s.song_key))",
            [],
            |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?, r.get::<_, u32>(2)?)),
        )?;
        Ok(SoundCounts {
            songs,
            analysed,
            unavailable,
        })
    }

    /// The next songs to analyse, most useful first: liked songs, then songs the person has played,
    /// then downloaded songs, then the rest, newest first within each. A song counts as needing
    /// work when it has no row, or its row is `ok` but from another profile version or another
    /// embedding model than `model` (`""` when no embedding model is in use). Unreadable songs
    /// are left alone.
    pub fn pending_sounds(
        &self,
        version: u32,
        model: &str,
        limit: u32,
    ) -> Result<Vec<PendingSound>> {
        let mut st = self.conn.prepare(
            "SELECT t.id, t.song_key, t.source_item_id, t.container, t.duration_ms, t.source_id,
                    CASE
                      WHEN EXISTS (SELECT 1 FROM likes l WHERE l.song_key = t.song_key) THEN 0
                      WHEN EXISTS (SELECT 1 FROM play_events p WHERE p.song_key = t.song_key) THEN 1
                      WHEN EXISTS (SELECT 1 FROM download_songs d WHERE d.track_id = t.id) THEN 2
                      ELSE 3
                    END AS tier
             FROM tracks t
             LEFT JOIN sound_profiles s ON s.song_key = t.song_key
             WHERE t.id = (SELECT min(id) FROM tracks u WHERE u.song_key = t.song_key)
               AND (s.song_key IS NULL
                    OR (s.state = 'ok' AND (s.version <> ?1 OR ifnull(s.model, '') <> ?2)))
             ORDER BY tier, ifnull(t.date_added_utc_ms, 0) DESC, t.id
             LIMIT ?3",
        )?;
        let rows = st
            .query_map(params![version, model, limit], |r| {
                Ok(PendingSound {
                    track_id: r.get(0)?,
                    song_key: r.get(1)?,
                    source_id: r.get(5)?,
                    source_item_id: r.get(2)?,
                    container: r.get(3)?,
                    duration_ms: r.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Forget every analysis (Settings > Sound analysis > Delete). Returns how many rows went.
    pub fn delete_sounds(&self) -> Result<u32> {
        Ok(self.conn.execute("DELETE FROM sound_profiles", [])? as u32)
    }

    /// Forget the songs that could not be read, so they are tried again.
    pub fn retry_unavailable_sounds(&self) -> Result<u32> {
        Ok(self
            .conn
            .execute("DELETE FROM sound_profiles WHERE state = 'unavailable'", [])?
            as u32)
    }
}

fn row_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<SoundRow> {
    Ok(SoundRow {
        song_key: r.get(0)?,
        version: r.get(1)?,
        state: r.get(2)?,
        reason: r.get(3)?,
        profile: r.get(4)?,
        model: r.get(5)?,
        embedding: r.get(6)?,
        analysed_utc_ms: r.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library of tracks by raw rows (the tests here are about the sound table, not the mirror).
    fn library(s: &Store, songs: &[(&str, i64)]) {
        s.conn
            .execute(
                "INSERT INTO sources (id, kind, name, base_url, server_id, user_id, user_name, created_utc_ms)
                 VALUES (1, 'jellyfin', 'n', 'https://m.example', 's', 'u', 'n', 1)",
                [],
            )
            .ok();
        for (i, (key, added)) in songs.iter().enumerate() {
            s.conn
                .execute(
                    "INSERT INTO tracks (source_id, source_item_id, title, sort_title, search_name, artist_display,
                                         album_artist_display, duration_ms, song_key, date_added_utc_ms, has_lyrics, container)
                     VALUES (1, ?1, ?2, ?2, ?2, 'A', 'A', 200000, ?3, ?4, 0, 'mp3')",
                    params![format!("item{i}"), key, key, added],
                )
                .unwrap();
        }
    }

    fn ok(key: &str, model: Option<&str>) -> SoundRow {
        SoundRow {
            song_key: key.into(),
            version: 1,
            state: "ok".into(),
            reason: None,
            profile: Some(vec![1, 2, 3]),
            model: model.map(str::to_owned),
            embedding: model.map(|_| vec![9, 9]),
            analysed_utc_ms: 100,
        }
    }

    #[test]
    fn a_row_is_saved_replaced_and_read_back() {
        let s = Store::open_in_memory().unwrap();
        s.put_sound(&ok("a", None)).unwrap();
        s.put_sound(&ok("a", Some("openl3-music-1"))).unwrap();
        let back = s.sound("a").unwrap().unwrap();
        assert_eq!(back.model.as_deref(), Some("openl3-music-1"));
        assert_eq!(back.embedding, Some(vec![9, 9]));
        assert_eq!(s.sounds_ok().unwrap().len(), 1);
        assert!(s.sound("zzz").unwrap().is_none());
    }

    #[test]
    fn the_next_songs_are_the_most_useful_ones_and_each_song_once() {
        let s = Store::open_in_memory().unwrap();
        library(
            &s,
            &[
                ("old", 1),
                ("new", 9),
                ("played", 2),
                ("liked", 3),
                ("dup", 4),
                ("dup", 5),
            ],
        );
        s.conn
            .execute(
                "INSERT INTO likes (song_key, liked_utc_ms) VALUES ('liked', 1)",
                [],
            )
            .unwrap();
        s.conn
            .execute(
                "INSERT INTO play_events (id, device_id, hlc, song_key, source_kind, source_item_id, title, artist,
                     duration_ms, started_utc_ms, utc_offset_minutes, local_hour, local_weekday, listened_ms,
                     end_reason, started_from, chosen_by, updated_utc_ms)
                 VALUES ('p1', 'd', 'h', 'played', 'jellyfin', 'i', 't', 'a', 1000, 1, 0, 0, 0, 1000, 'finished', 'album', 'user', 1)",
                [],
            )
            .unwrap();
        let next: Vec<String> = s
            .pending_sounds(1, "", 10)
            .unwrap()
            .into_iter()
            .map(|p| p.song_key)
            .collect();
        // Liked, then played, then the rest newest first; the song on two albums once.
        assert_eq!(next, ["liked", "played", "new", "dup", "old"]);
        assert_eq!(s.pending_sounds(1, "", 2).unwrap().len(), 2);
    }

    #[test]
    fn analysed_songs_are_not_pending_until_the_version_or_model_changes() {
        let s = Store::open_in_memory().unwrap();
        library(&s, &[("a", 1), ("b", 2), ("c", 3)]);
        s.put_sound(&ok("a", None)).unwrap();
        s.put_sound(&SoundRow {
            state: "unavailable".into(),
            reason: Some("Opus".into()),
            profile: None,
            ..ok("b", None)
        })
        .unwrap();
        let keys = |m: &str, v: u32| -> Vec<String> {
            s.pending_sounds(v, m, 10)
                .unwrap()
                .into_iter()
                .map(|p| p.song_key)
                .collect()
        };
        assert_eq!(keys("", 1), ["c"], "an unreadable song is left alone");
        assert_eq!(
            keys("openl3-music-1", 1),
            ["c", "a"],
            "a model arrives: the basic-only rows are redone"
        );
        assert_eq!(keys("", 2), ["c", "a"], "a new profile version: redone");
        assert_eq!(s.retry_unavailable_sounds().unwrap(), 1);
        assert_eq!(keys("", 1), ["c", "b"]);
    }

    #[test]
    fn counts_cover_songs_in_the_library_only() {
        let s = Store::open_in_memory().unwrap();
        library(&s, &[("a", 1), ("b", 2), ("dup", 3), ("dup", 4)]);
        s.put_sound(&ok("a", None)).unwrap();
        s.put_sound(&SoundRow {
            state: "unavailable".into(),
            profile: None,
            ..ok("b", None)
        })
        .unwrap();
        s.put_sound(&ok("gone", None)).unwrap(); // a song that left the library
        assert_eq!(
            s.sound_counts().unwrap(),
            SoundCounts {
                songs: 3,
                analysed: 1,
                unavailable: 1
            }
        );
        assert_eq!(s.delete_sounds().unwrap(), 3);
        assert_eq!(s.sound_counts().unwrap().analysed, 0);
    }
}
