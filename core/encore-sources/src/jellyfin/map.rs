//! Jellyfin items to the normalized model.
//!
//! - Durations are in ticks of 100 ns (`RunTimeTicks`).
//! - The MusicBrainz recording ID is `ProviderIds.MusicBrainzRecording`; Jellyfin
//!   reads it from the `MUSICBRAINZ_TRACKID` tag, and `MusicBrainzTrack` holds the
//!   release-track ID instead (AudioFileProber.cs in Jellyfin 12.0, lines 533-557).
//! - `NormalizationGain` and `AlbumNormalizationGain` are ReplayGain-style dB.

use std::collections::HashMap;

use super::dto::{BaseItem, NameGuidPair};
use super::time::parse_utc_ms;
use crate::model::{Album, Artist, ArtistCredit, Playlist, PlaylistEntry, Track};

pub(crate) fn provider(ids: &Option<HashMap<String, Option<String>>>, key: &str) -> Option<String> {
    ids.as_ref()?
        .get(key)?
        .as_ref()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn credits(pairs: &Option<Vec<NameGuidPair>>) -> Vec<ArtistCredit> {
    pairs
        .iter()
        .flatten()
        .filter_map(|p| {
            let name = p.name.as_deref()?.trim();
            (!name.is_empty()).then(|| ArtistCredit {
                source_item_id: p.id.clone(),
                name: name.to_owned(),
            })
        })
        .collect()
}

fn name_or_unknown(n: &Option<String>) -> String {
    n.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown")
        .to_owned()
}

fn sort_or(item: &BaseItem, name: &str) -> String {
    item.sort_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(name)
        .to_owned()
}

fn ticks_to_ms(t: Option<i64>) -> u64 {
    t.map(|t| (t.max(0) / 10_000) as u64).unwrap_or(0)
}

fn small(n: Option<i64>) -> Option<u32> {
    n.and_then(|v| u32::try_from(v).ok())
}

pub(crate) fn has_primary_image(item: &BaseItem) -> bool {
    primary_image_tag(item).is_some()
}

/// The tag of the item's own cover; it changes whenever the cover does.
pub(crate) fn primary_image_tag(item: &BaseItem) -> Option<String> {
    item.image_tags.as_ref()?.get("Primary")?.clone()
}

/// The linked album artists, or the plain `AlbumArtist` name when none are linked.
fn album_artists_of(item: &BaseItem) -> Vec<ArtistCredit> {
    let linked = credits(&item.album_artists);
    if !linked.is_empty() {
        return linked;
    }
    item.album_artist
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|a| {
            vec![ArtistCredit {
                source_item_id: None,
                name: a.to_owned(),
            }]
        })
        .unwrap_or_default()
}

pub(crate) fn to_track(item: &BaseItem) -> Option<Track> {
    let id = item.id.clone()?;
    let title = name_or_unknown(&item.name);
    let mut artists = credits(&item.artist_items);
    if artists.is_empty() {
        artists = item
            .artists
            .iter()
            .flatten()
            .filter(|n| !n.trim().is_empty())
            .map(|n| ArtistCredit {
                source_item_id: None,
                name: n.trim().to_owned(),
            })
            .collect();
    }
    let album_artists = album_artists_of(item);
    Some(Track {
        source_item_id: id,
        sort_title: sort_or(item, &title),
        title,
        album_item_id: item.album_id.clone(),
        album_title: item.album.clone(),
        artists,
        album_artists,
        disc_number: small(item.parent_index_number),
        track_number: small(item.index_number),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        year: item.production_year,
        container: item.container.clone(),
        bitrate: None,
        mbid_recording: provider(&item.provider_ids, "MusicBrainzRecording"),
        genres: item.genres.clone().unwrap_or_default(),
        date_added_utc_ms: item.date_created.as_deref().and_then(parse_utc_ms),
        has_lyrics: item.has_lyrics.unwrap_or(false),
        track_gain_db: item.normalization_gain,
        album_gain_db: item.album_normalization_gain,
    })
}

pub(crate) fn to_album(item: &BaseItem) -> Option<Album> {
    let id = item.id.clone()?;
    let title = name_or_unknown(&item.name);
    let album_artists = album_artists_of(item);
    Some(Album {
        source_item_id: id,
        sort_title: sort_or(item, &title),
        title,
        album_artists,
        year: item.production_year,
        mbid: provider(&item.provider_ids, "MusicBrainzAlbum"),
        genres: item.genres.clone().unwrap_or_default(),
        date_added_utc_ms: item.date_created.as_deref().and_then(parse_utc_ms),
        has_artwork: has_primary_image(item),
        artwork_tag: primary_image_tag(item),
    })
}

