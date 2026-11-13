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

/// Migration 3: Phase 3. Lyrics and palettes cached per item, the sync event log,
/// the outbox of server writes, likes, undo, and the skip signal on each play.
const V3: &str = r#"
-- Artwork changes refresh the palette made from it; a shared playlist can be
-- read-only; favourites come from the server's user data.
ALTER TABLE albums    ADD COLUMN artwork_tag TEXT;
ALTER TABLE playlists ADD COLUMN artwork_tag TEXT;
ALTER TABLE playlists ADD COLUMN can_edit INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tracks    ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;
CREATE INDEX tracks_favorite ON tracks (favorite) WHERE favorite = 1;

-- Lyrics as parsed, so the lyrics view works offline once a song's lyrics were
-- fetched. model_json NULL: the source has none for this song.
CREATE TABLE lyrics_cache (
    track_id       INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    origin         TEXT    NOT NULL,
    format         TEXT    NOT NULL,
    model_json     TEXT,
    fetched_utc_ms INTEGER NOT NULL
);
CREATE TRIGGER lyrics_cache_stale AFTER UPDATE OF has_lyrics, duration_ms ON tracks
WHEN OLD.has_lyrics IS NOT NEW.has_lyrics OR OLD.duration_ms IS NOT NEW.duration_ms
BEGIN
    DELETE FROM lyrics_cache WHERE track_id = NEW.id;
END;

-- One colour palette per album or playlist (board 04), made once from the artwork
-- with the tag it was made from. palette_json NULL: no artwork.
CREATE TABLE artwork_palettes (
    item_kind    TEXT    NOT NULL,
    item_id      INTEGER NOT NULL,
    artwork_tag  TEXT,
    palette_json TEXT,
    made_utc_ms  INTEGER NOT NULL,
    PRIMARY KEY (item_kind, item_id)
);
CREATE TRIGGER albums_palette_gone AFTER DELETE ON albums
BEGIN
    DELETE FROM artwork_palettes WHERE item_kind = 'album' AND item_id = OLD.id;
END;
CREATE TRIGGER playlists_palette_gone AFTER DELETE ON playlists
BEGIN
    DELETE FROM artwork_palettes WHERE item_kind = 'playlist' AND item_id = OLD.id;
END;

