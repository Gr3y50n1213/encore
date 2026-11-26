//! Library sync for Plex (ADR core-0023 point 8).
//!
//! A full sync reads every music library (`/library/sections`, `type=artist`): artists
//! (type 8), albums (type 9) and tracks (type 10), `page_size` at a time, then the audio
//! playlists with their items. Favourites (a rating of 4.5 stars or more) and play counts
//! (`viewCount`, `lastViewedAt`) come with the tracks, so they cost nothing extra. A full
//! delta lists everything, so the mirror removes what it doesn't hold.
//!
//! Each section has a `contentChangedAt` counter that the server moves whenever its
//! content changes (not on ratings or plays). The cursor keeps every section's counter
//! and when the last full sync ran: `plex1:s=<key>.<counter>,...;full=<ms>`. When no
//! counter moved, no section came or went, and the last full sync is under a day old
//! ([`FULL_SYNC_EVERY_MS`]), the sync is a light one: the playlists (whole) and the
//! favourites (one filtered listing per section), and the cursor stays. Play counts from
//! other apps come with the next full sync.

use std::collections::HashSet;

use super::client::PlexClient;
use super::dto::{Directory, Metadata};
use super::map;
use crate::error::Result;
use crate::jellyfin::SyncProgress;
use crate::model::{Cursor, LibraryDelta, Playlist};

/// The largest page asked for.
pub const MAX_PAGE: u32 = 1000;
/// How long an unchanged library goes without a full sync.
pub const FULL_SYNC_EVERY_MS: i64 = 24 * 60 * 60 * 1000;

const CURSOR_PREFIX: &str = "plex1:s=";
const FULL_AT: &str = ";full=";

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The sections' change counters, as the cursor writes them (sorted by key). None when a
/// section has no counter (an older server): then every sync is a full one.
fn counters(sections: &[Directory]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for s in sections {
        let c = s.content_changed_at?;
        parts.push(format!("{}.{c}", s.key.as_deref()?));
    }
    parts.sort();
    Some(parts.join(","))
}

fn cursor_for(counters: &str, full_at: i64) -> Cursor {
    Cursor(format!("{CURSOR_PREFIX}{counters}{FULL_AT}{full_at}"))
}

/// The counters and full-sync time a cursor holds, if it is a Plex cursor.
fn read_cursor(c: &Cursor) -> Option<(&str, i64)> {
    let (counters, at) = c.0.strip_prefix(CURSOR_PREFIX)?.split_once(FULL_AT)?;
    Some((counters, at.parse().ok()?))
}

/// Whether the library changed since the sync that gave `since`.
pub async fn library_changed(client: &PlexClient, since: Option<&Cursor>) -> Result<bool> {
    let Some((before, _)) = since.and_then(read_cursor) else {
        return Ok(true);
    };
    let sections = client.music_sections().await?;
    Ok(counters(&sections).as_deref() != Some(before))
}

