//! The routes end to end, in process: sign-in, refresh, devices, push, pull (with
//! the long-poll timed), delete, the status page and the rate limit.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;

/// Stand-in media server: tokens "good-sam" and "good-alex" belong to two users of
/// server "srv"; "down" can't be reached; anything else is refused.
#[derive(Debug)]
struct StubCheck;

impl MediaCheck for StubCheck {
    fn check<'a>(&'a self, server_id: &'a str, token: &'a str) -> CheckFuture<'a> {
        Box::pin(async move {
            if server_id != "srv" {
                return Err(CheckError::UnknownServer);
            }
            match token {
                "good-sam" => Ok(CheckedUser {
                    user_id: "u-sam".into(),
                    user_name: "Sam".into(),
                }),
                "good-alex" => Ok(CheckedUser {
                    user_id: "u-alex".into(),
                    user_name: "Alex".into(),
                }),
                "down" => Err(CheckError::Unreachable("timed out".into())),
                _ => Err(CheckError::Rejected),
            }
        })
    }
}

fn config() -> Config {
    Config {
        bind: bind_address(None).unwrap(),
        data_dir: PathBuf::from("unused"),
        jellyfin_urls: jellyfin_urls("http://jellyfin:8096").unwrap(),
        access_ttl: Duration::from_secs(3600),
        refresh_ttl: Duration::from_secs(86_400),
        max_wait: Duration::from_secs(5),
    }
}

fn server() -> (Router, Arc<AppState>) {
    let state = Arc::new(AppState::with_db(
        db::Db::open_in_memory(),
        config(),
        Arc::new(StubCheck),
    ));
    (router_from(&state, "192.168.1.20:5000"), state)
}

