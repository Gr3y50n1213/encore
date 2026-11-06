//! The normalized library model every adapter maps into (plan section 4).
//!
//! Items carry the source's own ID (`source_item_id`). The store gives each one a
//! stable internal row ID per profile; nothing here depends on that ID.

use serde::{Deserialize, Serialize};

/// Which kind of media server an item came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Jellyfin,
}

impl SourceKind {
    /// The stable name stored in the database and shown in logs.
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Jellyfin => "jellyfin",
        }
    }

    pub fn parse(s: &str) -> Option<SourceKind> {
        match s {
            "jellyfin" => Some(SourceKind::Jellyfin),
            _ => None,
        }
    }
}

/// What a source can do. The UI hides anything that is false, so it never shows a
/// dead button (plan section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub lyrics: bool,
    pub transcoding: bool,
    pub playlists_writable: bool,
    pub quick_connect: bool,
    pub playback_reporting: bool,
}

/// A credited artist on a track or album.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistCredit {
    /// The source's ID for the artist, when the server links one.
    pub source_item_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub source_item_id: String,
    pub name: String,
    pub sort_name: String,
    pub mbid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub source_item_id: String,
    pub title: String,
    pub sort_title: String,
    pub album_artists: Vec<ArtistCredit>,
    pub year: Option<i32>,
    pub mbid: Option<String>,
    pub genres: Vec<String>,
    /// When the server added it, UTC milliseconds.
    pub date_added_utc_ms: Option<i64>,
    pub has_artwork: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub source_item_id: String,
    pub title: String,
    pub sort_title: String,
    pub album_item_id: Option<String>,
    pub album_title: Option<String>,
    pub artists: Vec<ArtistCredit>,
    pub album_artists: Vec<ArtistCredit>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub duration_ms: u64,
    pub year: Option<i32>,
    pub container: Option<String>,
    pub bitrate: Option<u32>,
    /// MusicBrainz recording ID, when tagged.
    pub mbid_recording: Option<String>,
    pub genres: Vec<String>,
    pub date_added_utc_ms: Option<i64>,
    pub has_lyrics: bool,
    /// ReplayGain track and album gain in dB, when the server reports them.
    pub track_gain_db: Option<f64>,
    pub album_gain_db: Option<f64>,
}

impl Track {
    /// The artist name the song key uses: the first credited artist, else the first
    /// album artist, else empty.
    pub fn key_artist(&self) -> &str {
        self.artists
            .first()
            .or_else(|| self.album_artists.first())
            .map(|a| a.name.as_str())
            .unwrap_or("")
    }

    /// The stable song key (`song_key` module).
    pub fn song_key(&self) -> String {
        crate::song_key::song_key(
            self.mbid_recording.as_deref(),
            self.key_artist(),
            &self.title,
            self.duration_ms,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistEntry {
    /// The server's ID for this entry in the playlist (needed to remove it later).
    pub entry_id: Option<String>,
    pub track_item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub source_item_id: String,
    pub name: String,
    pub date_added_utc_ms: Option<i64>,
    pub entries: Vec<PlaylistEntry>,
}

/// Where a library sync continues from. Opaque to everything but the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor(pub String);

/// The result of one library sync: everything new or changed since the cursor,
/// and the IDs that no longer exist on the server.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LibraryDelta {
    /// True when this delta is the whole library (a first sync).
    pub full: bool,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    /// Source item IDs (of any type) that were removed on the server.
    pub removed_item_ids: Vec<String>,
    /// Pass this back to the next sync.
    pub cursor: Option<Cursor>,
}
