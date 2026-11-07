//! What this device sends to the companion server, and how what other devices sent
//! is applied (plan section 5; docs/adr/core-0010-sync.md).
//!
//! - Plays are sent when they start and again once they end (version 0, then 1).
//!   Another device's play is added to the log, or replaces a version it has
//!   that hadn't ended.
//! - Lyric jumps are sent once.
//! - Edit events (likes, playlist edits, undos) are sent once. Likes merge
//!   last-writer-wins per song by (hybrid clock, device). Playlist edits name the
//!   playlist by its media-server ID; the playlist is then re-read from the
//!   media server, which is the truth for playlists.
//!
//! Guest, Private listening and Learn-from-my-listening-off write no plays, so
//! there is nothing of theirs to send.

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::{PlayEventRecord, Result, Store};

/// The settings key of the pull cursor.
const CURSOR_KEY: &str = "companion.cursor";

/// An event on its way to the server.
#[derive(Debug, Clone, PartialEq)]
pub struct OutEvent {
    pub id: String,
    pub kind: String,
    pub hlc: String,
    pub version: i64,
    pub payload: Value,
    pub created_utc_ms: i64,
}

/// An event another device wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct InEvent {
    pub id: String,
    pub device_id: String,
    pub kind: String,
    pub hlc: String,
    pub version: i64,
    pub payload: Value,
    pub created_utc_ms: i64,
}

/// What applying a batch changed, so the core can refresh screens and the media
/// server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Plays added or brought up to date.
    pub plays: u32,
    /// Songs whose like changed here, and to what.
    pub likes: Vec<(String, bool)>,
    /// Songs this device had decided differently and lost: send the winner to the
    /// media server.
    pub reassert: Vec<(String, bool)>,
    /// Local playlists to re-read from the media server.
    pub playlists: Vec<i64>,
    /// A playlist this device doesn't have yet: sync the library.
    pub new_playlists: bool,
    /// The newest clock seen, for this device's clock to observe.
    pub newest_hlc: Option<String>,
}

/// The songs an edit event likes or unlikes, if it is one.
pub fn like_effect(kind: &str, payload: &Value) -> Option<(Vec<String>, bool)> {
    let liked = match (kind, payload["undoes"].as_str()) {
        ("like", _) => true,
        ("unlike", _) => false,
        ("undo", Some("like")) => false,
        ("undo", Some("unlike")) => true,
        _ => return None,
    };
    let keys: Vec<String> = payload["songKeys"]
        .as_array()?
        .iter()
        .filter_map(|k| k.as_str().map(str::to_owned))
        .collect();
    Some((keys, liked))
}

const PLAY_COLS: &str =
    "id, device_id, hlc, song_key, source_kind, source_item_id, title, artist, album,
     duration_ms, started_utc_ms, utc_offset_minutes, local_hour, local_weekday, listened_ms,
     end_reason, started_from, context_id, chosen_by, updated_utc_ms, signal, origin, seeks";

fn play_json(p: &PlayEventRecord) -> Value {
    json!({
        "deviceId": p.device_id, "songKey": p.song_key, "sourceKind": p.source_kind,
        "sourceItemId": p.source_item_id, "title": p.title, "artist": p.artist,
        "album": p.album, "durationMs": p.duration_ms, "startedUtcMs": p.started_utc_ms,
        "utcOffsetMinutes": p.utc_offset_minutes, "localHour": p.local_hour,
        "localWeekday": p.local_weekday, "listenedMs": p.listened_ms,
        "endReason": p.end_reason, "startedFrom": p.started_from, "contextId": p.context_id,
        "chosenBy": p.chosen_by, "updatedUtcMs": p.updated_utc_ms, "signal": p.signal,
        "origin": p.origin, "seeks": p.seeks,
    })
}