-- The local event log that Phase 4 syncs between devices (plan section 5): one
-- row per user action, with a ULID, the device and a hybrid logical clock.
CREATE TABLE events (
    id             TEXT    PRIMARY KEY,
    device_id      TEXT    NOT NULL,
    hlc            TEXT    NOT NULL,
    kind           TEXT    NOT NULL,
    payload        TEXT    NOT NULL,
    created_utc_ms INTEGER NOT NULL,
    synced         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX events_unsynced ON events (hlc) WHERE synced = 0;

-- Writes to a media server that have not been accepted yet, in the order they
-- must be sent. A write survives a crash or a closed app and is sent on the next
-- chance; `edit_id` ties it to its event and undo entry.
CREATE TABLE outbox (
    seq             INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id       INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    edit_id         TEXT    NOT NULL,
    -- The playlist the write changes, if any: a sync leaves its songs alone until
    -- the write is accepted.
    playlist_id     INTEGER,
    op              TEXT    NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    next_try_utc_ms INTEGER NOT NULL DEFAULT 0,
    state           TEXT    NOT NULL DEFAULT 'pending'
);
CREATE INDEX outbox_pending ON outbox (source_id, seq) WHERE state = 'pending';

-- Liked songs, by song key: the same song on two albums is liked once.
CREATE TABLE likes (
    song_key     TEXT    PRIMARY KEY,
    liked_utc_ms INTEGER NOT NULL
);

-- What undoing an edit does, until it expires.
CREATE TABLE undo_entries (
    edit_id        TEXT    PRIMARY KEY,
    kind           TEXT    NOT NULL,
    inverse        TEXT    NOT NULL,
    expires_utc_ms INTEGER NOT NULL,
    undone         INTEGER NOT NULL DEFAULT 0
);

-- The skip signal (plan section 21): how the play is read, what moved on from it,
-- and how many times the listener seeked. NULL for plays logged before Phase 3.
ALTER TABLE play_events ADD COLUMN signal TEXT;
ALTER TABLE play_events ADD COLUMN origin TEXT;
ALTER TABLE play_events ADD COLUMN seeks INTEGER NOT NULL DEFAULT 0;
"#;

/// Migration 4: which edit put each playlist row there, so undoing that edit
/// removes exactly those rows even after other edits moved them. A pull from the
/// server clears it (the server's rows are nobody's edit).
const V4: &str = r#"
ALTER TABLE playlist_items ADD COLUMN edit_id TEXT;
"#;

/// Migration 5: lyrics search (board 07: "lyrics included when the server has
/// them"). One row per lyric line, so a match can show and jump to its line. The
/// rowid is `track_id * 4096 + line`, so a song's lines are one rowid range; they
/// go when its kept lyrics go.
///
/// Words, not trigrams (the name index's choice): lyrics are searched by words and
/// the start of the word being typed, and on 400,000 lines a trigram phrase reads
/// every trigram's huge list (174 ms for a long phrase; the budget is 50).
///
/// Lyrics kept before this migration were never indexed, so they are let go and
/// fetched again (and indexed) when next shown or fetched ahead. No app with
/// migration 3 has shipped, so in practice this drops nothing.
const V5: &str = r#"
DELETE FROM lyrics_cache;
CREATE VIRTUAL TABLE lyrics_index USING fts5 (
    text,
    start_ms UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);
CREATE TRIGGER lyrics_index_gone AFTER DELETE ON lyrics_cache
BEGIN
    DELETE FROM lyrics_index WHERE rowid BETWEEN OLD.track_id * 4096 AND OLD.track_id * 4096 + 4095;
END;
"#;

/// Migration 6: sync with the companion server (Phase 4), and the queue kept per
/// profile (board 14: "the new profile's last queue loads paused").
///
/// - `play_events.synced` now counts what was sent: 0 nothing, 1 sent while the
///   play was going, 2 sent once ended (plays from other devices arrive as 2).
/// - `like_clock` holds, per song, the clock of the last like or unlike that
///   decided it, so likes merge last-writer-wins (plan section 5). `from_here` is
///   1 when this device decided it: if another device's later decision wins, this
///   device sends the winner to the media server too, so it ends where the clock
///   says even if this device's own write reached it last.
const V6: &str = r#"
CREATE INDEX play_events_unsent ON play_events (hlc) WHERE synced < 2;
CREATE INDEX lyric_jumps_unsent ON lyric_jumps (hlc) WHERE synced = 0;
CREATE TABLE like_clock (
    song_key   TEXT    PRIMARY KEY,
    liked      INTEGER NOT NULL,
    hlc        TEXT    NOT NULL,
    device_id  TEXT    NOT NULL,
    from_here  INTEGER NOT NULL
);
CREATE TABLE saved_queue (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    queue_json    TEXT    NOT NULL,
    saved_utc_ms  INTEGER NOT NULL
);
"#;

/// Migration 7: smart picks (Phase 5; docs/adr/core-0012).
///
/// - The media server's own play counts and last-played times per song (from
///   other apps): the engine's cold-start prior.
/// - `not_interested`: one row per item (a song key or an artist), last write
///   wins by (clock, device) when devices disagree; `removed` keeps a removal so
///   an older add from another device can't bring it back.
/// - `feedback`: More like this and Fewer like this, appended.
/// - `recs_state`: the learning epoch, the mix set and its pins, hides and titles.
const V7: &str = r#"
ALTER TABLE tracks ADD COLUMN server_play_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN server_last_played_utc_ms INTEGER;
CREATE TABLE not_interested (
    scope       TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    label       TEXT    NOT NULL,
    reason      TEXT    NOT NULL,
    bucket      INTEGER NOT NULL,
    at_utc_ms   INTEGER NOT NULL,
    hlc         TEXT    NOT NULL,
    device_id   TEXT    NOT NULL,
    removed     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, value)
);
CREATE TABLE feedback (
    id          TEXT    PRIMARY KEY,
    song_key    TEXT    NOT NULL,
    more        INTEGER NOT NULL,
    bucket      INTEGER NOT NULL,
    at_utc_ms   INTEGER NOT NULL
);
CREATE TABLE recs_state (
    key         TEXT    PRIMARY KEY,
    value       TEXT    NOT NULL
);
"#;

