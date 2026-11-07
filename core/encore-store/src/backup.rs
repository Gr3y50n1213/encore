//! Backups, restores and the safety copy taken before a migration
//! (docs/adr/core-0003-store-and-migrations.md). Nothing here loses data: every
//! step that replaces a database first keeps the one it replaces.
//!
//! Copies use SQLite's online backup, never a file copy: the database runs in WAL
//! mode, so a plain copy of the file can miss the newest writes.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, MAIN_DB, OpenFlags};

use crate::{Result, Store, StoreError};

/// How many safety copies taken before migrations are kept per profile.
const KEEP_MIGRATION_COPIES: usize = 2;

/// `profile.sqlite` -> `profile.sqlite.<suffix>`.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Copy a live connection's database to `dest`, atomically: a half-written copy is
/// never left under the final name.
fn copy_live(conn: &Connection, dest: &Path) -> Result<()> {
    let tmp = sibling(dest, "partial");
    let _ = std::fs::remove_file(&tmp);
    conn.backup(MAIN_DB, &tmp, None::<fn(rusqlite::backup::Progress)>)?;
    std::fs::rename(&tmp, dest)
        .map_err(|e| StoreError::Backup(format!("couldn't finish the copy ({e})")))?;
    Ok(())
}

/// Keep a copy of `path` before migrating it from `from_version`, and prune old ones.
/// Called only for an existing database below the newest schema.
pub(crate) fn copy_before_migration(
    conn: &Connection,
    path: &Path,
    to_version: u32,
) -> Result<PathBuf> {
    let dest = sibling(path, &format!("before-v{to_version}"));
    copy_live(conn, &dest)?;
    prune_migration_copies(path)?;
    Ok(dest)
}

