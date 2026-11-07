//! Storage for the play log (plan section 5). The rules (what counts, privacy,
//! checkpoints) live in encore-core; this module only reads and writes rows.

use rusqlite::{OptionalExtension, Row, params};

use crate::{Result, Store};

/// One play, with every field plan section 5 requires from the first build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayEventRecord {
    pub id: String,
    pub device_id: String,
    pub hlc: String,
    pub song_key: String,
    pub source_kind: String,
    pub source_item_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: u64,
    pub started_utc_ms: i64,
    pub utc_offset_minutes: i32,
    pub local_hour: u8,
    pub local_weekday: u8,
    pub listened_ms: u64,
    pub end_reason: String,
    pub started_from: String,
    pub context_id: Option<String>,
    pub chosen_by: String,
    pub updated_utc_ms: i64,
    /// How the play reads for smart picks (plan section 21), set when it ends;
    /// None while it plays and for plays logged before Phase 3.
    pub signal: Option<String>,
    /// What moved on from it: button, swipe, headset, car, lockScreen, keyboard
    /// or auto (the song ended).
    pub origin: Option<String>,
    /// Times the listener dragged the progress bar during it.
    pub seeks: u32,
}

/// A tap on a lyric line (plan section 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricJumpRecord {
    pub id: String,
    pub play_event_id: String,
    pub device_id: String,
    pub hlc: String,
    pub line_index: u32,
    pub line_start_ms: u64,
    pub at_utc_ms: i64,
}

/// The value `end_reason` holds while the song is still playing.
pub const IN_PROGRESS: &str = "inProgress";

const COLS: &str =
    "id, device_id, hlc, song_key, source_kind, source_item_id, title, artist, album,
     duration_ms, started_utc_ms, utc_offset_minutes, local_hour, local_weekday, listened_ms,
     end_reason, started_from, context_id, chosen_by, updated_utc_ms, signal, origin, seeks";

fn row(r: &Row<'_>) -> rusqlite::Result<PlayEventRecord> {
    Ok(PlayEventRecord {
        id: r.get(0)?,
        device_id: r.get(1)?,
        hlc: r.get(2)?,
        song_key: r.get(3)?,
        source_kind: r.get(4)?,
        source_item_id: r.get(5)?,
        title: r.get(6)?,
        artist: r.get(7)?,
        album: r.get(8)?,
        duration_ms: r.get::<_, i64>(9)? as u64,
        started_utc_ms: r.get(10)?,
        utc_offset_minutes: r.get(11)?,
        local_hour: r.get(12)?,
        local_weekday: r.get(13)?,
        listened_ms: r.get::<_, i64>(14)? as u64,
        end_reason: r.get(15)?,
        started_from: r.get(16)?,
        context_id: r.get(17)?,
        chosen_by: r.get(18)?,
        updated_utc_ms: r.get(19)?,
        signal: r.get(20)?,
        origin: r.get(21)?,
        seeks: r.get(22)?,
    })
}

impl Store {
    pub fn insert_play_event(&self, e: &PlayEventRecord) -> Result<()> {
        self.conn.execute(
            &format!("INSERT INTO play_events ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"),
            params![
                e.id,
                e.device_id,
                e.hlc,
                e.song_key,
                e.source_kind,
                e.source_item_id,
                e.title,
                e.artist,
                e.album,
                e.duration_ms as i64,
                e.started_utc_ms,
                e.utc_offset_minutes,
                e.local_hour,
                e.local_weekday,
                e.listened_ms as i64,
                e.end_reason,
                e.started_from,
                e.context_id,
                e.chosen_by,
                e.updated_utc_ms,
                e.signal,
                e.origin,
                e.seeks
            ],
        )?;
        Ok(())
    }

