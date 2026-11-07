//! The server's SQLite database: profiles, devices, token hashes and the synced
//! event log. Versioned migrations through `PRAGMA user_version`, as in the core.
//!
//! Tokens are stored only as SHA-256 hashes, so a copy of the database can't be
//! used to sign in. Looking a token up by its hash also means no comparison of the
//! token itself can leak timing.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Every migration, oldest first. Append only.
const MIGRATIONS: &[&str] = &[V1];

const V1: &str = r#"
-- One row per media-server account: a profile is (media server, user ID), plan
-- section 5. The user name is shown on the admin page only.
CREATE TABLE profiles (
    id               INTEGER PRIMARY KEY,
    media_server_id  TEXT    NOT NULL,
    media_user_id    TEXT    NOT NULL,
    user_name        TEXT    NOT NULL,
    created_utc_ms   INTEGER NOT NULL,
    UNIQUE (media_server_id, media_user_id)
);
-- A device signed in to a profile. `client_device_id` is the app's own stable ID.
CREATE TABLE devices (
    id               TEXT    PRIMARY KEY,
    profile_id       INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    client_device_id TEXT    NOT NULL,
    name             TEXT    NOT NULL,
    platform         TEXT    NOT NULL,
    created_utc_ms   INTEGER NOT NULL,
    last_seen_utc_ms INTEGER NOT NULL,
    revoked_utc_ms   INTEGER,
    UNIQUE (profile_id, client_device_id)
);
CREATE TABLE access_tokens (
    hash             TEXT    PRIMARY KEY,
    device_id        TEXT    NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    expires_utc_ms   INTEGER NOT NULL
);
-- Refresh tokens rotate: each is used once. A used one coming back means it was
-- copied, so the device is revoked (stolen-token detection), except a retry: the
-- same token again within 2 minutes while its replacement is still unused (the
-- app lost the reply, say, or crashed before saving the new token).
CREATE TABLE refresh_tokens (
    hash             TEXT    PRIMARY KEY,
    device_id        TEXT    NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    expires_utc_ms   INTEGER NOT NULL,
    used_utc_ms      INTEGER,
    replaced_by      TEXT
);
-- The synced event log. `seq` is the pull cursor. An event is unique per profile
-- by its ULID; a newer `version` of the same event (a play that has since ended)
-- replaces it and gets a new `seq`, so devices that already pulled it see the end.
CREATE TABLE events (
    seq              INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    id               TEXT    NOT NULL,
    device_id        TEXT    NOT NULL,
    kind             TEXT    NOT NULL,
    hlc              TEXT    NOT NULL,
    version          INTEGER NOT NULL,
    payload          TEXT    NOT NULL,
    created_utc_ms   INTEGER NOT NULL,
    received_utc_ms  INTEGER NOT NULL,
    UNIQUE (profile_id, id)
);
CREATE INDEX events_by_profile ON events (profile_id, seq);
"#;

pub(crate) type Result<T> = std::result::Result<T, rusqlite::Error>;

/// How long the same refresh token may be retried while its replacement is unused.
pub(crate) const RETRY_WINDOW_MS: i64 = 120_000;

/// A refresh token's device, expiry, use, the device's revocation, profile and
/// app device ID.
type RefreshRow = (String, i64, Option<i64>, Option<i64>, i64, String);

/// Lower-case hex SHA-256 of a token.
pub(crate) fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A new random token: a prefix and 32 random bytes in hex.
pub(crate) fn new_token(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system's random source");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{prefix}{hex}")
}

/// A new device ID: a ULID from the time and 80 random bits.
fn new_device_id(now: i64) -> String {
    let mut r = [0u8; 16];
    getrandom::fill(&mut r).expect("the operating system's random source");
    ulid::Ulid::from_parts(now.max(0) as u64, u128::from_le_bytes(r)).to_string()
}

/// A token pair as handed to a device.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Tokens {
    pub access: String,
    pub access_expires_utc_ms: i64,
    pub refresh: String,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("access_expires_utc_ms", &self.access_expires_utc_ms)
            .finish_non_exhaustive()
    }
}

/// The device an access token belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Authed {
    pub profile_id: i64,
    pub device_id: String,
    pub client_device_id: String,
}

