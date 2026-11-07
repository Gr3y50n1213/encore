//! The per-profile SQLite database, its versioned migrations, the library mirror,
//! browse and search, and the play log.
//!
//! Plan sections 4, 5 and 6. One file per profile; nothing personal is shared
//! between profiles. Tokens are never stored here (plan section 12).

#![forbid(unsafe_code)]

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

mod backup;
mod browse;
mod cache;
mod edits;
mod export;
mod mirror;
mod playlog;
mod schema;
mod search;
mod sync;

pub use backup::{BackupInfo, migration_copies};
pub use browse::*;
pub use cache::{CachedLyrics, CachedPalette};
pub use edits::{EventRecord, OutboxRow, PlaylistItemRow, PlaylistRecord, UndoRow};
pub use export::{EXPORT_FORMAT, EXPORT_VERSION, ExportCounts, ExportHeader};
pub use mirror::{ApplyStats, LOCAL_ITEM_PREFIX};
pub use playlog::*;
pub use search::{LyricHit, SearchLimits, SearchResults, TopResult, fold_for_search};
pub use sync::{Applied, InEvent, OutEvent, like_effect};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "this database was written by a newer version of the app (schema {found}, this app supports {supported})"
    )]
    NewerSchema { found: u32, supported: u32 },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("backup: {0}")]
    Backup(String),
    #[error("{0}")]
    Edit(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// One profile's database.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

/// A media server this profile signs in to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub server_id: String,
    pub server_version: Option<String>,
    pub user_id: String,
    pub user_name: String,
    pub pinned_cert_sha256: Option<String>,
    pub sync_cursor: Option<String>,
    pub last_sync_utc_ms: Option<i64>,
    pub created_utc_ms: i64,
}

/// What `add_source` needs.
#[derive(Debug, Clone)]
pub struct NewSource<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub base_url: &'a str,
    pub server_id: &'a str,
    pub server_version: Option<&'a str>,
    pub user_id: &'a str,
    pub user_name: &'a str,
    pub pinned_cert_sha256: Option<&'a str>,
    pub created_utc_ms: i64,
}

impl Store {
    /// Open (or create) a profile database and bring it to the newest schema. An
    /// existing database below the newest schema is copied to
    /// `<path>.before-v<N>` first (the newest two copies are kept), so a failed or
    /// faulty migration never loses data.
    pub fn open(path: &Path) -> Result<Store> {
        backup::open_migrating(path, schema::MIGRATIONS)
    }

    /// A private database in memory, for tests and for Guest sessions that must
    /// leave nothing behind.
    pub fn open_in_memory() -> Result<Store> {
        Self::setup_with(Connection::open_in_memory()?, schema::MIGRATIONS)
    }

