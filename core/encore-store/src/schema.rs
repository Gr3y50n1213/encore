//! Versioned migrations (docs/adr/core-0003-store-and-migrations.md).
//!
//! `PRAGMA user_version` holds the number of migrations applied. Each migration runs
//! once, inside a transaction, in order. A released migration is never edited; a
//! change is a new migration at the end of the list.

use rusqlite::Connection;

use crate::Result;

/// Migration 1: the Phase 2 schema.
const V1: &str = r#"
-- Media servers this profile signs in to. The access token is never stored here:
-- it lives in the OS keychain or keystore, keyed by profile and source.
CREATE TABLE sources (
    id               INTEGER PRIMARY KEY,
    kind             TEXT    NOT NULL,
    name             TEXT    NOT NULL,
    base_url         TEXT    NOT NULL,
    server_id        TEXT    NOT NULL,
    server_version   TEXT,
    user_id          TEXT    NOT NULL,
    user_name        TEXT    NOT NULL,
    pinned_cert_sha256 TEXT,
    sync_cursor      TEXT,
    last_sync_utc_ms INTEGER,
    created_utc_ms   INTEGER NOT NULL,
    UNIQUE (server_id, user_id)
);

CREATE TABLE artists (
    id             INTEGER PRIMARY KEY,
    source_id      INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    source_item_id TEXT    NOT NULL,
    name           TEXT    NOT NULL,
    sort_name      TEXT    NOT NULL,
    search_name    TEXT    NOT NULL,
    mbid           TEXT,
    UNIQUE (source_id, source_item_id)
);
CREATE INDEX artists_sort ON artists (sort_name COLLATE NOCASE);
CREATE INDEX artists_search ON artists (search_name);

CREATE TABLE albums (
    id                INTEGER PRIMARY KEY,
    source_id         INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    source_item_id    TEXT    NOT NULL,
    title             TEXT    NOT NULL,
    sort_title        TEXT    NOT NULL,
    search_name       TEXT    NOT NULL,
    artist_display    TEXT    NOT NULL,
    year              INTEGER,
    mbid              TEXT,
    date_added_utc_ms INTEGER,
    has_artwork       INTEGER NOT NULL,
    UNIQUE (source_id, source_item_id)
);
CREATE INDEX albums_sort ON albums (sort_title COLLATE NOCASE);
CREATE INDEX albums_added ON albums (date_added_utc_ms);
CREATE INDEX albums_search ON albums (search_name);

CREATE TABLE album_artists (
    album_id  INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL,
    name      TEXT    NOT NULL,
    source_item_id TEXT,
    PRIMARY KEY (album_id, position)
);
CREATE INDEX album_artists_item ON album_artists (source_item_id);

CREATE TABLE tracks (
    id                INTEGER PRIMARY KEY,
    source_id         INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    source_item_id    TEXT    NOT NULL,
    title             TEXT    NOT NULL,
    sort_title        TEXT    NOT NULL,
    search_name       TEXT    NOT NULL,
    album_id          INTEGER REFERENCES albums(id) ON DELETE SET NULL,
    album_item_id     TEXT,
    album_title       TEXT,
    artist_display    TEXT    NOT NULL,
    album_artist_display TEXT NOT NULL,
    disc_number       INTEGER,
    track_number      INTEGER,
    duration_ms       INTEGER NOT NULL,
    year              INTEGER,
    container         TEXT,
    bitrate           INTEGER,
    mbid_recording    TEXT,
    song_key          TEXT    NOT NULL,
    date_added_utc_ms INTEGER,
    has_lyrics        INTEGER NOT NULL,
    track_gain_db     REAL,
    album_gain_db     REAL,
    UNIQUE (source_id, source_item_id)
);
CREATE INDEX tracks_album ON tracks (album_id, disc_number, track_number);
CREATE INDEX tracks_album_item ON tracks (source_id, album_item_id);
CREATE INDEX tracks_sort ON tracks (sort_title COLLATE NOCASE);
CREATE INDEX tracks_added ON tracks (date_added_utc_ms);
CREATE INDEX tracks_song_key ON tracks (song_key);
CREATE INDEX tracks_search ON tracks (search_name);

CREATE TABLE track_artists (
    track_id  INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL,
    name      TEXT    NOT NULL,
    source_item_id TEXT,
    PRIMARY KEY (track_id, position)
);
CREATE INDEX track_artists_item ON track_artists (source_item_id);

