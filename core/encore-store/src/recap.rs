//! What the year recap reads (Phase 11, ADR core-0025): the plays of a time span with
//! the library facts the engine needs beside them, the artists heard before it, and
//! each album's song count. The recap engine itself is `encore-recap`; this module
//! only reads rows.

use std::collections::BTreeMap;

use rusqlite::params;

use crate::playlog::IN_PROGRESS;
use crate::{Result, Store};

/// One finished play, as the recap engine counts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecapPlayRow {
    pub id: String,
    pub song_key: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub started_utc_ms: i64,
    pub utc_offset_minutes: i32,
    pub listened_ms: u64,
    pub duration_ms: u64,
    pub end_reason: String,
    pub started_from: String,
    /// The library's first genre for the song (by name), if it has one.
    pub genre: Option<String>,
    /// The song's album in the library, for its cover palette and song count.
    pub album_id: Option<i64>,
    /// Lyric lines tapped during the play: (line index, line start in ms).
    pub lyric_jumps: Vec<(u32, u64)>,
}

/// An artist heard before a time span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistBeforeRow {
    pub artist: String,
    pub plays: u64,
    pub last_played_utc_ms: i64,
}

impl Store {
    /// Every finished play that started in `[from_utc_ms, to_utc_ms)`, oldest first,
    /// with its genre, album and lyric taps.
    pub fn recap_plays(&self, from_utc_ms: i64, to_utc_ms: i64) -> Result<Vec<RecapPlayRow>> {
        let mut st = self.conn.prepare_cached(
            "SELECT p.id, p.song_key, p.title, p.artist, p.album, p.started_utc_ms,
                    p.utc_offset_minutes, p.listened_ms, p.duration_ms, p.end_reason,
                    p.started_from,
                    (SELECT g.name FROM tracks t JOIN track_genres g ON g.track_id = t.id
                      WHERE t.song_key = p.song_key ORDER BY t.id, g.name LIMIT 1),
                    (SELECT t.album_id FROM tracks t
                      WHERE t.song_key = p.song_key AND t.album_id IS NOT NULL
                      ORDER BY t.id LIMIT 1)
             FROM play_events p
             WHERE p.started_utc_ms >= ?1 AND p.started_utc_ms < ?2 AND p.end_reason <> ?3
             ORDER BY p.started_utc_ms, p.id",
        )?;
        let mut plays: Vec<RecapPlayRow> = st
            .query_map(params![from_utc_ms, to_utc_ms, IN_PROGRESS], |r| {
                Ok(RecapPlayRow {
                    id: r.get(0)?,
                    song_key: r.get(1)?,
                    title: r.get(2)?,
                    artist: r.get(3)?,
                    album: r.get(4)?,
                    started_utc_ms: r.get(5)?,
                    utc_offset_minutes: r.get(6)?,
                    listened_ms: r.get::<_, i64>(7)?.max(0) as u64,
                    duration_ms: r.get::<_, i64>(8)?.max(0) as u64,
                    end_reason: r.get(9)?,
                    started_from: r.get(10)?,
                    genre: r.get(11)?,
                    album_id: r.get(12)?,
                    lyric_jumps: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut jumps: BTreeMap<String, Vec<(u32, u64)>> = BTreeMap::new();
        let mut st = self.conn.prepare_cached(
            "SELECT j.play_event_id, j.line_index, j.line_start_ms
             FROM lyric_jumps j JOIN play_events p ON p.id = j.play_event_id
             WHERE p.started_utc_ms >= ?1 AND p.started_utc_ms < ?2
             ORDER BY j.play_event_id, j.at_utc_ms, j.id",
        )?;
        let rows = st.query_map(params![from_utc_ms, to_utc_ms], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?.max(0) as u32,
                r.get::<_, i64>(2)?.max(0) as u64,
            ))
        })?;
        for row in rows {
            let (play, line, start) = row?;
            jumps.entry(play).or_default().push((line, start));
        }
        for p in &mut plays {
            if let Some(j) = jumps.remove(&p.id) {
                p.lyric_jumps = j;
            }
        }
        Ok(plays)
    }

    /// Each artist (as the play log names them) heard before `before_utc_ms`: how many
    /// finished plays, and when last.
    pub fn recap_artists_before(&self, before_utc_ms: i64) -> Result<Vec<ArtistBeforeRow>> {
        let mut st = self.conn.prepare_cached(
            "SELECT artist, count(*), max(started_utc_ms) FROM play_events
             WHERE started_utc_ms < ?1 AND end_reason <> ?2
             GROUP BY artist ORDER BY artist",
        )?;
        let rows = st.query_map(params![before_utc_ms, IN_PROGRESS], |r| {
            Ok(ArtistBeforeRow {
                artist: r.get(0)?,
                plays: r.get::<_, i64>(1)?.max(0) as u64,
                last_played_utc_ms: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// How many songs the library has on an album ("played start to finish").
    pub fn album_song_count(&self, album_id: i64) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE album_id = ?1",
            [album_id],
            |r| r.get::<_, i64>(0),
        )? as u32)
    }
}

#[cfg(test)]
mod tests {
    use crate::LyricJumpRecord;
    use crate::playlog::tests::event;
    use crate::test_support::store_with_source;

    #[test]
    fn a_span_reads_finished_plays_with_their_lyric_taps() {
        let (store, _) = store_with_source();
        let mut a = event("p1", "0001");
        a.started_utc_ms = 1_000;
        a.end_reason = "ended".into();
        a.listened_ms = 190_000;
        let mut b = event("p2", "0002");
        b.started_utc_ms = 2_000;
        b.artist = "Marigold Ave".into();
        b.end_reason = "skipped".into();
        // Still playing: not counted.
        let mut c = event("p3", "0003");
        c.started_utc_ms = 3_000;
        // Before the span: an artist heard before.
        let mut old = event("p0", "0000");
        old.started_utc_ms = 10;
        old.end_reason = "ended".into();
        for e in [&a, &b, &c, &old] {
            store.insert_play_event(e).unwrap();
        }
        store
            .insert_lyric_jump(&LyricJumpRecord {
                id: "j1".into(),
                play_event_id: "p1".into(),
                device_id: "dev-1".into(),
                hlc: "0001".into(),
                line_index: 7,
                line_start_ms: 42_000,
                at_utc_ms: 1_500,
            })
            .unwrap();

        let plays = store.recap_plays(500, 10_000).unwrap();
        assert_eq!(
            plays.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["p1", "p2"]
        );
        assert_eq!(plays[0].lyric_jumps, vec![(7, 42_000)]);
        assert_eq!(plays[0].listened_ms, 190_000);
        assert!(plays[1].lyric_jumps.is_empty());

        let before = store.recap_artists_before(500).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].artist, "Ember Cove");
        assert_eq!((before[0].plays, before[0].last_played_utc_ms), (1, 10));
    }
}