/// Migration 8: downloads (Phase 8; docs/adr/core-0017).
///
/// - `download_collections`: what this profile asked to keep: a song, an album,
///   a playlist, a mix, Liked songs or Smart downloads (`item` is '' for the last
///   two). `auto` marks the ones the app keeps by itself (Liked songs, Smart).
/// - `download_members`: the songs each one holds now, in its order; recomputed
///   when the library, likes, a playlist or a mix change.
/// - `download_songs`: each wanted song once, however many collections hold it,
///   with its file's key (the file itself is named in the device registry, so two
///   profiles share it on disk but never this list; board 14).
/// - `download_covers`: a downloaded album's cover, for showing it offline.
/// - `download_signals`: songs the person chose to download (the +1.5 "added"
///   signal, section 21). Nothing is written under Private listening.
///
/// No foreign key to `tracks`: a song that leaves the library must still be found
/// here, so its file is let go.
const V8: &str = r#"
CREATE TABLE download_collections (
    kind          TEXT    NOT NULL,
    item          TEXT    NOT NULL,
    title         TEXT    NOT NULL,
    added_utc_ms  INTEGER NOT NULL,
    auto          INTEGER NOT NULL DEFAULT 0,
    paused        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, item)
);
CREATE TABLE download_members (
    kind      TEXT    NOT NULL,
    item      TEXT    NOT NULL,
    track_id  INTEGER NOT NULL,
    position  INTEGER NOT NULL,
    PRIMARY KEY (kind, item, track_id),
    FOREIGN KEY (kind, item) REFERENCES download_collections (kind, item) ON DELETE CASCADE
);
CREATE INDEX download_members_by_track ON download_members (track_id);
CREATE TABLE download_songs (
    track_id       INTEGER PRIMARY KEY,
    file_key       TEXT    NOT NULL,
    state          TEXT    NOT NULL,
    bytes          INTEGER NOT NULL DEFAULT 0,
    attempts       INTEGER NOT NULL DEFAULT 0,
    error          TEXT,
    queued_utc_ms  INTEGER NOT NULL,
    saved_utc_ms   INTEGER
);
CREATE TABLE download_covers (
    album_id   INTEGER PRIMARY KEY,
    file_key   TEXT    NOT NULL,
    bytes      INTEGER NOT NULL
);
CREATE TABLE download_signals (
    song_key    TEXT    NOT NULL,
    at_utc_ms   INTEGER NOT NULL
);
"#;

/// Migration 9: sound analysis (Phase 10; docs/adr/core-0019).
///
/// One row per song key: what the song sounds like, made once from about 30 seconds of its audio.
/// `profile` is the basic profile (tempo, key, loudness, energy, brightness, texture) as
/// `encore_analysis` encodes it; `embedding` is a neural model's vector for "sounds like", with the
/// model's name in `model` so a better model can replace it. `unavailable` marks audio that could
/// not be read, with the reason; such a song is never guessed at. No foreign key to `tracks`, as
/// with downloads: a row about a song that left the library is harmless and is not counted.
const V9: &str = r#"
CREATE TABLE sound_profiles (
    song_key         TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    state            TEXT    NOT NULL,
    reason           TEXT,
    profile          BLOB,
    model            TEXT,
    embedding        BLOB,
    analysed_utc_ms  INTEGER NOT NULL
);
"#;

/// Every migration, oldest first. Append only.
pub(crate) const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5, V6, V7, V8, V9];

/// Bring the database up to the newest schema. Returns the version it ended at.
#[cfg(test)]
pub(crate) fn migrate(conn: &mut Connection) -> Result<u32> {
    migrate_with(conn, MIGRATIONS)
}

/// `migrate` over a given list, so tests can prove what a failing migration does.
pub(crate) fn migrate_with(conn: &mut Connection, migrations: &[&str]) -> Result<u32> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let target = migrations.len() as u32;
    if current > target {
        return Err(crate::StoreError::NewerSchema {
            found: current,
            supported: target,
        });
    }
    for (index, sql) in migrations.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (index + 1) as u32)?;
        tx.commit()?;
    }
    Ok(target)
}