fn router_from(state: &Arc<AppState>, client: &str) -> Router {
    let addr: SocketAddr = client.parse().unwrap();
    app(state.clone()).layer(MockConnectInfo(addr))
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req
            .header("Content-Type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, v)
}

async fn sign_in(app: &Router, token: &str, device: &str) -> Value {
    let (s, v) = call(
        app,
        "POST",
        "/auth/exchange",
        None,
        Some(json!({
            "server_id": "srv", "media_token": token, "device_id": device,
            "device_name": device, "platform": "ios"
        })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

fn like(id: &str, song: &str) -> Value {
    json!({ "id": id, "kind": "like", "hlc": "0000000000001-0000", "version": 0,
            "payload": { "songKey": song }, "created_utc_ms": 1 })
}

#[tokio::test]
async fn health_reports_both_versions() {
    let (app, _) = server();
    let (status, json) = call(&app, "GET", "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["server_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["core_version"], encore_core::core_version());
    assert_eq!(
        call(&app, "GET", "/nope", None, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_media_server_token_is_exchanged_for_the_servers_own() {
    let (app, _) = server();
    let v = sign_in(&app, "good-sam", "phone").await;
    let access = v["access_token"].as_str().unwrap();
    assert!(access.starts_with("eat_"));
    assert!(v["refresh_token"].as_str().unwrap().starts_with("ert_"));
    assert_eq!(v["user_name"], "Sam");
    // The Jellyfin token is never handed back or kept.
    assert!(!v.to_string().contains("good-sam"));

    let body = |token: &str, server: &str| {
        Some(
            json!({ "server_id": server, "media_token": token, "device_id": "d",
                     "device_name": "d", "platform": "ios" }),
        )
    };
    let (s, e) = call(&app, "POST", "/auth/exchange", None, body("stolen", "srv")).await;
    assert_eq!(
        (s, e["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("unauthorized"))
    );
    let (s, _) = call(
        &app,
        "POST",
        "/auth/exchange",
        None,
        body("good-sam", "other"),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "an untrusted media server");
    let (s, _) = call(&app, "POST", "/auth/exchange", None, body("down", "srv")).await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
    let (s, _) = call(&app, "POST", "/auth/exchange", None, body("", "srv")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // Without a token, or with a made-up one, nothing answers.
    assert_eq!(
        call(&app, "GET", "/devices", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/devices", Some("eat_made_up"), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn refresh_rotates_and_a_reused_refresh_token_signs_the_device_out() {
    let (app, _) = server();
    let v = sign_in(&app, "good-sam", "phone").await;
    let first = v["refresh_token"].as_str().unwrap().to_owned();
    let refresh = |t: &str| Some(json!({ "refresh_token": t }));
    let (s, r) = call(&app, "POST", "/auth/refresh", None, refresh(&first)).await;
    assert_eq!(s, StatusCode::OK);
    let new_access = r["access_token"].as_str().unwrap().to_owned();
    assert_eq!(
        call(&app, "GET", "/devices", Some(&new_access), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/devices", v["access_token"].as_str(), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "the old access token stops"
    );
    // The new refresh token is used, so the first one coming back was copied.
    let (s, r2) = call(
        &app,
        "POST",
        "/auth/refresh",
        None,
        refresh(r["refresh_token"].as_str().unwrap()),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, e) = call(&app, "POST", "/auth/refresh", None, refresh(&first)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(e["reason"].as_str().unwrap().contains("somewhere else"));
    assert_eq!(
        call(&app, "GET", "/devices", r2["access_token"].as_str(), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "reuse signs the device out"
    );
}

#[tokio::test]
async fn a_like_on_the_phone_reaches_the_waiting_mac_within_a_second() {
    let (app, _) = server();
    let phone = sign_in(&app, "good-sam", "phone").await;
    let mac = sign_in(&app, "good-sam", "mac").await;
    let phone_t = phone["access_token"].as_str().unwrap().to_owned();
    let mac_t = mac["access_token"].as_str().unwrap().to_owned();

    // The Mac is waiting (long-poll) before the phone likes anything.
    let waiting = {
        let app = app.clone();
        let mac_t = mac_t.clone();
        tokio::spawn(async move {
            let t = Instant::now();
            let r = call(
                &app,
                "GET",
                "/sync/pull?cursor=0&wait=5",
                Some(&mac_t),
                None,
            )
            .await;
            (r, t.elapsed())
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    let pushed_at = Instant::now();
    let (s, r) = call(
        &app,
        "POST",
        "/sync/push",
        Some(&phone_t),
        Some(json!({ "events": [like("e1", "k1")] })),
    )
    .await;
    assert_eq!((s, r["accepted"][0].as_str()), (StatusCode::OK, Some("e1")));
    let ((s, got), _) = waiting.await.unwrap();
    let latency = pushed_at.elapsed();
    assert_eq!(s, StatusCode::OK);
    assert_eq!(got["events"][0]["id"], "e1");
    assert_eq!(got["events"][0]["device_id"], "phone");
    assert_eq!(got["events"][0]["payload"]["songKey"], "k1");
    assert!(latency < Duration::from_secs(1), "{latency:?}");
    println!(
        "like reached the waiting device in {:.1} ms",
        latency.as_secs_f64() * 1e3
    );

    // Nothing new: a short wait ends empty with the same cursor.
    let cursor = got["cursor"].as_i64().unwrap();
    let (_, empty) = call(
        &app,
        "GET",
        &format!("/sync/pull?cursor={cursor}&wait=1"),
        Some(&mac_t),
        None,
    )
    .await;
    assert_eq!(empty["events"], json!([]));
    assert_eq!(empty["cursor"].as_i64(), Some(cursor));
    // The phone doesn't get its own like back.
    let (_, own) = call(&app, "GET", "/sync/pull?cursor=0", Some(&phone_t), None).await;
    assert_eq!(own["events"], json!([]));
}

#[tokio::test]
async fn two_people_on_one_ipad_never_see_each_others_events() {
    let (app, _) = server();
    let sam = sign_in(&app, "good-sam", "ipad").await;
    let alex = sign_in(&app, "good-alex", "ipad").await;
    let sam_phone = sign_in(&app, "good-sam", "sam-phone").await;
    call(
        &app,
        "POST",
        "/sync/push",
        sam["access_token"].as_str(),
        Some(json!({ "events": [like("s1", "k")] })),
    )
    .await;
    let (_, a) = call(
        &app,
        "GET",
        "/sync/pull?cursor=0",
        alex["access_token"].as_str(),
        None,
    )
    .await;
    assert_eq!(a["events"], json!([]), "Alex sees nothing of Sam's");
    let (_, p) = call(
        &app,
        "GET",
        "/sync/pull?cursor=0",
        sam_phone["access_token"].as_str(),
        None,
    )
    .await;
    assert_eq!(p["events"][0]["id"], "s1", "Sam's own phone does");
    let (_, d) = call(&app, "GET", "/devices", alex["access_token"].as_str(), None).await;
    assert_eq!(d["devices"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_revoked_device_and_a_deleted_profile_stop_working() {
    let (app, state) = server();
    let phone = sign_in(&app, "good-sam", "phone").await;
    let mac = sign_in(&app, "good-sam", "mac").await;
    let mac_t = mac["access_token"].as_str().unwrap();
    let phone_t = phone["access_token"].as_str().unwrap();
    // From the Mac, sign the lost phone out.
    let (s, d) = call(&app, "GET", "/devices", Some(mac_t), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(d["this_device"], mac["device_id"]);
    let path = format!("/devices/{}", phone["device_id"].as_str().unwrap());
    assert_eq!(
        call(&app, "DELETE", &path, Some(mac_t), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/sync/pull", Some(phone_t), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (s, _) = call(
        &app,
        "POST",
        "/auth/refresh",
        None,
        Some(json!({ "refresh_token": phone["refresh_token"] })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(
        call(&app, "DELETE", "/devices/nope", Some(mac_t), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    call(
        &app,
        "POST",
        "/sync/push",
        Some(mac_t),
        Some(json!({ "events": [like("m1", "k")] })),
    )
    .await;
    assert_eq!(
        call(&app, "DELETE", "/profile", Some(mac_t), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/devices", Some(mac_t), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        state.db.status().unwrap(),
        Status {
            profiles: 0,
            devices: 0,
            events: 0
        }
    );
}

#[tokio::test]
async fn pushes_are_validated_and_bounded() {
    let (app, _) = server();
    let t = sign_in(&app, "good-sam", "phone").await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let too_many: Vec<Value> = (0..=MAX_PUSH)
        .map(|i| like(&format!("e{i}"), "k"))
        .collect();
    assert_eq!(
        call(
            &app,
            "POST",
            "/sync/push",
            Some(&t),
            Some(json!({ "events": too_many }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let bad = json!({ "id": "", "kind": "like", "hlc": "x", "payload": {}, "created_utc_ms": 1 });
    assert_eq!(
        call(
            &app,
            "POST",
            "/sync/push",
            Some(&t),
            Some(json!({ "events": [bad] }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let big = json!({ "id": "big", "kind": "like", "hlc": "x", "created_utc_ms": 1,
                      "payload": { "x": "y".repeat(70_000) } });
    assert_eq!(
        call(
            &app,
            "POST",
            "/sync/push",
            Some(&t),
            Some(json!({ "events": [big] }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn sign_in_is_rate_limited_per_address() {
    let (app, state) = server();
    for _ in 0..10 {
        sign_in(&app, "good-sam", "phone").await;
    }
    let body = json!({ "server_id": "srv", "media_token": "good-sam", "device_id": "phone",
                       "device_name": "p", "platform": "ios" });
    let resp = app
        .clone()
        .oneshot(
            Request::post("/auth/exchange")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(resp.headers()["retry-after"], "6");
    // Another address isn't held up.
    let other = router_from(&state, "192.168.1.21:5000");
    sign_in(&other, "good-sam", "phone").await;
}

#[tokio::test]
async fn the_status_page_answers_only_on_the_home_network() {
    let (app, state) = server();
    sign_in(&app, "good-sam", "phone").await;
    let (s, v) = call(&app, "GET", "/admin/status", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (v["profiles"].as_i64(), v["devices"].as_i64()),
        (Some(1), Some(1))
    );
    assert!(!v.to_string().contains("Sam"), "counts only");
    for local in [
        "127.0.0.1:1",
        "10.0.0.2:1",
        "100.64.1.1:1",
        "[::1]:1",
        "[fd00::1]:1",
        "[::ffff:192.168.0.9]:1",
    ] {
        assert_eq!(
            call(
                &router_from(&state, local),
                "GET",
                "/admin/status",
                None,
                None
            )
            .await
            .0,
            StatusCode::OK,
            "{local}"
        );
    }
    for public in ["8.8.8.8:1", "[2001:db8::1]:1"] {
        assert_eq!(
            call(
                &router_from(&state, public),
                "GET",
                "/admin/status",
                None,
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "{public}"
        );
    }
}

/// The real check against Jellyfin's replies (stand-ins until the recording).
#[tokio::test]
async fn the_jellyfin_check_asks_the_trusted_server_who_a_token_belongs_to() {
    use encore_sources::{FixtureExchange, FixtureTransport};
    let ex = |path: &str, status: u16, body: Value| FixtureExchange {
        origin: None,
        method: "GET".into(),
        path: path.into(),
        query: Default::default(),
        status,
        body: Some(body),
        body_text: None,
        body_hex: None,
        error: None,
        date: None,
    };
    let info = json!({ "ProductName": "Jellyfin Server", "Id": "srv-1", "Version": "10.10.7",
                       "ServerName": "Home" });
    let good = FixtureTransport::new(vec![
        ex("/System/Info/Public", 200, info.clone()),
        ex(
            "/Users/Me",
            200,
            json!({ "Id": "u1", "Name": "Sam", "ServerId": "srv-1" }),
        ),
    ]);
    let urls = jellyfin_urls("http://jellyfin:8096").unwrap();
    let check = JellyfinCheck::new(urls.clone(), Arc::new(good));
    assert_eq!(
        check.check("srv-1", "t").await,
        Ok(CheckedUser {
            user_id: "u1".into(),
            user_name: "Sam".into()
        })
    );
    assert_eq!(
        check.check("someone-else", "t").await,
        Err(CheckError::UnknownServer)
    );

    let refused = FixtureTransport::new(vec![
        ex("/System/Info/Public", 200, info),
        ex("/Users/Me", 401, json!({})),
    ]);
    let check = JellyfinCheck::new(urls, Arc::new(refused));
    assert_eq!(check.check("srv-1", "t").await, Err(CheckError::Rejected));
}

/// The real check on the owner's real replies (tools/fixtures/jellyfin/
/// recorded-phase4.json, read-only, 7 November 2026): the token names its user;
/// after sign-out the same token is refused.
#[tokio::test]
async fn the_jellyfin_check_on_the_recorded_server() {
    use encore_sources::FixtureTransport;
    const RECORDED: &str = include_str!("../../tools/fixtures/jellyfin/recorded-phase4.json");
    let exchanges: Value = serde_json::from_str(RECORDED).unwrap();
    let me = exchanges
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["path"] == "/Users/Me" && x["status"] == 200)
        .unwrap();
    let server_id = me["body"]["ServerId"].as_str().unwrap().to_owned();
    let user_id = me["body"]["Id"].as_str().unwrap().to_owned();
    // The recording names the server by its anonymized address.
    let urls = jellyfin_urls("http://jellyfin.local:8096").unwrap();
    let check = JellyfinCheck::new(
        urls,
        Arc::new(FixtureTransport::from_json(RECORDED).unwrap()),
    );
    let user = check.check(&server_id, "anonymized-token").await.unwrap();
    assert_eq!(user.user_id, user_id);
    assert_eq!(user.user_name, me["body"]["Name"].as_str().unwrap());
    // The recording's next /Users/Me is after sign-out: refused.
    assert_eq!(
        check.check(&server_id, "anonymized-token").await,
        Err(CheckError::Rejected)
    );
    // Another server's ID isn't this one.
    assert_eq!(
        check.check("someone-elses-server", "t").await,
        Err(CheckError::UnknownServer)
    );
}
