//! The device registry (board 14; ADR core-0011): one small database in the data
//! folder, outside every profile, for what the profile switcher needs before a
//! profile is opened: order, display names, avatars, PIN locks, and the device's
//! own state (the last active profile, a Guest session).
//!
//! It holds nothing from anyone's listening, and no token. A PIN is kept only as a
//! salted, iterated hash made by encore-core; it is a screen lock, not encryption
//! (plan section 6).

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{Result, StoreError};

const MIGRATIONS: &[&str] = &[R1, R2, R3];

const R1: &str = r#"
CREATE TABLE profiles (
    profile_id      TEXT    PRIMARY KEY,
    display_name    TEXT,
    sort_order      INTEGER NOT NULL,
    avatar_base_url TEXT,
    avatar_user_id  TEXT,
    avatar_tag      TEXT,
    added_utc_ms    INTEGER NOT NULL
);
CREATE TABLE pins (
    profile_id          TEXT    PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    salt                TEXT    NOT NULL,
    hash                TEXT    NOT NULL,
    failed              INTEGER NOT NULL DEFAULT 0,
    locked_until_utc_ms INTEGER NOT NULL DEFAULT 0,
    lockouts            INTEGER NOT NULL DEFAULT 0,
    required_once       INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE device_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Downloaded files on this device (Phase 8; docs/adr/core-0017). Two profiles
/// that download the same song from the same server share one file, so the file
/// is named here, outside every profile, with how many profiles hold it. Never
/// which ones: each profile's own list says what it holds (board 14).
const R2: &str = r#"
CREATE TABLE download_files (
    file_key   TEXT    PRIMARY KEY,
    file_name  TEXT    NOT NULL,
    bytes      INTEGER NOT NULL,
    refs       INTEGER NOT NULL
);
"#;

/// The sleep timer (Phase 9; board 31): one per device, not per profile, so it
/// carries on through a profile switch. It names no profile and nothing that was
/// played. `ends_at_utc_ms` is set for a timer of minutes; the end of a song or an
/// album is worked out from the queue by encore-core when it is read.
const R3: &str = r#"
CREATE TABLE sleep_timer (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    mode            TEXT    NOT NULL,
    set_utc_ms      INTEGER NOT NULL,
    ends_at_utc_ms  INTEGER
);
"#;

/// The device's sleep timer as stored (the rules are encore-core's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SleepTimerRecord {
    /// "minutes", "endOfSong" or "endOfAlbum".
    pub mode: String,
    pub set_utc_ms: i64,
    /// Set for a timer of minutes.
    pub ends_at_utc_ms: Option<i64>,
}

/// A downloaded file as the device knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFileRecord {
    pub file_key: String,
    /// Inside the downloads folder.
    pub file_name: String,
    pub bytes: i64,
    /// How many profiles hold it.
    pub refs: i64,
}

/// A profile as the switcher lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryProfile {
    pub profile_id: String,
    /// Set by the user (Manage profiles); None shows the media-server user name.
    pub display_name: Option<String>,
    pub sort_order: i64,
    /// The media server's user image: its address, the user and the image tag.
    pub avatar: Option<(String, String, String)>,
    pub added_utc_ms: i64,
}

/// A profile's PIN lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinRecord {
    pub salt: String,
    pub hash: String,
    pub failed: u32,
    pub locked_until_utc_ms: i64,
    pub lockouts: u32,
    /// The device's biometrics changed: the PIN is needed once.
    pub required_once: bool,
}

#[derive(Debug)]
pub struct Registry {
    conn: Connection,
}