pub(crate) fn to_artist(item: &BaseItem) -> Option<Artist> {
    let id = item.id.clone()?;
    let name = name_or_unknown(&item.name);
    Some(Artist {
        source_item_id: id,
        sort_name: sort_or(item, &name),
        name,
        mbid: provider(&item.provider_ids, "MusicBrainzArtist"),
    })
}

pub(crate) fn to_playlist(item: &BaseItem, entries: Vec<PlaylistEntry>) -> Option<Playlist> {
    Some(Playlist {
        source_item_id: item.id.clone()?,
        name: name_or_unknown(&item.name),
        date_added_utc_ms: item.date_created.as_deref().and_then(parse_utc_ms),
        entries,
        artwork_tag: primary_image_tag(item),
        can_edit: true,
    })
}

pub(crate) fn to_playlist_entry(item: &BaseItem) -> Option<PlaylistEntry> {
    Some(PlaylistEntry {
        entry_id: item.playlist_item_id.clone(),
        track_item_id: item.id.clone()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(json: &str) -> BaseItem {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_tagged_track_maps_every_field() {
        let t = to_track(&item(
            r#"{"Id":"t1","Name":"Slow Tide","SortName":"slow tide","Type":"Audio","Album":"Golden Hour Static",
                "AlbumId":"al1","AlbumArtist":"Ember Cove","AlbumArtists":[{"Name":"Ember Cove","Id":"ar1"}],
                "ArtistItems":[{"Name":"Ember Cove","Id":"ar1"},{"Name":"Guest","Id":"ar9"}],
                "Artists":["Ember Cove","Guest"],"IndexNumber":3,"ParentIndexNumber":1,
                "RunTimeTicks":2224000000,"ProductionYear":2024,"Container":"flac","Genres":["Ambient"],
                "ProviderIds":{"MusicBrainzRecording":"5b11f4ce-a62d-471e-81fc-a69a8278c7da","MusicBrainzTrack":"other"},
                "DateCreated":"2025-11-06T03:33:20.0000000Z","HasLyrics":true,
                "NormalizationGain":-6.5,"AlbumNormalizationGain":-5.9,"ImageTags":{"Primary":"tag"}}"#,
        ))
        .unwrap();
        assert_eq!(t.duration_ms, 222_400);
        assert_eq!((t.disc_number, t.track_number), (Some(1), Some(3)));
        assert_eq!(
            t.mbid_recording.as_deref(),
            Some("5b11f4ce-a62d-471e-81fc-a69a8278c7da")
        );
        assert_eq!(t.artists.len(), 2);
        assert_eq!(t.artists[1].source_item_id.as_deref(), Some("ar9"));
        assert_eq!(t.album_artists[0].name, "Ember Cove");
        assert_eq!(t.date_added_utc_ms, Some(1_762_400_000_000));
        assert_eq!((t.track_gain_db, t.album_gain_db), (Some(-6.5), Some(-5.9)));
        assert!(t.has_lyrics);
        assert_eq!(t.sort_title, "slow tide");
        assert!(t.song_key().starts_with("mb:"));
    }

    #[test]
    fn a_sparse_track_still_maps() {
        let t = to_track(&item(r#"{"Id":"t2","Name":null,"Artists":["Solo"],"ArtistItems":null,"Genres":null,"ProviderIds":{"MusicBrainzRecording":null}}"#)).unwrap();
        assert_eq!(t.title, "Unknown");
        assert_eq!(t.artists[0].name, "Solo");
        assert_eq!(t.duration_ms, 0);
        assert_eq!(t.mbid_recording, None);
        assert!(t.genres.is_empty());
        assert!(t.song_key().starts_with("t1:"));
        assert!(to_track(&item(r#"{"Name":"no id"}"#)).is_none());
    }

    #[test]
    fn albums_artists_and_playlists() {
        let a = to_album(&item(r#"{"Id":"al1","Name":"Golden Hour Static","AlbumArtist":"Ember Cove","ProductionYear":2024,"ImageTags":{"Primary":"x"},"ProviderIds":{"MusicBrainzAlbum":"m"}}"#)).unwrap();
        assert_eq!(a.album_artists[0].name, "Ember Cove");
        assert!(a.has_artwork);
        assert_eq!(a.mbid.as_deref(), Some("m"));
        let ar = to_artist(&item(
            r#"{"Id":"ar1","Name":"The Ember Cove","SortName":"Ember Cove, The"}"#,
        ))
        .unwrap();
        assert_eq!(
            (ar.name.as_str(), ar.sort_name.as_str()),
            ("The Ember Cove", "Ember Cove, The")
        );
        let e = to_playlist_entry(&item(r#"{"Id":"t1","PlaylistItemId":"e7"}"#)).unwrap();
        let p = to_playlist(&item(r#"{"Id":"pl1","Name":"Evening Drive"}"#), vec![e]).unwrap();
        assert_eq!(p.entries[0].entry_id.as_deref(), Some("e7"));
    }
}