/// The safety copies of `path`, oldest version first.
pub fn migration_copies(path: &Path) -> Vec<PathBuf> {
    let Some(dir) = path.parent() else {
        return vec![];
    };
    let prefix = format!(
        "{}.before-v",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    let mut found: Vec<(u32, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let v = name.strip_prefix(&prefix)?.parse::<u32>().ok()?;
            Some((v, e.path()))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

fn prune_migration_copies(path: &Path) -> Result<()> {
    let copies = migration_copies(path);
    let extra = copies.len().saturating_sub(KEEP_MIGRATION_COPIES);
    for old in &copies[..extra] {
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

/// What a backup file holds, checked without changing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    pub schema_version: u32,
}

impl Store {
    /// Write a consistent copy of this database to `dest` (safe while it is in use).
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        copy_live(&self.conn, dest)
    }

    /// Check that `path` is an intact Encore profile database this app can open:
    /// SQLite's integrity check passes, it has the profile tables, and its schema is
    /// not newer than this app's.
    pub fn check_backup(path: &Path) -> Result<BackupInfo> {
        let bad = |why: &str| StoreError::Backup(why.to_owned());
        if !path.is_file() {
            return Err(bad("the backup file doesn't exist"));
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| bad("the file isn't a database"))?;
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|_| bad("the file isn't a database or is damaged"))?;
        if integrity != "ok" {
            return Err(bad("the database is damaged"));
        }
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let latest = Store::latest_schema_version();
        if version > latest {
            return Err(StoreError::NewerSchema {
                found: version,
                supported: latest,
            });
        }
        let has_profile_tables: bool = conn.query_row(
            "SELECT count(*) = 2 FROM sqlite_master WHERE type = 'table' AND name IN ('sources', 'play_events')",
            [],
            |r| r.get(0),
        )?;
        if version == 0 || !has_profile_tables {
            return Err(bad("the file isn't an Encore profile"));
        }
        Ok(BackupInfo {
            schema_version: version,
        })
    }

    /// Replace the database at `live` with the backup at `from`, then open it (an
    /// older backup is migrated forward). The database being replaced is kept as
    /// `<live>.before-restore`. Nothing may have `live` open.
    pub fn restore(live: &Path, from: &Path) -> Result<Store> {
        Store::check_backup(from)?;
        if live.exists() {
            let current = Connection::open(live)?;
            copy_live(&current, &sibling(live, "before-restore"))?;
            drop(current);
        }
        let source = Connection::open_with_flags(from, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let tmp = sibling(live, "restoring");
        let _ = std::fs::remove_file(&tmp);
        source.backup(MAIN_DB, &tmp, None::<fn(rusqlite::backup::Progress)>)?;
        drop(source);
        // The old database's WAL belongs to the old file; it must not be replayed
        // into the restored one.
        for suffix in ["-wal", "-shm"] {
            let mut p = live.as_os_str().to_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(p));
        }
        std::fs::rename(&tmp, live).map_err(|e| {
            StoreError::Edit(format!(
                "The backup is fine, but the profile's file couldn't be replaced ({e}). Close the app and try again."
            ))
        })?;
        Store::open(live)
    }
}

/// Open `path` with a given migration list: the copy before migrating, then the
/// migrations. `Store::open` uses the real list; tests pass a failing one.
pub(crate) fn open_migrating(path: &Path, migrations: &[&str]) -> Result<Store> {
    let existed = path.is_file();
    let conn = Connection::open(path)?;
    if existed {
        let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let target = migrations.len() as u32;
        if current > 0 && current < target {
            copy_before_migration(&conn, path, target)?;
        }
    }
    Store::setup_with(conn, migrations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewSource, schema};

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir(name: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("encore-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    fn with_data(path: &Path) -> Store {
        let s = Store::open(path).unwrap();
        s.add_source(&NewSource {
            kind: "jellyfin",
            name: "Home",
            base_url: "http://jellyfin.local:8096",
            server_id: "srv",
            server_version: Some("12.0.0"),
            user_id: "u",
            user_name: "Ana",
            pinned_cert_sha256: None,
            created_utc_ms: 1,
        })
        .unwrap();
        s.set_setting("mode", "normal").unwrap();
        s
    }

    fn dump(s: &Store) -> Vec<String> {
        let mut out = vec![];
        for table in ["sources", "settings", "play_events"] {
            let mut st = s
                .conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1"))
                .unwrap();
            let n = st.column_count();
            let rows = st
                .query_map([], |r| {
                    Ok((0..n)
                        .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                        .collect::<Vec<_>>()
                        .join("|"))
                })
                .unwrap();
            out.extend(rows.map(|r| format!("{table}: {}", r.unwrap())));
        }
        out
    }

    #[test]
    fn a_backup_taken_while_in_use_restores_every_row() {
        let d = dir("roundtrip");
        let live = d.0.join("library.sqlite");
        let store = with_data(&live);
        // Written after the last checkpoint, so it lives only in the WAL: a plain
        // file copy would miss it.
        store.set_setting("late", "in the wal").unwrap();
        let backup = d.0.join("backup.sqlite");
        store.backup_to(&backup).unwrap();
        let before = dump(&store);
        assert_eq!(
            Store::check_backup(&backup).unwrap().schema_version,
            Store::latest_schema_version()
        );

        let other = d.0.join("other").join("library.sqlite");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        let restored = Store::restore(&other, &backup).unwrap();
        assert_eq!(dump(&restored), before);
        assert_eq!(
            restored.setting("late").unwrap().as_deref(),
            Some("in the wal")
        );
    }

    #[test]
    fn restoring_keeps_the_database_it_replaces() {
        let d = dir("keep");
        let live = d.0.join("library.sqlite");
        let store = with_data(&live);
        let backup = d.0.join("backup.sqlite");
        store.backup_to(&backup).unwrap();
        store
            .set_setting("mode", "changed after the backup")
            .unwrap();
        drop(store);

        let restored = Store::restore(&live, &backup).unwrap();
        assert_eq!(restored.setting("mode").unwrap().as_deref(), Some("normal"));
        let kept = Store::open(&sibling(&live, "before-restore")).unwrap();
        assert_eq!(
            kept.setting("mode").unwrap().as_deref(),
            Some("changed after the backup")
        );
    }

    #[test]
    fn damaged_foreign_and_newer_files_are_refused_and_nothing_is_replaced() {
        let d = dir("refuse");
        let live = d.0.join("library.sqlite");
        drop(with_data(&live));

        let garbage = d.0.join("garbage.sqlite");
        std::fs::write(&garbage, b"not a database at all, just bytes").unwrap();
        assert!(matches!(
            Store::check_backup(&garbage),
            Err(StoreError::Backup(_))
        ));

        let foreign = d.0.join("foreign.sqlite");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE notes (x); PRAGMA user_version = 1;")
            .unwrap();
        assert!(matches!(
            Store::check_backup(&foreign),
            Err(StoreError::Backup(_))
        ));

        let newer = d.0.join("newer.sqlite");
        drop(with_data(&newer));
        Connection::open(&newer)
            .unwrap()
            .pragma_update(None, "user_version", 999)
            .unwrap();
        assert!(matches!(
            Store::check_backup(&newer),
            Err(StoreError::NewerSchema { found: 999, .. })
        ));

        // A refused restore leaves the live database exactly as it was.
        for bad in [&garbage, &foreign, &newer, &d.0.join("missing.sqlite")] {
            assert!(Store::restore(&live, bad).is_err());
        }
        assert_eq!(
            Store::open(&live)
                .unwrap()
                .setting("mode")
                .unwrap()
                .as_deref(),
            Some("normal")
        );
        assert!(!sibling(&live, "before-restore").exists());
    }

    #[test]
    fn a_copy_is_kept_before_migrating_and_old_copies_are_pruned() {
        let d = dir("premigrate");
        let path = d.0.join("library.sqlite");
        {
            let mut conn = Connection::open(&path).unwrap();
            schema::migrate_with(&mut conn, &schema::MIGRATIONS[..1]).unwrap();
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('v1', 'data')",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.setting("v1").unwrap().as_deref(), Some("data"));
        let copies = migration_copies(&path);
        assert_eq!(copies.len(), 1, "{copies:?}");
        let copy = Connection::open(&copies[0]).unwrap();
        let v: u32 = copy
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 1, "the copy is the database as it was before migrating");
        drop(copy);
        drop(s);

        // Opening an up-to-date database takes no copy; a fresh one neither.
        drop(Store::open(&path).unwrap());
        assert_eq!(migration_copies(&path).len(), 1);
        let fresh = d.0.join("fresh.sqlite");
        drop(Store::open(&fresh).unwrap());
        assert!(migration_copies(&fresh).is_empty());

        // Only the newest two copies are kept.
        for v in [7, 8, 9] {
            std::fs::copy(&path, sibling(&path, &format!("before-v{v}"))).unwrap();
        }
        prune_migration_copies(&path).unwrap();
        let left: Vec<String> = migration_copies(&path)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            left,
            ["library.sqlite.before-v8", "library.sqlite.before-v9"]
        );
    }

    #[test]
    fn a_failing_migration_changes_nothing_and_the_copy_stays() {
        let d = dir("failing");
        let path = d.0.join("library.sqlite");
        drop(with_data(&path));
        let mut broken: Vec<&str> = schema::MIGRATIONS.to_vec();
        broken.push("CREATE TABLE half_done (x); THIS IS NOT SQL;");
        let err = open_migrating(&path, &broken).unwrap_err();
        assert!(matches!(err, StoreError::Sqlite(_)), "{err}");

        // The database is still at the last good version, with its data, and the
        // half-run migration left nothing behind.
        let s = Store::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), Store::latest_schema_version());
        assert_eq!(s.setting("mode").unwrap().as_deref(), Some("normal"));
        let half: i64 = s
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'half_done'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(half, 0);
        let copy = migration_copies(&path);
        assert_eq!(copy.len(), 1);
        assert_eq!(
            Store::open(&copy[0])
                .unwrap()
                .setting("mode")
                .unwrap()
                .as_deref(),
            Some("normal")
        );
    }
}
