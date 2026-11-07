//! The parts of Jellyfin's replies the adapter reads, named as in the 12.0 OpenAPI
//! document (`/api-docs/openapi.json` on the server). Every field is optional,
//! because the server leaves out what it does not know.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PublicSystemInfo {
    pub server_name: Option<String>,
    pub version: Option<String>,
    pub product_name: Option<String>,
    pub id: Option<String>,
    pub startup_wizard_completed: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserDto {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AuthenticationResult {
    pub user: Option<UserDto>,
    pub access_token: Option<String>,
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct QuickConnectResult {
    pub authenticated: Option<bool>,
    pub secret: Option<String>,
    pub code: Option<String>,
    pub date_added: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct NameGuidPair {
    pub name: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct BaseItem {
    pub id: Option<String>,
    pub name: Option<String>,
    pub sort_name: Option<String>,
    #[serde(rename = "Type")]
    pub kind: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_artist: Option<String>,
    pub album_artists: Option<Vec<NameGuidPair>>,
    pub artist_items: Option<Vec<NameGuidPair>>,
    pub artists: Option<Vec<String>>,
    pub index_number: Option<i64>,
    pub parent_index_number: Option<i64>,
    pub run_time_ticks: Option<i64>,
    pub production_year: Option<i32>,
    pub container: Option<String>,
    pub genres: Option<Vec<String>>,
    pub provider_ids: Option<HashMap<String, Option<String>>>,
    pub date_created: Option<String>,
    pub has_lyrics: Option<bool>,
    pub normalization_gain: Option<f64>,
    pub album_normalization_gain: Option<f64>,
    pub image_tags: Option<HashMap<String, Option<String>>>,
    pub playlist_item_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ItemsResult {
    pub items: Option<Vec<BaseItem>>,
    pub total_record_count: Option<i64>,
    pub start_index: Option<i64>,
    /// Not in the JSON: the server's clock when it answered (HTTP `Date`).
    #[serde(skip)]
    pub server_date_utc_ms: Option<i64>,
}

/// `POST /Playlists` reply.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlaylistCreated {
    pub id: Option<String>,
}
