//! Smart picks on the companion server (Phase 5; docs/adr/core-0013): the daily
//! mixes are built here, from the profile's synced event log and the song list a
//! device sends, with the same engine the devices run (encore-recs), so a device
//! without a server builds the same set itself.
//!
//! - `PUT /catalog`: the profile's song list (no audio, no token), replaced whole;
//!   `GET /catalog/hash` lets a device skip sending an unchanged one.
//! - `POST /recs/home`: today's mix set, built at the first request after 06:00 in
//!   the profile's time zone, after 25 new plays, or on Regenerate; never
//!   replacing the set a device says is playing.
//! - `POST /recs/model`: the profile's learned ranker (Phase 10, docs/adr/core-0020),
//!   trained here from every device's plays when it is due (`learn::due`, the same rule
//!   a device without a server follows), so phones don't spend the battery on it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use encore_recs as recs;
use serde::{Deserialize, Serialize};

use crate::auth::Device;
use crate::db::Event;
use crate::error::ApiError;
use crate::{AppState, now_ms};

/// The longest a build may take (smartPicks.budgets.mixBuildSeconds).
const BUILD_LIMIT: Duration = Duration::from_secs(20);
/// The longest a device waits for training; it goes on and is kept if it takes longer.
const TRAIN_LIMIT: Duration = Duration::from_secs(30);

/// The profile's stored ranker, if any.
fn trained_of(state: &AppState, profile_id: i64) -> Result<Option<recs::learn::Trained>, ApiError> {
    Ok(state
        .db
        .profile_value(profile_id, crate::db::RANKER_KEY)?
        .and_then(|j| serde_json::from_str(&j).ok()))
}

