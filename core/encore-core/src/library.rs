//! Library sync: a source's delta into the profile's mirror (plan section 4).
//!
//! The store is not held across network calls: the cursor and the known IDs are
//! read, the store is released, the source syncs, and the result is applied in one
//! transaction. Browsing keeps working the whole time.

use std::sync::Mutex;

use encore_sources::jellyfin::SyncProgress;
use encore_sources::{Cursor, MediaSource, SourceError};
use encore_store::{ApplyStats, Store, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the profile store is unavailable")]
    Poisoned,
}

/// Sync one source of the open profile. `page_size` is `library.syncPageSize`;
/// `now_utc_ms` is saved as the time of the last sync (board 18's "last sync").
pub async fn sync_source(
    store: &Mutex<Store>,
    source_id: i64,
    source: &dyn MediaSource,
    page_size: u32,
    now_utc_ms: i64,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<ApplyStats, SyncError> {
    let (cursor, known) = {
        let s = store.lock().map_err(|_| SyncError::Poisoned)?;
        let record = s.source(source_id)?;
        (
            record.sync_cursor.map(Cursor),
            s.mirrored_item_ids(source_id)?,
        )
    };
    let delta = source
        .sync_library(cursor.as_ref(), &known, page_size, progress)
        .await?;
    let s = store.lock().map_err(|_| SyncError::Poisoned)?;
    let stats = s.apply_delta(source_id, &delta)?;
    s.set_sync_state(
        source_id,
        delta.cursor.as_ref().map(|c| c.0.as_str()),
        now_utc_ms,
    )?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use encore_sources::jellyfin::{self, DeviceInfo, JellyfinClient};
    use encore_sources::{FixtureTransport, JellyfinSource, Transport};
    use encore_store::{NewSource, Page, SearchLimits};

    use super::*;

    const RECORDED: &str = include_str!("../../../tools/fixtures/jellyfin/recorded.json");

    /// A store with one Jellyfin source, signed in through the recording.
    async fn setup() -> (Mutex<Store>, i64, JellyfinSource) {
        let t = Arc::new(FixtureTransport::from_json(RECORDED).unwrap());
        let device = DeviceInfo {
            client: "Encore".into(),
            device_name: "Replay".into(),
            device_id: "replay".into(),
            version: "0.1.0".into(),
        };
        let plan =
            jellyfin::plan_address("http://jellyfin.local:8096", &[443, 8920], 8096).unwrap();
        let info = jellyfin::probe(
            t.clone() as Arc<dyn Transport>,
            &plan,
            &device,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        let c = JellyfinClient::new(t, info.base_url.clone(), device, Duration::from_secs(5));
        let code = c.quick_connect_start().await.unwrap();
        while !c.quick_connect_poll(&code.secret).await.unwrap() {}
        let session = c.quick_connect_finish(&code.secret).await.unwrap();
        let store = Store::open_in_memory().unwrap();
        let id = store
            .add_source(&NewSource {
                kind: "jellyfin",
                name: &info.name,
                base_url: info.base_url.as_str(),
                server_id: &info.server_id,
                server_version: Some(&info.version),
                user_id: &session.user_id,
                user_name: &session.user_name,
                pinned_cert_sha256: None,
                created_utc_ms: 1,
            })
            .unwrap();
        let source = JellyfinSource {
            client: c.signed_in(session.token, session.user_id),
            quick_connect: true,
        };
        (Mutex::new(store), id, source)
    }

    const ALL: Page = Page {
        offset: 0,
        limit: 1000,
    };

    #[tokio::test]
    async fn a_recorded_library_lands_in_the_mirror_and_can_be_browsed_offline() {
        let (store, id, source) = setup().await;
        let stats = sync_source(&store, id, &source, 50, 1_000, &|_| {})
            .await
            .unwrap();
        assert_eq!(
            (stats.artists, stats.albums, stats.tracks, stats.playlists),
            (26, 38, 150, 1)
        );
        let s = store.lock().unwrap();
        assert_eq!(s.track_count(id).unwrap(), 150);
        let record = s.source(id).unwrap();
        assert!(record.sync_cursor.is_some());
        assert_eq!(record.last_sync_utc_ms, Some(1_000));

        // Browse: every album lists its songs in disc and track order.
        let albums = s.albums(encore_store::AlbumSort::Title, ALL).unwrap();
        assert_eq!(albums.len(), 38);
        let mut songs_on_albums = 0;
        for a in &albums {
            let tracks = s.album_tracks(a.id).unwrap();
            let order: Vec<(u32, u32)> = tracks
                .iter()
                .map(|t| (t.disc_number.unwrap_or(1), t.track_number.unwrap_or(0)))
                .collect();
            let mut sorted = order.clone();
            sorted.sort();
            assert_eq!(order, sorted, "an album plays in disc and track order");
            songs_on_albums += tracks.len();
        }
        assert_eq!(songs_on_albums, 150);
        assert_eq!(s.artists(ALL).unwrap().len(), 26);
        assert_eq!(s.playlists(ALL).unwrap().len(), 1);

        // Search finds a song by part of its (anonymized) title.
        let some = s
            .tracks(
                encore_store::TrackSort::Title,
                Page {
                    offset: 0,
                    limit: 1,
                },
            )
            .unwrap();
        let title = &some[0].title;
        let part: String = title.chars().take(4).collect();
        let limits = SearchLimits {
            songs: 20,
            albums: 10,
            artists: 10,
            playlists: 10,
        };
        let found = s.search(&part, &limits, 2).unwrap();
        assert!(found.songs.iter().any(|t| &t.title == title) || found.top.is_some());
    }

    #[tokio::test]
    async fn the_next_sync_is_a_delta_that_changes_nothing() {
        let (store, id, source) = setup().await;
        sync_source(&store, id, &source, 50, 1_000, &|_| {})
            .await
            .unwrap();
        let before = store.lock().unwrap().mirrored_item_ids(id).unwrap();
        let stats = sync_source(&store, id, &source, 50, 2_000, &|_| {})
            .await
            .unwrap();
        assert_eq!(stats.removed, 0);
        assert_eq!((stats.artists, stats.albums, stats.tracks), (0, 0, 0));
        let s = store.lock().unwrap();
        assert_eq!(s.mirrored_item_ids(id).unwrap(), before);
        assert_eq!(s.source(id).unwrap().last_sync_utc_ms, Some(2_000));
    }
}
