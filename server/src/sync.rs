//! Push and pull of the event log (plan section 5). Pull can wait: with `wait`,
//! it is answered the moment another device of the profile pushes, so a like on
//! the phone reaches the Mac within a second without a WebSocket (that comes with
//! handoff in Phase 7).

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{RawQuery, State};
use serde::{Deserialize, Serialize};

use crate::auth::Device;
use crate::db::Event;
use crate::error::ApiError;
use crate::{AppState, now_ms};

/// At most this many events per push.
pub const MAX_PUSH: usize = 500;
/// At most this many events per pull.
pub const MAX_PULL: u32 = 1000;
/// An event's payload is small (a like, a play); this bounds a bad one.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
pub(crate) struct PushBody {
    events: Vec<Event>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PushReply {
    /// IDs the server now holds: mark these sent.
    accepted: Vec<String>,
}

fn valid(e: &Event) -> Result<(), ApiError> {
    let ok = !e.id.is_empty()
        && e.id.len() <= 64
        && !e.kind.is_empty()
        && e.kind.len() <= 64
        && e.hlc.len() <= 64
        && e.version >= 0
        && e.payload.to_string().len() <= MAX_PAYLOAD_BYTES;
    if ok {
        Ok(())
    } else {
        Err(ApiError::BadRequest(format!(
            "event {:?} is malformed or too large",
            e.id.chars().take(64).collect::<String>()
        )))
    }
}

/// `POST /sync/push`.
pub(crate) async fn push(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    Json(body): Json<PushBody>,
) -> Result<Json<PushReply>, ApiError> {
    if body.events.len() > MAX_PUSH {
        return Err(ApiError::BadRequest(format!(
            "at most {MAX_PUSH} events per push"
        )));
    }
    body.events.iter().try_for_each(valid)?;
    let (accepted, changed) = state.db.push(&d, &body.events, now_ms())?;
    if changed {
        state.wake(d.profile_id);
        state.live.nudge(d.profile_id, &d.device_id);
    }
    Ok(Json(PushReply { accepted }))
}

/// `cursor`, `wait` (seconds) and `limit`, read with the url crate's form decoder
/// (axum's own query extractor would add a library). Unknown keys are ignored.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct PullQuery {
    cursor: i64,
    wait: u64,
    limit: Option<u32>,
}

impl PullQuery {
    pub(crate) fn parse(raw: Option<&str>) -> Result<PullQuery, ApiError> {
        let mut q = PullQuery::default();
        for (k, v) in url::form_urlencoded::parse(raw.unwrap_or("").as_bytes()) {
            let bad = || ApiError::BadRequest(format!("`{k}` must be a whole number"));
            match k.as_ref() {
                "cursor" => q.cursor = v.parse().map_err(|_| bad())?,
                "wait" => q.wait = v.parse().map_err(|_| bad())?,
                "limit" => q.limit = Some(v.parse().map_err(|_| bad())?),
                _ => {}
            }
        }
        Ok(q)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PullReply {
    events: Vec<Event>,
    /// Pass this as `cursor` next time.
    cursor: i64,
    /// More are waiting: pull again at once.
    more: bool,
}

/// `GET /sync/pull?cursor=&wait=&limit=`.
pub(crate) async fn pull(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    RawQuery(raw): RawQuery,
) -> Result<Json<PullReply>, ApiError> {
    let q = PullQuery::parse(raw.as_deref())?;
    let limit = q.limit.unwrap_or(MAX_PULL).clamp(1, MAX_PULL);
    let wait = Duration::from_secs(q.wait).min(state.config.max_wait);
    // Listen before reading, so a push between the read and the wait isn't missed.
    let waiter = state.waiter(d.profile_id);
    let notified = waiter.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();
    let (mut events, mut cursor, mut more) = state.db.pull(&d, q.cursor, limit)?;
    if events.is_empty() && !more && !wait.is_zero() {
        let _ = tokio::time::timeout(wait, notified).await;
        (events, cursor, more) = state.db.pull(&d, cursor, limit)?;
    }
    // A revoked device (or a deleted profile) stops at the next request.
    if state.db.authed_device_exists(&d)? {
        Ok(Json(PullReply {
            events,
            cursor,
            more,
        }))
    } else {
        Err(ApiError::Unauthorized(
            "This device's sign-in has ended. Sign in again.".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pull_query_is_read_strictly() {
        assert_eq!(PullQuery::parse(None).unwrap(), PullQuery::default());
        let q = PullQuery::parse(Some("cursor=42&wait=25&limit=10&x=y")).unwrap();
        assert_eq!(
            q,
            PullQuery {
                cursor: 42,
                wait: 25,
                limit: Some(10)
            }
        );
        assert!(PullQuery::parse(Some("cursor=abc")).is_err());
        assert!(PullQuery::parse(Some("wait=-1")).is_err());
    }
}
