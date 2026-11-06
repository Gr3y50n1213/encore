//! Browse queries over the mirror: paged, sorted and offline (plan section 4).

use rusqlite::{OptionalExtension, Row, params};

use crate::{Result, Store};

#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    pub id: i64,
    pub source_id: i64,
    pub source_item_id: String,
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album_id: Option<i64>,
    pub album_title: Option<String>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub duration_ms: u64,
    pub year: Option<i32>,
    pub container: Option<String>,
    pub bitrate: Option<u32>,
    pub has_lyrics: bool,
    pub song_key: String,
    pub track_gain_db: Option<f64>,
    pub album_gain_db: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumRow {
    pub id: i64,
    pub source_id: i64,
    pub source_item_id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub track_count: u32,
    pub duration_ms: u64,
    pub date_added_utc_ms: Option<i64>,
    pub has_artwork: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistRow {
    pub id: i64,
    pub source_id: i64,
    pub source_item_id: String,
    pub name: String,
    pub album_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistRow {
    pub id: i64,
    pub source_id: i64,
    pub source_item_id: String,
    pub name: String,
    pub track_count: u32,
    pub duration_ms: u64,
}

/// One entry of a playlist. `track` is None when the song has left the library;
/// the entry stays so nothing vanishes silently (board 20, item 5).
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistEntryRow {
    pub position: u32,
    pub track_item_id: String,
    pub track: Option<TrackRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenreRow {
    pub name: String,
    pub track_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumSort {
    Title,
    Artist,
    Year,
    RecentlyAdded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSort {
    Title,
    Artist,
    Album,
    RecentlyAdded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub offset: u32,
    pub limit: u32,
}

const TRACK_COLS: &str =
    "t.id, t.source_id, t.source_item_id, t.title, t.artist_display, t.album_artist_display,
     t.album_id, t.album_title, t.disc_number, t.track_number, t.duration_ms, t.year, t.container,
     t.bitrate, t.has_lyrics, t.song_key, t.track_gain_db, t.album_gain_db";

fn track_row(r: &Row<'_>) -> rusqlite::Result<TrackRow> {
    Ok(TrackRow {
        id: r.get(0)?,
        source_id: r.get(1)?,
        source_item_id: r.get(2)?,
        title: r.get(3)?,
        artist: r.get(4)?,
        album_artist: r.get(5)?,
        album_id: r.get(6)?,
        album_title: r.get(7)?,
        disc_number: r.get(8)?,
        track_number: r.get(9)?,
        duration_ms: r.get::<_, i64>(10)? as u64,
        year: r.get(11)?,
        container: r.get(12)?,
        bitrate: r.get(13)?,
        has_lyrics: r.get(14)?,
        song_key: r.get(15)?,
        track_gain_db: r.get(16)?,
        album_gain_db: r.get(17)?,
    })
}

const ALBUM_COLS: &str = "a.id, a.source_id, a.source_item_id, a.title, a.artist_display, a.year,
     (SELECT count(*) FROM tracks t WHERE t.album_id = a.id),
     (SELECT coalesce(sum(t.duration_ms), 0) FROM tracks t WHERE t.album_id = a.id),
     a.date_added_utc_ms, a.has_artwork";

fn album_row(r: &Row<'_>) -> rusqlite::Result<AlbumRow> {
    Ok(AlbumRow {
        id: r.get(0)?,
        source_id: r.get(1)?,
        source_item_id: r.get(2)?,
        title: r.get(3)?,
        artist: r.get(4)?,
        year: r.get(5)?,
        track_count: r.get(6)?,
        duration_ms: r.get::<_, i64>(7)? as u64,
        date_added_utc_ms: r.get(8)?,
        has_artwork: r.get(9)?,
    })
}

const ARTIST_COLS: &str = "ar.id, ar.source_id, ar.source_item_id, ar.name,
     (SELECT count(DISTINCT aa.album_id) FROM album_artists aa JOIN albums al ON al.id = aa.album_id
      WHERE aa.source_item_id = ar.source_item_id AND al.source_id = ar.source_id)";

fn artist_row(r: &Row<'_>) -> rusqlite::Result<ArtistRow> {
    Ok(ArtistRow {
        id: r.get(0)?,
        source_id: r.get(1)?,
        source_item_id: r.get(2)?,
        name: r.get(3)?,
        album_count: r.get(4)?,
    })
}

const PLAYLIST_COLS: &str = "p.id, p.source_id, p.source_item_id, p.name,
     (SELECT count(*) FROM playlist_items i WHERE i.playlist_id = p.id),
     (SELECT coalesce(sum(t.duration_ms), 0) FROM playlist_items i
      JOIN tracks t ON t.source_id = p.source_id AND t.source_item_id = i.track_item_id
      WHERE i.playlist_id = p.id)";

fn playlist_row(r: &Row<'_>) -> rusqlite::Result<PlaylistRow> {
    Ok(PlaylistRow {
        id: r.get(0)?,
        source_id: r.get(1)?,
        source_item_id: r.get(2)?,
        name: r.get(3)?,
        track_count: r.get(4)?,
        duration_ms: r.get::<_, i64>(5)? as u64,
    })
}

impl Store {
    pub(crate) fn track_row(&self, id: i64) -> Result<Option<TrackRow>> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {TRACK_COLS} FROM tracks t WHERE t.id = ?1"
            ))?
            .query_row([id], track_row)
            .optional()?)
    }

    pub(crate) fn album_row(&self, id: i64) -> Result<Option<AlbumRow>> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {ALBUM_COLS} FROM albums a WHERE a.id = ?1"
            ))?
            .query_row([id], album_row)
            .optional()?)
    }

    pub(crate) fn artist_row(&self, id: i64) -> Result<Option<ArtistRow>> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {ARTIST_COLS} FROM artists ar WHERE ar.id = ?1"
            ))?
            .query_row([id], artist_row)
            .optional()?)
    }

    pub(crate) fn playlist_row(&self, id: i64) -> Result<Option<PlaylistRow>> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {PLAYLIST_COLS} FROM playlists p WHERE p.id = ?1"
            ))?
            .query_row([id], playlist_row)
            .optional()?)
    }

    pub fn track(&self, id: i64) -> Result<TrackRow> {
        self.track_row(id)?
            .ok_or_else(|| crate::StoreError::NotFound(format!("song {id}")))
    }

    pub fn album(&self, id: i64) -> Result<AlbumRow> {
        self.album_row(id)?
            .ok_or_else(|| crate::StoreError::NotFound(format!("album {id}")))
    }

    pub fn artist(&self, id: i64) -> Result<ArtistRow> {
        self.artist_row(id)?
            .ok_or_else(|| crate::StoreError::NotFound(format!("artist {id}")))
    }

    pub fn playlist(&self, id: i64) -> Result<PlaylistRow> {
        self.playlist_row(id)?
            .ok_or_else(|| crate::StoreError::NotFound(format!("playlist {id}")))
    }

    /// A song by its source and the source's ID (the queue and the play log use this).
    pub fn track_by_item(&self, source_id: i64, source_item_id: &str) -> Result<Option<TrackRow>> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {TRACK_COLS} FROM tracks t WHERE t.source_id = ?1 AND t.source_item_id = ?2"
            ))?
            .query_row(params![source_id, source_item_id], track_row)
            .optional()?)
    }

    pub fn albums(&self, sort: AlbumSort, page: Page) -> Result<Vec<AlbumRow>> {
        let order = match sort {
            AlbumSort::Title => "a.sort_title COLLATE NOCASE, a.id",
            AlbumSort::Artist => {
                "a.artist_display COLLATE NOCASE, a.year, a.sort_title COLLATE NOCASE, a.id"
            }
            AlbumSort::Year => "a.year DESC NULLS LAST, a.sort_title COLLATE NOCASE, a.id",
            AlbumSort::RecentlyAdded => "a.date_added_utc_ms DESC NULLS LAST, a.id DESC",
        };
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ALBUM_COLS} FROM albums a ORDER BY {order} LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = st.query_map(params![page.limit, page.offset], album_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// An album's songs in disc and track order.
    pub fn album_tracks(&self, album_id: i64) -> Result<Vec<TrackRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {TRACK_COLS} FROM tracks t WHERE t.album_id = ?1
             ORDER BY coalesce(t.disc_number, 1), t.track_number NULLS LAST, t.sort_title COLLATE NOCASE"
        ))?;
        let rows = st.query_map([album_id], track_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn tracks(&self, sort: TrackSort, page: Page) -> Result<Vec<TrackRow>> {
        let order = match sort {
            TrackSort::Title => "t.sort_title COLLATE NOCASE, t.id",
            TrackSort::Artist => {
                "t.artist_display COLLATE NOCASE, t.album_title COLLATE NOCASE, t.disc_number, t.track_number, t.id"
            }
            TrackSort::Album => "t.album_title COLLATE NOCASE, t.disc_number, t.track_number, t.id",
            TrackSort::RecentlyAdded => "t.date_added_utc_ms DESC NULLS LAST, t.id DESC",
        };
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {TRACK_COLS} FROM tracks t ORDER BY {order} LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = st.query_map(params![page.limit, page.offset], track_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn artists(&self, page: Page) -> Result<Vec<ArtistRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ARTIST_COLS} FROM artists ar ORDER BY ar.sort_name COLLATE NOCASE, ar.id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = st.query_map(params![page.limit, page.offset], artist_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Albums an artist is credited on as album artist, newest first.
    pub fn artist_albums(&self, artist_id: i64) -> Result<Vec<AlbumRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ALBUM_COLS} FROM albums a
             WHERE a.id IN (SELECT aa.album_id FROM album_artists aa JOIN artists ar
                            ON ar.source_item_id = aa.source_item_id WHERE ar.id = ?1)
               AND a.source_id = (SELECT source_id FROM artists WHERE id = ?1)
             ORDER BY a.year DESC NULLS LAST, a.sort_title COLLATE NOCASE"
        ))?;
        let rows = st.query_map([artist_id], album_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Songs that credit an artist, in album order.
    pub fn artist_tracks(&self, artist_id: i64, page: Page) -> Result<Vec<TrackRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {TRACK_COLS} FROM tracks t
             WHERE t.id IN (SELECT ta.track_id FROM track_artists ta JOIN artists ar
                            ON ar.source_item_id = ta.source_item_id WHERE ar.id = ?1)
               AND t.source_id = (SELECT source_id FROM artists WHERE id = ?1)
             ORDER BY t.album_title COLLATE NOCASE, t.disc_number, t.track_number, t.id
             LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = st.query_map(params![artist_id, page.limit, page.offset], track_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn playlists(&self, page: Page) -> Result<Vec<PlaylistRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {PLAYLIST_COLS} FROM playlists p ORDER BY p.name COLLATE NOCASE, p.id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = st.query_map(params![page.limit, page.offset], playlist_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn playlist_entries(&self, playlist_id: i64) -> Result<Vec<PlaylistEntryRow>> {
        let mut st = self.conn.prepare_cached(
            "SELECT i.position, i.track_item_id, t.id FROM playlist_items i
             JOIN playlists p ON p.id = i.playlist_id
             LEFT JOIN tracks t ON t.source_id = p.source_id AND t.source_item_id = i.track_item_id
             WHERE i.playlist_id = ?1 ORDER BY i.position",
        )?;
        let raw: Vec<(u32, String, Option<i64>)> = st
            .query_map([playlist_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::with_capacity(raw.len());
        for (position, track_item_id, id) in raw {
            let track = match id {
                Some(id) => self.track_row(id)?,
                None => None,
            };
            out.push(PlaylistEntryRow {
                position,
                track_item_id,
                track,
            });
        }
        Ok(out)
    }

    pub fn genres(&self) -> Result<Vec<GenreRow>> {
        let mut st = self.conn.prepare_cached(
            "SELECT name, count(*) FROM track_genres GROUP BY name COLLATE NOCASE ORDER BY name COLLATE NOCASE",
        )?;
        let rows = st.query_map([], |r| {
            Ok(GenreRow {
                name: r.get(0)?,
                track_count: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn genre_tracks(&self, genre: &str, page: Page) -> Result<Vec<TrackRow>> {
        let mut st = self.conn.prepare_cached(&format!(
            // EXISTS, not IN: SQLite then walks tracks_by_artist in order and checks
            // each song's genres by primary key, instead of sorting the whole genre.
            "SELECT {TRACK_COLS} FROM tracks t
             WHERE EXISTS (SELECT 1 FROM track_genres g WHERE g.track_id = t.id AND g.name = ?1 COLLATE NOCASE)
             ORDER BY t.artist_display COLLATE NOCASE, t.album_title COLLATE NOCASE, t.disc_number, t.track_number, t.id
             LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = st.query_map(params![genre, page.limit, page.offset], track_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Songs per source, for the server row on board 18.
    pub fn track_count(&self, source_id: i64) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE source_id = ?1",
            [source_id],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::tests::sample_delta;
    use crate::test_support::store_with_source;

    const ALL: Page = Page {
        offset: 0,
        limit: 100,
    };

    fn store() -> (Store, i64) {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        (store, src)
    }

    #[test]
    fn albums_list_with_counts_and_durations() {
        let (s, _) = store();
        let albums = s.albums(AlbumSort::Title, ALL).unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "Golden Hour Static");
        assert_eq!(albums[0].artist, "Ember Cove");
        assert_eq!(albums[0].track_count, 3);
        assert_eq!(albums[0].duration_ms, 201_000 + 202_000 + 203_000);
    }

    #[test]
    fn an_album_plays_in_track_order() {
        let (s, _) = store();
        let album = &s.albums(AlbumSort::Title, ALL).unwrap()[0];
        let titles: Vec<String> = s
            .album_tracks(album.id)
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(titles, ["Slow Tide", "Paper Lanterns", "Count the Hours"]);
    }

    #[test]
    fn songs_page_and_sort() {
        let (s, _) = store();
        let first_two: Vec<String> = s
            .tracks(
                TrackSort::Title,
                Page {
                    offset: 0,
                    limit: 2,
                },
            )
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(first_two, ["Count the Hours", "Paper Lanterns"]);
        let recent = s.tracks(TrackSort::RecentlyAdded, ALL).unwrap();
        assert_eq!(recent[0].title, "Count the Hours");
    }

    #[test]
    fn artist_pages_show_their_albums_and_songs() {
        let (s, _) = store();
        let artist = &s.artists(ALL).unwrap()[0];
        assert_eq!(artist.name, "Ember Cove");
        assert_eq!(artist.album_count, 1);
        assert_eq!(s.artist_albums(artist.id).unwrap().len(), 1);
        assert_eq!(s.artist_tracks(artist.id, ALL).unwrap().len(), 3);
    }

    #[test]
    fn playlists_keep_order_and_show_missing_songs() {
        let (s, src) = store();
        let pl = &s.playlists(ALL).unwrap()[0];
        assert_eq!(pl.name, "Evening Drive");
        assert_eq!(pl.track_count, 2);
        let entries = s.playlist_entries(pl.id).unwrap();
        assert_eq!(entries[0].track.as_ref().unwrap().title, "Count the Hours");
        s.apply_delta(
            src,
            &encore_sources::LibraryDelta {
                removed_item_ids: vec!["t3".into()],
                ..Default::default()
            },
        )
        .unwrap();
        let entries = s.playlist_entries(pl.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].track.is_none());
        assert_eq!(entries[0].track_item_id, "t3");
    }

    #[test]
    fn genres_count_songs() {
        let (s, _) = store();
        assert_eq!(
            s.genres().unwrap(),
            vec![GenreRow {
                name: "Ambient".into(),
                track_count: 3
            }]
        );
        assert_eq!(s.genre_tracks("ambient", ALL).unwrap().len(), 3);
    }

    #[test]
    fn missing_items_say_not_found() {
        let (s, src) = store();
        assert!(matches!(s.album(999), Err(crate::StoreError::NotFound(_))));
        assert!(s.track_by_item(src, "nope").unwrap().is_none());
        assert_eq!(s.track_count(src).unwrap(), 3);
    }
}
