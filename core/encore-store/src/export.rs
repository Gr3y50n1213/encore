//! Export everything a profile holds as one open JSON file (board 19, item 5;
//! PLAN-PRODUCT: "Export everything (JSON of history, likes, playlists, recaps)").
//!
//! The format is documented in docs/core/EXPORT-FORMAT.md. It is written row by
//! row, so a long play log never sits in memory, to a temporary file that is
//! renamed into place at the end: a half-written export never looks finished.
//! Tokens are never in it (they live in the keychain, not this database).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use rusqlite::types::ValueRef;
use serde_json::{Map, Value};

use crate::{Result, Store, StoreError};

/// The format's name and version, the first two fields of every export.
pub const EXPORT_FORMAT: &str = "encore-export";
pub const EXPORT_VERSION: u32 = 1;

/// What went into an export, for the "Export finished" row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExportCounts {
    pub plays: u64,
    pub likes: u64,
    pub playlists: u64,
    pub playlist_songs: u64,
    pub bytes: u64,
}

/// Who and what the export is for; the database doesn't know the profile's name.
#[derive(Debug, Clone)]
pub struct ExportHeader<'a> {
    pub profile_id: &'a str,
    pub profile_name: Option<&'a str>,
    pub app_version: &'a str,
    pub exported_utc_ms: i64,
}

/// One section: a name and the query whose rows are its objects. Column names are
/// the field names; NULL becomes null.
struct Section {
    name: &'static str,
    sql: &'static str,
}

const SECTIONS: &[Section] = &[
    Section {
        name: "servers",
        sql: "SELECT kind, name, base_url, server_id, server_version, user_name,
                     last_sync_utc_ms, created_utc_ms AS added_utc_ms
              FROM sources ORDER BY id",
    },
    Section {
        name: "likes",
        sql: "SELECT l.song_key, l.liked_utc_ms, t.title, t.artist_display AS artist,
                     t.album_title AS album, t.mbid_recording AS musicbrainz_recording
              FROM likes l
              LEFT JOIN tracks t ON t.id = (SELECT min(id) FROM tracks WHERE song_key = l.song_key)
              ORDER BY l.liked_utc_ms, l.song_key",
    },
    Section {
        name: "plays",
        sql: "SELECT id, device_id, hlc, song_key, title, artist, album, duration_ms,
                     started_utc_ms, utc_offset_minutes, listened_ms, end_reason, signal,
                     origin, seeks, started_from, context_id, chosen_by, source_kind,
                     source_item_id
              FROM play_events ORDER BY started_utc_ms, id",
    },
    Section {
        name: "lyric_jumps",
        sql: "SELECT j.play_event_id AS play_id, j.line_index, j.line_start_ms, j.at_utc_ms
              FROM lyric_jumps j ORDER BY j.at_utc_ms, j.id",
    },
    Section {
        name: "recent_searches",
        sql: "SELECT query, used_utc_ms FROM recent_searches ORDER BY used_utc_ms DESC",
    },
];

fn value(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
        // No exported column holds bytes; written as hex if one ever does.
        ValueRef::Blob(b) => Value::from(b.iter().map(|x| format!("{x:02x}")).collect::<String>()),
    }
}

