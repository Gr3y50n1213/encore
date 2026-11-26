//! The Subsonic JSON shapes Encore reads (opensubsonic.github.io/docs/responses/).
//!
//! Servers differ in small ways, so every field is optional and read leniently: an ID
//! or a number may come as a string or a number, and a list holding one item may come
//! as that item alone (older Subsonic JSON did this). An entry that can't be read is
//! skipped instead of failing the whole reply.

use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A list, a single item standing for a list of one, or nothing.
pub(crate) fn many<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let read = |v: Value| serde_json::from_value::<T>(v).ok();
    Ok(match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.into_iter().filter_map(read).collect(),
        Some(one) => read(one).into_iter().collect(),
    })
}

/// A string, or a number or boolean written as one.
pub(crate) fn text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    })
}

/// A whole number, or one written as a string.
pub(crate) fn int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok().map(|f| f.round() as i64),
        _ => None,
    })
}

/// A number with a fraction, or one written as a string.
pub(crate) fn float<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

/// A boolean, or "true"/"false".
pub(crate) fn flag<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Bool(b)) => Some(b),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

/// `ItemGenre`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Genre {
    #[serde(default, deserialize_with = "text")]
    pub(crate) name: Option<String>,
}

/// `ArtistID3` (also the short form inside a song's `artists` and `albumArtists`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) music_brainz_id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) sort_name: Option<String>,
}

/// `ReplayGain`: gains in dB.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReplayGain {
    #[serde(default, deserialize_with = "float")]
    pub(crate) track_gain: Option<f64>,
    #[serde(default, deserialize_with = "float")]
    pub(crate) album_gain: Option<f64>,
}

/// `Child`: a song. `duration` is in seconds.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Child {
    #[serde(default, deserialize_with = "text")]
    pub(crate) id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) title: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) album: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) artist: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) track: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) year: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) genre: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) suffix: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) duration: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) bit_rate: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) play_count: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) disc_number: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) created: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) album_id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) artist_id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) starred: Option<String>,
    /// OpenSubsonic: when last played.
    #[serde(default, deserialize_with = "text")]
    pub(crate) played: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) music_brainz_id: Option<String>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) genres: Vec<Genre>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) artists: Vec<ArtistDto>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) album_artists: Vec<ArtistDto>,
    #[serde(default)]
    pub(crate) replay_gain: Option<ReplayGain>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) sort_name: Option<String>,
    #[serde(default, deserialize_with = "flag")]
    pub(crate) is_dir: Option<bool>,
}

/// `AlbumID3`, and `AlbumID3WithSongs` when `song` is there.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) artist: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) artist_id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) cover_art: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) song_count: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) created: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) year: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) genre: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) music_brainz_id: Option<String>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) genres: Vec<Genre>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) artists: Vec<ArtistDto>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) sort_name: Option<String>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) song: Vec<Child>,
}

/// `getArtists`: `artists.index[].artist[]`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ArtistsDto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) index: Vec<IndexDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct IndexDto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) artist: Vec<ArtistDto>,
}

/// `getIndexes`: only `lastModified` is read (milliseconds since 1970).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexesDto {
    #[serde(default, deserialize_with = "int")]
    pub(crate) last_modified: Option<i64>,
}

/// `getAlbumList2`: `albumList2.album[]`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct AlbumListDto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) album: Vec<AlbumDto>,
}

/// `Playlist`, and `PlaylistWithSongs` when `entry` is there.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) owner: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) created: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) cover_art: Option<String>,
    /// OpenSubsonic: true when this user may not change it.
    #[serde(default, deserialize_with = "flag")]
    pub(crate) readonly: Option<bool>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) entry: Vec<Child>,
}

/// `getStarred2`: the songs this user starred (artists and albums are not used).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Starred2Dto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) song: Vec<Child>,
}

/// `getPlaylists`: `playlists.playlist[]`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct PlaylistsDto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) playlist: Vec<PlaylistDto>,
}

/// One `structuredLyrics` entry (OpenSubsonic `songLyrics`). `offset` in ms, positive
/// shows the lyrics sooner; `start` in ms.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredLyrics {
    #[serde(default, deserialize_with = "text")]
    pub(crate) lang: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) offset: Option<i64>,
    #[serde(default, deserialize_with = "flag")]
    pub(crate) synced: Option<bool>,
    #[serde(default, deserialize_with = "many")]
    pub(crate) line: Vec<LyricLine>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct LyricLine {
    #[serde(default, deserialize_with = "int")]
    pub(crate) start: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) value: Option<String>,
}

/// `getLyricsBySongId`: `lyricsList.structuredLyrics[]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LyricsListDto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) structured_lyrics: Vec<StructuredLyrics>,
}

/// `getLyrics`: `lyrics.value` holds the text.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct PlainLyricsDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) value: Option<String>,
}

/// `getUser`'s `user`, and `tokenInfo` (both carry `username`).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct UserDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) username: Option<String>,
}

/// One `openSubsonicExtensions` entry.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ExtensionDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) name: Option<String>,
}

/// `getOpenSubsonicExtensions`' list, under its own key.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtensionsDto {
    #[serde(default, deserialize_with = "many")]
    pub(crate) open_subsonic_extensions: Vec<ExtensionDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_reading() {
        let c: Child = serde_json::from_str(
            r#"{"id":123,"title":"Low Lantern","track":"4","duration":201.6,"genres":{"name":"Folk"},
                "artists":[{"id":"ar1","name":"Paper Harbour"},"not an artist"],"replayGain":{"trackGain":"-7.25"}}"#,
        )
        .unwrap();
        assert_eq!(c.id.as_deref(), Some("123"));
        assert_eq!(c.track, Some(4));
        assert_eq!(c.duration, Some(202));
        assert_eq!(c.genres.len(), 1);
        assert_eq!(c.artists.len(), 1);
        assert_eq!(c.replay_gain.unwrap().track_gain, Some(-7.25));
        let p: PlaylistDto = serde_json::from_str(r#"{"id":"p","readonly":"true"}"#).unwrap();
        assert_eq!(p.readonly, Some(true));
        assert!(p.entry.is_empty());
    }
}
