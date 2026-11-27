//! Shared sound analysis (Phase 10, docs/adr/core-0019): what songs sound like, worked out once
//! by a device and kept here so the other devices of the same media server don't analyse the
//! same song again (and don't read it from the media server again).
//!
//! What is kept is about songs, not people: the song key, the analysis's numbers, the model that
//! made them, or why the audio could not be read. Rows are scoped to the media server the
//! profile belongs to, and a device only ever gets rows for keys it asks about (no listing), so
//! nothing says what is in anyone's library.
//!
//! - `POST /sound/lookup`: the rows held for up to 200 song keys.
//! - `POST /sound/claim`: "I'll analyse these" for up to 50 keys; each is granted to one device
//!   for an hour, and only when no row is held for it.
//! - `PUT /sound`: up to 200 rows a device made; a newer version replaces an older one.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::auth::Device;
use crate::error::ApiError;
use crate::{AppState, now_ms};

const MAX_LOOKUP: usize = 200;
const MAX_CLAIM: usize = 50;
const MAX_PUT: usize = 200;
/// The highest analysis version accepted. The core's analysis is at a low single digit;
/// this leaves room for many years of new versions.
pub(crate) const MAX_VERSION: u32 = 1000;

/// How long a claim keeps other devices off a song.
pub(crate) const CLAIM_MS: i64 = 60 * 60_000;

/// One song's analysis as a device made it (the core's `SoundRow`, bytes as hex).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedSound {
    pub song_key: String,
    pub version: u32,
    /// "ok" or "unavailable".
    pub state: String,
    pub reason: Option<String>,
    pub profile: Option<String>,
    pub model: Option<String>,
    pub embedding: Option<String>,
    pub analysed_utc_ms: i64,
}

impl SharedSound {
    /// Sizes and shapes a real row has; anything else is refused whole.
    fn check(&self) -> Result<(), String> {
        let hex = |s: &Option<String>, max: usize| {
            s.as_ref().is_none_or(|v| {
                v.len() <= max
                    && v.len().is_multiple_of(2)
                    && v.bytes().all(|b| b.is_ascii_hexdigit())
            })
        };
        if self.song_key.is_empty() || self.song_key.len() > 128 {
            return Err("a song key is 1 to 128 characters".into());
        }
        // A row of a version no analysis has could never be replaced, and would stand
        // for every listener of the media server (security review, 27 November 2026).
        if self.version == 0 || self.version > MAX_VERSION {
            return Err(format!("`version` is 1 to {MAX_VERSION}"));
        }
        if self.state != "ok" && self.state != "unavailable" {
            return Err("`state` is ok or unavailable".into());
        }
        if self.state == "ok" && self.profile.is_none() {
            return Err("an ok row has a profile".into());
        }
        if !hex(&self.profile, 2 * 1024) || !hex(&self.embedding, 2 * 16 * 1024) {
            return Err("`profile` and `embedding` are hex, at most 1 KB and 16 KB".into());
        }
        if self.reason.as_ref().is_some_and(|r| r.len() > 300)
            || self.model.as_ref().is_some_and(|m| m.len() > 64)
        {
            return Err("`reason` is at most 300 characters and `model` 64".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct KeysBody {
    keys: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LookupReply {
    sounds: Vec<SharedSound>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClaimReply {
    granted: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PutBody {
    sounds: Vec<SharedSound>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PutReply {
    stored: u32,
}

fn keys_ok(keys: &[String], max: usize) -> Result<(), ApiError> {
    if keys.len() > max || keys.iter().any(|k| k.is_empty() || k.len() > 128) {
        return Err(ApiError::BadRequest(format!(
            "`keys` is at most {max} song keys of 1 to 128 characters"
        )));
    }
    Ok(())
}

fn server_of(state: &AppState, profile_id: i64) -> Result<String, ApiError> {
    state
        .db
        .media_server_of(profile_id)?
        .ok_or_else(|| ApiError::NotFound("The profile is gone.".into()))
}

/// `POST /sound/lookup`.
pub(crate) async fn lookup(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<KeysBody>,
) -> Result<Json<LookupReply>, ApiError> {
    keys_ok(&body.keys, MAX_LOOKUP)?;
    let server = server_of(&state, d.profile_id)?;
    let sounds = state
        .db
        .sounds(&server, &body.keys)?
        .into_iter()
        .filter_map(|j| serde_json::from_str(&j).ok())
        .collect();
    Ok(Json(LookupReply { sounds }))
}

/// `POST /sound/claim`.
pub(crate) async fn claim(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<KeysBody>,
) -> Result<Json<ClaimReply>, ApiError> {
    keys_ok(&body.keys, MAX_CLAIM)?;
    let server = server_of(&state, d.profile_id)?;
    let granted = state
        .db
        .claim_sounds(&server, &d.device_id, &body.keys, now_ms())?;
    Ok(Json(ClaimReply { granted }))
}

/// `PUT /sound`.
pub(crate) async fn put(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<PutBody>,
) -> Result<Json<PutReply>, ApiError> {
    if body.sounds.len() > MAX_PUT {
        return Err(ApiError::BadRequest(format!(
            "at most {MAX_PUT} rows at a time"
        )));
    }
    for s in &body.sounds {
        s.check().map_err(ApiError::BadRequest)?;
    }
    let server = server_of(&state, d.profile_id)?;
    let rows: Vec<(String, u32, String)> = body
        .sounds
        .iter()
        .map(|s| {
            serde_json::to_string(s)
                .map(|j| (s.song_key.clone(), s.version, j))
                .map_err(|e| ApiError::Internal(e.to_string()))
        })
        .collect::<Result<_, _>>()?;
    let stored = state.db.put_sounds(&server, &rows, now_ms())?;
    Ok(Json(PutReply { stored }))
}
