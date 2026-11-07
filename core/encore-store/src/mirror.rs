//! Applying a library sync to the mirror (plan section 4: deltas, not rescans).
//!
//! `apply_delta` runs in one transaction and is idempotent: applying the same delta
//! twice leaves the database exactly as applying it once.

use std::collections::HashSet;

use encore_sources::{Album, Artist, ArtistCredit, LibraryDelta, Playlist, Track};
use rusqlite::{Transaction, params};

use crate::search::fold_for_search;
use crate::{Result, Store};

/// What one `apply_delta` changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyStats {
    pub artists: usize,
    pub albums: usize,
    pub tracks: usize,
    pub playlists: usize,
    pub removed: usize,
}

pub(crate) const KIND_TRACK: i64 = 0;
pub(crate) const KIND_ALBUM: i64 = 1;
pub(crate) const KIND_ARTIST: i64 = 2;
pub(crate) const KIND_PLAYLIST: i64 = 3;

/// The search index row of an item (see the schema).
pub(crate) fn index_rowid(id: i64, kind: i64) -> i64 {
    id * 4 + kind
}

impl Store {
    /// Apply one sync from `source_id`. A full delta replaces the source's whole
    /// mirror: anything not in it is removed.
    pub fn apply_delta(&self, source_id: i64, d: &LibraryDelta) -> Result<ApplyStats> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stats = ApplyStats::default();
        for a in &d.artists {
            upsert_artist(&tx, source_id, a)?;
            stats.artists += 1;
        }
        for a in &d.albums {
            upsert_album(&tx, source_id, a)?;
            stats.albums += 1;
        }
        for t in &d.tracks {
            upsert_track(&tx, source_id, t)?;
            stats.tracks += 1;
        }
        for p in &d.playlists {
            upsert_playlist(&tx, source_id, p)?;
            stats.playlists += 1;
        }
        // An album can arrive in a later page than its tracks.
        tx.execute(
            "UPDATE tracks SET album_id = (SELECT a.id FROM albums a
                                           WHERE a.source_id = tracks.source_id
                                             AND a.source_item_id = tracks.album_item_id)
             WHERE source_id = ?1 AND album_id IS NULL AND album_item_id IS NOT NULL",
            [source_id],
        )?;
        let removed: Vec<&str> = if d.full {
            Vec::new()
        } else {
            d.removed_item_ids.iter().map(String::as_str).collect()
        };
        stats.removed += remove_items(&tx, source_id, &removed)?;
        if d.full {
            stats.removed += remove_missing(&tx, source_id, d)?;
        }
        if let Some(favorites) = &d.favorite_item_ids {
            set_favorites(&tx, source_id, favorites)?;
        }
        if let Some(counts) = &d.play_counts {
            // A whole set, like favourites: songs not in it have no plays elsewhere.
            tx.execute(
                "UPDATE tracks SET server_play_count = 0, server_last_played_utc_ms = NULL
                 WHERE source_id = ?1",
                [source_id],
            )?;
            let mut st = tx.prepare_cached(
                "UPDATE tracks SET server_play_count = ?3, server_last_played_utc_ms = ?4
                 WHERE source_id = ?1 AND source_item_id = ?2",
            )?;
            for (item, count, last) in counts {
                st.execute(rusqlite::params![source_id, item, count, last])?;
            }
        }
        tx.commit()?;
        Ok(stats)
    }

    /// Every item ID mirrored from a source, for working out removals.
    pub fn mirrored_item_ids(&self, source_id: i64) -> Result<HashSet<String>> {
        let mut st = self.conn.prepare(
            "SELECT source_item_id FROM tracks WHERE source_id = ?1
             UNION ALL SELECT source_item_id FROM albums WHERE source_id = ?1
             UNION ALL SELECT source_item_id FROM artists WHERE source_id = ?1
             UNION ALL SELECT source_item_id FROM playlists WHERE source_id = ?1",
        )?;
        let ids = st.query_map([source_id], |r| r.get::<_, String>(0))?;
        Ok(ids.collect::<rusqlite::Result<_>>()?)
    }
}

