//! The per-profile SQLite database, its versioned migrations, the library mirror,
//! browse and search, and the play log.
//!
//! Plan sections 4, 5 and 6. One file per profile; nothing personal is shared
//! between profiles. Tokens are never stored here (plan section 12).

#![forbid(unsafe_code)]

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

mod browse;
mod mirror;
mod playlog;
mod schema;
mod search;

pub use browse::*;
pub use mirror::ApplyStats;
pub use playlog::*;
pub use search::{SearchLimits, SearchResults, TopResult, fold_for_search};

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
    /// Open (or create) a profile database and bring it to the newest schema.
    pub fn open(path: &Path) -> Result<Store> {
        Self::setup(Connection::open(path)?)
    }

    /// A private database in memory, for tests and for Guest sessions that must
    /// leave nothing behind.
    pub fn open_in_memory() -> Result<Store> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(mut conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate(&mut conn)?;
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