#[derive(Debug)]
pub(crate) enum Refreshed {
    Ok {
        tokens: Tokens,
        authed: Authed,
    },
    /// Unknown, expired, or its device revoked.
    Invalid,
    /// Used before: the device has now been revoked.
    Reused,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub created_utc_ms: i64,
    pub last_seen_utc_ms: i64,
    pub revoked: bool,
}

/// One event as devices push and pull it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: String,
    /// The device that wrote it (the app's own device ID). Ignored on push: the
    /// server uses the signed-in device's.
    #[serde(default)]
    pub device_id: String,
    pub kind: String,
    pub hlc: String,
    /// Grows when the same event changes (a play that ended). 0 for most.
    #[serde(default)]
    pub version: i64,
    pub payload: serde_json::Value,
    pub created_utc_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Status {
    pub profiles: i64,
    pub devices: i64,
    pub events: i64,
}

#[derive(Debug)]
pub(crate) struct Db {
    conn: Mutex<Connection>,
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current as usize > MIGRATIONS.len() {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "this database is from a newer server (schema {current}); this one knows {}",
            MIGRATIONS.len()
        )));
    }
    for (i, m) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(m)?;
        tx.pragma_update(None, "user_version", (i + 1) as u32)?;
        tx.commit()?;
    }
    Ok(())
}