fn display(credits: &[ArtistCredit]) -> String {
    credits
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn reindex(tx: &Transaction<'_>, id: i64, kind: i64, text: &str) -> Result<()> {
    let rowid = index_rowid(id, kind);
    tx.prepare_cached("DELETE FROM search_index WHERE rowid = ?1")?
        .execute([rowid])?;
    tx.prepare_cached("INSERT INTO search_index (rowid, text) VALUES (?1, ?2)")?
        .execute(params![rowid, text])?;
    Ok(())
}

fn upsert_artist(tx: &Transaction<'_>, source_id: i64, a: &Artist) -> Result<()> {
    let id: i64 = tx
        .prepare_cached(
            "INSERT INTO artists (source_id, source_item_id, name, sort_name, search_name, mbid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (source_id, source_item_id) DO UPDATE SET
                 name = excluded.name, sort_name = excluded.sort_name,
                 search_name = excluded.search_name, mbid = excluded.mbid
             RETURNING id",
        )?
        .query_row(
            params![
                source_id,
                a.source_item_id,
                a.name,
                a.sort_name,
                fold_for_search(&a.name),
                a.mbid
            ],
            |r| r.get(0),
        )?;
    reindex(tx, id, KIND_ARTIST, &a.name)
}

fn upsert_album(tx: &Transaction<'_>, source_id: i64, a: &Album) -> Result<()> {
    let id: i64 = tx
        .prepare_cached(
            "INSERT INTO albums (source_id, source_item_id, title, sort_title, search_name, artist_display,
                                 year, mbid, date_added_utc_ms, has_artwork, artwork_tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT (source_id, source_item_id) DO UPDATE SET
                 title = excluded.title, sort_title = excluded.sort_title,
                 search_name = excluded.search_name, artist_display = excluded.artist_display,
                 year = excluded.year, mbid = excluded.mbid,
                 date_added_utc_ms = excluded.date_added_utc_ms, has_artwork = excluded.has_artwork,
                 artwork_tag = excluded.artwork_tag
             RETURNING id",
        )?
        .query_row(
            params![
                source_id,
                a.source_item_id,
                a.title,
                a.sort_title,
                fold_for_search(&a.title),
                display(&a.album_artists),
                a.year,
                a.mbid,
                a.date_added_utc_ms,
                a.has_artwork,
                a.artwork_tag
            ],
            |r| r.get(0),
        )?;
    tx.prepare_cached("DELETE FROM album_artists WHERE album_id = ?1")?
        .execute([id])?;
    let mut ins = tx.prepare_cached(
        "INSERT INTO album_artists (album_id, position, name, source_item_id) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (i, c) in a.album_artists.iter().enumerate() {
        ins.execute(params![id, i as i64, c.name, c.source_item_id])?;
    }
    reindex(tx, id, KIND_ALBUM, &a.title)
}

fn upsert_track(tx: &Transaction<'_>, source_id: i64, t: &Track) -> Result<()> {
    let album_id: Option<i64> = match &t.album_item_id {
        Some(item) => tx
            .prepare_cached("SELECT id FROM albums WHERE source_id = ?1 AND source_item_id = ?2")?
            .query_row(params![source_id, item], |r| r.get(0))
            .ok(),
        None => None,
    };
    let id: i64 = tx
        .prepare_cached(
            "INSERT INTO tracks (source_id, source_item_id, title, sort_title, search_name, album_id,
                                 album_item_id, album_title, artist_display, album_artist_display,
                                 disc_number, track_number, duration_ms, year, container, bitrate,
                                 mbid_recording, song_key, date_added_utc_ms, has_lyrics,
                                 track_gain_db, album_gain_db)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                     ?19, ?20, ?21, ?22)
             ON CONFLICT (source_id, source_item_id) DO UPDATE SET
                 title = excluded.title, sort_title = excluded.sort_title,
                 search_name = excluded.search_name, album_id = excluded.album_id,
                 album_item_id = excluded.album_item_id, album_title = excluded.album_title,
                 artist_display = excluded.artist_display,
                 album_artist_display = excluded.album_artist_display,
                 disc_number = excluded.disc_number, track_number = excluded.track_number,
                 duration_ms = excluded.duration_ms, year = excluded.year,
                 container = excluded.container, bitrate = excluded.bitrate,
                 mbid_recording = excluded.mbid_recording, song_key = excluded.song_key,
                 date_added_utc_ms = excluded.date_added_utc_ms, has_lyrics = excluded.has_lyrics,
                 track_gain_db = excluded.track_gain_db, album_gain_db = excluded.album_gain_db
             RETURNING id",
        )?
        .query_row(
            params![
                source_id,
                t.source_item_id,
                t.title,
                t.sort_title,
                fold_for_search(&t.title),
                album_id,
                t.album_item_id,
                t.album_title,
                display(&t.artists),
                display(&t.album_artists),
                t.disc_number,
                t.track_number,
                t.duration_ms as i64,
                t.year,
                t.container,
                t.bitrate,
                t.mbid_recording,
                t.song_key(),
                t.date_added_utc_ms,
                t.has_lyrics,
                t.track_gain_db,
                t.album_gain_db
            ],
            |r| r.get(0),
        )?;
    tx.prepare_cached("DELETE FROM track_artists WHERE track_id = ?1")?
        .execute([id])?;
    let mut ins = tx.prepare_cached(
        "INSERT INTO track_artists (track_id, position, name, source_item_id) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (i, c) in t.artists.iter().enumerate() {
        ins.execute(params![id, i as i64, c.name, c.source_item_id])?;
    }
    tx.prepare_cached("DELETE FROM track_genres WHERE track_id = ?1")?
        .execute([id])?;
    let mut genre =
        tx.prepare_cached("INSERT OR IGNORE INTO track_genres (track_id, name) VALUES (?1, ?2)")?;
    for g in &t.genres {
        genre.execute(params![id, g])?;
    }
    reindex(tx, id, KIND_TRACK, &t.title)
}

