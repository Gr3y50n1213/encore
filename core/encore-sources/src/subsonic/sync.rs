//! Library sync for Subsonic servers (ADR core-0021 point 5).
//!
//! Subsonic has no change feed, so every sync is a full one: `full` is always true and
//! the mirror removes whatever this delta doesn't hold (`LibraryDelta`, `mirror.rs`),
//! so `removed_item_ids` stays empty. In order:
//! - artists (`getArtists`);
//! - albums (`getAlbumList2`, `alphabeticalByName`, `page_size` at a time, at most
//!   500, the spec's limit) and each album's songs (`getAlbum`);
//! - playlists with their songs (`getPlaylists`, `getPlaylist`);
//! - favourites and play counts from the songs' own `starred`, `playCount` and
//!   (OpenSubsonic) `played` fields, so they cost no extra request.
//!
//! The cursor holds `getIndexes`' `lastModified` from the start of the sync and when
//! the last full sync ran (`subsonic1:lm=<ms>;full=<ms>`). When `lastModified` hasn't
//! moved and the last full sync is less than a day old ([`FULL_SYNC_EVERY_MS`]), the
//! sync is a light one: only the playlists and the favourites, which `lastModified`
//! doesn't cover, instead of every album (one request each). Play counts from other
//! apps come with the next full sync.

use std::collections::HashSet;

use super::client::SubsonicClient;
use super::dto::{AlbumDto, AlbumListDto, ArtistsDto, Child, Starred2Dto};
use super::map;
use crate::error::Result;
use crate::jellyfin::SyncProgress;
use crate::model::{Cursor, LibraryDelta, Playlist};

/// `getAlbumList2`'s largest page (opensubsonic.github.io/docs/endpoints/getalbumlist2).
pub const MAX_ALBUM_PAGE: u32 = 500;

const CURSOR_PREFIX: &str = "subsonic1:lm=";
const FULL_AT: &str = ";full=";

/// How long an unchanged library goes without a full sync: anything `lastModified`
/// misses (play counts from other apps, a server that doesn't move it) comes in at
/// least this often.
pub const FULL_SYNC_EVERY_MS: i64 = 24 * 60 * 60 * 1000;

/// The `lastModified` a cursor holds, if it is a Subsonic cursor.
pub fn cursor_last_modified(cursor: &Cursor) -> Option<i64> {
    let rest = cursor.0.strip_prefix(CURSOR_PREFIX)?;
    rest.split(';').next()?.parse().ok()
}

/// When the full sync that made the cursor ran; None for older cursors.
fn cursor_full_at(cursor: &Cursor) -> Option<i64> {
    let rest = cursor.0.strip_prefix(CURSOR_PREFIX)?;
    rest.split_once(FULL_AT)?.1.parse().ok()
}