/// Run one sync: a light one when nothing changed (module notes), else a full one.
pub async fn sync_library(
    client: &PlexClient,
    since: Option<&Cursor>,
    known_ids: &HashSet<String>,
    page_size: u32,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<LibraryDelta> {
    sync_library_at(client, since, known_ids, page_size, progress, now_ms()).await
}

/// [`sync_library`] at a given time, for the tests.
pub(crate) async fn sync_library_at(
    client: &PlexClient,
    since: Option<&Cursor>,
    known_ids: &HashSet<String>,
    page_size: u32,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    now: i64,
) -> Result<LibraryDelta> {
    let _ = known_ids;
    // Read first, so a change made while the sync runs moves it past this cursor.
    let sections = client.music_sections().await?;
    let now_counters = counters(&sections);
    if let (Some((before, at)), Some(c)) = (since.and_then(read_cursor), &now_counters)
        && before == c
        && (0..FULL_SYNC_EVERY_MS).contains(&(now - at))
    {
        return light_sync(client, &sections, since, progress).await;
    }

    let (mut done, mut total) = (0u64, 0u64);
    let report = |done: u64, total: u64| {
        progress(SyncProgress {
            done,
            total: total.max(done),
        })
    };
    let size = page_size.clamp(1, MAX_PAGE);
    let mut delta = LibraryDelta {
        full: true,
        playlists_complete: true,
        ..LibraryDelta::default()
    };
    let mut favorites: Vec<String> = Vec::new();
    let mut plays: Vec<(String, u32, Option<i64>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for s in &sections {
        let Some(key) = s.key.as_deref() else {
            continue;
        };
        for item_type in [8u8, 9, 10] {
            let mut start: u64 = 0;
            let mut counted = false;
            loop {
                let page = client.page(key, item_type, start, size, &[]).await?;
                if !counted {
                    total += page.total_size.unwrap_or(page.metadata.len() as i64).max(0) as u64;
                    counted = true;
                }
                let n = page.metadata.len() as u64;
                for m in &page.metadata {
                    match item_type {
                        8 => delta.artists.extend(map::to_artist(m)),
                        9 => delta.albums.extend(map::to_album(m)),
                        _ => {
                            if let Some(t) = map::to_track(m)
                                && seen.insert(t.source_item_id.clone())
                            {
                                user_data(m, &t.source_item_id, &mut favorites, &mut plays);
                                delta.tracks.push(t);
                            }
                        }
                    }
                }
                done += n;
                report(done, total);
                start += n;
                // The server's total says when to stop (it may send fewer than asked);
                // without one, a short page is the last.
                let last = match page.total_size {
                    Some(t) => start >= t.max(0) as u64,
                    None => n < u64::from(size),
                };
                if n == 0 || last {
                    break;
                }
            }
        }
    }
    delta.playlists = all_playlists(client, &mut done, &mut total, &report).await?;
    delta.favorite_item_ids = Some(favorites);
    delta.play_counts = Some(plays);
    delta.cursor = now_counters.map(|c| cursor_for(&c, now));
    Ok(delta)
}

/// A track's like and play count, read while it is at hand.
fn user_data(
    m: &Metadata,
    id: &str,
    favorites: &mut Vec<String>,
    plays: &mut Vec<(String, u32, Option<i64>)>,
) {
    if map::is_liked(m) {
        favorites.push(id.to_owned());
    }
    if let Some(n) = m
        .view_count
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
    {
        plays.push((id.to_owned(), n, map::seconds_to_ms(m.last_viewed_at)));
    }
}

/// Every audio playlist of this user, with its items.
async fn all_playlists(
    client: &PlexClient,
    done: &mut u64,
    total: &mut u64,
    report: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<Vec<Playlist>> {
    let listed = client.playlists().await?;
    *total += listed.len() as u64;
    let mut out = Vec::with_capacity(listed.len());
    for p in &listed {
        let Some(id) = p.rating_key.as_deref() else {
            continue;
        };
        let items = client.playlist_items(id).await?;
        if let Some(pl) = map::to_playlist(p, &items) {
            client.remember_cover(&pl.source_item_id, pl.artwork_tag.as_deref());
            out.push(pl);
        }
        *done += 1;
        report(*done, *total);
    }
    Ok(out)
}

/// Nothing changed in the libraries: only the playlists (whole, so one deleted elsewhere
/// goes) and the favourites, from one filtered listing per section. The cursor stays, so
/// a full sync still comes a day after the last one.
async fn light_sync(
    client: &PlexClient,
    sections: &[Directory],
    since: Option<&Cursor>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<LibraryDelta> {
    let (mut done, mut total) = (0u64, 0u64);
    let report = |done: u64, total: u64| {
        progress(SyncProgress {
            done,
            total: total.max(done),
        })
    };
    let mut favorites = Vec::new();
    for s in sections {
        let Some(key) = s.key.as_deref() else {
            continue;
        };
        let mut start = 0u64;
        loop {
            // `>>=` is Plex's "greater than"; the exact line is drawn here again.
            let page = client
                .page(key, 10, start, MAX_PAGE, &[("userRating>>", "8")])
                .await?;
            let n = page.metadata.len() as u64;
            favorites.extend(
                page.metadata
                    .iter()
                    .filter(|m| map::is_liked(m))
                    .filter_map(map::track_id_of),
            );
            start += n;
            if n < u64::from(MAX_PAGE) || n == 0 {
                break;
            }
        }
    }
    let playlists = all_playlists(client, &mut done, &mut total, &report).await?;
    report(done, total);
    Ok(LibraryDelta {
        full: false,
        playlists,
        playlists_complete: true,
        favorite_item_ids: Some(favorites),
        cursor: since.cloned(),
        ..LibraryDelta::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_and_sort_their_sections() {
        let s = |key: &str, c: i64| Directory {
            key: Some(key.into()),
            content_changed_at: Some(c),
            ..Directory::default()
        };
        let counters = counters(&[s("4", 90), s("1", 27)]).unwrap();
        assert_eq!(counters, "1.27,4.90");
        let c = cursor_for(&counters, 1_762_500_000_000);
        assert_eq!(c.0, "plex1:s=1.27,4.90;full=1762500000000");
        assert_eq!(read_cursor(&c), Some(("1.27,4.90", 1_762_500_000_000)));
        assert_eq!(read_cursor(&Cursor("subsonic1:lm=1;full=2".into())), None);
        // A section without a counter: no cursor, so every sync is full.
        assert_eq!(super::counters(&[Directory::default()]), None);
    }
}