CREATE TABLE track_genres (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    name     TEXT    NOT NULL,
    PRIMARY KEY (track_id, name)
);
CREATE INDEX track_genres_name ON track_genres (name COLLATE NOCASE);

CREATE TABLE playlists (
    id                INTEGER PRIMARY KEY,
    source_id         INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    source_item_id    TEXT    NOT NULL,
    name              TEXT    NOT NULL,
    search_name       TEXT    NOT NULL,
    date_added_utc_ms INTEGER,
    UNIQUE (source_id, source_item_id)
);
CREATE INDEX playlists_search ON playlists (search_name);

CREATE TABLE playlist_items (
    playlist_id   INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    track_item_id TEXT    NOT NULL,
    entry_id      TEXT,
    PRIMARY KEY (playlist_id, position)
);

-- Full-text search over names (board 07). Trigrams match any part of a word.
-- rowid = item id * 4 + kind (0 track, 1 album, 2 artist, 3 playlist), so an item's
-- row is found without scanning the index.
CREATE VIRTUAL TABLE search_index USING fts5 (
    text,
    tokenize = 'trigram remove_diacritics 1'
);
-- How many names hold each trigram, so the typo pass starts from the rarest ones.
CREATE VIRTUAL TABLE search_vocab USING fts5vocab (search_index, 'row');

CREATE TABLE recent_searches (
    query       TEXT    PRIMARY KEY,
    used_utc_ms INTEGER NOT NULL
);

-- The play log (plan section 5). Append-only; a row is written when the play starts
-- and updated while it plays. Song names are copied in, because the history must
-- survive the song leaving the library. No location is ever stored.
CREATE TABLE play_events (
    id                 TEXT    PRIMARY KEY,
    device_id          TEXT    NOT NULL,
    hlc                TEXT    NOT NULL,
    song_key           TEXT    NOT NULL,
    source_kind        TEXT    NOT NULL,
    source_item_id     TEXT    NOT NULL,
    title              TEXT    NOT NULL,
    artist             TEXT    NOT NULL,
    album              TEXT,
    duration_ms        INTEGER NOT NULL,
    started_utc_ms     INTEGER NOT NULL,
    utc_offset_minutes INTEGER NOT NULL,
    local_hour         INTEGER NOT NULL,
    local_weekday      INTEGER NOT NULL,
    listened_ms        INTEGER NOT NULL,
    end_reason         TEXT    NOT NULL,
    started_from       TEXT    NOT NULL,
    context_id         TEXT,
    chosen_by          TEXT    NOT NULL,
    updated_utc_ms     INTEGER NOT NULL,
    synced             INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX play_events_started ON play_events (started_utc_ms);
CREATE INDEX play_events_song ON play_events (song_key);
CREATE INDEX play_events_open ON play_events (end_reason) WHERE end_reason = 'inProgress';

CREATE TABLE lyric_jumps (
    id             TEXT    PRIMARY KEY,
    play_event_id  TEXT    NOT NULL REFERENCES play_events(id),
    device_id      TEXT    NOT NULL,
    hlc            TEXT    NOT NULL,
    line_index     INTEGER NOT NULL,
    line_start_ms  INTEGER NOT NULL,
    at_utc_ms      INTEGER NOT NULL,
    synced         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Every migration, oldest first. Append only.
/// Migration 2: an index for every browse order, so a deep page of a large library
/// is read in index order instead of sorting all of it (measured in
/// tests/search_budget.rs: songs by artist took seconds on 50,000 songs). Each
/// index matches its ORDER BY in browse.rs column for column, collation included;
/// the row id ends every index, which is the `id` tie-break.
const V2: &str = r#"
CREATE INDEX albums_by_artist ON albums (artist_display COLLATE NOCASE, year, sort_title COLLATE NOCASE);
CREATE INDEX albums_by_year ON albums (year DESC, sort_title COLLATE NOCASE);
CREATE INDEX tracks_by_artist ON tracks (artist_display COLLATE NOCASE, album_title COLLATE NOCASE, disc_number, track_number);
CREATE INDEX tracks_by_album ON tracks (album_title COLLATE NOCASE, disc_number, track_number);
"#;

pub(crate) const MIGRATIONS: &[&str] = &[V1, V2];

/// Bring the database up to the newest schema. Returns the version it ended at.
pub(crate) fn migrate(conn: &mut Connection) -> Result<u32> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let target = MIGRATIONS.len() as u32;
    if current > target {
        return Err(crate::StoreError::NewerSchema {
            found: current,
            supported: target,
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (index + 1) as u32)?;
        tx.commit()?;
    }
    Ok(target)
}