fn cursor_for(last_modified: i64, full_at: i64) -> Cursor {
    Cursor(format!("{CURSOR_PREFIX}{last_modified}{FULL_AT}{full_at}"))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Whether the library changed since the sync that gave `since`: true when there is
/// no cursor, it isn't readable, or the server's `lastModified` is different (or
/// unknown). Playlists, likes and play counts are not covered by `lastModified`; a
/// pull to refresh syncs them whatever this says.
pub async fn library_changed(client: &SubsonicClient, since: Option<&Cursor>) -> Result<bool> {
    let Some(before) = since.and_then(cursor_last_modified) else {
        return Ok(true);
    };
    Ok(client.last_modified().await? != Some(before))
}

/// Run one sync: a light one when nothing changed (see the module notes), else a full
/// one. `known_ids` is not needed (a full delta lists everything); `page_size` is
/// `library.syncPageSize`.
pub async fn sync_library(
    client: &SubsonicClient,
    since: Option<&Cursor>,
    known_ids: &HashSet<String>,
    page_size: u32,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<LibraryDelta> {
    sync_library_at(client, since, known_ids, page_size, progress, now_ms()).await
}

/// [`sync_library`] at a given time, for the tests.
pub(crate) async fn sync_library_at(
    client: &SubsonicClient,
    since: Option<&Cursor>,
    known_ids: &HashSet<String>,
    page_size: u32,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    now: i64,
) -> Result<LibraryDelta> {
    let _ = known_ids;
    // Read first, so a change made while the sync runs moves it past this cursor.
    // A server that can't say gives no cursor: the next check says "changed".
    let last_modified = client.last_modified().await.ok().flatten();
    if let (Some(since), Some(now_lm)) = (since, last_modified)
        && cursor_last_modified(since) == Some(now_lm)
        && cursor_full_at(since).is_some_and(|at| (0..FULL_SYNC_EVERY_MS).contains(&(now - at)))
    {
        return light_sync(client, since, progress).await;
    }
    let user = client.user_name().to_owned();
    let mut done: u64 = 0;
    let mut total: u64 = 0;
    let report = |done: u64, total: u64| {
        progress(SyncProgress {
            done,
            total: total.max(done),
        })
    };

    let index: ArtistsDto = client.fetch("getArtists", &[], "artists").await?;
    let artists: Vec<_> = index
        .index
        .iter()
        .flat_map(|i| i.artist.iter())
        .filter_map(map::to_artist)
        .collect();
    done += artists.len() as u64;
    total += artists.len() as u64;
    report(done, total);

    // Albums, page by page. A page shorter than asked for is the last; a server that
    // ignores `offset` and repeats a page is caught by the seen set.
    let size = page_size.clamp(1, MAX_ALBUM_PAGE);
    let size_s = size.to_string();
    let mut album_dtos: Vec<AlbumDto> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut offset: u64 = 0;
    loop {
        let offset_s = offset.to_string();
        let page: AlbumListDto = client
            .fetch(
                "getAlbumList2",
                &[
                    ("type", "alphabeticalByName"),
                    ("size", size_s.as_str()),
                    ("offset", offset_s.as_str()),
                ],
                "albumList2",
            )
            .await?;
        let n = page.album.len() as u64;
        let mut fresh = 0u64;
        for a in page.album {
            if let Some(id) = a.id.clone()
                && seen.insert(id)
            {
                total += 1 + a.song_count.unwrap_or(0).max(0) as u64;
                album_dtos.push(a);
                fresh += 1;
            }
        }
        done += fresh;
        report(done, total);
        offset += n;
        if n < u64::from(size) || fresh == 0 {
            break;
        }
    }

    let mut albums = Vec::with_capacity(album_dtos.len());
    let mut tracks = Vec::new();
    let mut favorite_item_ids: Vec<String> = Vec::new();
    let mut play_counts: Vec<(String, u32, Option<i64>)> = Vec::new();
    let mut track_ids: HashSet<String> = HashSet::new();
    for listed in &album_dtos {
        let Some(id) = listed.id.as_deref() else {
            continue;
        };
        // `getAlbum` gives the album again with its songs; prefer its fields.
        let full: AlbumDto = client.fetch("getAlbum", &[("id", id)], "album").await?;
        let album = if full.id.is_some() { &full } else { listed };
        if let Some(a) = map::to_album(album) {
            client.remember_cover(&a.source_item_id, a.artwork_tag.as_deref());
            albums.push(a);
        }
        let expected = listed.song_count.unwrap_or(0).max(0) as u64;
        let mut got = 0u64;
        for c in &album.song {
            if let Some(t) = map::to_track(c, Some(album))
                && track_ids.insert(t.source_item_id.clone())
            {
                user_data(
                    c,
                    &t.source_item_id,
                    &mut favorite_item_ids,
                    &mut play_counts,
                );
                tracks.push(t);
                got += 1;
            }
        }
        // Keep the count honest when an album had more or fewer songs than it said.
        total = total.saturating_sub(expected) + got;
        done += got;
        report(done, total);
    }

    let playlists = all_playlists(client, &user, &mut done, &mut total, &report).await?;

    Ok(LibraryDelta {
        full: true,
        artists,
        albums,
        tracks,
        playlists,
        playlists_complete: true,
        removed_item_ids: Vec::new(),
        favorite_item_ids: Some(favorite_item_ids),
        play_counts: Some(play_counts),
        cursor: last_modified.map(|lm| cursor_for(lm, now)),
    })
}

/// Every playlist this user can see, with its songs.
async fn all_playlists(
    client: &SubsonicClient,
    user: &str,
    done: &mut u64,
    total: &mut u64,
    report: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<Vec<Playlist>> {
    let listed = client.playlists().await?;
    *total += listed.len() as u64;
    let mut playlists = Vec::with_capacity(listed.len());
    for p in &listed {
        let Some(id) = p.id.as_deref() else { continue };
        let full = client.playlist(id).await?;
        let full = if full.id.is_some() { &full } else { p };
        if let Some(pl) = map::to_playlist(full, user) {
            client.remember_cover(&pl.source_item_id, pl.artwork_tag.as_deref());
            playlists.push(pl);
        }
        *done += 1;
        report(*done, *total);
    }
    Ok(playlists)
}

/// The library hasn't changed: only what `lastModified` doesn't cover. The playlists
/// come whole (`playlists_complete`, so one deleted elsewhere goes), the favourites
/// from `getStarred2`, and the cursor stays as it was, so a full sync still comes a
/// day after the last one.
async fn light_sync(
    client: &SubsonicClient,
    since: &Cursor,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<LibraryDelta> {
    let user = client.user_name().to_owned();
    let (mut done, mut total) = (0u64, 0u64);
    let report = |done: u64, total: u64| {
        progress(SyncProgress {
            done,
            total: total.max(done),
        })
    };
    let starred: Starred2Dto = client.fetch("getStarred2", &[], "starred2").await?;
    let favorite_item_ids: Vec<String> = starred
        .song
        .iter()
        .filter_map(|c| c.id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    let playlists = all_playlists(client, &user, &mut done, &mut total, &report).await?;
    report(done, total);
    Ok(LibraryDelta {
        full: false,
        playlists,
        playlists_complete: true,
        favorite_item_ids: Some(favorite_item_ids),
        cursor: Some(since.clone()),
        ..LibraryDelta::default()
    })
}

/// A song's favourite mark and play count (Subsonic's `starred`, `playCount` and
/// OpenSubsonic's `played`), read while the song is at hand.
fn user_data(
    c: &Child,
    id: &str,
    favorites: &mut Vec<String>,
    plays: &mut Vec<(String, u32, Option<i64>)>,
) {
    if c.starred.as_deref().is_some_and(|s| !s.trim().is_empty()) {
        favorites.push(id.to_owned());
    }
    if let Some(n) = c
        .play_count
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
    {
        let last = c
            .played
            .as_deref()
            .and_then(crate::jellyfin::time::parse_utc_ms);
        plays.push((id.to_owned(), n, last));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip() {
        let c = cursor_for(1_762_400_000_000, 1_762_500_000_000);
        assert_eq!(c.0, "subsonic1:lm=1762400000000;full=1762500000000");
        assert_eq!(cursor_last_modified(&c), Some(1_762_400_000_000));
        assert_eq!(cursor_full_at(&c), Some(1_762_500_000_000));
        // Cursors from before the light sync: read, but no full sync time.
        let old = Cursor("subsonic1:lm=1762400000000".into());
        assert_eq!(cursor_last_modified(&old), Some(1_762_400_000_000));
        assert_eq!(cursor_full_at(&old), None);
        assert_eq!(
            cursor_last_modified(&Cursor("2026-11-06T00:00:00Z".into())),
            None
        );
    }
}
