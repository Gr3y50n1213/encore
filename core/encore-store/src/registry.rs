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

const MIGRATIONS: &[&str] = &[R1];

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

#[cfg(test)]
mod tests {
    use super::*;

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