fn upsert_playlist(tx: &Transaction<'_>, source_id: i64, p: &Playlist) -> Result<()> {
    let id: i64 = tx
        .prepare_cached(
            "INSERT INTO playlists (source_id, source_item_id, name, search_name, date_added_utc_ms,
                                    artwork_tag, can_edit)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (source_id, source_item_id) DO UPDATE SET
                 name = excluded.name, search_name = excluded.search_name,
                 date_added_utc_ms = excluded.date_added_utc_ms,
                 artwork_tag = excluded.artwork_tag, can_edit = excluded.can_edit
             RETURNING id",
        )?
        .query_row(
            params![
                source_id,
                p.source_item_id,
                p.name,
                fold_for_search(&p.name),
                p.date_added_utc_ms,
                p.artwork_tag,
                p.can_edit
            ],
            |r| r.get(0),
        )?;
    reindex(tx, id, KIND_PLAYLIST, &p.name)?;
    // Edits made on this device and not yet accepted by the server win: the pull
    // must not put back the songs as they were before them.
    let unsent: bool = tx
        .prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM outbox WHERE playlist_id = ?1 AND state = 'pending')",
        )?
        .query_row([id], |r| r.get(0))?;
    if unsent {
        return Ok(());
    }
    tx.prepare_cached("DELETE FROM playlist_items WHERE playlist_id = ?1")?
        .execute([id])?;
    let mut ins = tx.prepare_cached(
        "INSERT INTO playlist_items (playlist_id, position, track_item_id, entry_id) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (i, e) in p.entries.iter().enumerate() {
        ins.execute(params![id, i as i64, e.track_item_id, e.entry_id])?;
    }
    Ok(())
}

/// The ID a playlist made on this device has until the server gives it its own.
pub const LOCAL_ITEM_PREFIX: &str = "local:";

