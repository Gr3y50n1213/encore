//! The Plex Media Server JSON shapes Encore reads (asked for with `Accept:
//! application/json`; every reply is a `MediaContainer`).
//!
//! Read leniently, as for Subsonic: every field is optional, IDs and numbers may come as
//! strings or numbers, and an entry that can't be read is skipped. Plex's JSON can repeat
//! a key inside one object (seen in `/system/agents`); only fields Encore reads are
//! declared, so a repeat elsewhere is ignored.

use serde::Deserialize;

use crate::subsonic::dto::{flag, float, int, many, text};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Envelope {
    #[serde(rename = "MediaContainer", default)]
    pub(crate) container: Container,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Container {
    #[serde(default, deserialize_with = "int")]
    pub(crate) total_size: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) machine_identifier: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) version: Option<String>,
    #[serde(default, deserialize_with = "flag")]
    pub(crate) claimed: Option<bool>,
    #[serde(rename = "Metadata", default, deserialize_with = "many")]
    pub(crate) metadata: Vec<Metadata>,
    #[serde(rename = "Directory", default, deserialize_with = "many")]
    pub(crate) directory: Vec<Directory>,
}

/// A library section (`/library/sections`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Directory {
    #[serde(default, deserialize_with = "text")]
    pub(crate) key: Option<String>,
    #[serde(default, rename = "type", deserialize_with = "text")]
    pub(crate) kind: Option<String>,
    /// A counter the server moves whenever the section's content changes.
    #[serde(default, deserialize_with = "int")]
    pub(crate) content_changed_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Tag {
    #[serde(default, deserialize_with = "text")]
    pub(crate) tag: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct GuidDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamDto {
    /// 2 audio, 4 lyrics.
    #[serde(default, deserialize_with = "int")]
    pub(crate) stream_type: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) codec: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) format: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) key: Option<String>,
    #[serde(default, deserialize_with = "float")]
    pub(crate) gain: Option<f64>,
    #[serde(default, deserialize_with = "float")]
    pub(crate) album_gain: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PartDto {
    #[serde(default, deserialize_with = "text")]
    pub(crate) id: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) container: Option<String>,
    #[serde(rename = "Stream", default, deserialize_with = "many")]
    pub(crate) stream: Vec<StreamDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaDto {
    #[serde(default, deserialize_with = "int")]
    pub(crate) bitrate: Option<i64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) container: Option<String>,
    #[serde(rename = "Part", default, deserialize_with = "many")]
    pub(crate) part: Vec<PartDto>,
}

/// An artist, album, track or playlist.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Metadata {
    #[serde(default, deserialize_with = "text")]
    pub(crate) rating_key: Option<String>,
    #[serde(default, rename = "type", deserialize_with = "text")]
    pub(crate) kind: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) title: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) title_sort: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) parent_rating_key: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) parent_title: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) grandparent_rating_key: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) grandparent_title: Option<String>,
    /// A track's own artist when it differs from the album's.
    #[serde(default, deserialize_with = "text")]
    pub(crate) original_title: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) index: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) parent_index: Option<i64>,
    /// Milliseconds.
    #[serde(default, deserialize_with = "int")]
    pub(crate) duration: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) year: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) parent_year: Option<i64>,
    /// Seconds since 1970.
    #[serde(default, deserialize_with = "int")]
    pub(crate) added_at: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) view_count: Option<i64>,
    #[serde(default, deserialize_with = "int")]
    pub(crate) last_viewed_at: Option<i64>,
    /// 0 to 10 (half stars).
    #[serde(default, deserialize_with = "float")]
    pub(crate) user_rating: Option<f64>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) thumb: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) composite: Option<String>,
    #[serde(default, deserialize_with = "flag")]
    pub(crate) smart: Option<bool>,
    #[serde(default, deserialize_with = "text")]
    pub(crate) playlist_type: Option<String>,
    #[serde(rename = "playlistItemID", default, deserialize_with = "text")]
    pub(crate) playlist_item_id: Option<String>,
    #[serde(rename = "Media", default, deserialize_with = "many")]
    pub(crate) media: Vec<MediaDto>,
    #[serde(rename = "Genre", default, deserialize_with = "many")]
    pub(crate) genre: Vec<Tag>,
    #[serde(rename = "Guid", default, deserialize_with = "many")]
    pub(crate) guid: Vec<GuidDto>,
}