    /// Save progress of a play that is still going. Ignored once the play ended.
    pub fn checkpoint_play_event(
        &self,
        id: &str,
        listened_ms: u64,
        at_utc_ms: i64,
    ) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE play_events SET listened_ms = ?2, updated_utc_ms = ?3
             WHERE id = ?1 AND end_reason = ?4",
            params![id, listened_ms as i64, at_utc_ms, IN_PROGRESS],
        )?;
        Ok(n == 1)
    }

    /// Close a play, with how it reads and what moved on from it. Only a play that
    /// is still in progress can be closed.
    pub fn finish_play_event(
        &self,
        id: &str,
        listened_ms: u64,
        end_reason: &str,
        signal: Option<&str>,
        origin: Option<&str>,
        at_utc_ms: i64,
    ) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE play_events SET listened_ms = ?2, end_reason = ?3, signal = ?4, origin = ?5,
                 updated_utc_ms = ?6
             WHERE id = ?1 AND end_reason = ?7",
            params![
                id,
                listened_ms as i64,
                end_reason,
                signal,
                origin,
                at_utc_ms,
                IN_PROGRESS
            ],
        )?;
        Ok(n == 1)
    }

    /// The listener dragged the progress bar: counted, never a skip.
    pub fn add_seek(&self, id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE play_events SET seeks = seeks + 1 WHERE id = ?1 AND end_reason = ?2",
            params![id, IN_PROGRESS],
        )?;
        Ok(n == 1)
    }

    /// Remove a play still in progress, and its lyric jumps: a song passed over by
    /// quick skips was never really played (board 42).
    pub fn discard_play_event(&self, id: &str) -> Result<bool> {
        self.in_transaction(|s| {
            s.conn
                .execute("DELETE FROM lyric_jumps WHERE play_event_id = ?1", [id])?;
            let n = s.conn.execute(
                "DELETE FROM play_events WHERE id = ?1 AND end_reason = ?2",
                params![id, IN_PROGRESS],
            )?;
            Ok(n == 1)
        })
    }

    /// Whether the song was heard (at least `min_listened_ms`) in a play that
    /// started at or after `since_utc_ms`, other than `except_id`.
    pub fn heard_since(
        &self,
        song_key: &str,
        since_utc_ms: i64,
        min_listened_ms: u64,
        except_id: &str,
    ) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM play_events WHERE song_key = ?1 AND started_utc_ms >= ?2
                 AND listened_ms >= ?3 AND id <> ?4)",
            params![song_key, since_utc_ms, min_listened_ms as i64, except_id],
            |r| r.get(0),
        )?)
    }

    pub fn play_event(&self, id: &str) -> Result<Option<PlayEventRecord>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM play_events WHERE id = ?1"),
                [id],
                row,
            )
            .optional()?)
    }

    /// Plays still marked in progress (after a crash, these are closed on launch).
    pub fn open_play_events(&self) -> Result<Vec<PlayEventRecord>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLS} FROM play_events WHERE end_reason = ?1 ORDER BY hlc"
        ))?;
        let rows = st.query_map([IN_PROGRESS], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Newest first: the Recently played history.
    pub fn recent_play_events(&self, limit: u32) -> Result<Vec<PlayEventRecord>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLS} FROM play_events ORDER BY hlc DESC LIMIT ?1"
        ))?;
        let rows = st.query_map([limit], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn insert_lyric_jump(&self, j: &LyricJumpRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO lyric_jumps (id, play_event_id, device_id, hlc, line_index, line_start_ms, at_utc_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![j.id, j.play_event_id, j.device_id, j.hlc, j.line_index, j.line_start_ms as i64, j.at_utc_ms],
        )?;
        Ok(())
    }

    pub fn lyric_jumps(&self, play_event_id: &str) -> Result<Vec<LyricJumpRecord>> {
        let mut st = self.conn.prepare(
            "SELECT id, play_event_id, device_id, hlc, line_index, line_start_ms, at_utc_ms
             FROM lyric_jumps WHERE play_event_id = ?1 ORDER BY hlc",
        )?;
        let rows = st.query_map([play_event_id], |r| {
            Ok(LyricJumpRecord {
                id: r.get(0)?,
                play_event_id: r.get(1)?,
                device_id: r.get(2)?,
                hlc: r.get(3)?,
                line_index: r.get(4)?,
                line_start_ms: r.get::<_, i64>(5)? as u64,
                at_utc_ms: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Rows in the play log and in the lyric-jump log. The privacy tests read these
    /// straight from the database (section 24: "checked at the database level").
    pub fn listening_row_counts(&self) -> Result<(u64, u64)> {
        let plays: i64 = self
            .conn
            .query_row("SELECT count(*) FROM play_events", [], |r| r.get(0))?;
        let jumps: i64 = self
            .conn
            .query_row("SELECT count(*) FROM lyric_jumps", [], |r| r.get(0))?;
        Ok((plays as u64, jumps as u64))
    }

    /// The newest clock value this database holds, so the clock resumes after it.
    pub fn newest_hlc(&self) -> Result<Option<String>> {
        Ok(self.conn.query_row(
            "SELECT max(h) FROM (SELECT max(hlc) AS h FROM play_events UNION ALL SELECT max(hlc) FROM lyric_jumps)",
            [],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn event(id: &str, hlc: &str) -> PlayEventRecord {
        PlayEventRecord {
            id: id.into(),
            device_id: "dev-1".into(),
            hlc: hlc.into(),
            song_key: "t1:abc:200".into(),
            source_kind: "jellyfin".into(),
            source_item_id: "t1".into(),
            title: "Slow Tide".into(),
            artist: "Ember Cove".into(),
            album: Some("Golden Hour Static".into()),
            duration_ms: 200_000,
            started_utc_ms: 1_762_400_000_000,
            utc_offset_minutes: -300,
            local_hour: 21,
            local_weekday: 4,
            listened_ms: 0,
            end_reason: IN_PROGRESS.into(),
            started_from: "album".into(),
            context_id: Some("al1".into()),
            chosen_by: "user".into(),
            updated_utc_ms: 1_762_400_000_000,
            signal: None,
            origin: None,
            seeks: 0,
        }
    }

    #[test]
    fn every_field_round_trips() {
        let s = Store::open_in_memory().unwrap();
        let e = event("01J0000000000000000000000A", "1762400000000-00000");
        s.insert_play_event(&e).unwrap();
        assert_eq!(s.play_event(&e.id).unwrap().unwrap(), e);
    }

    #[test]
    fn checkpoints_and_finishing_only_touch_open_plays() {
        let s = Store::open_in_memory().unwrap();
        let e = event("A", "1762400000000-00000");
        s.insert_play_event(&e).unwrap();
        assert!(s.checkpoint_play_event("A", 10_000, 2).unwrap());
        assert_eq!(s.open_play_events().unwrap().len(), 1);
        assert!(s.add_seek("A").unwrap());
        assert!(s.add_seek("A").unwrap());
        assert!(
            s.finish_play_event(
                "A",
                12_000,
                "skipped",
                Some("skippedEarly"),
                Some("swipe"),
                3
            )
            .unwrap()
        );
        assert!(!s.checkpoint_play_event("A", 99_000, 4).unwrap());
        assert!(
            !s.add_seek("A").unwrap(),
            "a closed play takes no more seeks"
        );
        assert!(
            !s.finish_play_event("A", 99_000, "finished", None, None, 5)
                .unwrap()
        );
        let got = s.play_event("A").unwrap().unwrap();
        assert_eq!(
            (got.listened_ms, got.end_reason.as_str(), got.updated_utc_ms),
            (12_000, "skipped", 3)
        );
        assert_eq!(
            (got.signal.as_deref(), got.origin.as_deref(), got.seeks),
            (Some("skippedEarly"), Some("swipe"), 2)
        );
        assert!(s.open_play_events().unwrap().is_empty());
    }

    #[test]
    fn a_passed_over_play_is_removed_with_its_jumps_and_heard_since_looks_back() {
        let s = Store::open_in_memory().unwrap();
        let mut e = event("A", "1762400000000-00000");
        s.insert_play_event(&e).unwrap();
        s.insert_lyric_jump(&LyricJumpRecord {
            id: "J".into(),
            play_event_id: "A".into(),
            device_id: "dev-1".into(),
            hlc: "1762400000001-00000".into(),
            line_index: 1,
            line_start_ms: 500,
            at_utc_ms: 1,
        })
        .unwrap();
        assert!(s.discard_play_event("A").unwrap());
        assert_eq!(s.listening_row_counts().unwrap(), (0, 0));

        // heard_since: only plays heard long enough, in the window, other than itself.
        e.id = "B".into();
        e.listened_ms = 40_000;
        s.insert_play_event(&e).unwrap();
        let at = e.started_utc_ms;
        assert!(s.heard_since(&e.song_key, at - 1, 30_000, "C").unwrap());
        assert!(
            !s.heard_since(&e.song_key, at + 1, 30_000, "C").unwrap(),
            "too long ago"
        );
        assert!(
            !s.heard_since(&e.song_key, at - 1, 50_000, "C").unwrap(),
            "not heard long enough"
        );
        assert!(
            !s.heard_since(&e.song_key, at - 1, 30_000, "B").unwrap(),
            "itself"
        );
        assert!(!s.heard_since("other", at - 1, 30_000, "C").unwrap());
        // A finished play can't be removed.
        s.finish_play_event("B", 40_000, "finished", Some("finished"), None, 2)
            .unwrap();
        assert!(!s.discard_play_event("B").unwrap());
    }

    #[test]
    fn lyric_jumps_belong_to_a_play_and_advance_the_clock() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.newest_hlc().unwrap(), None);
        s.insert_play_event(&event("A", "1762400000000-00000"))
            .unwrap();
        s.insert_lyric_jump(&LyricJumpRecord {
            id: "J".into(),
            play_event_id: "A".into(),
            device_id: "dev-1".into(),
            hlc: "1762400005000-00001".into(),
            line_index: 7,
            line_start_ms: 41_200,
            at_utc_ms: 1_762_400_005_000,
        })
        .unwrap();
        assert_eq!(s.lyric_jumps("A").unwrap()[0].line_index, 7);
        assert_eq!(
            s.newest_hlc().unwrap().as_deref(),
            Some("1762400005000-00001")
        );
        assert_eq!(s.listening_row_counts().unwrap(), (1, 1));
    }

    #[test]
    fn a_jump_needs_its_play() {
        let s = Store::open_in_memory().unwrap();
        let orphan = LyricJumpRecord {
            id: "J".into(),
            play_event_id: "missing".into(),
            device_id: "d".into(),
            hlc: "1-00000".into(),
            line_index: 0,
            line_start_ms: 0,
            at_utc_ms: 0,
        };
        assert!(s.insert_lyric_jump(&orphan).is_err());
    }

    #[test]
    fn recent_history_is_newest_first() {
        let s = Store::open_in_memory().unwrap();
        s.insert_play_event(&event("A", "1762400000000-00000"))
            .unwrap();
        s.insert_play_event(&event("B", "1762400300000-00000"))
            .unwrap();
        let ids: Vec<String> = s
            .recent_play_events(10)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, ["B", "A"]);
    }
}
