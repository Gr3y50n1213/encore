//! Library sync for Jellyfin (plan section 4: deltas, not rescans).
//!
//! - First sync: every artist, album, song and playlist, page by page.
//! - Later syncs: only items saved since the cursor (`minDateLastSaved`), plus an
//!   ID-only listing to find what was removed, plus every playlist again (they are
//!   few, and an edit to a playlist's songs is not guaranteed to touch its date).
//! - The cursor is the server's own clock (its HTTP `Date` header) when the sync
//!   began, minus a margin, so a wrong clock on the phone cannot skip changes and
//!   an item saved while the sync ran is fetched again next time. Jellyfin does not
//!   return `DateLastSaved` on items (it is not in BaseItemDto in 12.0), so the
//!   newest-item approach is not available; the recording showed this.

use std::collections::HashSet;

use super::client::JellyfinClient;
use super::dto::BaseItem;
use super::map;
use super::time::format_utc;
use crate::error::Result;
use crate::model::{Cursor, LibraryDelta};

/// How far before the server's clock at the start the next delta begins.
pub const CURSOR_MARGIN_MS: i64 = 60_000;

/// Progress for "Indexing 3,200 of 12,480" (board 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncProgress {
    pub done: u64,
    pub total: u64,
}

const FIELDS: &str = "SortName,Genres,ProviderIds,DateCreated";

struct Pager<'a> {
    client: &'a JellyfinClient,
    page_size: u32,
    /// The server's clock on the first reply of this sync.
    started_at: Option<i64>,
    done: u64,
    total: u64,
}

impl Pager<'_> {
    /// Every item of `kind`, optionally only those saved since `since`.
    async fn all(
        &mut self,
        kind: &str,
        since: Option<&str>,
        progress: &(dyn Fn(SyncProgress) + Send + Sync),
    ) -> Result<Vec<BaseItem>> {
        let mut out = Vec::new();
        let mut start: u64 = 0;
        let limit = self.page_size.to_string();
        let mut counted = false;
        loop {
            let start_s = start.to_string();
            let mut q: Vec<(&str, &str)> = vec![
                ("includeItemTypes", kind),
                ("recursive", "true"),
                ("fields", FIELDS),
                ("enableImageTypes", "Primary"),
                ("imageTypeLimit", "1"),
                ("enableUserData", "false"),
                ("enableTotalRecordCount", "true"),
                ("sortBy", "SortName"),
                ("sortOrder", "Ascending"),
                ("startIndex", start_s.as_str()),
                ("limit", limit.as_str()),
            ];
            if let Some(s) = since {
                q.push(("minDateLastSaved", s));
            }
            let page = self.client.items(&q).await?;
            if self.started_at.is_none() {
                self.started_at = page.server_date_utc_ms;
            }
            if !counted {
                self.total += page.total_record_count.unwrap_or(0).max(0) as u64;
                counted = true;
            }
            let items = page.items.unwrap_or_default();
            let n = items.len() as u64;
            out.extend(items);
            self.done += n;
            progress(SyncProgress {
                done: self.done,
                total: self.total.max(self.done),
            });
            start += n;
            let total = page.total_record_count.unwrap_or(0).max(0) as u64;
            if n == 0 || start >= total {
                break;
            }
        }
        Ok(out)
    }

    /// The IDs of everything on the server now, for finding removals.
    async fn all_ids(&mut self) -> Result<HashSet<String>> {
        let mut ids = HashSet::new();
        let mut start: u64 = 0;
        let limit = (self.page_size * 4).to_string();
        loop {
            let start_s = start.to_string();
            let page = self
                .client
                .items(&[
                    ("includeItemTypes", "Audio,MusicAlbum,MusicArtist,Playlist"),
                    ("recursive", "true"),
                    ("enableImages", "false"),
                    ("enableUserData", "false"),
                    ("enableTotalRecordCount", "true"),
                    ("sortBy", "SortName"),
                    ("startIndex", start_s.as_str()),
                    ("limit", limit.as_str()),
                ])
                .await?;
            let items = page.items.unwrap_or_default();
            let n = items.len() as u64;
            ids.extend(items.into_iter().filter_map(|i| i.id));
            start += n;
            if n == 0 || start >= page.total_record_count.unwrap_or(0).max(0) as u64 {
                break;
            }
        }
        Ok(ids)
    }
}

/// Run one sync. `known_ids` are the item IDs already mirrored from this server;
/// `page_size` is `library.syncPageSize`.
pub async fn sync_library(
    client: &JellyfinClient,
    since: Option<&Cursor>,
    known_ids: &HashSet<String>,
    page_size: u32,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<LibraryDelta> {
    let mut pager = Pager {
        client,
        page_size: page_size.max(1),
        started_at: None,
        done: 0,
        total: 0,
    };
    let since_s = since.map(|c| c.0.as_str());
    let artists = pager.all("MusicArtist", since_s, progress).await?;
    let albums = pager.all("MusicAlbum", since_s, progress).await?;
    let tracks = pager.all("Audio", since_s, progress).await?;
    // Playlists: always all of them, with their songs.
    let playlist_items = pager.all("Playlist", None, progress).await?;
    let mut playlists = Vec::with_capacity(playlist_items.len());
    for p in &playlist_items {
        let Some(id) = p.id.as_deref() else { continue };
        let entries = client.playlist_items(id).await?.items.unwrap_or_default();
        if let Some(pl) = map::to_playlist(
            p,
            entries.iter().filter_map(map::to_playlist_entry).collect(),
        ) {
            playlists.push(pl);
        }
    }
    let removed_item_ids = if since.is_some() {
        let now = pager.all_ids().await?;
        let mut gone: Vec<String> = known_ids
            .iter()
            .filter(|id| !now.contains(*id))
            .cloned()
            .collect();
        gone.sort();
        gone
    } else {
        Vec::new()
    };
    // Without a server clock the next sync is a full one: slower, never wrong.
    let cursor = pager
        .started_at
        .map(|ms| Cursor(format_utc(ms - CURSOR_MARGIN_MS)));
    Ok(LibraryDelta {
        full: since.is_none(),
        artists: artists.iter().filter_map(map::to_artist).collect(),
        albums: albums.iter().filter_map(map::to_album).collect(),
        tracks: tracks.iter().filter_map(map::to_track).collect(),
        playlists,
        removed_item_ids,
        favorite_item_ids: None,
        cursor,
    })
}