impl Db {
    pub(crate) fn open(path: &Path) -> Result<Db> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&mut conn)?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Db {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrate(&mut conn).unwrap();
        Db {
            conn: Mutex::new(conn),
        }
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite consistent (each write is
        // one transaction), so a poisoned lock is still usable.
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The profile for a media-server account, made on first sign-in.
    pub(crate) fn upsert_profile(
        &self,
        server_id: &str,
        user_id: &str,
        user_name: &str,
        now: i64,
    ) -> Result<i64> {
        let c = self.conn();
        c.execute(
            "INSERT INTO profiles (media_server_id, media_user_id, user_name, created_utc_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (media_server_id, media_user_id) DO UPDATE SET user_name = excluded.user_name",
            params![server_id, user_id, user_name, now],
        )?;
        c.query_row(
            "SELECT id FROM profiles WHERE media_server_id = ?1 AND media_user_id = ?2",
            params![server_id, user_id],
            |r| r.get(0),
        )
    }

    /// Sign a device in (again): its row is made or un-revoked, its old tokens go,
    /// and it gets a new pair.
    pub(crate) fn sign_in_device(
        &self,
        profile_id: i64,
        client_device_id: &str,
        name: &str,
        platform: &str,
        now: i64,
        ttl: (i64, i64),
    ) -> Result<(String, Tokens)> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM devices WHERE profile_id = ?1 AND client_device_id = ?2",
                params![profile_id, client_device_id],
                |r| r.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE devices SET name = ?2, platform = ?3, last_seen_utc_ms = ?4,
                         revoked_utc_ms = NULL WHERE id = ?1",
                    params![id, name, platform, now],
                )?;
                tx.execute("DELETE FROM access_tokens WHERE device_id = ?1", [&id])?;
                tx.execute("DELETE FROM refresh_tokens WHERE device_id = ?1", [&id])?;
                id
            }
            None => {
                let id = new_device_id(now);
                tx.execute(
                    "INSERT INTO devices (id, profile_id, client_device_id, name, platform,
                         created_utc_ms, last_seen_utc_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                    params![id, profile_id, client_device_id, name, platform, now],
                )?;
                id
            }
        };
        let tokens = issue(&tx, &id, now, ttl)?;
        tx.commit()?;
        Ok((id, tokens))
    }

    /// The device an access token belongs to, if it is valid now.
    pub(crate) fn authed(&self, access_token: &str, now: i64) -> Result<Option<Authed>> {
        let c = self.conn();
        let found = c
            .query_row(
                "SELECT d.profile_id, d.id, d.client_device_id, d.last_seen_utc_ms
                 FROM access_tokens t JOIN devices d ON d.id = t.device_id
                 WHERE t.hash = ?1 AND t.expires_utc_ms > ?2 AND d.revoked_utc_ms IS NULL",
                params![hash(access_token), now],
                |r| {
                    Ok((
                        Authed {
                            profile_id: r.get(0)?,
                            device_id: r.get(1)?,
                            client_device_id: r.get(2)?,
                        },
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((authed, last_seen)) = found else {
            return Ok(None);
        };
        // "Last seen" to the minute is enough, and spares a write per request.
        if now - last_seen > 60_000 {
            c.execute(
                "UPDATE devices SET last_seen_utc_ms = ?2 WHERE id = ?1",
                params![authed.device_id, now],
            )?;
        }
        Ok(Some(authed))
    }

    /// Swap a refresh token for a new pair.
    pub(crate) fn refresh(
        &self,
        refresh_token: &str,
        now: i64,
        ttl: (i64, i64),
    ) -> Result<Refreshed> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        let found: Option<RefreshRow> = tx
            .query_row(
                "SELECT t.device_id, t.expires_utc_ms, t.used_utc_ms, d.revoked_utc_ms,
                        d.profile_id, d.client_device_id
                 FROM refresh_tokens t JOIN devices d ON d.id = t.device_id WHERE t.hash = ?1",
                [hash(refresh_token)],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((device, expires, used, revoked, profile_id, client_device_id)) = found else {
            return Ok(Refreshed::Invalid);
        };
        if let Some(used_at) = used {
            // A retry: its replacement was never used, and it's soon after.
            let replacement_unused: bool = tx.query_row(
                "SELECT count(*) > 0 FROM refresh_tokens r JOIN refresh_tokens o ON o.replaced_by = r.hash
                 WHERE o.hash = ?1 AND r.used_utc_ms IS NULL",
                [hash(refresh_token)],
                |r| r.get(0),
            )?;
            if !(replacement_unused && now - used_at <= RETRY_WINDOW_MS && revoked.is_none()) {
                revoke(&tx, &device, now)?;
                tx.commit()?;
                return Ok(Refreshed::Reused);
            }
            tx.execute(
                "DELETE FROM refresh_tokens WHERE hash = (SELECT replaced_by FROM refresh_tokens WHERE hash = ?1)",
                [hash(refresh_token)],
            )?;
        }
        if revoked.is_some() || expires <= now {
            return Ok(Refreshed::Invalid);
        }
        // Old access tokens of this device stop working with the new pair.
        tx.execute("DELETE FROM access_tokens WHERE device_id = ?1", [&device])?;
        let tokens = issue(&tx, &device, now, ttl)?;
        tx.execute(
            "UPDATE refresh_tokens SET used_utc_ms = coalesce(used_utc_ms, ?2), replaced_by = ?3
             WHERE hash = ?1",
            params![hash(refresh_token), now, hash(&tokens.refresh)],
        )?;
        tx.commit()?;
        Ok(Refreshed::Ok {
            tokens,
            authed: Authed {
                profile_id,
                device_id: device,
                client_device_id,
            },
        })
    }

    /// Whether a signed-in device is still signed in (not revoked, profile kept):
    /// checked again after a long-poll wait.
    pub(crate) fn authed_device_exists(&self, a: &Authed) -> Result<bool> {
        self.conn().query_row(
            "SELECT count(*) > 0 FROM devices WHERE id = ?1 AND revoked_utc_ms IS NULL",
            [&a.device_id],
            |r| r.get(0),
        )
    }

    pub(crate) fn devices(&self, profile_id: i64) -> Result<Vec<DeviceRow>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT id, name, platform, created_utc_ms, last_seen_utc_ms, revoked_utc_ms IS NOT NULL
             FROM devices WHERE profile_id = ?1 ORDER BY created_utc_ms, id",
        )?;
        st.query_map([profile_id], |r| {
            Ok(DeviceRow {
                id: r.get(0)?,
                name: r.get(1)?,
                platform: r.get(2)?,
                created_utc_ms: r.get(3)?,
                last_seen_utc_ms: r.get(4)?,
                revoked: r.get(5)?,
            })
        })?
        .collect()
    }

    /// Revoke one of the profile's devices. False when it isn't the profile's.
    pub(crate) fn revoke_device(&self, profile_id: i64, device_id: &str, now: i64) -> Result<bool> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        let mine: bool = tx.query_row(
            "SELECT count(*) > 0 FROM devices WHERE id = ?1 AND profile_id = ?2",
            params![device_id, profile_id],
            |r| r.get(0),
        )?;
        if mine {
            revoke(&tx, device_id, now)?;
        }
        tx.commit()?;
        Ok(mine)
    }

    /// Board 19: delete the profile and everything synced for it, on the server.
    pub(crate) fn delete_profile(&self, profile_id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM profiles WHERE id = ?1", [profile_id])?;
        Ok(())
    }

    /// Store pushed events; returns the IDs now held (new, updated, or already
    /// there), so the device can mark them sent. An older or equal version of an
    /// event already held is accepted and ignored.
    pub(crate) fn push(
        &self,
        authed: &Authed,
        events: &[Event],
        now: i64,
    ) -> Result<(Vec<String>, bool)> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        let mut accepted = Vec::with_capacity(events.len());
        let mut changed = false;
        for e in events {
            let held: Option<i64> = tx
                .query_row(
                    "SELECT version FROM events WHERE profile_id = ?1 AND id = ?2",
                    params![authed.profile_id, e.id],
                    |r| r.get(0),
                )
                .optional()?;
            match held {
                Some(v) if v >= e.version => {}
                held => {
                    if held.is_some() {
                        tx.execute(
                            "DELETE FROM events WHERE profile_id = ?1 AND id = ?2",
                            params![authed.profile_id, e.id],
                        )?;
                    }
                    tx.execute(
                        "INSERT INTO events (profile_id, id, device_id, kind, hlc, version, payload,
                             created_utc_ms, received_utc_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![
                            authed.profile_id,
                            e.id,
                            authed.client_device_id,
                            e.kind,
                            e.hlc,
                            e.version,
                            e.payload.to_string(),
                            e.created_utc_ms,
                            now
                        ],
                    )?;
                    changed = true;
                }
            }
            accepted.push(e.id.clone());
        }
        tx.commit()?;
        Ok((accepted, changed))
    }

    /// Events after `cursor` written by other devices, oldest first, at most
    /// `limit`; and the new cursor (past this device's own events too) and whether
    /// more are waiting.
    pub(crate) fn pull(
        &self,
        authed: &Authed,
        cursor: i64,
        limit: u32,
    ) -> Result<(Vec<Event>, i64, bool)> {
        let c = self.conn();
        let mut st = c.prepare_cached(
            "SELECT seq, id, device_id, kind, hlc, version, payload, created_utc_ms
             FROM events WHERE profile_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
        )?;
        let rows: Vec<(i64, Event)> = st
            .query_map(params![authed.profile_id, cursor, limit + 1], |r| {
                let payload: String = r.get(6)?;
                Ok((
                    r.get(0)?,
                    Event {
                        id: r.get(1)?,
                        device_id: r.get(2)?,
                        kind: r.get(3)?,
                        hlc: r.get(4)?,
                        version: r.get(5)?,
                        payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
                        created_utc_ms: r.get(7)?,
                    },
                ))
            })?
            .collect::<Result<_>>()?;
        let more = rows.len() > limit as usize;
        let mut next = cursor;
        let mut out = Vec::new();
        for (seq, e) in rows.into_iter().take(limit as usize) {
            next = seq;
            if e.device_id != authed.client_device_id {
                out.push(e);
            }
        }
        Ok((out, next, more))
    }

    pub(crate) fn status(&self) -> Result<Status> {
        self.conn().query_row(
            "SELECT (SELECT count(*) FROM profiles), (SELECT count(*) FROM devices
                 WHERE revoked_utc_ms IS NULL), (SELECT count(*) FROM events)",
            [],
            |r| {
                Ok(Status {
                    profiles: r.get(0)?,
                    devices: r.get(1)?,
                    events: r.get(2)?,
                })
            },
        )
    }

    /// A consistent copy while the server runs (SQLite's online backup).
    pub(crate) fn backup_to(&self, dest: &Path) -> Result<()> {
        let c = self.conn();
        let mut out = Connection::open(dest)?;
        let backup = rusqlite::backup::Backup::new(&c, &mut out)?;
        backup.run_to_completion(256, std::time::Duration::from_millis(0), None)?;
        Ok(())
    }
}