/// Counts bytes as they are written.
struct Counting<W> {
    inner: W,
    bytes: u64,
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn io(e: impl std::fmt::Display) -> StoreError {
    StoreError::Backup(format!("couldn't write the export ({e})"))
}

impl Store {
    /// Write the export to `dest` (replacing it), and say what went in.
    pub fn export_to(&self, dest: &Path, header: &ExportHeader<'_>) -> Result<ExportCounts> {
        let tmp = dest.with_extension("partial");
        let result = self.write_export(&tmp, header);
        match result {
            Ok(counts) => {
                std::fs::rename(&tmp, dest).map_err(io)?;
                Ok(counts)
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
    }

    fn write_export(&self, path: &Path, h: &ExportHeader<'_>) -> Result<ExportCounts> {
        let file = File::create(path).map_err(io)?;
        let mut w = Counting {
            inner: BufWriter::new(file),
            bytes: 0,
        };
        let mut counts = ExportCounts::default();
        let head = serde_json::json!({
            "format": EXPORT_FORMAT,
            "version": EXPORT_VERSION,
            "exported_utc_ms": h.exported_utc_ms,
            "app": format!("Encore core {}", h.app_version),
            "profile": { "id": h.profile_id, "name": h.profile_name },
        });
        // The header's fields, then each section, in one object.
        let head = serde_json::to_string(&head).map_err(io)?;
        w.write_all(&head.as_bytes()[..head.len() - 1])
            .map_err(io)?;

        let mut settings = Map::new();
        {
            let mut st = self
                .conn
                .prepare("SELECT key, value FROM settings ORDER BY key")?;
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                settings.insert(r.get(0)?, Value::from(r.get::<_, String>(1)?));
            }
        }
        write!(w, ",\n\"settings\":").map_err(io)?;
        serde_json::to_writer(&mut w, &settings).map_err(io)?;

        for section in SECTIONS {
            write!(w, ",\n\"{}\":[", section.name).map_err(io)?;
            let mut st = self.conn.prepare(section.sql)?;
            let names: Vec<String> = st.column_names().map(str::to_owned).collect();
            let mut rows = st.query([])?;
            let mut n = 0u64;
            while let Some(r) = rows.next()? {
                let mut obj = Map::new();
                for (i, name) in names.iter().enumerate() {
                    obj.insert(name.clone(), value(r.get_ref(i)?));
                }
                w.write_all(if n == 0 { b"\n" } else { b",\n" })
                    .map_err(io)?;
                serde_json::to_writer(&mut w, &obj).map_err(io)?;
                n += 1;
            }
            w.write_all(b"]").map_err(io)?;
            match section.name {
                "plays" => counts.plays = n,
                "likes" => counts.likes = n,
                _ => {}
            }
        }

        // Playlists with their songs in order. A song no longer in the library keeps
        // its server ID, so the entry isn't silently lost.
        write!(w, ",\n\"playlists\":[").map_err(io)?;
        let lists: Vec<(i64, String, Option<String>, Option<i64>)> = {
            let mut st = self.conn.prepare(
                "SELECT id, name, source_item_id, date_added_utc_ms FROM playlists ORDER BY name, id",
            )?;
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut items = self.conn.prepare(
            "SELECT i.track_item_id, t.song_key, t.title, t.artist_display, t.album_title
             FROM playlist_items i
             JOIN playlists p ON p.id = i.playlist_id
             LEFT JOIN tracks t ON t.source_id = p.source_id AND t.source_item_id = i.track_item_id
             WHERE i.playlist_id = ?1 ORDER BY i.position",
        )?;
        for (n, (id, name, item_id, added)) in lists.into_iter().enumerate() {
            let songs: Vec<Value> = items
                .query_map([id], |r| {
                    Ok(serde_json::json!({
                        "server_item_id": r.get::<_, String>(0)?,
                        "song_key": r.get::<_, Option<String>>(1)?,
                        "title": r.get::<_, Option<String>>(2)?,
                        "artist": r.get::<_, Option<String>>(3)?,
                        "album": r.get::<_, Option<String>>(4)?,
                    }))
                })?
                .collect::<rusqlite::Result<_>>()?;
            counts.playlist_songs += songs.len() as u64;
            let local = item_id
                .as_deref()
                .is_none_or(|i| i.starts_with(crate::LOCAL_ITEM_PREFIX));
            let pl = serde_json::json!({
                "name": name,
                "server_item_id": if local { None } else { item_id },
                "added_utc_ms": added,
                "songs": songs,
            });
            w.write_all(if n == 0 { b"\n" } else { b",\n" })
                .map_err(io)?;
            serde_json::to_writer(&mut w, &pl).map_err(io)?;
            counts.playlists += 1;
        }
        // Recaps come in a later phase; the key is there so readers can rely on it.
        w.write_all(b"],\n\"recaps\":[]\n}\n").map_err(io)?;
        w.flush().map_err(io)?;
        let bytes = w.bytes;
        w.inner
            .into_inner()
            .map_err(|e| io(e.error()))?
            .sync_all()
            .map_err(io)?;
        counts.bytes = bytes;
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::tests::sample_delta;
    use crate::test_support::store_with_source;

    fn header() -> ExportHeader<'static> {
        ExportHeader {
            profile_id: "p1",
            profile_name: Some("Sam"),
            app_version: "0.1.0",
            exported_utc_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn everything_the_profile_holds_is_in_one_json_file() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        store.set_setting("privacy.learn", "on").unwrap();
        store.add_recent_search("lanterns \"live\"", 5, 10).unwrap();
        store
            .conn
            .execute_batch(
                "INSERT INTO likes (song_key, liked_utc_ms)
                     SELECT song_key, 42 FROM tracks ORDER BY id LIMIT 1;
                 INSERT INTO likes (song_key, liked_utc_ms) VALUES ('gone|song|1', 43);
                 INSERT INTO play_events (id, device_id, hlc, song_key, source_kind,
                     source_item_id, title, artist, album, duration_ms, started_utc_ms,
                     utc_offset_minutes, local_hour, local_weekday, listened_ms, end_reason,
                     started_from, context_id, chosen_by, updated_utc_ms, synced, signal,
                     origin, seeks)
                 VALUES ('01J', 'dev', 'h', 'k', 'jellyfin', 't1', 'Café ☕ \"Song\"', 'A',
                     'B', 200000, 1000, -300, 9, 2, 150000, 'skipped', 'album', 'al1', 'user',
                     1000, 0, 'skippedLate', 'swipe', 1);",
            )
            .unwrap();
        let dir = std::env::temp_dir().join(format!("encore-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("export.json");
        let counts = store.export_to(&dest, &header()).unwrap();
        let text = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(counts.bytes, text.len() as u64);
        assert!(!dir.join("export.partial").exists());

        let v: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(v["format"], EXPORT_FORMAT);
        assert_eq!(v["version"], EXPORT_VERSION);
        assert_eq!(v["profile"]["name"], "Sam");
        assert_eq!(v["settings"]["privacy.learn"], "on");
        assert_eq!(v["servers"][0]["kind"], "jellyfin");
        assert_eq!(v["recent_searches"][0]["query"], "lanterns \"live\"");
        assert_eq!(v["recaps"], Value::Array(vec![]));

        // Plays keep every field, text exactly (quotes, accents, emoji).
        assert_eq!(counts.plays, 1);
        let play = &v["plays"][0];
        assert_eq!(play["title"], "Café ☕ \"Song\"");
        assert_eq!(play["utc_offset_minutes"], -300);
        assert_eq!(play["signal"], "skippedLate");
        assert_eq!(play["origin"], "swipe");

        // A like whose song left the library is kept, with its key.
        assert_eq!(counts.likes, 2);
        let likes = v["likes"].as_array().unwrap();
        assert!(likes[0]["title"].is_string());
        assert_eq!(likes[1]["song_key"], "gone|song|1");
        assert_eq!(likes[1]["title"], Value::Null);

        // Playlists, songs in order with their names.
        let lists = v["playlists"].as_array().unwrap();
        assert_eq!(counts.playlists, lists.len() as u64);
        let songs: u64 = lists
            .iter()
            .map(|p| p["songs"].as_array().unwrap().len() as u64)
            .sum();
        assert_eq!(counts.playlist_songs, songs);
        assert!(songs > 0);
        assert!(lists[0]["songs"][0]["title"].is_string());

        // Exporting again replaces the file.
        store.conn.execute("DELETE FROM play_events", []).unwrap();
        assert_eq!(store.export_to(&dest, &header()).unwrap().plays, 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_export_that_cannot_be_written_leaves_nothing_behind() {
        let (store, _) = store_with_source();
        let dest = std::env::temp_dir()
            .join(format!("encore-no-such-dir-{}", std::process::id()))
            .join("export.json");
        assert!(matches!(
            store.export_to(&dest, &header()),
            Err(StoreError::Backup(_))
        ));
        assert!(!dest.exists());
    }
}
