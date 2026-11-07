//! The live link (plan section 11; Phase 7): one WebSocket per device, `GET /ws`,
//! signed in like every route. It carries presence (what each device plays), handoff
//! and remote-control messages between the devices of one profile, and a nudge when
//! another device pushed events (so sync pulls at once; long-poll stays the fallback).
//! Design: docs/adr/core-0015-live-link.md.
//!
//! Text frames of JSON with a `type`:
//! - device to server: `presence {presence}`, `relay {to, msgId, body}`, `ping {t}`;
//! - server to device: `welcome {deviceId, serverMs, devices: [{deviceId, online,
//!   presence}]}`, `presence {deviceId, online, presence, serverMs}`, `relay {from,
//!   msgId, body, serverMs}`, `undeliverable {to, msgId}`, `pong {t, serverMs}`,
//!   `nudge`, `devices` (names changed: read /devices again), `error {reason}`.
//!
//! A device sees only its own profile's devices: relays to any other device are
//! refused as undeliverable, whatever the ID. The server keeps presence in memory
//! only and logs no message bodies.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::auth::Device;
use crate::{AppState, now_ms};

/// At most this many bytes per message (a handoff snapshot of a long queue fits).
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;
/// At most this many messages from one device in one second.
pub const MAX_PER_SECOND: u32 = 40;
/// The server pings every so often; a link silent for three of these is dropped.
pub const PING_EVERY: Duration = Duration::from_secs(15);
/// How often an open link re-checks that its device is still signed in.
const RECHECK_EVERY: Duration = Duration::from_secs(60);
/// Outgoing messages queued for one slow device before it is dropped.
const QUEUE: usize = 256;

#[derive(Debug)]
struct Conn {
    id: u64,
    tx: mpsc::Sender<Out>,
    presence: Option<Value>,
}