#[derive(Debug, Deserialize)]
pub(crate) struct CatalogBody {
    hash: String,
    songs: Vec<recs::Song>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CatalogHash {
    hash: Option<String>,
}

/// `PUT /catalog`.
pub(crate) async fn put_catalog(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<CatalogBody>,
) -> Result<Json<CatalogHash>, ApiError> {
    if body.hash.is_empty() || body.hash.len() > 128 {
        return Err(ApiError::BadRequest(
            "`hash` must be 1 to 128 characters".into(),
        ));
    }
    let songs =
        serde_json::to_string(&body.songs).map_err(|e| ApiError::Internal(e.to_string()))?;
    state
        .db
        .put_catalog(d.profile_id, &body.hash, &songs, now_ms())?;
    Ok(Json(CatalogHash {
        hash: Some(body.hash),
    }))
}

/// `GET /catalog/hash`.
pub(crate) async fn catalog_hash(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
) -> Result<Json<CatalogHash>, ApiError> {
    Ok(Json(CatalogHash {
        hash: state.db.catalog(d.profile_id)?.map(|(h, _)| h),
    }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct HomeBody {
    utc_offset_minutes: i32,
    hidden: Vec<String>,
    recent_titles: Vec<String>,
    regenerate: bool,
    /// The mix a device is playing now: its set isn't replaced.
    playing_mix_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HomeReply {
    set: recs::MixSet,
    built_now: bool,
}

/// What a like, unlike or undo of one says (the same rule as the core's).
fn like_effect(kind: &str, payload: &serde_json::Value) -> Option<(Vec<String>, bool)> {
    let liked = match (kind, payload["undoes"].as_str()) {
        ("like", _) => true,
        ("unlike", _) | ("undo", Some("like")) => false,
        ("undo", Some("unlike")) => true,
        _ => return None,
    };
    let keys = payload["songKeys"]
        .as_array()?
        .iter()
        .filter_map(|k| k.as_str().map(str::to_owned))
        .collect();
    Some((keys, liked))
}

/// The engine's history from the synced log: plays, likes (last writer wins by
/// clock and device; favourites from the catalog too), playlist adds, feedback,
/// Not interested (last writer wins per item), the learning epoch.
pub fn history_from(events: &[Event], catalog: &recs::Catalog) -> recs::History {
    let mut h = recs::History {
        epoch_start_utc_ms: i64::MIN,
        ..recs::History::default()
    };
    let mut likes: HashMap<String, (bool, String, String, i64)> = HashMap::new();
    for s in catalog.songs() {
        if s.server_favorite {
            likes.insert(s.key.clone(), (true, String::new(), String::new(), 0));
        }
    }
    let mut not_interested: HashMap<(String, String), (String, String, serde_json::Value, i64)> =
        HashMap::new();
    for e in events {
        let v = &e.payload;
        match e.kind.as_str() {
            "play" => {
                // A part handed off to another device: that device's play counts.
                let Some(key) = v["songKey"]
                    .as_str()
                    .filter(|_| v["endReason"] != "handedOff")
                else {
                    continue;
                };
                h.plays.push(recs::Play {
                    song_key: key.to_owned(),
                    started_utc_ms: v["startedUtcMs"].as_i64().unwrap_or(e.created_utc_ms),
                    utc_offset_minutes: v["utcOffsetMinutes"].as_i64().unwrap_or(0) as i32,
                    listened_ms: v["listenedMs"].as_u64().unwrap_or(0),
                    outcome: v["signal"].as_str().and_then(recs::Outcome::from_signal),
                    app_chosen: v["chosenBy"].as_str() == Some("app"),
                    strong_intent: matches!(
                        v["startedFrom"].as_str(),
                        Some("search" | "queue" | "history")
                    ),
                });
            }
            "playlistAdd" | "playlistCreate" => {
                for k in v["songKeys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str())
                {
                    h.added.push((k.to_owned(), e.created_utc_ms));
                }
            }
            "feedback" => {
                if let Some(k) = v["songKey"].as_str() {
                    h.feedback.push(recs::Feedback {
                        song_key: k.to_owned(),
                        more: v["more"].as_bool().unwrap_or(true),
                        at_utc_ms: e.created_utc_ms,
                        bucket: v["bucket"].as_u64().unwrap_or(0) as u8,
                    });
                }
            }
            "notInterested" => {
                let (Some(scope), Some(value)) = (v["scope"].as_str(), v["value"].as_str()) else {
                    continue;
                };
                let k = (scope.to_owned(), value.to_owned());
                let newer = not_interested.get(&k).is_none_or(|(hlc, dev, _, _)| {
                    (e.hlc.as_str(), e.device_id.as_str()) > (hlc.as_str(), dev.as_str())
                });
                if newer {
                    not_interested.insert(
                        k,
                        (
                            e.hlc.clone(),
                            e.device_id.clone(),
                            v.clone(),
                            e.created_utc_ms,
                        ),
                    );
                }
            }
            "resetSuggestions" => {
                if let Some(t) = v["epochUtcMs"].as_i64() {
                    h.epoch_start_utc_ms = h.epoch_start_utc_ms.max(t);
                }
            }
            kind => {
                if let Some((keys, liked)) = like_effect(kind, v) {
                    for k in keys {
                        let newer = likes.get(&k).is_none_or(|(_, hlc, dev, _)| {
                            (e.hlc.as_str(), e.device_id.as_str()) > (hlc.as_str(), dev.as_str())
                        });
                        if newer {
                            likes.insert(
                                k,
                                (liked, e.hlc.clone(), e.device_id.clone(), e.created_utc_ms),
                            );
                        }
                    }
                }
            }
        }
    }
    h.plays.sort_by(|a, b| {
        a.started_utc_ms
            .cmp(&b.started_utc_ms)
            .then(a.song_key.cmp(&b.song_key))
    });
    let mut liked: Vec<(String, i64)> = likes
        .into_iter()
        .filter(|(_, v)| v.0)
        .map(|(k, v)| (k, v.3))
        .collect();
    liked.sort();
    h.likes = liked;
    let mut ni: Vec<recs::NotInterested> = not_interested
        .into_iter()
        .filter(|(_, (_, _, v, _))| !v["removed"].as_bool().unwrap_or(false))
        .map(|((scope, value), (_, _, v, at))| recs::NotInterested {
            scope: if scope == "artist" {
                recs::Scope::Artist(value)
            } else {
                recs::Scope::Song(value)
            },
            reason: match v["reason"].as_str() {
                Some("heardTooMuch") => recs::NotInterestedReason::HeardTooMuch,
                Some("wrongMoment") => recs::NotInterestedReason::WrongMoment,
                _ => recs::NotInterestedReason::NotMyTaste,
            },
            at_utc_ms: at,
            bucket: v["bucket"].as_u64().unwrap_or(0) as u8,
        })
        .collect();
    ni.sort_by(|a, b| {
        a.at_utc_ms
            .cmp(&b.at_utc_ms)
            .then(format!("{:?}", a.scope).cmp(&format!("{:?}", b.scope)))
    });
    h.not_interested = ni;
    h
}

/// Whether yesterday's set is due for a new one (the same rules as a device's).
fn due(old: &recs::MixSet, model: &recs::Model, now: i64, offset: i32, body: &HomeBody) -> bool {
    let playing = body
        .playing_mix_id
        .as_ref()
        .is_some_and(|id| old.mixes.iter().any(|m| &m.id == id));
    if playing {
        return false;
    }
    let local = now + i64::from(offset) * 60_000;
    let today = local.div_euclid(recs::DAY_MS);
    let hour = local.rem_euclid(recs::DAY_MS) / 3_600_000;
    let p = recs::Params::default();
    body.regenerate
        || (old.day < today && (hour >= 6 || old.day < today - 1))
        || model.plays_counted >= old.plays_counted + 25
        || old.learning != (model.plays_counted < p.full_from_plays)
}

/// `POST /recs/home`.
pub(crate) async fn home(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<HomeBody>,
) -> Result<Json<HomeReply>, ApiError> {
    let Some((_, songs)) = state.db.catalog(d.profile_id)? else {
        return Err(ApiError::NotFound(
            "No song list yet: a device sends one after its next library sync.".into(),
        ));
    };
    // Read before the history, so a clear that lands meanwhile is seen at the end.
    let mark = state.db.cleared_mark(d.profile_id)?;
    let events = state.db.all_events(d.profile_id)?;
    let old: Option<recs::MixSet> = state
        .db
        .mix_set(d.profile_id)?
        .and_then(|j| serde_json::from_str(&j).ok());
    let trained = trained_of(&state, d.profile_id)?;
    let now = now_ms();
    let profile = d.profile_id;
    let Some(job) = state.start_job(profile, "mixes") else {
        // A build for this profile is still running: the last good set, if any.
        return match state
            .db
            .mix_set(profile)?
            .and_then(|j| serde_json::from_str(&j).ok())
        {
            Some(set) => Ok(Json(HomeReply {
                set,
                built_now: false,
            })),
            None => Err(ApiError::Unavailable(
                "The mixes are still being built. Try again in a moment.".into(),
            )),
        };
    };
    let built = tokio::time::timeout(
        BUILD_LIMIT,
        tokio::task::spawn_blocking(move || -> Result<(recs::MixSet, bool), String> {
            let _job = job;
            let songs: Vec<recs::Song> = serde_json::from_str(&songs).map_err(|e| e.to_string())?;
            let catalog = recs::Catalog::new(songs);
            let history = history_from(&events, &catalog);
            let p = recs::Params::default();
            let ranker = trained
                .as_ref()
                .and_then(|t| t.ranker_for(&history))
                .cloned()
                .map(Arc::new);
            let model = recs::Model::build(&history, &catalog, &p, now, body.utc_offset_minutes)
                .with_ranker(ranker);
            if let Some(old) = &old
                && !due(old, &model, now, body.utc_offset_minutes, &body)
            {
                return Ok((old.clone(), false));
            }
            let set = recs::build_mixes(
                &model,
                &catalog,
                &p,
                &recs::Previous {
                    set: old.as_ref(),
                    hidden: &body.hidden,
                    recent_titles: &body.recent_titles,
                    regenerate: body.regenerate,
                },
            );
            Ok((set, true))
        }),
    )
    .await;
    let (set, built_now) = match built {
        Ok(Ok(Ok(x))) => x,
        Ok(Ok(Err(e))) => return Err(ApiError::Internal(format!("mix build: {e}"))),
        Ok(Err(e)) => return Err(ApiError::Internal(format!("mix build: {e}"))),
        Err(_) => {
            // Too slow: the last good set, if there is one (Out of date on the device).
            match state
                .db
                .mix_set(profile)?
                .and_then(|j| serde_json::from_str(&j).ok())
            {
                Some(set) => {
                    return Ok(Json(HomeReply {
                        set,
                        built_now: false,
                    }));
                }
                None => {
                    return Err(ApiError::Unavailable(
                        "The mixes took too long to build.".into(),
                    ));
                }
            }
        }
    };
    if built_now {
        let json = serde_json::to_string(&set).map_err(|e| ApiError::Internal(e.to_string()))?;
        if !state.db.put_unless_cleared(profile, None, &json, &mark)? {
            return Err(ApiError::Unavailable(
                "The listening history was just cleared. Try again in a moment.".into(),
            ));
        }
    }
    Ok(Json(HomeReply { set, built_now }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ModelBody {
    utc_offset_minutes: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelReply {
    /// None until the profile has enough plays to train from.
    trained: Option<recs::learn::Trained>,
    trained_now: bool,
}

/// `POST /recs/model`: the profile's learned ranker, trained first when it is due. Training that
/// takes longer than [`TRAIN_LIMIT`] finishes in the background and the last one is sent.
pub(crate) async fn model(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<ModelBody>,
) -> Result<Json<ModelReply>, ApiError> {
    let Some((_, songs)) = state.db.catalog(d.profile_id)? else {
        return Err(ApiError::NotFound(
            "No song list yet: a device sends one after its next library sync.".into(),
        ));
    };
    let last = trained_of(&state, d.profile_id)?;
    let mark = state.db.cleared_mark(d.profile_id)?;
    let events = state.db.all_events(d.profile_id)?;
    let now = now_ms();
    let profile = d.profile_id;
    let Some(guard) = state.start_job(profile, "training") else {
        // Training for this profile is still running: the last one.
        return Ok(Json(ModelReply {
            trained: last,
            trained_now: false,
        }));
    };
    let (held, worker) = (last.clone(), state.clone());
    let job =
        tokio::task::spawn_blocking(move || -> Result<Option<recs::learn::Trained>, String> {
            let _job = guard;
            let songs: Vec<recs::Song> = serde_json::from_str(&songs).map_err(|e| e.to_string())?;
            let catalog = recs::Catalog::new(songs);
            let history = history_from(&events, &catalog);
            if !recs::learn::due(held.as_ref(), &history, now) {
                return Ok(None);
            }
            let p = recs::Params::default();
            let t = recs::learn::retrain(&history, &catalog, &p, now, body.utc_offset_minutes);
            let json = serde_json::to_string(&t).map_err(|e| e.to_string())?;
            // Kept only if the history wasn't cleared while it trained.
            let kept = worker
                .db
                .put_unless_cleared(profile, Some(crate::db::RANKER_KEY), &json, &mark)
                .map_err(|e| e.to_string())?;
            Ok(kept.then_some(t))
        });
    match tokio::time::timeout(TRAIN_LIMIT, job).await {
        Ok(Ok(Ok(Some(t)))) => Ok(Json(ModelReply {
            trained: Some(t),
            trained_now: true,
        })),
        // Not trained now: what is stored now (a clear meanwhile took the old one).
        Ok(Ok(Ok(None))) | Err(_) => Ok(Json(ModelReply {
            trained: trained_of(&state, profile)?,
            trained_now: false,
        })),
        Ok(Ok(Err(e))) => Err(ApiError::Internal(format!("training: {e}"))),
        Ok(Err(e)) => Err(ApiError::Internal(format!("training: {e}"))),
    }
}