fn play_from(e: &InEvent) -> Option<PlayEventRecord> {
    let v = &e.payload;
    let s = |k: &str| v[k].as_str().map(str::to_owned);
    Some(PlayEventRecord {
        id: e.id.clone(),
        device_id: e.device_id.clone(),
        hlc: e.hlc.clone(),
        song_key: s("songKey")?,
        source_kind: s("sourceKind")?,
        source_item_id: s("sourceItemId")?,
        title: s("title")?,
        artist: s("artist")?,
        album: s("album"),
        duration_ms: v["durationMs"].as_u64()?,
        started_utc_ms: v["startedUtcMs"].as_i64()?,
        utc_offset_minutes: v["utcOffsetMinutes"].as_i64()? as i32,
        local_hour: v["localHour"].as_u64()? as u8,
        local_weekday: v["localWeekday"].as_u64()? as u8,
        listened_ms: v["listenedMs"].as_u64()?,
        end_reason: s("endReason")?,
        started_from: s("startedFrom")?,
        context_id: s("contextId"),
        chosen_by: s("chosenBy")?,
        updated_utc_ms: v["updatedUtcMs"].as_i64()?,
        signal: s("signal"),
        origin: s("origin"),
        seeks: v["seeks"].as_u64().unwrap_or(0) as u32,
    })
}

/// Whether (hlc, device) `a` is later than `b`: the clock decides, and the device
/// ID breaks a tie the same way on every device.
fn later(a: (&str, &str), b: (&str, &str)) -> bool {
    a > b
}

impl Store {
    /// What this device still has to send, oldest first: plays, then lyric jumps
    /// (so a jump never arrives before its play), then edit events.
    pub fn pending_out(&self, own_device: &str, limit: u32) -> Result<Vec<OutEvent>> {
        let mut out = Vec::new();
        {
            let mut st = self.conn.prepare_cached(&format!(
                "SELECT {PLAY_COLS} FROM play_events
                 WHERE device_id = ?1
                   AND synced < CASE WHEN end_reason = '{}' THEN 1 ELSE 2 END
                 ORDER BY hlc LIMIT ?2",
                crate::playlog::IN_PROGRESS
            ))?;
            let rows = st.query_map(params![own_device, limit], crate::playlog::row)?;
            for p in rows {
                let p = p?;
                out.push(OutEvent {
                    version: i64::from(p.end_reason != crate::playlog::IN_PROGRESS),
                    payload: play_json(&p),
                    created_utc_ms: p.started_utc_ms,
                    kind: "play".into(),
                    id: p.id,
                    hlc: p.hlc,
                });
            }
        }
        let left = limit.saturating_sub(out.len() as u32);
        if left > 0 {
            let mut st = self.conn.prepare_cached(
                "SELECT id, play_event_id, hlc, line_index, line_start_ms, at_utc_ms
                 FROM lyric_jumps WHERE synced = 0 AND device_id = ?1 ORDER BY hlc LIMIT ?2",
            )?;
            let rows = st.query_map(params![own_device, left], |r| {
                Ok(OutEvent {
                    id: r.get(0)?,
                    kind: "lyricJump".into(),
                    hlc: r.get(2)?,
                    version: 0,
                    payload: json!({
                        "playId": r.get::<_, String>(1)?,
                        "lineIndex": r.get::<_, i64>(3)?,
                        "lineStartMs": r.get::<_, i64>(4)?,
                    }),
                    created_utc_ms: r.get(5)?,
                })
            })?;
            for j in rows {
                out.push(j?);
            }
        }
        let left = limit.saturating_sub(out.len() as u32);
        if left > 0 {
            let mut st = self.conn.prepare_cached(
                "SELECT id, hlc, kind, payload, created_utc_ms FROM events
                 WHERE synced = 0 AND device_id = ?1 ORDER BY hlc LIMIT ?2",
            )?;
            let rows = st.query_map(params![own_device, left], |r| {
                let payload: String = r.get(3)?;
                Ok(OutEvent {
                    id: r.get(0)?,
                    hlc: r.get(1)?,
                    kind: r.get(2)?,
                    version: 0,
                    payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
                    created_utc_ms: r.get(4)?,
                })
            })?;
            for e in rows {
                out.push(e?);
            }
        }
        Ok(out)
    }