/// What the hub asks a connection's task to do.
#[derive(Debug)]
enum Out {
    Text(String),
    /// The device was signed out or the profile deleted: say so and close.
    Close(&'static str),
}

/// Open links by profile, then by the server's device ID.
#[derive(Debug, Default)]
pub(crate) struct Hub {
    profiles: Mutex<HashMap<i64, HashMap<String, Conn>>>,
    next: AtomicU64,
}

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, HashMap<String, Conn>>> {
        self.profiles.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn send_to(conn: &Conn, msg: &Value) {
        // A full queue means the device stopped reading: it reconnects later.
        let _ = conn.tx.try_send(Out::Text(msg.to_string()));
    }

    /// Everyone of the profile except `except`.
    fn broadcast(&self, profile_id: i64, except: Option<&str>, msg: &Value) {
        if let Some(devices) = self.lock().get(&profile_id) {
            for (id, c) in devices {
                if Some(id.as_str()) != except {
                    Hub::send_to(c, msg);
                }
            }
        }
    }

    /// Another device pushed sync events: pull now.
    pub(crate) fn nudge(&self, profile_id: i64, except: &str) {
        self.broadcast(profile_id, Some(except), &json!({ "type": "nudge" }));
    }

    /// A device was renamed or removed: read /devices again.
    pub(crate) fn devices_changed(&self, profile_id: i64) {
        self.broadcast(profile_id, None, &json!({ "type": "devices" }));
    }

    /// Close a device's link (signed out), or every link of a profile (deleted).
    pub(crate) fn kick(&self, profile_id: i64, device_id: Option<&str>) {
        let mut all = self.lock();
        if let Some(devices) = all.get_mut(&profile_id) {
            devices.retain(|id, c| {
                let hit = device_id.is_none_or(|d| d == id);
                if hit {
                    let _ = c.tx.try_send(Out::Close("signed out"));
                }
                !hit
            });
        }
        drop(all);
        if let Some(d) = device_id {
            self.broadcast(profile_id, None, &offline(d));
        }
    }

    /// Which of the profile's devices are online, with their latest presence.
    pub(crate) fn online(&self, profile_id: i64) -> HashMap<String, Option<Value>> {
        self.lock()
            .get(&profile_id)
            .map(|d| {
                d.iter()
                    .map(|(k, c)| (k.clone(), c.presence.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn join(&self, profile_id: i64, device_id: &str, tx: mpsc::Sender<Out>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let mut all = self.lock();
        let devices = all.entry(profile_id).or_default();
        // A device that reconnects replaces its older link.
        if let Some(old) = devices.insert(
            device_id.to_owned(),
            Conn {
                id,
                tx,
                presence: None,
            },
        ) {
            let _ = old
                .tx
                .try_send(Out::Close("replaced by a newer connection"));
        }
        id
    }

    fn leave(&self, profile_id: i64, device_id: &str, conn_id: u64) {
        let mut all = self.lock();
        let Some(devices) = all.get_mut(&profile_id) else {
            return;
        };
        if devices.get(device_id).is_some_and(|c| c.id == conn_id) {
            devices.remove(device_id);
            if devices.is_empty() {
                all.remove(&profile_id);
            }
            drop(all);
            self.broadcast(profile_id, None, &offline(device_id));
        }
    }

    fn set_presence(&self, profile_id: i64, device_id: &str, conn_id: u64, presence: Value) {
        let mut all = self.lock();
        let Some(c) = all
            .get_mut(&profile_id)
            .and_then(|d| d.get_mut(device_id))
            .filter(|c| c.id == conn_id)
        else {
            return;
        };
        c.presence = Some(presence.clone());
        drop(all);
        self.broadcast(
            profile_id,
            Some(device_id),
            &json!({ "type": "presence", "deviceId": device_id, "online": true,
                     "presence": presence, "serverMs": now_ms() }),
        );
    }

    /// Relay to one device of the same profile. False when it isn't online there.
    fn relay(&self, profile_id: i64, from: &str, to: &str, msg_id: &Value, body: Value) -> bool {
        let all = self.lock();
        match all.get(&profile_id).and_then(|d| d.get(to)) {
            Some(c) if to != from => {
                Hub::send_to(
                    c,
                    &json!({ "type": "relay", "from": from, "msgId": msg_id,
                             "body": body, "serverMs": now_ms() }),
                );
                true
            }
            _ => false,
        }
    }
}

fn offline(device_id: &str) -> Value {
    json!({ "type": "presence", "deviceId": device_id, "online": false,
            "presence": null, "serverMs": now_ms() })
}

/// `GET /ws`.
pub(crate) async fn ws(
    State(state): State<Arc<AppState>>,
    Device(d): Device,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| run(state, d.profile_id, d.device_id, socket))
}

async fn run(state: Arc<AppState>, profile_id: i64, me: String, mut socket: WebSocket) {
    let (tx, mut rx) = mpsc::channel::<Out>(QUEUE);
    let conn_id = state.live.join(profile_id, &me, tx);
    state.db.touch_device(&me, now_ms()).ok();
    let devices: Vec<Value> = state
        .live
        .online(profile_id)
        .into_iter()
        .filter(|(id, _)| *id != me)
        .map(|(id, p)| json!({ "deviceId": id, "online": true, "presence": p }))
        .collect();
    let welcome =
        json!({ "type": "welcome", "deviceId": me, "serverMs": now_ms(), "devices": devices });
    if socket
        .send(Message::Text(welcome.to_string().into()))
        .await
        .is_err()
    {
        state.live.leave(profile_id, &me, conn_id);
        return;
    }
    state.live.broadcast(
        profile_id,
        Some(&me),
        &json!({ "type": "presence", "deviceId": me, "online": true,
                 "presence": null, "serverMs": now_ms() }),
    );

    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    let mut recheck = tokio::time::interval(RECHECK_EVERY);
    recheck.tick().await;
    let mut heard = Instant::now();
    let (mut window, mut in_window) = (Instant::now(), 0u32);
    let close = |reason: &'static str| {
        Message::Close(Some(CloseFrame {
            code: 1008,
            reason: reason.into(),
        }))
    };
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(msg)) = incoming else { break };
                heard = Instant::now();
                if window.elapsed() >= Duration::from_secs(1) {
                    (window, in_window) = (Instant::now(), 0);
                }
                in_window += 1;
                if in_window > MAX_PER_SECOND {
                    let _ = socket.send(close("too many messages")).await;
                    break;
                }
                match msg {
                    Message::Text(text) => {
                        if let Some(reply) = handle(&state, profile_id, &me, conn_id, text.as_str())
                            && socket.send(Message::Text(reply.to_string().into())).await.is_err()
                        {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    // Pings are answered by the library; pongs only mark the link alive.
                    _ => {}
                }
            }
            out = rx.recv() => {
                match out {
                    Some(Out::Text(t)) => {
                        if socket.send(Message::Text(t.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Out::Close(reason)) => {
                        let _ = socket.send(close(reason)).await;
                        break;
                    }
                    None => break,
                }
            }
            _ = ping.tick() => {
                if heard.elapsed() > PING_EVERY * 3 {
                    break;
                }
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    break;
                }
            }
            _ = recheck.tick() => {
                let a = crate::db::Authed { profile_id, device_id: me.clone(), client_device_id: String::new() };
                if !state.db.authed_device_exists(&a).unwrap_or(false) {
                    let _ = socket.send(close("signed out")).await;
                    break;
                }
            }
        }
    }
    state.live.leave(profile_id, &me, conn_id);
    state.db.touch_device(&me, now_ms()).ok();
}

/// One message from a device; the reply to send it, if any.
fn handle(state: &AppState, profile_id: i64, me: &str, conn_id: u64, text: &str) -> Option<Value> {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return Some(json!({ "type": "error", "reason": "Not JSON." }));
    };
    match v["type"].as_str() {
        Some("ping") => Some(json!({ "type": "pong", "t": v["t"], "serverMs": now_ms() })),
        Some("presence") if v["presence"].is_object() => {
            state
                .live
                .set_presence(profile_id, me, conn_id, v["presence"].clone());
            None
        }
        Some("relay") => {
            let to = v["to"].as_str().unwrap_or("");
            let msg_id = v["msgId"].clone();
            if state
                .live
                .relay(profile_id, me, to, &msg_id, v["body"].clone())
            {
                None
            } else {
                Some(json!({ "type": "undeliverable", "to": to, "msgId": msg_id }))
            }
        }
        _ => Some(json!({ "type": "error", "reason": "Unknown message type." })),
    }
}