impl Registry {
    pub fn open(path: &Path) -> Result<Registry> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::migrate(&mut conn)?;
        Ok(Registry { conn })
    }

    pub fn open_in_memory() -> Result<Registry> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Self::migrate(&mut conn)?;
        Ok(Registry { conn })
    }

    fn migrate(conn: &mut Connection) -> Result<()> {
        let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if current as usize > MIGRATIONS.len() {
            return Err(StoreError::NewerSchema {
                found: current,
                supported: MIGRATIONS.len() as u32,
            });
        }
        for (i, m) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(m)?;
            tx.pragma_update(None, "user_version", (i + 1) as u32)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Add a profile at the end of the list (nothing if it is there).
    pub fn ensure(&self, profile_id: &str, now_utc_ms: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO profiles (profile_id, sort_order, added_utc_ms)
             VALUES (?1, (SELECT coalesce(max(sort_order), 0) + 1 FROM profiles), ?2)",
            params![profile_id, now_utc_ms],
        )?;
        Ok(())
    }

    /// Every profile, in the user's order.
    pub fn profiles(&self) -> Result<Vec<RegistryProfile>> {
        let mut st = self.conn.prepare(
            "SELECT profile_id, display_name, sort_order, avatar_base_url, avatar_user_id,
                    avatar_tag, added_utc_ms
             FROM profiles ORDER BY sort_order, added_utc_ms, profile_id",
        )?;
        let rows = st.query_map([], |r| {
            let avatar = match (
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ) {
                (Some(b), Some(u), Some(t)) => Some((b, u, t)),
                _ => None,
            };
            Ok(RegistryProfile {
                profile_id: r.get(0)?,
                display_name: r.get(1)?,
                sort_order: r.get(2)?,
                avatar,
                added_utc_ms: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_display_name(&self, profile_id: &str, name: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE profiles SET display_name = ?2 WHERE profile_id = ?1",
            params![profile_id, name],
        )?;
        Ok(())
    }

    pub fn set_avatar(&self, profile_id: &str, avatar: Option<(&str, &str, &str)>) -> Result<()> {
        let (b, u, t) = match avatar {
            Some((b, u, t)) => (Some(b), Some(u), Some(t)),
            None => (None, None, None),
        };
        self.conn.execute(
            "UPDATE profiles SET avatar_base_url = ?2, avatar_user_id = ?3, avatar_tag = ?4
             WHERE profile_id = ?1",
            params![profile_id, b, u, t],
        )?;
        Ok(())
    }

    /// The new order; profiles not named keep their place after these.
    pub fn reorder(&self, profile_ids: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (i, id) in profile_ids.iter().enumerate() {
            tx.execute(
                "UPDATE profiles SET sort_order = ?2 WHERE profile_id = ?1",
                params![id, i as i64 - profile_ids.len() as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove(&self, profile_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM profiles WHERE profile_id = ?1", [profile_id])?;
        Ok(())
    }

    pub fn pin(&self, profile_id: &str) -> Result<Option<PinRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT salt, hash, failed, locked_until_utc_ms, lockouts, required_once
                 FROM pins WHERE profile_id = ?1",
                [profile_id],
                |r| {
                    Ok(PinRecord {
                        salt: r.get(0)?,
                        hash: r.get(1)?,
                        failed: r.get(2)?,
                        locked_until_utc_ms: r.get(3)?,
                        lockouts: r.get(4)?,
                        required_once: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn set_pin(&self, profile_id: &str, salt: &str, hash: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO pins (profile_id, salt, hash) VALUES (?1, ?2, ?3)
             ON CONFLICT (profile_id) DO UPDATE SET salt = excluded.salt, hash = excluded.hash,
                 failed = 0, locked_until_utc_ms = 0, lockouts = 0, required_once = 0",
            params![profile_id, salt, hash],
        )?;
        Ok(())
    }

    pub fn clear_pin(&self, profile_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM pins WHERE profile_id = ?1", [profile_id])?;
        Ok(())
    }

    /// Write the result of a PIN try (the rules are encore-core's).
    pub fn set_pin_state(
        &self,
        profile_id: &str,
        failed: u32,
        locked_until_utc_ms: i64,
        lockouts: u32,
        required_once: bool,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE pins SET failed = ?2, locked_until_utc_ms = ?3, lockouts = ?4, required_once = ?5
             WHERE profile_id = ?1",
            params![profile_id, failed, locked_until_utc_ms, lockouts, required_once],
        )?;
        Ok(())
    }

    /// The device's biometrics changed: every PIN is needed once.
    pub fn require_pins_once(&self) -> Result<()> {
        self.conn.execute("UPDATE pins SET required_once = 1", [])?;
        Ok(())
    }

    pub fn state(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM device_state WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn download_file(&self, file_key: &str) -> Result<Option<DownloadFileRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT file_key, file_name, bytes, refs FROM download_files WHERE file_key = ?1",
                [file_key],
                download_file_row,
            )
            .optional()?)
    }

    pub fn download_files(&self) -> Result<Vec<DownloadFileRecord>> {
        let mut st = self.conn.prepare(
            "SELECT file_key, file_name, bytes, refs FROM download_files ORDER BY file_key",
        )?;
        let rows = st.query_map([], download_file_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// One more profile holds this file (written now, or already on disk).
    pub fn hold_download_file(&self, file_key: &str, file_name: &str, bytes: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO download_files (file_key, file_name, bytes, refs) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT (file_key) DO UPDATE SET refs = refs + 1",
            params![file_key, file_name, bytes],
        )?;
        Ok(())
    }

    /// One profile lets go. The file's name comes back when no one holds it any
    /// more, for the caller to delete.
    pub fn release_download_file(&self, file_key: &str) -> Result<Option<String>> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE download_files SET refs = refs - 1 WHERE file_key = ?1",
            [file_key],
        )?;
        let gone: Option<String> = tx
            .query_row(
                "SELECT file_name FROM download_files WHERE file_key = ?1 AND refs <= 0",
                [file_key],
                |r| r.get(0),
            )
            .optional()?;
        if gone.is_some() {
            tx.execute("DELETE FROM download_files WHERE file_key = ?1", [file_key])?;
        }
        tx.commit()?;
        Ok(gone)
    }

    /// The tidy-up's count (encore-core recounts from every profile's list).
    pub fn set_download_refs(&self, file_key: &str, refs: i64) -> Result<()> {
        if refs <= 0 {
            self.conn
                .execute("DELETE FROM download_files WHERE file_key = ?1", [file_key])?;
        } else {
            self.conn.execute(
                "UPDATE download_files SET refs = ?2 WHERE file_key = ?1",
                params![file_key, refs],
            )?;
        }
        Ok(())
    }

    /// The device's sleep timer, if one is set.
    pub fn sleep_timer(&self) -> Result<Option<SleepTimerRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT mode, set_utc_ms, ends_at_utc_ms FROM sleep_timer WHERE id = 1",
                [],
                |r| {
                    Ok(SleepTimerRecord {
                        mode: r.get(0)?,
                        set_utc_ms: r.get(1)?,
                        ends_at_utc_ms: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Set the sleep timer, replacing the one there was.
    pub fn set_sleep_timer(&self, timer: &SleepTimerRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sleep_timer (id, mode, set_utc_ms, ends_at_utc_ms) VALUES (1, ?1, ?2, ?3)
             ON CONFLICT (id) DO UPDATE SET mode = excluded.mode, set_utc_ms = excluded.set_utc_ms,
                 ends_at_utc_ms = excluded.ends_at_utc_ms",
            params![timer.mode, timer.set_utc_ms, timer.ends_at_utc_ms],
        )?;
        Ok(())
    }

    /// Clear the sleep timer. Returns whether there was one.
    pub fn clear_sleep_timer(&self) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM sleep_timer", [])? > 0)
    }

    pub fn set_state(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.conn.execute(
                "INSERT INTO device_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?,
            None => self
                .conn
                .execute("DELETE FROM device_state WHERE key = ?1", [key])?,
        };
        Ok(())
    }
}

fn download_file_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadFileRecord> {
    Ok(DownloadFileRecord {
        file_key: r.get(0)?,
        file_name: r.get(1)?,
        bytes: r.get(2)?,
        refs: r.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_file_goes_only_when_the_last_profile_lets_go() {
        let r = Registry::open_in_memory().unwrap();
        r.hold_download_file("k", "k.flac", 30).unwrap();
        r.hold_download_file("k", "k.flac", 30).unwrap();
        assert_eq!(r.download_file("k").unwrap().unwrap().refs, 2);
        assert_eq!(
            r.release_download_file("k").unwrap(),
            None,
            "one profile still holds it"
        );
        assert_eq!(
            r.release_download_file("k").unwrap().as_deref(),
            Some("k.flac")
        );
        assert_eq!(r.download_file("k").unwrap(), None);
        assert_eq!(
            r.release_download_file("k").unwrap(),
            None,
            "letting go twice is harmless"
        );
        r.hold_download_file("j", "j.mp3", 5).unwrap();
        r.set_download_refs("j", 0).unwrap();
        assert!(r.download_files().unwrap().is_empty());
    }

    #[test]
    fn the_sleep_timer_is_one_row_for_the_device() {
        let r = Registry::open_in_memory().unwrap();
        assert_eq!(r.sleep_timer().unwrap(), None);
        r.set_sleep_timer(&SleepTimerRecord {
            mode: "minutes".into(),
            set_utc_ms: 10,
            ends_at_utc_ms: Some(1_800_010),
        })
        .unwrap();
        let song = SleepTimerRecord {
            mode: "endOfSong".into(),
            set_utc_ms: 20,
            ends_at_utc_ms: None,
        };
        r.set_sleep_timer(&song).unwrap();
        assert_eq!(
            r.sleep_timer().unwrap(),
            Some(song),
            "a new timer replaces the old"
        );
        assert!(r.clear_sleep_timer().unwrap());
        assert!(!r.clear_sleep_timer().unwrap(), "nothing left to clear");
        assert_eq!(r.sleep_timer().unwrap(), None);
    }

    #[test]
    fn profiles_keep_their_order_names_avatars_and_pins() {
        let r = Registry::open_in_memory().unwrap();
        r.ensure("jf-a", 1).unwrap();
        r.ensure("jf-b", 2).unwrap();
        r.ensure("jf-a", 3).unwrap();
        let ids = |r: &Registry| -> Vec<String> {
            r.profiles()
                .unwrap()
                .into_iter()
                .map(|p| p.profile_id)
                .collect()
        };
        assert_eq!(ids(&r), ["jf-a", "jf-b"]);
        r.reorder(&["jf-b".into()]).unwrap();
        assert_eq!(ids(&r), ["jf-b", "jf-a"]);
        r.set_display_name("jf-a", Some("Kids")).unwrap();
        r.set_avatar("jf-a", Some(("http://j:8096", "u1", "tag1")))
            .unwrap();
        let a = r.profiles().unwrap().pop().unwrap();
        assert_eq!(a.display_name.as_deref(), Some("Kids"));
        assert_eq!(
            a.avatar,
            Some(("http://j:8096".into(), "u1".into(), "tag1".into()))
        );

        assert_eq!(r.pin("jf-a").unwrap(), None);
        r.set_pin("jf-a", "salt", "hash").unwrap();
        r.set_pin_state("jf-a", 5, 99, 1, false).unwrap();
        r.require_pins_once().unwrap();
        let p = r.pin("jf-a").unwrap().unwrap();
        assert_eq!(
            (p.failed, p.locked_until_utc_ms, p.lockouts, p.required_once),
            (5, 99, 1, true)
        );
        // A new PIN starts clean.
        r.set_pin("jf-a", "s2", "h2").unwrap();
        assert_eq!(r.pin("jf-a").unwrap().unwrap().failed, 0);

        r.set_state("last_active", Some("jf-b")).unwrap();
        assert_eq!(r.state("last_active").unwrap().as_deref(), Some("jf-b"));
        r.set_state("last_active", None).unwrap();
        assert_eq!(r.state("last_active").unwrap(), None);

        r.remove("jf-a").unwrap();
        assert_eq!(ids(&r), ["jf-b"]);
        assert_eq!(r.pin("jf-a").unwrap(), None, "its PIN goes with it");
    }
}