fn issue(
    tx: &rusqlite::Transaction<'_>,
    device: &str,
    now: i64,
    ttl: (i64, i64),
) -> Result<Tokens> {
    let access = new_token("eat_");
    let refresh = new_token("ert_");
    let access_expires_utc_ms = now + ttl.0;
    tx.execute(
        "INSERT INTO access_tokens (hash, device_id, expires_utc_ms) VALUES (?1, ?2, ?3)",
        params![hash(&access), device, access_expires_utc_ms],
    )?;
    tx.execute(
        "INSERT INTO refresh_tokens (hash, device_id, expires_utc_ms) VALUES (?1, ?2, ?3)",
        params![hash(&refresh), device, now + ttl.1],
    )?;
    // Used refresh tokens are kept a while to catch reuse, then pruned.
    tx.execute(
        "DELETE FROM refresh_tokens WHERE expires_utc_ms <= ?1",
        [now],
    )?;
    Ok(Tokens {
        access,
        access_expires_utc_ms,
        refresh,
    })
}

fn revoke(tx: &rusqlite::Transaction<'_>, device: &str, now: i64) -> Result<()> {
    tx.execute(
        "UPDATE devices SET revoked_utc_ms = ?2 WHERE id = ?1 AND revoked_utc_ms IS NULL",
        params![device, now],
    )?;
    tx.execute("DELETE FROM access_tokens WHERE device_id = ?1", [device])?;
    tx.execute("DELETE FROM refresh_tokens WHERE device_id = ?1", [device])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTL: (i64, i64) = (3_600_000, 86_400_000);

    fn signed_in(db: &Db, device: &str) -> (Authed, Tokens) {
        let p = db.upsert_profile("srv", "user-1", "Sam", 0).unwrap();
        let (_, t) = db
            .sign_in_device(p, device, "Phone", "ios", 0, TTL)
            .unwrap();
        (db.authed(&t.access, 1).unwrap().unwrap(), t)
    }

    fn event(id: &str, version: i64) -> Event {
        Event {
            id: id.into(),
            device_id: String::new(),
            kind: "like".into(),
            hlc: "0000000000001-0000".into(),
            version,
            payload: serde_json::json!({"songKey": "k"}),
            created_utc_ms: 5,
        }
    }

    #[test]
    fn tokens_are_kept_only_as_hashes() {
        let db = Db::open_in_memory();
        let (_, t) = signed_in(&db, "d1");
        assert!(t.access.starts_with("eat_") && t.access.len() == 68);
        assert_ne!(t.access, t.refresh);
        let c = db.conn();
        let stored: Vec<String> = c
            .prepare("SELECT hash FROM access_tokens UNION ALL SELECT hash FROM refresh_tokens")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_>>()
            .unwrap();
        assert_eq!(stored.len(), 2);
        assert!(
            stored
                .iter()
                .all(|h| h.len() == 64 && *h != t.access && *h != t.refresh)
        );
    }

    #[test]
    fn an_access_token_expires_and_refresh_rotates_once() {
        let db = Db::open_in_memory();
        let (_, t) = signed_in(&db, "d1");
        assert!(
            db.authed(&t.access, TTL.0 + 1).unwrap().is_none(),
            "expired"
        );
        assert!(db.authed("eat_nope", 1).unwrap().is_none());

        let Refreshed::Ok { tokens: t2, .. } = db.refresh(&t.refresh, 10, TTL).unwrap() else {
            panic!("refresh")
        };
        assert!(
            db.authed(&t.access, 11).unwrap().is_none(),
            "the old access token stops"
        );
        assert!(db.authed(&t2.access, 11).unwrap().is_some());

        // The same token again soon, its replacement unused: a retry (the app lost
        // the reply), so a new pair, and the unused replacement stops working.
        let Refreshed::Ok { tokens: t3, .. } = db.refresh(&t.refresh, 12, TTL).unwrap() else {
            panic!("a retry")
        };
        assert!(matches!(
            db.refresh(&t2.refresh, 13, TTL).unwrap(),
            Refreshed::Invalid
        ));
        let Refreshed::Ok { tokens: t4, .. } = db.refresh(&t3.refresh, 14, TTL).unwrap() else {
            panic!("refresh")
        };
        // The first token once more, now that its replacement was used: someone
        // copied it, so the device is revoked.
        assert!(matches!(
            db.refresh(&t.refresh, 15, TTL).unwrap(),
            Refreshed::Reused
        ));
        assert!(db.authed(&t4.access, 16).unwrap().is_none());
        assert!(matches!(
            db.refresh(&t4.refresh, 17, TTL).unwrap(),
            Refreshed::Invalid
        ));
        // A retry long after is theft too.
        let db = Db::open_in_memory();
        let (_, t) = signed_in(&db, "d1");
        db.refresh(&t.refresh, 10, TTL).unwrap();
        assert!(matches!(
            db.refresh(&t.refresh, 10 + RETRY_WINDOW_MS + 1, TTL)
                .unwrap(),
            Refreshed::Reused
        ));
        assert!(db.devices(1).unwrap()[0].revoked);
    }

    #[test]
    fn signing_in_again_un_revokes_and_replaces_tokens() {
        let db = Db::open_in_memory();
        let (a, t) = signed_in(&db, "d1");
        assert!(db.revoke_device(a.profile_id, &a.device_id, 5).unwrap());
        assert!(db.authed(&t.access, 6).unwrap().is_none());
        assert!(
            !db.revoke_device(a.profile_id + 1, &a.device_id, 5).unwrap(),
            "not yours"
        );
        let (id, t2) = db
            .sign_in_device(a.profile_id, "d1", "Phone 2", "ios", 7, TTL)
            .unwrap();
        assert_eq!(id, a.device_id, "the same device row");
        assert!(db.authed(&t2.access, 8).unwrap().is_some());
        assert_eq!(db.devices(a.profile_id).unwrap()[0].name, "Phone 2");
    }

    #[test]
    fn events_push_once_update_by_version_and_pull_from_other_devices() {
        let db = Db::open_in_memory();
        let (phone, _) = signed_in(&db, "phone");
        let (mac, _) = signed_in(&db, "mac");
        let (ids, changed) = db
            .push(&phone, &[event("e1", 0), event("e2", 0)], 1)
            .unwrap();
        assert_eq!((ids.len(), changed), (2, true));
        // Pushed again (a retry): accepted, nothing changes.
        let (ids, changed) = db.push(&phone, &[event("e1", 0)], 2).unwrap();
        assert_eq!((ids, changed), (vec!["e1".to_owned()], false));

        let (got, cursor, more) = db.pull(&mac, 0, 100).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].device_id, "phone");
        assert!(!more);
        // The phone doesn't get its own events back, but its cursor moves past them.
        let (own, phone_cursor, _) = db.pull(&phone, 0, 100).unwrap();
        assert!(own.is_empty());
        assert_eq!(phone_cursor, cursor);

        // A newer version (the play ended) comes again after the cursor.
        db.push(&phone, &[event("e1", 3)], 3).unwrap();
        let (got, _, _) = db.pull(&mac, cursor, 100).unwrap();
        assert_eq!((got.len(), got[0].version), (1, 3));
        // Paging.
        let (got, c1, more) = db.pull(&mac, 0, 1).unwrap();
        assert_eq!((got.len(), more), (1, true));
        assert!(c1 > 0);
    }

    #[test]
    fn profiles_never_see_each_others_events_and_delete_removes_everything() {
        let db = Db::open_in_memory();
        let (sam, _) = signed_in(&db, "ipad");
        let alex_p = db.upsert_profile("srv", "user-2", "Alex", 0).unwrap();
        let (_, t) = db
            .sign_in_device(alex_p, "ipad", "iPad", "ios", 0, TTL)
            .unwrap();
        let alex = db.authed(&t.access, 1).unwrap().unwrap();
        assert_ne!(sam.profile_id, alex.profile_id);
        db.push(&sam, &[event("sam-play", 0)], 1).unwrap();
        // Same physical device, other profile: nothing of Sam's.
        let (got, _, _) = db.pull(&alex, 0, 100).unwrap();
        assert!(got.is_empty());

        db.delete_profile(sam.profile_id).unwrap();
        let s = db.status().unwrap();
        assert_eq!((s.profiles, s.devices, s.events), (1, 1, 0));
        assert!(db.authed("anything", 1).unwrap().is_none());
    }

    #[test]
    fn a_backup_restores_every_row() {
        let db = Db::open_in_memory();
        let (sam, _) = signed_in(&db, "phone");
        db.push(&sam, &[event("e1", 0)], 1).unwrap();
        let dir = std::env::temp_dir().join(format!("encore-server-backup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("backup.sqlite");
        let _ = std::fs::remove_file(&file);
        db.backup_to(&file).unwrap();
        let restored = Db::open(&file).unwrap();
        assert_eq!(restored.status().unwrap(), db.status().unwrap());
        drop(restored);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_database_from_a_newer_server_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(migrate(&mut conn).is_err());
    }
}
