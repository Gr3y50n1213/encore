//! Per-item caches: parsed lyrics per song, colour palettes per album or playlist.
//! Both are made once and read offline after that (migration 3).

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// A song's lyrics as last fetched. `model_json` None: the source had none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedLyrics {
    pub origin: String,
    pub format: String,
    pub model_json: Option<String>,
    pub fetched_utc_ms: i64,
}

/// An item's palette as last made. `palette_json` None: it has no artwork.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedPalette {
    pub artwork_tag: Option<String>,
    pub palette_json: Option<String>,
    pub made_utc_ms: i64,
}

impl Store {
    pub fn cached_lyrics(&self, track_id: i64) -> Result<Option<CachedLyrics>> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT origin, format, model_json, fetched_utc_ms FROM lyrics_cache WHERE track_id = ?1",
            )?
            .query_row([track_id], |r| {
                Ok(CachedLyrics {
                    origin: r.get(0)?,
                    format: r.get(1)?,
                    model_json: r.get(2)?,
                    fetched_utc_ms: r.get(3)?,
                })
            })
            .optional()?)
    }

    pub fn save_lyrics(&self, track_id: i64, cached: &CachedLyrics) -> Result<()> {
        self.conn
            .prepare_cached(
                "INSERT INTO lyrics_cache (track_id, origin, format, model_json, fetched_utc_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (track_id) DO UPDATE SET origin = excluded.origin,
                     format = excluded.format, model_json = excluded.model_json,
                     fetched_utc_ms = excluded.fetched_utc_ms",
            )?
            .execute(params![
                track_id,
                cached.origin,
                cached.format,
                cached.model_json,
                cached.fetched_utc_ms
            ])?;
        Ok(())
    }

    /// `kind` is "album" or "playlist".
    pub fn cached_palette(&self, kind: &str, item_id: i64) -> Result<Option<CachedPalette>> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT artwork_tag, palette_json, made_utc_ms FROM artwork_palettes
                 WHERE item_kind = ?1 AND item_id = ?2",
            )?
            .query_row(params![kind, item_id], |r| {
                Ok(CachedPalette {
                    artwork_tag: r.get(0)?,
                    palette_json: r.get(1)?,
                    made_utc_ms: r.get(2)?,
                })
            })
            .optional()?)
    }

    pub fn save_palette(&self, kind: &str, item_id: i64, p: &CachedPalette) -> Result<()> {
        self.conn
            .prepare_cached(
                "INSERT INTO artwork_palettes (item_kind, item_id, artwork_tag, palette_json, made_utc_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (item_kind, item_id) DO UPDATE SET artwork_tag = excluded.artwork_tag,
                     palette_json = excluded.palette_json, made_utc_ms = excluded.made_utc_ms",
            )?
            .execute(params![kind, item_id, p.artwork_tag, p.palette_json, p.made_utc_ms])?;
        Ok(())
    }

    /// An album's or playlist's source, its ID there and its current artwork tag
    /// (None: no artwork). None when there is no such item.
    pub fn artwork_tag(
        &self,
        kind: &str,
        item_id: i64,
    ) -> Result<Option<(i64, String, Option<String>)>> {
        let table = match kind {
            "album" => "albums",
            "playlist" => "playlists",
            _ => return Ok(None),
        };
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT source_id, source_item_id, artwork_tag FROM {table} WHERE id = ?1"
            ))?
            .query_row([item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::tests::sample_delta;
    use crate::test_support::store_with_source;

    #[test]
    fn lyrics_and_palettes_are_cached_and_replaced() {
        let (store, src) = store_with_source();
        store.apply_delta(src, &sample_delta()).unwrap();
        let track: i64 = store
            .conn
            .query_row("SELECT min(id) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(store.cached_lyrics(track).unwrap(), None);
        let mut c = CachedLyrics {
            origin: "mediaServer".into(),
            format: "jellyfin".into(),
            model_json: Some("{}".into()),
            fetched_utc_ms: 5,
        };
        store.save_lyrics(track, &c).unwrap();
        assert_eq!(store.cached_lyrics(track).unwrap(), Some(c.clone()));
        c.model_json = None;
        c.fetched_utc_ms = 9;
        store.save_lyrics(track, &c).unwrap();
        assert_eq!(store.cached_lyrics(track).unwrap(), Some(c));

        let album: i64 = store
            .conn
            .query_row("SELECT id FROM albums", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            store.artwork_tag("album", album).unwrap(),
            Some((src, "al1".into(), Some("al-cover-1".into())))
        );
        assert_eq!(store.artwork_tag("song", album).unwrap(), None);
        let p = CachedPalette {
            artwork_tag: Some("al-cover-1".into()),
            palette_json: Some("{\"scheme\":\"dark\"}".into()),
            made_utc_ms: 7,
        };
        store.save_palette("album", album, &p).unwrap();
        assert_eq!(store.cached_palette("album", album).unwrap(), Some(p));
        assert_eq!(store.cached_palette("playlist", album).unwrap(), None);
    }
}
