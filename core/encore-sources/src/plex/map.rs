//! Plex metadata to the normalized model (ADR core-0023 point 6).
//!
//! - Artists are type 8, albums 9, tracks 10. A track's album is its parent, the album's
//!   artist its grandparent; `originalTitle` is the track's own artist when it differs.
//! - Durations are already milliseconds; `addedAt` and `lastViewedAt` are seconds.
//! - A track's ID is `"<ratingKey>:<part ID>"` ([`track_id`]): the rating key is what
//!   playlists, ratings and metadata take; the part is the file, which streaming,
//!   downloads and sound analysis need (`/library/parts/<id>/file`) and which can't be
//!   looked up while building a stream address. A track with no file is skipped.
//! - MusicBrainz IDs come from the `Guid` list (`mbid://...`) when the server sends one.
//! - An album's `thumb` is `/library/metadata/<id>/thumb/<time>`: the time changes with
//!   the picture, so it is the album's artwork tag.
//! - "Liked" is a rating of 4.5 stars or more (`userRating` 9 or 10, ADR core-0023 point
//!   7); a like writes 5 stars and an unlike clears the rating.

use super::dto::{Metadata, StreamDto};
use crate::model::{Album, Artist, ArtistCredit, Playlist, PlaylistEntry, Track};

/// The rating from which a song counts as liked (0 to 10, two per star).
pub(crate) const LIKED_FROM: f64 = 9.0;

