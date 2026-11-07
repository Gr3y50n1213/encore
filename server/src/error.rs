//! Every error reply: a status and `{"error": kind, "reason": sentence}`. Reasons
//! never contain a token.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub(crate) enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    TooManyRequests {
        retry_after_s: u64,
    },
    /// The media server couldn't be reached to check a sign-in.
    Unavailable(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, reason) = match self {
            ApiError::BadRequest(r) => (StatusCode::BAD_REQUEST, "badRequest", r),
            ApiError::Unauthorized(r) => (StatusCode::UNAUTHORIZED, "unauthorized", r),
            ApiError::Forbidden(r) => (StatusCode::FORBIDDEN, "forbidden", r),
            ApiError::NotFound(r) => (StatusCode::NOT_FOUND, "notFound", r),
            ApiError::TooManyRequests { retry_after_s } => {
                let mut resp = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "error": "tooManyRequests",
                        "reason": format!("Too many sign-in attempts. Try again in {retry_after_s} s."),
                    })),
                )
                    .into_response();
                if let Ok(v) = HeaderValue::from_str(&retry_after_s.to_string()) {
                    resp.headers_mut().insert(header::RETRY_AFTER, v);
                }
                return resp;
            }
            ApiError::Unavailable(r) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable", r),
            ApiError::Internal(r) => {
                eprintln!("encore-server: internal error: {r}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "Something went wrong on the server; it was logged.".to_owned(),
                )
            }
        };
        (
            status,
            Json(serde_json::json!({ "error": kind, "reason": reason })),
        )
            .into_response()
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        // Queries are parameterised, so a database error never carries a token.
        ApiError::Internal(format!("database: {e}"))
    }
}