    /// How many things are waiting to be sent (for "Waiting to sync").
    pub fn pending_out_count(&self, own_device: &str) -> Result<u32> {
        Ok(self.conn.query_row(
            &format!(
                "SELECT (SELECT count(*) FROM play_events WHERE device_id = ?1
                     AND synced < CASE WHEN end_reason = '{}' THEN 1 ELSE 2 END)
                  + (SELECT count(*) FROM lyric_jumps WHERE synced = 0 AND device_id = ?1)
                  + (SELECT count(*) FROM events WHERE synced = 0 AND device_id = ?1)",
                crate::playlog::IN_PROGRESS
            ),
            [own_device],
            |r| r.get(0),
        )?)
    }

    /// The server holds these now.
    pub fn mark_sent(&self, sent: &[OutEvent]) -> Result<()> {
        self.in_transaction(|s| {
            for e in sent {
                match e.kind.as_str() {
                    "play" => s.conn.execute(
                        "UPDATE play_events SET synced = max(synced, ?2) WHERE id = ?1",
                        params![e.id, e.version + 1],
                    )?,
                    "lyricJump" => s
                        .conn
                        .execute("UPDATE lyric_jumps SET synced = 1 WHERE id = ?1", [&e.id])?,
                    _ => s
                        .conn
                        .execute("UPDATE events SET synced = 1 WHERE id = ?1", [&e.id])?,
                };
            }
            Ok(())
        })
    }

    /// Record who last decided each song's like (called for every local like,
    /// unlike or undo of one).
    pub(crate) fn note_like_decision(
        &self,
        keys: &[String],
        liked: bool,
        hlc: &str,
        device_id: &str,
        from_here: bool,
    ) -> Result<()> {
        let mut st = self.conn.prepare_cached(
            "INSERT INTO like_clock (song_key, liked, hlc, device_id, from_here)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (song_key) DO UPDATE SET liked = excluded.liked, hlc = excluded.hlc,
                 device_id = excluded.device_id, from_here = excluded.from_here",
        )?;
        for k in keys {
            st.execute(params![k, liked, hlc, device_id, from_here])?;
        }
        Ok(())
    }

    /// Apply what other devices sent. Unknown kinds are skipped (a newer app may
    /// send more), and so is anything malformed.
    pub fn apply_remote(&self, events: &[InEvent]) -> Result<Applied> {
        self.in_transaction(|s| {
            let mut a = Applied::default();
            for e in events {
                if a.newest_hlc.as_deref().is_none_or(|n| e.hlc.as_str() > n) {
                    a.newest_hlc = Some(e.hlc.clone());
                }
                match e.kind.as_str() {
                    "play" => {
                        if s.apply_play(e)? {
                            a.plays += 1;
                        }
                    }
                    "lyricJump" => s.apply_jump(e)?,
                    "playlistCreate" => a.new_playlists = true,
                    k if k.starts_with("playlist") => match s.playlist_named_by(&e.payload)? {
                        Some(id) => {
                            if !a.playlists.contains(&id) {
                                a.playlists.push(id);
                            }
                        }
                        None => a.new_playlists = true,
                    },
                    _ => {
                        if let Some((keys, liked)) = like_effect(&e.kind, &e.payload) {
                            s.apply_like(e, &keys, liked, &mut a)?;
                        }
                    }
                }
            }
            Ok(a)
        })
    }

    fn apply_play(&self, e: &InEvent) -> Result<bool> {
        let Some(p) = play_from(e) else {
            return Ok(false);
        };
        let held: Option<String> = self
            .conn
            .query_row(
                "SELECT end_reason FROM play_events WHERE id = ?1",
                [&p.id],
                |r| r.get(0),
            )
            .optional()?;
        match held {
            None => {
                self.insert_play_event(&p)?;
            }
            // Only a version that ended replaces one still going.
            Some(r) if r == crate::playlog::IN_PROGRESS && p.end_reason != r => {
                self.conn.execute(
                    "UPDATE play_events SET listened_ms = ?2, end_reason = ?3, signal = ?4,
                         origin = ?5, seeks = ?6, updated_utc_ms = ?7 WHERE id = ?1",
                    params![
                        p.id,
                        p.listened_ms as i64,
                        p.end_reason,
                        p.signal,
                        p.origin,
                        p.seeks,
                        p.updated_utc_ms
                    ],
                )?;
            }
            Some(_) => return Ok(false),
        }
        self.conn
            .execute("UPDATE play_events SET synced = 2 WHERE id = ?1", [&p.id])?;
        Ok(true)
    }

    fn apply_jump(&self, e: &InEvent) -> Result<()> {
        let v = &e.payload;
        let (Some(play), Some(line), Some(start)) = (
            v["playId"].as_str(),
            v["lineIndex"].as_i64(),
            v["lineStartMs"].as_i64(),
        ) else {
            return Ok(());
        };
        self.conn.execute(
            "INSERT OR IGNORE INTO lyric_jumps (id, play_event_id, device_id, hlc, line_index,
                 line_start_ms, at_utc_ms, synced)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, 1 WHERE EXISTS (SELECT 1 FROM play_events WHERE id = ?2)",
            params![e.id, play, e.device_id, e.hlc, line, start, e.created_utc_ms],
        )?;
        Ok(())
    }

    /// The local playlist a playlist event names (media server ID and the
    /// playlist's ID there).
    fn playlist_named_by(&self, payload: &Value) -> Result<Option<i64>> {
        let (Some(server), Some(item)) =
            (payload["serverId"].as_str(), payload["playlist"].as_str())
        else {
            return Ok(None);
        };
        Ok(self
            .conn
            .query_row(
                "SELECT p.id FROM playlists p JOIN sources s ON s.id = p.source_id
                 WHERE s.server_id = ?1 AND p.source_item_id = ?2",
                params![server, item],
                |r| r.get(0),
            )
            .optional()?)
    }

    fn apply_like(&self, e: &InEvent, keys: &[String], liked: bool, a: &mut Applied) -> Result<()> {
        for k in keys {
            let held: Option<(bool, String, String, bool)> = self
                .conn
                .query_row(
                    "SELECT liked, hlc, device_id, from_here FROM like_clock WHERE song_key = ?1",
                    [k],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            if let Some((_, hlc, device, _)) = &held
                && !later((&e.hlc, &e.device_id), (hlc, device))
            {
                continue;
            }
            if liked {
                self.conn.execute(
                    "INSERT OR IGNORE INTO likes (song_key, liked_utc_ms) VALUES (?1, ?2)",
                    params![k, e.created_utc_ms],
                )?;
            } else {
                self.conn
                    .execute("DELETE FROM likes WHERE song_key = ?1", [k])?;
            }
            self.conn.execute(
                "UPDATE tracks SET favorite = ?2 WHERE song_key = ?1",
                params![k, liked],
            )?;
            self.note_like_decision(std::slice::from_ref(k), liked, &e.hlc, &e.device_id, false)?;
            a.likes.push((k.clone(), liked));
            if let Some((was, _, _, true)) = held
                && was != liked
            {
                a.reassert.push((k.clone(), liked));
            }
        }
        Ok(())
    }

    /// The library's songs with this key (one per album it is on).
    pub fn track_ids_for_key(&self, song_key: &str) -> Result<Vec<i64>> {
        let mut st = self
            .conn
            .prepare_cached("SELECT id FROM tracks WHERE song_key = ?1 ORDER BY id")?;
        let ids = st.query_map([song_key], |r| r.get(0))?;
        Ok(ids.collect::<rusqlite::Result<_>>()?)
    }

    /// The pull cursor (0 before the first pull).
    pub fn sync_cursor(&self) -> Result<i64> {
        Ok(self
            .setting(CURSOR_KEY)?
            .and_then(|c| c.parse().ok())
            .unwrap_or(0))
    }

    pub fn set_sync_cursor(&self, cursor: i64) -> Result<()> {
        self.set_setting(CURSOR_KEY, &cursor.to_string())
    }

    /// Keep the queue as it is now, to load (paused) when the profile opens again.
    pub fn save_queue(&self, queue_json: &str, now_utc_ms: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO saved_queue (id, queue_json, saved_utc_ms) VALUES (1, ?1, ?2)
             ON CONFLICT (id) DO UPDATE SET queue_json = excluded.queue_json,
                 saved_utc_ms = excluded.saved_utc_ms",
            params![queue_json, now_utc_ms],
        )?;
        Ok(())
    }

    pub fn saved_queue(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT queue_json FROM saved_queue WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::tests::sample_delta;
    use crate::test_support::store_with_source;
    use crate::{EventRecord, LyricJumpRecord};

    fn play(id: &str, device: &str, hlc: &str, ended: bool) -> PlayEventRecord {
        PlayEventRecord {
            id: id.into(),
            device_id: device.into(),
            hlc: hlc.into(),
            song_key: "k1".into(),
            source_kind: "jellyfin".into(),
            source_item_id: "t1".into(),
            title: "Song".into(),
            artist: "A".into(),
            album: None,
            duration_ms: 200_000,
            started_utc_ms: 1_000,
            utc_offset_minutes: 60,
            local_hour: 9,
            local_weekday: 3,
            listened_ms: if ended { 200_000 } else { 0 },
            end_reason: if ended {
                "finished"
            } else {
                crate::playlog::IN_PROGRESS
            }
            .into(),
            started_from: "album".into(),
            context_id: None,
            chosen_by: "user".into(),
            updated_utc_ms: 1_000,
            signal: ended.then(|| "finished".into()),
            origin: None,
            seeks: 0,
        }
    }

    fn incoming(o: &OutEvent, device: &str) -> InEvent {
        InEvent {
            id: o.id.clone(),
            device_id: device.into(),
            kind: o.kind.clone(),
            hlc: o.hlc.clone(),
            version: o.version,
            payload: o.payload.clone(),
            created_utc_ms: o.created_utc_ms,
        }
    }

    fn like_event(id: &str, kind: &str, keys: &[&str], hlc: &str, device: &str) -> InEvent {
        InEvent {
            id: id.into(),
            device_id: device.into(),
            kind: kind.into(),
            hlc: hlc.into(),
            version: 0,
            payload: json!({ "songKeys": keys }),
            created_utc_ms: 5,
        }
    }

    fn local_event(store: &Store, id: &str, kind: &str, payload: Value, hlc: &str) {
        store
            .insert_event(&EventRecord {
                id: id.into(),
                device_id: "phone".into(),
                hlc: hlc.into(),
                kind: kind.into(),
                payload: payload.to_string(),
                created_utc_ms: 1,
            })
            .unwrap();
    }

    #[test]
    fn a_play_is_sent_when_it_starts_and_again_when_it_ends() {
        let (store, _) = store_with_source();
        store
            .insert_play_event(&play("p1", "phone", "h1", false))
            .unwrap();
        store
            .insert_lyric_jump(&LyricJumpRecord {
                id: "j1".into(),
                play_event_id: "p1".into(),
                device_id: "phone".into(),
                hlc: "h2".into(),
                line_index: 3,
                line_start_ms: 12_000,
                at_utc_ms: 2_000,
            })
            .unwrap();
        local_event(&store, "e1", "like", json!({ "songKeys": ["k1"] }), "h3");
        let out = store.pending_out("phone", 100).unwrap();
        let kinds: Vec<&str> = out.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(
            kinds,
            ["play", "lyricJump", "like"],
            "plays first, then jumps"
        );
        assert_eq!(out[0].version, 0);
        assert_eq!(store.pending_out_count("phone").unwrap(), 3);
        store.mark_sent(&out).unwrap();
        assert!(store.pending_out("phone", 100).unwrap().is_empty());

        store
            .finish_play_event("p1", 200_000, "finished", Some("finished"), None, 9)
            .unwrap();
        let out = store.pending_out("phone", 100).unwrap();
        assert_eq!((out.len(), out[0].version), (1, 1), "sent again, ended");
        assert_eq!(out[0].payload["endReason"], "finished");
        store.mark_sent(&out).unwrap();
        assert_eq!(store.pending_out_count("phone").unwrap(), 0);
        // Another device's events are never sent from here.
        assert!(store.pending_out("mac", 100).unwrap().is_empty());
    }

    #[test]
    fn another_devices_play_joins_the_log_and_its_end_replaces_it() {
        let (phone, _) = store_with_source();
        let (mac, _) = store_with_source();
        phone
            .insert_play_event(&play("p1", "phone", "h1", false))
            .unwrap();
        let started = phone.pending_out("phone", 10).unwrap();
        assert_eq!(
            mac.apply_remote(&[incoming(&started[0], "phone")])
                .unwrap()
                .plays,
            1
        );
        assert_eq!(
            mac.play_event("p1").unwrap().unwrap().end_reason,
            crate::playlog::IN_PROGRESS
        );
        phone.mark_sent(&started).unwrap();
        phone
            .finish_play_event(
                "p1",
                150_000,
                "skipped",
                Some("skippedLate"),
                Some("swipe"),
                9,
            )
            .unwrap();
        let ended = phone.pending_out("phone", 10).unwrap();
        let a = mac.apply_remote(&[incoming(&ended[0], "phone")]).unwrap();
        assert_eq!(a.plays, 1);
        let p = mac.play_event("p1").unwrap().unwrap();
        assert_eq!(
            (p.end_reason.as_str(), p.listened_ms, p.origin.as_deref()),
            ("skipped", 150_000, Some("swipe"))
        );
        // The older version arriving late changes nothing; the Mac never sends it back.
        assert_eq!(
            mac.apply_remote(&[incoming(&started[0], "phone")])
                .unwrap()
                .plays,
            0
        );
        assert!(mac.pending_out("mac", 10).unwrap().is_empty());
    }

    #[test]
    fn likes_merge_last_writer_wins_and_the_loser_reasserts_the_winner() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let key: String = store
            .conn
            .query_row("SELECT song_key FROM tracks ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        // This device liked it at h5.
        store.like(&key, 1).unwrap();
        local_event(
            &store,
            "mine",
            "like",
            json!({ "songKeys": [key] }),
            "0000000000005-0000",
        );
        // An older unlike from the Mac loses.
        let older = like_event("m1", "unlike", &[&key], "0000000000003-0000", "mac");
        let a = store.apply_remote(&[older]).unwrap();
        assert!(a.likes.is_empty());
        assert!(store.liked_keys().unwrap().contains(&key));
        // A later unlike from the Mac wins: unliked here, and this device sends the
        // unlike to the media server too (its own like may have reached it last).
        let newer = like_event("m2", "unlike", &[&key], "0000000000009-0000", "mac");
        let a = store.apply_remote(&[newer]).unwrap();
        assert_eq!(a.likes, vec![(key.clone(), false)]);
        assert_eq!(a.reassert, vec![(key.clone(), false)]);
        assert!(!store.liked_keys().unwrap().contains(&key));
        let fav: bool = store
            .conn
            .query_row(
                "SELECT favorite FROM tracks WHERE song_key = ?1",
                [&key],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!fav);
        // A tie on the clock goes the same way everywhere: the larger device ID.
        let tie_low = like_event("a", "like", &[&key], "0000000000009-0000", "aaa");
        assert!(store.apply_remote(&[tie_low]).unwrap().likes.is_empty());
        let tie_high = like_event("z", "like", &[&key], "0000000000009-0000", "zzz");
        assert_eq!(store.apply_remote(&[tie_high]).unwrap().likes.len(), 1);
        // An undo of a like is an unlike.
        let undo = InEvent {
            payload: json!({ "undoes": "like", "songKeys": [key] }),
            ..like_event("u", "undo", &[], "0000000000010-0000", "mac")
        };
        assert_eq!(
            store.apply_remote(&[undo]).unwrap().likes,
            vec![(key, false)]
        );
    }

    #[test]
    fn playlist_events_name_the_playlist_to_re_read_and_odd_events_are_skipped() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let (id, item, server): (i64, String, String) = store
            .conn
            .query_row(
                "SELECT p.id, p.source_item_id, s.server_id FROM playlists p JOIN sources s ON s.id = p.source_id LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        let ev = |kind: &str, payload: Value| InEvent {
            payload,
            ..like_event("x", kind, &[], "h", "mac")
        };
        let a = store
            .apply_remote(&[
                ev(
                    "playlistAdd",
                    json!({ "serverId": server, "playlist": item }),
                ),
                ev(
                    "playlistMove",
                    json!({ "serverId": server, "playlist": item }),
                ),
                ev(
                    "playlistAdd",
                    json!({ "serverId": server, "playlist": "elsewhere" }),
                ),
                ev("somethingNew", json!({})),
                ev("play", json!({ "broken": true })),
            ])
            .unwrap();
        assert_eq!(a.playlists, vec![id]);
        assert!(a.new_playlists);
        assert_eq!(a.plays, 0);
        assert_eq!(a.newest_hlc.as_deref(), Some("h"));
    }

    #[test]
    fn the_cursor_and_the_saved_queue_are_kept() {
        let (store, _) = store_with_source();
        assert_eq!(store.sync_cursor().unwrap(), 0);
        store.set_sync_cursor(42).unwrap();
        assert_eq!(store.sync_cursor().unwrap(), 42);
        assert_eq!(store.saved_queue().unwrap(), None);
        store.save_queue("{\"a\":1}", 5).unwrap();
        store.save_queue("{\"a\":2}", 6).unwrap();
        assert_eq!(store.saved_queue().unwrap().as_deref(), Some("{\"a\":2}"));
    }
}
