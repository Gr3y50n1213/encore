//! The companion server's HTTP routes.
//!
//! Phase 1 is a skeleton with one route. Auth, sync, the WebSocket, jobs and the
//! admin page arrive in their own phases (plan section 5; docs/ROADMAP.md).

use axum::{Json, Router, routing::get};
use serde::Serialize;
use std::net::SocketAddr;

/// Where the server listens unless `ENCORE_SERVER_BIND` says otherwise.
/// Loopback only by default: nothing is reachable from the network until the
/// owner chooses an address.
pub const DEFAULT_BIND: &str = "127.0.0.1:8097";

/// The environment variable that sets the listen address, e.g. `0.0.0.0:8097`.
pub const BIND_ENV: &str = "ENCORE_SERVER_BIND";

/// The reply to `GET /health`. It names no user, device or library.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Health {
    pub status: &'static str,
    pub server_version: &'static str,
    pub core_version: String,
}

/// All routes.
pub fn app() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        server_version: env!("CARGO_PKG_VERSION"),
        core_version: encore_core::core_version(),
    })
}

/// Reads the listen address from `value` (the environment variable), or the default.
pub fn bind_address(value: Option<&str>) -> Result<SocketAddr, String> {
    let text = value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_BIND);
    text.parse()
        .map_err(|_| format!("{BIND_ENV}={text:?} is not an address like 127.0.0.1:8097"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get_path(path: &str) -> (StatusCode, Vec<u8>) {
        let response = app()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, body)
    }

    #[tokio::test]
    async fn health_reports_both_versions() {
        let (status, body) = get_path("/health").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["server_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["core_version"], encore_core::core_version());
    }

    #[tokio::test]
    async fn unknown_paths_are_not_found() {
        let (status, _) = get_path("/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn binds_to_loopback_by_default() {
        let addr = bind_address(None).unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(bind_address(Some("  ")).unwrap(), addr);
    }

    #[test]
    fn bind_address_can_be_set() {
        assert_eq!(bind_address(Some("0.0.0.0:9000")).unwrap().port(), 9000);
    }

    #[test]
    fn a_bad_bind_address_is_an_error() {
        let err = bind_address(Some("not-an-address")).unwrap_err();
        assert!(err.contains(BIND_ENV));
    }
}