    fn setup_with(mut conn: Connection, migrations: &[&str]) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate_with(&mut conn, migrations)?;
        Ok(Store { conn })
    }

    /// The schema version this database is at.
    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// The newest schema this build knows.
    pub fn latest_schema_version() -> u32 {
        schema::MIGRATIONS.len() as u32
    }

    // ------------------------------------------------------------- sources

    pub fn add_source(&self, s: &NewSource<'_>) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sources (kind, name, base_url, server_id, server_version, user_id, user_name,
                                  pinned_cert_sha256, created_utc_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (server_id, user_id) DO UPDATE SET
                 name = excluded.name, base_url = excluded.base_url,
                 server_version = excluded.server_version, user_name = excluded.user_name,
                 pinned_cert_sha256 = excluded.pinned_cert_sha256",
            params![
                s.kind,
                s.name,
                s.base_url,
                s.server_id,
                s.server_version,
                s.user_id,
                s.user_name,
                s.pinned_cert_sha256,
                s.created_utc_ms
            ],
        )?;
        Ok(self.conn.query_row(
            "SELECT id FROM sources WHERE server_id = ?1 AND user_id = ?2",
            params![s.server_id, s.user_id],
            |r| r.get(0),
        )?)
    }

    pub fn sources(&self) -> Result<Vec<SourceRecord>> {
        let mut st = self.conn.prepare(&format!("{SOURCE_SELECT} ORDER BY id"))?;
        let rows = st.query_map([], source_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn source(&self, id: i64) -> Result<SourceRecord> {
        self.conn
            .query_row(&format!("{SOURCE_SELECT} WHERE id = ?1"), [id], source_row)
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("source {id}")))
    }

    /// Remove a source and everything mirrored from it. Nothing on the server is
    /// touched, and the play log keeps its history (board 18, item 2).
    pub fn remove_source(&self, id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM search_index WHERE rowid IN (
                 SELECT id * 4 + 0 FROM tracks WHERE source_id = ?1
                 UNION ALL SELECT id * 4 + 1 FROM albums WHERE source_id = ?1
                 UNION ALL SELECT id * 4 + 2 FROM artists WHERE source_id = ?1
                 UNION ALL SELECT id * 4 + 3 FROM playlists WHERE source_id = ?1)",
            [id],
        )?;
        tx.execute("DELETE FROM sources WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_sync_state(
        &self,
        id: i64,
        cursor: Option<&str>,
        last_sync_utc_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sources SET sync_cursor = ?2, last_sync_utc_ms = ?3 WHERE id = ?1",
            params![id, cursor, last_sync_utc_ms],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------- settings

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

const SOURCE_SELECT: &str = "SELECT id, kind, name, base_url, server_id, server_version, user_id, user_name,
                                    pinned_cert_sha256, sync_cursor, last_sync_utc_ms, created_utc_ms
                             FROM sources";

fn source_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SourceRecord> {
    Ok(SourceRecord {
        id: r.get(0)?,
        kind: r.get(1)?,
        name: r.get(2)?,
        base_url: r.get(3)?,
        server_id: r.get(4)?,
        server_version: r.get(5)?,
        user_id: r.get(6)?,
        user_name: r.get(7)?,
        pinned_cert_sha256: r.get(8)?,
        sync_cursor: r.get(9)?,
        last_sync_utc_ms: r.get(10)?,
        created_utc_ms: r.get(11)?,
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) fn store_with_source() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let id = store
            .add_source(&NewSource {
                kind: "jellyfin",
                name: "Home",
                base_url: "https://music.example",
                server_id: "srv",
                server_version: Some("12.0.0"),
                user_id: "usr",
                user_name: "listener",
                pinned_cert_sha256: None,
                created_utc_ms: 1,
            })
            .unwrap();
        (store, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::store_with_source;

    #[test]
    fn a_new_database_reaches_the_latest_schema() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.schema_version().unwrap(),
            Store::latest_schema_version()
        );
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let dir = std::env::temp_dir().join(format!("encore-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.sqlite");
        let _ = std::fs::remove_file(&path);
        {
            let s = Store::open(&path).unwrap();
            s.set_setting("k", "v").unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), Store::latest_schema_version());
        assert_eq!(s.setting("k").unwrap().as_deref(), Some("v"));
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A profile created by a build that knew only migration 1 keeps its data and
    /// gains the browse indexes of migration 2 when a newer build opens it.
    #[test]
    fn a_version_1_database_upgrades_with_its_data() {
        let dir = std::env::temp_dir().join(format!("encore-store-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.sqlite");
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(schema::MIGRATIONS[0]).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('kept', 'yes')",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), Store::latest_schema_version());
        assert_eq!(s.setting("kept").unwrap().as_deref(), Some("yes"));
        let indexes: i64 = s
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name IN
                 ('albums_by_artist', 'albums_by_year', 'tracks_by_artist', 'tracks_by_album')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 4);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Migration 3 adds columns to tables that already hold a library and history:
    /// the rows stay, the new columns get their defaults, and the new tables work.
    #[test]
    fn a_version_2_database_with_a_library_and_history_upgrades() {
        let dir = std::env::temp_dir().join(format!("encore-store-v2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.sqlite");
        let _ = std::fs::remove_file(&path);
        {
            let mut conn = Connection::open(&path).unwrap();
            schema::migrate_with(&mut conn, &schema::MIGRATIONS[..2]).unwrap();
            conn.execute_batch(
                "INSERT INTO sources (id, kind, name, base_url, server_id, user_id, user_name, created_utc_ms)
                     VALUES (1, 'jellyfin', 'Home', 'http://h', 's', 'u', 'Ana', 0);
                 INSERT INTO albums (id, source_id, source_item_id, title, sort_title, search_name,
                                     artist_display, has_artwork)
                     VALUES (1, 1, 'al', 'Golden Hour Static', 'golden', 'golden', 'Ember Cove', 1);
                 INSERT INTO tracks (id, source_id, source_item_id, title, sort_title, search_name,
                                     album_id, artist_display, album_artist_display, duration_ms,
                                     song_key, has_lyrics)
                     VALUES (1, 1, 't', 'Kiln', 'kiln', 'kiln', 1, 'Ember Cove', 'Ember Cove', 222400,
                             't1:5a0ab3d41d0b7ae62cddb08fa411c1f1:222', 1);
                 INSERT INTO playlists (id, source_id, source_item_id, name, search_name)
                     VALUES (1, 1, 'pl', 'Road', 'road');
                 INSERT INTO play_events (id, device_id, hlc, song_key, source_kind, source_item_id,
                     title, artist, duration_ms, started_utc_ms, utc_offset_minutes, local_hour,
                     local_weekday, listened_ms, end_reason, started_from, chosen_by, updated_utc_ms)
                     VALUES ('01J', 'd', '0000000000001-00000', 'k', 'jellyfin', 't', 'Kiln', 'Ember Cove',
                             222400, 1, 0, 0, 1, 1000, 'skipped', 'album', 'user', 1);",
            )
            .unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), Store::latest_schema_version());
        let (signal, seeks, origin): (Option<String>, i64, Option<String>) = s
            .conn
            .query_row("SELECT signal, seeks, origin FROM play_events", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((signal, seeks, origin), (None, 0, None));
        let (fav, can_edit, tag): (i64, i64, Option<String>) = s
            .conn
            .query_row(
                "SELECT t.favorite, p.can_edit, a.artwork_tag FROM tracks t, playlists p, albums a",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((fav, can_edit, tag), (0, 1, None));

        // The new tables hold rows, and their clean-up rules fire.
        s.conn
            .execute_batch(
                "INSERT INTO lyrics_cache VALUES (1, 'mediaServer', 'jellyfin', '{}', 5);
                 INSERT INTO artwork_palettes VALUES ('album', 1, 'tag', '{}', 5);
                 INSERT INTO lyrics_index (rowid, text, start_ms) VALUES (4096, 'a lantern line', 0);
                 UPDATE tracks SET has_lyrics = 0 WHERE id = 1;",
            )
            .unwrap();
        let (cached, indexed): (i64, i64) = s
            .conn
            .query_row(
                "SELECT (SELECT count(*) FROM lyrics_cache), (SELECT count(*) FROM lyrics_index)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (cached, indexed),
            (0, 0),
            "lyrics are fetched and indexed again when the song's lyrics change"
        );
        s.conn
            .execute("DELETE FROM albums WHERE id = 1", [])
            .unwrap();
        let palettes: i64 = s
            .conn
            .query_row("SELECT count(*) FROM artwork_palettes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(palettes, 0, "a removed album's palette goes with it");
        assert_eq!(migration_copies(&path).len(), 1);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_schema_is_refused_rather_than_damaged() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        let err = schema::migrate(&mut conn).unwrap_err();
        assert!(matches!(err, StoreError::NewerSchema { found: 999, .. }));
    }

    #[test]
    fn full_text_search_with_trigrams_is_compiled_in() {
        let store = Store::open_in_memory().unwrap();
        store
            .conn
            .execute(
                "INSERT INTO search_index (rowid, text) VALUES (4, 'Golden Hour')",
                [],
            )
            .unwrap();
        let n: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM search_index WHERE search_index MATCH '\"den hou\"'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn adding_the_same_server_account_twice_updates_it() {
        let (store, id) = store_with_source();
        let again = store
            .add_source(&NewSource {
                kind: "jellyfin",
                name: "Renamed",
                base_url: "https://music.example:8920",
                server_id: "srv",
                server_version: Some("12.0.1"),
                user_id: "usr",
                user_name: "listener",
                pinned_cert_sha256: None,
                created_utc_ms: 2,
            })
            .unwrap();
        assert_eq!(again, id);
        let s = store.source(id).unwrap();
        assert_eq!(s.name, "Renamed");
        assert_eq!(s.created_utc_ms, 1);
        assert_eq!(store.sources().unwrap().len(), 1);
    }

    #[test]
    fn sync_state_is_saved() {
        let (store, id) = store_with_source();
        store.set_sync_state(id, Some("cursor-1"), 99).unwrap();
        let s = store.source(id).unwrap();
        assert_eq!(s.sync_cursor.as_deref(), Some("cursor-1"));
        assert_eq!(s.last_sync_utc_ms, Some(99));
    }
}