/// Mark exactly `item_ids` as this source's favourites.
fn set_favorites(tx: &Transaction<'_>, source_id: i64, item_ids: &[String]) -> Result<()> {
    tx.prepare_cached("UPDATE tracks SET favorite = 0 WHERE source_id = ?1 AND favorite = 1")?
        .execute([source_id])?;
    let mut mark = tx.prepare_cached(
        "UPDATE tracks SET favorite = 1 WHERE source_id = ?1 AND source_item_id = ?2",
    )?;
    for item in item_ids {
        mark.execute(params![source_id, item])?;
    }
    Ok(())
}

const TABLES: [(&str, i64); 4] = [
    ("tracks", KIND_TRACK),
    ("albums", KIND_ALBUM),
    ("artists", KIND_ARTIST),
    ("playlists", KIND_PLAYLIST),
];

fn remove_items(tx: &Transaction<'_>, source_id: i64, item_ids: &[&str]) -> Result<usize> {
    let mut removed = 0;
    for item in item_ids {
        for (table, kind) in TABLES {
            let id: Option<i64> = tx
                .prepare_cached(&format!(
                    "SELECT id FROM {table} WHERE source_id = ?1 AND source_item_id = ?2"
                ))?
                .query_row(params![source_id, item], |r| r.get(0))
                .ok();
            if let Some(id) = id {
                tx.prepare_cached("DELETE FROM search_index WHERE rowid = ?1")?
                    .execute([index_rowid(id, kind)])?;
                tx.prepare_cached(&format!("DELETE FROM {table} WHERE id = ?1"))?
                    .execute([id])?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}

fn remove_missing(tx: &Transaction<'_>, source_id: i64, d: &LibraryDelta) -> Result<usize> {
    let keep: HashSet<&str> = d
        .artists
        .iter()
        .map(|a| a.source_item_id.as_str())
        .chain(d.albums.iter().map(|a| a.source_item_id.as_str()))
        .chain(d.tracks.iter().map(|t| t.source_item_id.as_str()))
        .chain(d.playlists.iter().map(|p| p.source_item_id.as_str()))
        .collect();
    let mut gone = Vec::new();
    for (table, _) in TABLES {
        let mut st = tx.prepare(&format!(
            "SELECT source_item_id FROM {table} WHERE source_id = ?1"
        ))?;
        let ids = st.query_map([source_id], |r| r.get::<_, String>(0))?;
        for id in ids {
            let id = id?;
            // A playlist made on this device the server hasn't seen yet is not gone.
            if !keep.contains(id.as_str()) && !id.starts_with(LOCAL_ITEM_PREFIX) {
                gone.push(id);
            }
        }
    }
    let refs: Vec<&str> = gone.iter().map(String::as_str).collect();
    remove_items(tx, source_id, &refs)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::test_support::store_with_source;
    use encore_sources::PlaylistEntry;

    pub(crate) fn credit(name: &str, id: &str) -> ArtistCredit {
        ArtistCredit {
            source_item_id: Some(id.into()),
            name: name.into(),
        }
    }

    pub(crate) fn track(id: &str, title: &str, album: &str, n: u32) -> Track {
        Track {
            source_item_id: id.into(),
            title: title.into(),
            sort_title: title.into(),
            album_item_id: Some(album.into()),
            album_title: Some("Golden Hour Static".into()),
            artists: vec![credit("Ember Cove", "ar1")],
            album_artists: vec![credit("Ember Cove", "ar1")],
            disc_number: Some(1),
            track_number: Some(n),
            duration_ms: 200_000 + u64::from(n) * 1000,
            year: Some(2024),
            container: Some("flac".into()),
            bitrate: Some(900_000),
            mbid_recording: None,
            genres: vec!["Ambient".into()],
            date_added_utc_ms: Some(1_700_000_000_000 + i64::from(n)),
            has_lyrics: n == 1,
            track_gain_db: Some(-6.5),
            album_gain_db: None,
        }
    }

    pub(crate) fn sample_delta() -> LibraryDelta {
        LibraryDelta {
            full: true,
            artists: vec![Artist {
                source_item_id: "ar1".into(),
                name: "Ember Cove".into(),
                sort_name: "Ember Cove".into(),
                mbid: None,
            }],
            albums: vec![Album {
                source_item_id: "al1".into(),
                title: "Golden Hour Static".into(),
                sort_title: "Golden Hour Static".into(),
                album_artists: vec![credit("Ember Cove", "ar1")],
                year: Some(2024),
                mbid: None,
                genres: vec!["Ambient".into()],
                date_added_utc_ms: Some(1_700_000_000_000),
                has_artwork: true,
                artwork_tag: Some("al-cover-1".into()),
            }],
            tracks: vec![
                track("t1", "Slow Tide", "al1", 1),
                track("t2", "Paper Lanterns", "al1", 2),
                track("t3", "Count the Hours", "al1", 3),
            ],
            playlists: vec![Playlist {
                source_item_id: "pl1".into(),
                name: "Evening Drive".into(),
                date_added_utc_ms: None,
                entries: vec![
                    PlaylistEntry {
                        entry_id: Some("e1".into()),
                        track_item_id: "t3".into(),
                    },
                    PlaylistEntry {
                        entry_id: Some("e2".into()),
                        track_item_id: "t1".into(),
                    },
                ],
                artwork_tag: Some("pl-cover-1".into()),
                can_edit: true,
            }],
            removed_item_ids: vec![],
            favorite_item_ids: None,
            play_counts: None,
            cursor: None,
        }
    }

    fn count(store: &Store, table: &str) -> i64 {
        store
            .conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    fn snapshot(store: &Store) -> Vec<i64> {
        [
            "artists",
            "albums",
            "tracks",
            "track_artists",
            "track_genres",
            "album_artists",
            "playlists",
            "playlist_items",
            "search_index",
        ]
        .iter()
        .map(|t| count(store, t))
        .collect()
    }

    #[test]
    fn a_full_sync_fills_the_mirror_and_the_search_index() {
        let (store, src) = store_with_source();
        let stats = store.apply_delta(src, &sample_delta()).unwrap();
        assert_eq!(
            stats,
            ApplyStats {
                artists: 1,
                albums: 1,
                tracks: 3,
                playlists: 1,
                removed: 0
            }
        );
        assert_eq!(snapshot(&store), vec![1, 1, 3, 3, 3, 1, 1, 2, 6]);
        let album_id: Option<i64> = store
            .conn
            .query_row(
                "SELECT album_id FROM tracks WHERE source_item_id = 't2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(album_id.is_some());
    }

    #[test]
    fn applying_the_same_sync_twice_changes_nothing() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let before = snapshot(&store);
        let ids_before = store.mirrored_item_ids(src).unwrap();
        store.apply_delta(src, &sample_delta()).unwrap();
        assert_eq!(snapshot(&store), before);
        assert_eq!(store.mirrored_item_ids(src).unwrap(), ids_before);
    }

    #[test]
    fn a_delta_updates_changed_items_and_removes_deleted_ones() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let mut changed = track("t2", "Paper Lanterns (Edit)", "al1", 2);
        changed.genres = vec![];
        let delta = LibraryDelta {
            full: false,
            tracks: vec![changed],
            removed_item_ids: vec!["t3".into(), "not-there".into()],
            favorite_item_ids: None,
            play_counts: None,
            ..LibraryDelta::default()
        };
        let stats = store.apply_delta(src, &delta).unwrap();
        assert_eq!(stats.tracks, 1);
        assert_eq!(stats.removed, 1);
        assert_eq!(count(&store, "tracks"), 2);
        assert_eq!(count(&store, "search_index"), 5);
        let title: String = store
            .conn
            .query_row(
                "SELECT title FROM tracks WHERE source_item_id = 't2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Paper Lanterns (Edit)");
        // The playlist keeps the entry until the server's playlist changes, and browse
        // shows it as unavailable, so nothing vanishes silently (board 20, item 5).
        assert_eq!(count(&store, "playlist_items"), 2);
    }

    #[test]
    fn a_full_sync_drops_what_the_server_no_longer_has() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let mut smaller = sample_delta();
        smaller.tracks.truncate(1);
        smaller.playlists.clear();
        let stats = store.apply_delta(src, &smaller).unwrap();
        assert_eq!(stats.removed, 3);
        assert_eq!(count(&store, "tracks"), 1);
        assert_eq!(count(&store, "playlists"), 0);
        assert_eq!(count(&store, "search_index"), 3);
    }

    #[test]
    fn artwork_tags_and_favourites_are_mirrored() {
        let (store, src) = store_with_source();
        let mut d = sample_delta();
        d.favorite_item_ids = Some(vec!["t1".into(), "t3".into(), "not-mirrored".into()]);
        store.apply_delta(src, &d).unwrap();
        let fav = |s: &Store| -> Vec<String> {
            let mut st = s
                .conn
                .prepare("SELECT source_item_id FROM tracks WHERE favorite = 1 ORDER BY 1")
                .unwrap();
            st.query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(fav(&store), ["t1", "t3"]);
        let tags: (Option<String>, Option<String>) = store
            .conn
            .query_row(
                "SELECT (SELECT artwork_tag FROM albums), (SELECT artwork_tag FROM playlists)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(tags, (Some("al-cover-1".into()), Some("pl-cover-1".into())));

        // Favourites arrive as the whole set: one cleared on the server is cleared
        // here; None (not reported) leaves them alone.
        d.full = false;
        d.favorite_item_ids = Some(vec!["t2".into()]);
        store.apply_delta(src, &d).unwrap();
        assert_eq!(fav(&store), ["t2"]);
        d.favorite_item_ids = None;
        store.apply_delta(src, &d).unwrap();
        assert_eq!(fav(&store), ["t2"]);
    }

    #[test]
    fn a_pull_keeps_unsent_playlist_edits_and_playlists_made_offline() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let pl: i64 = store
            .conn
            .query_row("SELECT id FROM playlists", [], |r| r.get(0))
            .unwrap();
        // An edit on this device (a third song) still waiting for the server, and a
        // playlist made offline that the server has never seen.
        store
            .conn
            .execute_batch(&format!(
                "INSERT INTO playlist_items (playlist_id, position, track_item_id) VALUES ({pl}, 2, 't2');
                 INSERT INTO outbox (source_id, edit_id, playlist_id, op) VALUES ({src}, 'e', {pl}, '{{}}');
                 INSERT INTO playlists (source_id, source_item_id, name, search_name)
                     VALUES ({src}, 'local:01J', 'Made offline', 'made offline');"
            ))
            .unwrap();
        store.apply_delta(src, &sample_delta()).unwrap();
        assert_eq!(count(&store, "playlist_items"), 3, "the unsent edit stays");
        assert_eq!(count(&store, "playlists"), 2, "the offline playlist stays");

        // Once the write is accepted, the server's version is the truth again.
        store
            .conn
            .execute("UPDATE outbox SET state = 'done'", [])
            .unwrap();
        store.apply_delta(src, &sample_delta()).unwrap();
        assert_eq!(count(&store, "playlist_items"), 2);
    }

    #[test]
    fn tracks_find_an_album_that_arrives_later() {
        let (store, src) = store_with_source();
        let mut first = sample_delta();
        first.full = false;
        first.albums.clear();
        store.apply_delta(src, &first).unwrap();
        let orphans: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM tracks WHERE album_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 3);
        let second = LibraryDelta {
            albums: sample_delta().albums,
            ..LibraryDelta::default()
        };
        store.apply_delta(src, &second).unwrap();
        let orphans: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM tracks WHERE album_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn removing_a_source_clears_its_mirror_and_index() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        store.remove_source(src).unwrap();
        assert_eq!(snapshot(&store), vec![0; 9]);
    }
}