fn clean(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn name_or_unknown(n: &Option<String>) -> String {
    clean(n).unwrap_or_else(|| "Unknown".to_owned())
}

fn small(n: Option<i64>) -> Option<u32> {
    n.and_then(|v| u32::try_from(v).ok()).filter(|v| *v > 0)
}

fn year(n: Option<i64>) -> Option<i32> {
    n.and_then(|v| i32::try_from(v).ok()).filter(|v| *v > 0)
}

pub(crate) fn seconds_to_ms(s: Option<i64>) -> Option<i64> {
    s.filter(|s| *s > 0).map(|s| s.saturating_mul(1000))
}

fn mbid(m: &Metadata) -> Option<String> {
    m.guid.iter().find_map(|g| {
        g.id.as_deref()?
            .strip_prefix("mbid://")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    })
}

fn genres(m: &Metadata) -> Vec<String> {
    m.genre.iter().filter_map(|g| clean(&g.tag)).collect()
}

/// A track's ID from its rating key and part ID.
pub(crate) fn track_id(rating_key: &str, part_id: &str) -> String {
    format!("{rating_key}:{part_id}")
}

/// The rating key of any item ID (a track's ID carries its part after a colon).
pub(crate) fn rating_key(item_id: &str) -> &str {
    item_id.split_once(':').map_or(item_id, |(k, _)| k)
}

/// The part ID of a track's ID, if it has one.
pub(crate) fn part_id(item_id: &str) -> Option<&str> {
    item_id
        .split_once(':')
        .map(|(_, p)| p)
        .filter(|p| !p.is_empty())
}

/// The first file of a track: its part ID and container.
fn first_part(m: &Metadata) -> Option<(String, Option<String>)> {
    m.media.iter().find_map(|md| {
        md.part.iter().find_map(|p| {
            Some((
                clean(&p.id)?,
                clean(&p.container).or_else(|| clean(&md.container)),
            ))
        })
    })
}

/// The ID of a track as listed, or None when it isn't a track with a file.
pub(crate) fn track_id_of(m: &Metadata) -> Option<String> {
    if m.kind.as_deref().is_some_and(|k| k != "track") {
        return None;
    }
    let (part, _) = first_part(m)?;
    Some(track_id(&clean(&m.rating_key)?, &part))
}

fn audio_stream(m: &Metadata) -> Option<&StreamDto> {
    m.media
        .iter()
        .flat_map(|md| md.part.iter())
        .flat_map(|p| p.stream.iter())
        .find(|s| s.stream_type == Some(2))
}

pub(crate) fn to_track(m: &Metadata) -> Option<Track> {
    let id = track_id_of(m)?;
    let (_, container) = first_part(m)?;
    let title = name_or_unknown(&m.title);
    let album_artist = clean(&m.grandparent_title).map(|name| ArtistCredit {
        source_item_id: clean(&m.grandparent_rating_key),
        name,
    });
    let artists = match clean(&m.original_title) {
        Some(name) => vec![ArtistCredit {
            source_item_id: None,
            name,
        }],
        None => album_artist.clone().into_iter().collect(),
    };
    let audio = audio_stream(m);
    Some(Track {
        source_item_id: id,
        sort_title: clean(&m.title_sort).unwrap_or_else(|| title.clone()),
        title,
        album_item_id: clean(&m.parent_rating_key),
        album_title: clean(&m.parent_title),
        artists,
        album_artists: album_artist.into_iter().collect(),
        disc_number: small(m.parent_index),
        track_number: small(m.index),
        duration_ms: m.duration.map(|d| d.max(0) as u64).unwrap_or(0),
        year: year(m.year).or_else(|| year(m.parent_year)),
        container: container.map(|c| c.to_ascii_lowercase()),
        bitrate: small(m.media.first().and_then(|md| md.bitrate)),
        mbid_recording: mbid(m),
        genres: genres(m),
        date_added_utc_ms: seconds_to_ms(m.added_at),
        // The listing doesn't carry a track's lyric streams; asking is the only way.
        has_lyrics: true,
        track_gain_db: audio.and_then(|s| s.gain),
        album_gain_db: audio.and_then(|s| s.album_gain),
    })
}

pub(crate) fn to_album(m: &Metadata) -> Option<Album> {
    let id = clean(&m.rating_key)?;
    let title = name_or_unknown(&m.title);
    let thumb = clean(&m.thumb);
    Some(Album {
        source_item_id: id,
        sort_title: clean(&m.title_sort).unwrap_or_else(|| title.clone()),
        title,
        album_artists: clean(&m.parent_title)
            .map(|name| ArtistCredit {
                source_item_id: clean(&m.parent_rating_key),
                name,
            })
            .into_iter()
            .collect(),
        year: year(m.year),
        mbid: mbid(m),
        genres: genres(m),
        date_added_utc_ms: seconds_to_ms(m.added_at),
        has_artwork: thumb.is_some(),
        artwork_tag: thumb,
    })
}

pub(crate) fn to_artist(m: &Metadata) -> Option<Artist> {
    let id = clean(&m.rating_key)?;
    let name = name_or_unknown(&m.title);
    Some(Artist {
        source_item_id: id,
        sort_name: clean(&m.title_sort).unwrap_or_else(|| name.clone()),
        name,
        mbid: mbid(m),
    })
}

/// A playlist's tracks as entries; the entry ID is Plex's `playlistItemID`.
pub(crate) fn to_entries(items: &[Metadata]) -> Vec<PlaylistEntry> {
    items
        .iter()
        .filter_map(|m| {
            Some(PlaylistEntry {
                entry_id: clean(&m.playlist_item_id),
                track_item_id: track_id_of(m)?,
            })
        })
        .collect()
}

/// An audio playlist. Plex lists only the user's own playlists, so every one can be
/// changed except a smart one (its songs come from its rules).
pub(crate) fn to_playlist(m: &Metadata, items: &[Metadata]) -> Option<Playlist> {
    if m.playlist_type.as_deref().is_some_and(|t| t != "audio") {
        return None;
    }
    Some(Playlist {
        source_item_id: clean(&m.rating_key)?,
        name: name_or_unknown(&m.title),
        date_added_utc_ms: seconds_to_ms(m.added_at),
        entries: to_entries(items),
        artwork_tag: clean(&m.composite).or_else(|| clean(&m.thumb)),
        can_edit: m.smart != Some(true),
    })
}

/// Whether a track's rating makes it a liked song.
pub(crate) fn is_liked(m: &Metadata) -> bool {
    m.user_rating.is_some_and(|r| r >= LIKED_FROM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(json: &str) -> Metadata {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_track_carries_its_part_and_its_own_artist() {
        let m = meta(
            r#"{"ratingKey":"31","type":"track","title":"Low Lantern","parentRatingKey":"30",
                "parentTitle":"Quiet Rooms","grandparentRatingKey":"29","grandparentTitle":"Paper Harbour",
                "originalTitle":"Paper Harbour feat. Wren","index":2,"parentIndex":1,"parentYear":2021,
                "duration":201000,"addedAt":1762400000,"userRating":10,
                "Guid":[{"id":"mbid://0b9d0c43-9a64-4a33-9e5a-0c8b0e1f1a2b"}],
                "Media":[{"bitrate":900,"container":"flac","Part":[{"id":"77","key":"/library/parts/77/1762400000/file.flac"}]}]}"#,
        );
        let t = to_track(&m).unwrap();
        assert_eq!(t.source_item_id, "31:77");
        assert_eq!(rating_key(&t.source_item_id), "31");
        assert_eq!(part_id(&t.source_item_id), Some("77"));
        assert_eq!(t.artists[0].name, "Paper Harbour feat. Wren");
        assert_eq!(t.album_artists[0].source_item_id.as_deref(), Some("29"));
        assert_eq!(t.album_item_id.as_deref(), Some("30"));
        assert_eq!((t.disc_number, t.track_number), (Some(1), Some(2)));
        assert_eq!(t.duration_ms, 201_000);
        assert_eq!(t.year, Some(2021));
        assert_eq!(t.container.as_deref(), Some("flac"));
        assert_eq!(t.date_added_utc_ms, Some(1_762_400_000_000));
        assert_eq!(
            t.mbid_recording.as_deref(),
            Some("0b9d0c43-9a64-4a33-9e5a-0c8b0e1f1a2b")
        );
        assert!(is_liked(&m));
        assert!(!is_liked(&meta(r#"{"userRating":8}"#)));
        // No file, no track.
        assert!(to_track(&meta(r#"{"ratingKey":"5","type":"track","title":"x"}"#)).is_none());
    }

    #[test]
    fn smart_playlists_are_read_only() {
        let p = meta(r#"{"ratingKey":"40","title":"Mix","smart":true,"playlistType":"audio"}"#);
        assert!(!to_playlist(&p, &[]).unwrap().can_edit);
        let v = meta(r#"{"ratingKey":"41","title":"Films","playlistType":"video"}"#);
        assert!(to_playlist(&v, &[]).is_none());
    }
}
