//! Handoff and remote control over the companion server's live link (plan section
//! 11; board 12; ADRs core-0015 and core-0016). One WebSocket per open profile with
//! a companion, beside the sync loop: presence (what each device plays), push,
//! pull, remote control. Without a companion, handoff is unavailable and the
//! output picker hides it.
//!
//! The rules without the network are in `crate::handoff`. Lock order: the live
//! state is a leaf lock: never take playback or the store while holding it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use super::sync_loop::{self, Session};
use super::{EncoreCore, Inner, Playback, Profile, lock, now_ms};
use crate::api::types::*;
use crate::handoff::{Banner, Clock, Presence, PresenceSong, Snapshot, WireEntry, start_position};
use crate::playlog::ListeningMode;
use crate::queue::{NewItem, QueueItem};
use encore_store::Store;

/// The sender waits this long for the target to start.
const CONFIRM_WITHIN: Duration = Duration::from_secs(5);
/// A pull waits this long for the playing device's offer.
const PULL_WITHIN: Duration = Duration::from_secs(6);
/// Remote control replies.
const REPLY_WITHIN: Duration = Duration::from_secs(3);
/// An offer not taken up by the platform in this time is dropped.
const INCOMING_FOR_MS: i64 = 15_000;
/// Offline devices leave the picker after 30 days (board 12, item 6).
const FORGET_AFTER_MS: i64 = 30 * 86_400_000;
const PING_EVERY: Duration = Duration::from_secs(10);
/// While playing, presence is sent again this often (position stays fresh).
const PRESENCE_EVERY: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct Known {
    name: String,
    platform: String,
    last_seen_utc_ms: i64,
}

#[derive(Debug)]
struct Incoming {
    transfer_id: String,
    from: String,
    snapshot: Snapshot,
    uid: u64,
    received_local_ms: i64,
    duration_ms: i64,
}

/// The live state of one open profile.
#[derive(Debug)]
pub(crate) struct Live {
    tx: Option<mpsc::Sender<String>>,
    connected: bool,
    reason: Option<String>,
    me: String,
    known: HashMap<String, Known>,
    online: HashMap<String, Option<Presence>>,
    clock: Clock,
    banner: Banner,
    playing: bool,
    position_ms: i64,
    position_at_local_ms: i64,
    volume: f64,
    outgoing: HashMap<String, oneshot::Sender<Result<(), String>>>,
    pulls: HashMap<String, oneshot::Sender<Result<(), String>>>,
    replies: HashMap<String, oneshot::Sender<Result<Value, String>>>,
    incoming: Option<Incoming>,
    controlling: Option<String>,
}

impl Default for Live {
    fn default() -> Live {
        Live {
            tx: None,
            connected: false,
            reason: None,
            me: String::new(),
            known: HashMap::new(),
            online: HashMap::new(),
            clock: Clock::default(),
            banner: Banner::default(),
            playing: false,
            position_ms: 0,
            position_at_local_ms: 0,
            volume: 1.0,
            outgoing: HashMap::new(),
            pulls: HashMap::new(),
            replies: HashMap::new(),
            incoming: None,
            controlling: None,
        }
    }
}

impl Live {
    /// A progress report. Presence goes out on its own timer, unless the position
    /// moved away from what the other devices estimate (more than 2 s): then true.
    pub(super) fn note_position(&mut self, position_ms: i64) -> bool {
        let now = now_ms();
        let drift = (self.position_now(now) - position_ms).abs() > 2_000;
        self.position_ms = position_ms.max(0);
        self.position_at_local_ms = now;
        drift && self.connected
    }

    /// Phase 9, WP2: whether the player plays, the position it last reported and
    /// when (UTC milliseconds; 0 before any report), for the widget snapshot.
    pub(super) fn play_state(&self) -> (bool, i64, i64) {
        (self.playing, self.position_ms, self.position_at_local_ms)
    }

    /// Phase 9, WP1: where the player is now (estimated since its last report) and
    /// whether it plays, for the sleep timer.
    pub(super) fn position_and_playing(&self, now: i64) -> (i64, bool) {
        (self.position_now(now), self.playing)
    }

    fn name_of(&self, id: &str) -> String {
        self.known
            .get(id)
            .map(|k| k.name.clone())
            .unwrap_or_else(|| "your other device".into())
    }

    fn position_now(&self, now: i64) -> i64 {
        if self.playing {
            self.position_ms + (now - self.position_at_local_ms).max(0)
        } else {
            self.position_ms
        }
    }

    fn send(&self, msg: Value) -> Result<(), String> {
        match (&self.tx, self.connected) {
            (Some(tx), true) => tx
                .try_send(msg.to_string())
                .map_err(|_| "The companion server can't be reached right now".into()),
            _ => Err(self
                .reason
                .clone()
                .unwrap_or_else(|| "The companion server can't be reached right now".into())),
        }
    }

    fn relay(&self, to: &str, msg_id: &str, body: Value) -> Result<(), String> {
        self.send(json!({ "type": "relay", "to": to, "msgId": msg_id, "body": body }))
    }
}

/// An offer placed in this device's queue: its entry, the queue, unknown songs,
/// the song's length and its uid.
type Placed = Option<(QueueEntry, QueueState, u32, i64, u64)>;

fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

fn guest(pb: &Playback) -> bool {
    pb.log.mode() == ListeningMode::Guest
}

fn wire_of(store: &Store, item: &QueueItem) -> Option<WireEntry> {
    let t = store.track(item.track_id).ok()?;
    Some(WireEntry {
        key: t.song_key,
        item: t.source_item_id,
        chosen_by: item.chosen_by.as_str().into(),
        started_from: item.started_from.as_str().into(),
        context_id: item.context_id.clone(),
    })
}

fn item_of(store: &Store, w: &WireEntry) -> Option<NewItem> {
    let track_id = store.track_ids_for_key(&w.key).ok()?.into_iter().next()?;
    Some(NewItem {
        track_id,
        chosen_by: serde_json::from_value(Value::String(w.chosen_by.clone()))
            .unwrap_or(crate::queue::ChosenBy::User),
        started_from: serde_json::from_value(Value::String(w.started_from.clone()))
            .unwrap_or(crate::queue::StartedFrom::Other),
        context_id: w.context_id.clone(),
    })
}

fn remote_song(store: &Store, s: &PresenceSong) -> RemoteSong {
    RemoteSong {
        title: s.title.clone(),
        artist: s.artist.clone(),
        album: s.album.clone(),
        duration_ms: s.duration_ms,
        song_id: store
            .track_ids_for_key(&s.key)
            .ok()
            .and_then(|v| v.into_iter().next()),
    }
}

fn presence_song(store: &Store, item: &QueueItem) -> Option<PresenceSong> {
    let t = store.track(item.track_id).ok()?;
    Some(PresenceSong {
        key: t.song_key,
        item: t.source_item_id,
        title: t.title,
        artist: t.artist,
        album: t.album_title,
        duration_ms: t.duration_ms as i64,
    })
}

impl Inner {
    fn live_session(&self, p: &Profile) -> Option<Arc<Session>> {
        lock(&p.companion).ok()?.clone()
    }

    /// This device's presence, from its queue and what the player reported.
    fn own_presence(&self, p: &Profile) -> Presence {
        let (song, private, guest) = match (lock(&p.playback), lock(&p.store)) {
            (Ok(pb), Ok(store)) => (
                pb.queue.current().and_then(|i| presence_song(&store, i)),
                pb.log.mode() == ListeningMode::Private,
                guest(&pb),
            ),
            _ => (None, false, false),
        };
        let Ok(live) = lock(&p.live) else {
            return Presence::default();
        };
        let now = now_ms();
        let state = match (&song, live.playing) {
            _ if guest => "idle",
            (None, _) => "idle",
            (Some(_), true) => "playing",
            (Some(_), false) => "paused",
        };
        Presence {
            state: state.into(),
            song: if private || guest { None } else { song },
            position_ms: live.position_now(now),
            at_server_ms: live.clock.to_server(now),
            volume: live.volume,
            private,
        }
    }

    /// Tell the profile's other devices what this one is doing now.
    pub(super) fn publish_presence(&self) {
        let Ok(p) = self.profile() else { return };
        let presence = self.own_presence(&p);
        if let Ok(live) = lock(&p.live) {
            let _ = live.send(json!({ "type": "presence", "presence": presence }));
        }
    }

    /// The player's state changed (started, progress, paused, seeked, ended).
    pub(super) fn note_playback(&self, playing: Option<bool>, position_ms: Option<i64>) {
        let Ok(p) = self.profile() else { return };
        if let Ok(mut live) = lock(&p.live) {
            let now = now_ms();
            let pos = position_ms.unwrap_or_else(|| live.position_now(now));
            live.position_ms = pos.max(0);
            live.position_at_local_ms = now;
            if let Some(on) = playing {
                live.playing = on;
            }
        }
        self.publish_presence();
        // Phase 9, WP2: the widgets' play state (never from progress ticks, which
        // go through `note_position`).
        self.widgets_soon();
    }

    fn output_list(&self, p: &Profile) -> OutputDevices {
        let guest_now = lock(&p.playback).map(|pb| guest(&pb)).unwrap_or(true);
        if guest_now {
            return OutputDevices {
                state: OutputState::NotAvailable,
                reason: Some("Handoff isn't available in Guest mode".into()),
                devices: Vec::new(),
            };
        }
        if self.live_session(p).is_none() {
            return OutputDevices {
                state: OutputState::NoCompanion,
                reason: None,
                devices: Vec::new(),
            };
        }
        let me_presence = self.own_presence(p);
        let Ok(store) = lock(&p.store) else {
            return OutputDevices {
                state: OutputState::NotAvailable,
                reason: None,
                devices: Vec::new(),
            };
        };
        let Ok(live) = lock(&p.live) else {
            return OutputDevices {
                state: OutputState::NotAvailable,
                reason: None,
                devices: Vec::new(),
            };
        };
        let now = now_ms();
        let server_now = live.clock.to_server(now);
        let row = |id: &str, presence: Option<&Presence>, online: bool, this: bool| {
            let k = live.known.get(id);
            let state = match presence.map(|p| p.state.as_str()) {
                _ if !online => DeviceState::Offline,
                Some("playing") => DeviceState::Playing,
                Some("paused") => DeviceState::Paused,
                _ => DeviceState::Idle,
            };
            OutputDevice {
                device_id: id.to_owned(),
                name: if this {
                    self.config.device_name.clone()
                } else {
                    k.map(|k| k.name.clone())
                        .unwrap_or_else(|| "Another device".into())
                },
                platform: if this {
                    self.config.platform.clone()
                } else {
                    k.map(|k| k.platform.clone()).unwrap_or_default()
                },
                this_device: this,
                state,
                song: presence
                    .filter(|_| online)
                    .and_then(|p| p.song.as_ref())
                    .map(|s| remote_song(&store, s)),
                position_ms: presence.map(|p| p.position_at(server_now)).unwrap_or(0),
                volume: presence.map(|p| p.volume).unwrap_or(1.0),
                private_listening: presence.is_some_and(|p| p.private),
                last_seen_utc_ms: if online || this {
                    now
                } else {
                    k.map(|k| k.last_seen_utc_ms).unwrap_or(0)
                },
            }
        };
        let mut devices = vec![row(&live.me, Some(&me_presence), true, true)];
        let mut others: Vec<OutputDevice> = live
            .known
            .iter()
            .filter(|(id, _)| **id != live.me)
            .filter(|(id, k)| {
                live.online.contains_key(*id) || now - k.last_seen_utc_ms < FORGET_AFTER_MS
            })
            .map(|(id, _)| {
                let online = live.online.contains_key(id);
                let presence = live.online.get(id).and_then(|p| p.as_ref());
                row(id, presence, online, false)
            })
            .collect();
        others.sort_by(|a, b| {
            (a.state == DeviceState::Offline, &a.name)
                .cmp(&(b.state == DeviceState::Offline, &b.name))
        });
        let any_others = !others.is_empty();
        devices.extend(others);
        let (state, reason) =
            if !live.connected {
                (
                    OutputState::Unreachable,
                    Some(live.reason.clone().unwrap_or_else(|| {
                        "The companion server can't be reached right now".into()
                    })),
                )
            } else if !any_others {
                (
                    OutputState::NoOtherDevices,
                    Some("Sign in on another device to hand off music.".into()),
                )
            } else {
                (OutputState::Ready, None)
            };
        OutputDevices {
            state,
            reason,
            devices,
        }
    }

    fn emit_devices(&self, p: &Profile) {
        let mut e = CoreEvent::of(EventKind::DevicesChanged);
        e.output_devices = Some(self.output_list(p));
        self.emit(e);
    }

    fn device_row(&self, p: &Profile, id: &str) -> Option<OutputDevice> {
        self.output_list(p)
            .devices
            .into_iter()
            .find(|d| d.device_id == id)
    }

    fn handoff_info(&self, transfer_id: &str, device_id: &str, name: String) -> HandoffInfo {
        HandoffInfo {
            transfer_id: transfer_id.to_owned(),
            device_id: device_id.to_owned(),
            device_name: name,
            entry: None,
            position_ms: 0,
            playing: false,
            volume: 1.0,
            unknown_songs: 0,
            reason: None,
        }
    }

    fn fail(&self, transfer_id: &str, device_id: &str, name: String, reason: String) {
        let mut info = self.handoff_info(transfer_id, device_id, name);
        info.reason = Some(reason);
        let mut e = CoreEvent::of(EventKind::HandoffFailed);
        e.handoff = Some(info);
        self.emit(e);
    }

    /// Package this device's queue and position (section 11, step 1).
    fn snapshot(&self, p: &Profile) -> Result<Snapshot, EncoreError> {
        let pb = lock(&p.playback)?;
        if guest(&pb) {
            return Err(EncoreError::NotAllowed {
                reason: "Handoff isn't available in Guest mode".into(),
            });
        }
        let store = lock(&p.store)?;
        let v = pb.queue.view();
        let current =
            v.current
                .and_then(|i| wire_of(&store, i))
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "Nothing is playing to hand off".into(),
                })?;
        let list = |items: &[QueueItem]| -> Vec<WireEntry> {
            items.iter().filter_map(|i| wire_of(&store, i)).collect()
        };
        let repeat = match v.repeat {
            crate::queue::RepeatMode::Off => "off",
            crate::queue::RepeatMode::All => "all",
            crate::queue::RepeatMode::One => "one",
        };
        let listened = pb.last_listened_ms;
        let radio = pb.radio_seed.clone();
        let snapshot_lists = (list(v.history), list(v.up_next), list(v.autoplay));
        let (shuffle, autoplay_enabled) = (v.shuffle, v.autoplay_enabled);
        drop(store);
        drop(pb);
        let live = lock(&p.live)?;
        let now = now_ms();
        Ok(Snapshot {
            history: snapshot_lists.0,
            current,
            up_next: snapshot_lists.1,
            autoplay: snapshot_lists.2,
            shuffle,
            repeat: repeat.into(),
            autoplay_enabled,
            radio_seed: radio,
            position_ms: live.position_now(now),
            at_server_ms: live.clock.to_server(now),
            playing: live.playing,
            volume: live.volume,
            listened_ms: listened,
        })
    }

    /// Push this device's playback to `to` (or answer a pull with `transfer_id`).
    async fn push_to(self: &Arc<Self>, to: String, transfer_id: String) -> Result<(), EncoreError> {
        let p = self.profile()?;
        let name = lock(&p.live)?.name_of(&to);
        let snapshot = self.snapshot(&p)?;
        let (tx, rx) = oneshot::channel();
        {
            let mut live = lock(&p.live)?;
            if !live.online.contains_key(&to) {
                return Err(EncoreError::Unreachable {
                    reason: format!("{name} is offline"),
                });
            }
            live.outgoing.insert(transfer_id.clone(), tx);
            let sent = live.relay(
                &to,
                &transfer_id,
                json!({ "kind": "offer", "transferId": transfer_id, "snapshot": snapshot }),
            );
            if let Err(reason) = sent {
                live.outgoing.remove(&transfer_id);
                return Err(EncoreError::Unreachable { reason });
            }
        }
        let outcome = match tokio::time::timeout(CONFIRM_WITHIN, rx).await {
            Ok(Ok(r)) => r,
            _ => Err(format!("{name} didn't start in time")),
        };
        if let Ok(mut live) = lock(&p.live) {
            live.outgoing.remove(&transfer_id);
        }
        match outcome {
            Ok(()) => {
                // Only now does this device stop (section 11, step 4).
                {
                    let mut pb = lock(&p.playback)?;
                    let store = lock(&p.store)?;
                    let listened = pb.last_listened_ms;
                    pb.log.finish(
                        &store,
                        listened,
                        crate::playlog::EndReason::HandedOff,
                        now_ms(),
                    )?;
                    pb.playing_uid = None;
                    pb.last_listened_ms = 0;
                }
                if let Ok(mut live) = lock(&p.live) {
                    live.playing = false;
                }
                self.publish_presence();
                // Phase 9, WP2: the widgets stop showing it as playing here.
                self.widgets_soon();
                // Phase 9, WP1: the sleep timer goes with the music (board 31): it
                // ends here, before the app is told to fade out.
                let _ = self.sleep_clear();
                let mut e = CoreEvent::of(EventKind::HandoffConfirmed);
                e.handoff = Some(self.handoff_info(&transfer_id, &to, name));
                self.emit(e);
                Ok(())
            }
            Err(reason) => {
                self.fail(&transfer_id, &to, name, reason.clone());
                Err(EncoreError::Unreachable { reason })
            }
        }
    }

    /// Another device offers its playback (section 11, steps 2 and 3).
    fn on_offer(&self, p: &Profile, from: &str, transfer_id: &str, snapshot: Snapshot) {
        let decline = |reason: &str| {
            if let Ok(live) = lock(&p.live) {
                let _ = live.relay(
                    from,
                    transfer_id,
                    json!({ "kind": "decline", "transferId": transfer_id, "reason": reason }),
                );
            }
        };
        let placed = (|| -> Result<Placed, EncoreError> {
            let mut pb = lock(&p.playback)?;
            if guest(&pb) {
                return Ok(None);
            }
            let store = lock(&p.store)?;
            let map = |list: &[WireEntry]| -> Vec<NewItem> {
                list.iter().filter_map(|w| item_of(&store, w)).collect()
            };
            let Some(current) = item_of(&store, &snapshot.current) else {
                return Ok(None);
            };
            let duration = store.track(current.track_id)?.duration_ms as i64;
            let (history, up_next, autoplay) = (
                map(&snapshot.history),
                map(&snapshot.up_next),
                map(&snapshot.autoplay),
            );
            let unknown =
                (snapshot.len() - 1 - history.len() - up_next.len() - autoplay.len()) as u32;
            // Whatever played here stops: the music moves here instead.
            let listened = pb.last_listened_ms;
            pb.log.finish(
                &store,
                listened,
                crate::playlog::EndReason::EndedBySystem,
                now_ms(),
            )?;
            pb.playing_uid = None;
            pb.last_listened_ms = 0;
            let repeat = match snapshot.repeat.as_str() {
                "all" => crate::queue::RepeatMode::All,
                "one" => crate::queue::RepeatMode::One,
                _ => crate::queue::RepeatMode::Off,
            };
            pb.queue.restore(
                history,
                current,
                up_next,
                autoplay,
                snapshot.shuffle,
                repeat,
                snapshot.autoplay_enabled,
            );
            pb.radio_seed = snapshot.radio_seed.clone();
            let mut probe = pb.queue.clone();
            let can_undo = probe.undo();
            let state = Inner::queue_state(&store, &pb, can_undo);
            let entry = state.current.clone().ok_or_else(|| EncoreError::NotFound {
                reason: "the song is missing".into(),
            })?;
            let uid = entry.uid as u64;
            Ok(Some((entry, state, unknown, duration, uid)))
        })();
        let (entry, state, unknown, duration, uid) = match placed {
            Ok(Some(x)) => x,
            Ok(None) => {
                let reason = if lock(&p.playback).map(|pb| guest(&pb)).unwrap_or(false) {
                    "That device is in Guest mode"
                } else {
                    "That song isn't in this device's library yet. Sync its library and try again."
                };
                return decline(reason);
            }
            Err(e) => return decline(&e.to_string()),
        };
        let (name, position) = {
            let Ok(mut live) = lock(&p.live) else { return };
            let server_now = live.clock.to_server(now_ms());
            let position = start_position(&snapshot, server_now, duration);
            live.position_ms = position;
            live.position_at_local_ms = now_ms();
            live.playing = false;
            live.volume = snapshot.volume;
            live.incoming = Some(Incoming {
                transfer_id: transfer_id.to_owned(),
                from: from.to_owned(),
                uid,
                received_local_ms: now_ms(),
                duration_ms: duration,
                snapshot: snapshot.clone(),
            });
            (live.name_of(from), position)
        };
        let mut e = CoreEvent::of(EventKind::QueueChanged);
        e.queue = Some(state.clone());
        self.emit(e);
        let mut e = CoreEvent::of(EventKind::NowPlaying);
        e.now_playing = state.current.clone();
        self.emit(e);
        // Phase 9, WP1: a song or album timer here follows the queue that arrived.
        self.sleep_queue_changed();
        let mut info = self.handoff_info(transfer_id, from, name);
        info.entry = Some(entry);
        info.position_ms = position;
        info.playing = snapshot.playing;
        info.volume = snapshot.volume;
        info.unknown_songs = unknown;
        let mut e = CoreEvent::of(EventKind::HandoffIncoming);
        e.handoff = Some(info);
        self.emit(e);
    }

    fn on_pull_request(self: &Arc<Self>, p: &Profile, from: &str, transfer_id: &str) {
        let playing = lock(&p.live).map(|l| l.playing).unwrap_or(false)
            && lock(&p.playback)
                .map(|pb| pb.queue.current().is_some() && !guest(&pb))
                .unwrap_or(false);
        if !playing {
            if let Ok(live) = lock(&p.live) {
                let _ = live.relay(
                    from,
                    transfer_id,
                    json!({ "kind": "decline", "transferId": transfer_id,
                            "reason": format!("Nothing is playing on {}", self.config.device_name) }),
                );
            }
            return;
        }
        let me = self.clone();
        let (to, id) = (from.to_owned(), transfer_id.to_owned());
        tokio::spawn(async move {
            let _ = me.push_to(to, id).await;
        });
    }

    /// Board 12, remote control: do what the controller asked.
    fn on_remote(&self, p: &Profile, from: &str, msg_id: &str, cmd: &Value) {
        let keys = |v: &Value| -> Vec<String> {
            v["keys"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|k| k.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let ids_for = |keys: &[String]| -> Vec<i64> {
            lock(&p.store)
                .map(|st| {
                    keys.iter()
                        .filter_map(|k| st.track_ids_for_key(k).ok()?.into_iter().next())
                        .collect()
                })
                .unwrap_or_default()
        };
        let uid = cmd["uid"].as_i64().unwrap_or(-1);
        let result: Result<(), String> = match cmd["kind"].as_str().unwrap_or("") {
            kind @ ("play" | "pause" | "next" | "previous" | "seekTo" | "setVolume") => {
                let mut c = RemoteCommand::of(match kind {
                    "play" => RemoteCommandKind::Play,
                    "pause" => RemoteCommandKind::Pause,
                    "next" => RemoteCommandKind::Next,
                    "previous" => RemoteCommandKind::Previous,
                    "seekTo" => RemoteCommandKind::SeekTo,
                    _ => RemoteCommandKind::SetVolume,
                });
                if kind == "seekTo" {
                    c.position_ms = Some(cmd["positionMs"].as_i64().unwrap_or(0));
                }
                if kind == "setVolume" {
                    c.level = Some(cmd["level"].as_f64().unwrap_or(1.0).clamp(0.0, 1.0));
                }
                let mut e = CoreEvent::of(EventKind::RemoteCommand);
                e.remote_command = Some(c);
                self.emit(e);
                Ok(())
            }
            "playNext" | "addToUpNext" => {
                let ids = ids_for(&keys(cmd));
                let next = cmd["kind"] == "playNext";
                let items = Inner::new_items(&ids, StartedFrom::Queue, None);
                self.with_queue(|pb, _| {
                    if next {
                        pb.queue.play_next(items)
                    } else {
                        pb.queue.add_to_up_next(items)
                    }
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
            }
            "remove" => self
                .with_queue(|pb, _| pb.queue.remove(uid as u64))
                .map(|_| ())
                .map_err(|e| e.to_string()),
            "move" => {
                let to = cmd["toIndex"].as_u64().unwrap_or(0) as usize;
                self.with_queue(|pb, _| pb.queue.move_item(uid as u64, to))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            "jumpTo" => self
                .with_queue(|pb, _| pb.queue.jump_to(uid as u64))
                .map(|_| ())
                .map_err(|e| e.to_string()),
            _ => Err("This device doesn't know that command".into()),
        };
        if let Ok(live) = lock(&p.live) {
            let _ = live.relay(
                from,
                msg_id,
                json!({ "kind": "remoteDone", "msgId": msg_id, "ok": result.is_ok(),
                        "reason": result.err() }),
            );
        }
        self.publish_presence();
    }

    fn on_queue_request(&self, p: &Profile, from: &str, msg_id: &str) {
        let body = (|| -> Option<Value> {
            let pb = lock(&p.playback).ok()?;
            let store = lock(&p.store).ok()?;
            let private = pb.log.mode() == ListeningMode::Private || guest(&pb);
            let v = pb.queue.view();
            let one = |i: &QueueItem| -> Option<Value> {
                let s = presence_song(&store, i)?;
                Some(json!({ "uid": i.uid, "song": s, "chosenBy": i.chosen_by.as_str() }))
            };
            let list = |items: &[QueueItem]| -> Vec<Value> {
                if private {
                    Vec::new()
                } else {
                    items.iter().filter_map(one).collect()
                }
            };
            Some(json!({
                "kind": "queue", "msgId": msg_id,
                "current": if private { None } else { v.current.and_then(one) },
                "upNext": list(v.up_next), "autoplay": list(v.autoplay),
            }))
        })();
        if let (Some(body), Ok(live)) = (body, lock(&p.live)) {
            let _ = live.relay(from, msg_id, body);
        }
    }

    /// One message from the server.
    fn on_message(self: &Arc<Self>, p: &Arc<Profile>, s: &Arc<Session>, v: Value) {
        let now = now_ms();
        match v["type"].as_str().unwrap_or("") {
            "welcome" => {
                let playing_others: Vec<String> = {
                    let Ok(mut live) = lock(&p.live) else { return };
                    live.me = v["deviceId"].as_str().unwrap_or("").to_owned();
                    live.online.clear();
                    for d in v["devices"].as_array().into_iter().flatten() {
                        if let Some(id) = d["deviceId"].as_str() {
                            let presence = serde_json::from_value(d["presence"].clone()).ok();
                            live.online.insert(id.to_owned(), presence);
                        }
                    }
                    live.online
                        .iter()
                        .filter(|(_, pr)| pr.as_ref().is_some_and(|p| p.state == "playing"))
                        .map(|(id, _)| id.clone())
                        .collect()
                };
                self.refresh_names(p.clone(), s.clone());
                self.emit_devices(p);
                for id in playing_others {
                    self.maybe_offer_continue(p, &id);
                }
            }
            "presence" => {
                let Some(id) = v["deviceId"].as_str() else {
                    return;
                };
                let online = v["online"].as_bool().unwrap_or(false);
                let controlled = {
                    let Ok(mut live) = lock(&p.live) else { return };
                    if online {
                        let presence: Option<Presence> =
                            serde_json::from_value(v["presence"].clone()).ok();
                        live.online.insert(id.to_owned(), presence);
                    } else {
                        live.online.remove(id);
                        if let Some(k) = live.known.get_mut(id) {
                            k.last_seen_utc_ms = now;
                        }
                    }
                    if !live.known.contains_key(id) {
                        drop(live);
                        self.refresh_names(p.clone(), s.clone());
                        false
                    } else {
                        live.controlling.as_deref() == Some(id)
                    }
                };
                self.emit_devices(p);
                if controlled && let Some(d) = self.device_row(p, id) {
                    let mut e = CoreEvent::of(EventKind::RemoteState);
                    e.device = Some(d);
                    self.emit(e);
                }
                self.maybe_offer_continue(p, id);
            }
            "pong" => {
                if let (Some(t), Some(server)) = (v["t"].as_i64(), v["serverMs"].as_i64())
                    && let Ok(mut live) = lock(&p.live)
                {
                    live.clock.observe(t, now, server);
                }
            }
            "devices" => self.refresh_names(p.clone(), s.clone()),
            "undeliverable" => {
                let id = v["msgId"].as_str().unwrap_or("").to_owned();
                let Ok(mut live) = lock(&p.live) else { return };
                let reason = format!(
                    "{} isn't reachable right now",
                    live.name_of(v["to"].as_str().unwrap_or(""))
                );
                if let Some(tx) = live.outgoing.remove(&id) {
                    let _ = tx.send(Err(reason.clone()));
                }
                if let Some(tx) = live.pulls.remove(&id) {
                    let _ = tx.send(Err(reason.clone()));
                }
                if let Some(tx) = live.replies.remove(&id) {
                    let _ = tx.send(Err(reason));
                }
            }
            "relay" => {
                let from = v["from"].as_str().unwrap_or("").to_owned();
                let body = &v["body"];
                let tid = body["transferId"].as_str().unwrap_or("").to_owned();
                let msg_id = v["msgId"].as_str().unwrap_or("").to_owned();
                match body["kind"].as_str().unwrap_or("") {
                    "offer" => {
                        let pulled = lock(&p.live).ok().and_then(|mut l| l.pulls.remove(&tid));
                        match serde_json::from_value::<Snapshot>(body["snapshot"].clone()) {
                            Ok(snap) => {
                                self.on_offer(p, &from, &tid, snap);
                                if let Some(tx) = pulled {
                                    let _ = tx.send(Ok(()));
                                }
                            }
                            Err(_) => {
                                if let Some(tx) = pulled {
                                    let _ = tx.send(Err(
                                        "The other device sent a handoff this one can't read"
                                            .into(),
                                    ));
                                }
                            }
                        }
                    }
                    "accept" => {
                        if let Ok(mut live) = lock(&p.live)
                            && let Some(tx) = live.outgoing.remove(&tid)
                        {
                            let _ = tx.send(Ok(()));
                        }
                    }
                    "decline" => {
                        let reason = body["reason"]
                            .as_str()
                            .unwrap_or("The other device couldn't start")
                            .to_owned();
                        if let Ok(mut live) = lock(&p.live) {
                            if let Some(tx) = live.outgoing.remove(&tid) {
                                let _ = tx.send(Err(reason.clone()));
                            }
                            if let Some(tx) = live.pulls.remove(&tid) {
                                let _ = tx.send(Err(reason));
                            }
                        }
                    }
                    "pullRequest" => self.on_pull_request(p, &from, &tid),
                    "remote" => self.on_remote(p, &from, &msg_id, &body["cmd"]),
                    "queueRequest" => self.on_queue_request(p, &from, &msg_id),
                    "remoteDone" | "queue" => {
                        let id = body["msgId"].as_str().unwrap_or(&msg_id).to_owned();
                        if let Ok(mut live) = lock(&p.live)
                            && let Some(tx) = live.replies.remove(&id)
                        {
                            let _ = tx.send(Ok(body.clone()));
                        }
                    }
                    _ => {}
                }
            }
            // "nudge": the long-poll pull already answers at once; "error": nothing to do.
            _ => {}
        }
    }

    fn maybe_offer_continue(&self, p: &Profile, id: &str) {
        let offer = {
            let Ok(mut live) = lock(&p.live) else { return };
            let it_plays = live
                .online
                .get(id)
                .and_then(|p| p.as_ref())
                .is_some_and(|p| p.state == "playing");
            let this_plays = live.playing;
            id != live.me && live.banner.offer(id, it_plays, this_plays)
        };
        let guest_now = lock(&p.playback).map(|pb| guest(&pb)).unwrap_or(true);
        if offer
            && !guest_now
            && let Some(d) = self.device_row(p, id)
        {
            let mut e = CoreEvent::of(EventKind::ContinueAvailable);
            e.device = Some(d);
            self.emit(e);
        }
    }

    /// Names, platforms and last-seen times come from `GET /devices`.
    fn refresh_names(self: &Arc<Self>, p: Arc<Profile>, s: Arc<Session>) {
        let inner = self.clone();
        tokio::spawn(async move {
            let got = sync_loop::signed(&inner, &s, |a| {
                let s = s.clone();
                async move { s.client.devices(&a).await }
            })
            .await;
            if let Ok((_, list)) = got {
                if let Ok(mut live) = lock(&p.live) {
                    live.known = list
                        .into_iter()
                        .filter(|d| !d.revoked)
                        .map(|d| {
                            (
                                d.id,
                                Known {
                                    name: d.name,
                                    platform: d.platform,
                                    last_seen_utc_ms: d.last_seen_utc_ms,
                                },
                            )
                        })
                        .collect();
                }
                inner.emit_devices(&p);
            }
        });
    }

    async fn ask(
        &self,
        p: &Profile,
        to: &str,
        body: impl FnOnce(&str) -> Value,
    ) -> Result<Value, EncoreError> {
        let msg_id = new_id();
        let (tx, rx) = oneshot::channel();
        {
            let mut live = lock(&p.live)?;
            live.replies.insert(msg_id.clone(), tx);
            if let Err(reason) = live.relay(to, &msg_id, body(&msg_id)) {
                live.replies.remove(&msg_id);
                return Err(EncoreError::Unreachable { reason });
            }
        }
        let got = tokio::time::timeout(REPLY_WITHIN, rx).await;
        if let Ok(mut live) = lock(&p.live) {
            live.replies.remove(&msg_id);
        }
        match got {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(reason))) => Err(EncoreError::Unreachable { reason }),
            _ => Err(EncoreError::Unreachable {
                reason: "The other device didn't answer".into(),
            }),
        }
    }
}

/// The live link's loop, one per open profile with a companion (beside the pusher
/// and the puller). Reconnects with backoff; stops with the session.
pub(super) async fn run(inner: Arc<Inner>, p: Arc<Profile>, s: Arc<Session>) {
    let mut backoff = 1;
    loop {
        if s.is_stopped() {
            return;
        }
        let token = sync_loop::access(&inner, &s, false).await;
        let link = match token {
            Ok(a) => {
                let url = s.client.base().join("ws").ok();
                match url {
                    Some(url) => encore_sources::ws::connect(
                        &url,
                        &[("Authorization".into(), format!("Bearer {a}"))],
                        None,
                        Duration::from_secs(10),
                    )
                    .await
                    .map_err(|e| e.to_string()),
                    None => Err("the companion's address can't take a live link".into()),
                }
            }
            Err(e) => Err(e.reason().to_owned()),
        };
        match link {
            Ok(link) => {
                backoff = 1;
                serve(&inner, &p, &s, link).await;
            }
            Err(reason) => {
                if let Ok(mut live) = lock(&p.live) {
                    live.reason = Some(reason);
                }
                // A token the server refused: the next round refreshes it.
                let _ = sync_loop::access(&inner, &s, true).await;
            }
        }
        if let Ok(mut live) = lock(&p.live) {
            live.connected = false;
            live.tx = None;
            live.online.clear();
            for (_, tx) in live.outgoing.drain() {
                let _ = tx.send(Err("The connection to the companion server dropped".into()));
            }
        }
        inner.emit_devices(&p);
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(60);
    }
}

async fn serve(
    inner: &Arc<Inner>,
    p: &Arc<Profile>,
    s: &Arc<Session>,
    mut link: encore_sources::ws::Link,
) {
    if let Ok(mut live) = lock(&p.live) {
        live.tx = Some(link.tx.clone());
        live.connected = true;
        live.reason = None;
        live.me = s.server_device_id.clone();
    }
    let ping = |tx: &mpsc::Sender<String>| {
        let _ = tx.try_send(json!({ "type": "ping", "t": now_ms() }).to_string());
    };
    ping(&link.tx);
    inner.publish_presence();
    let mut pings = tokio::time::interval(PING_EVERY);
    pings.tick().await;
    let mut presence = tokio::time::interval(PRESENCE_EVERY);
    presence.tick().await;
    loop {
        tokio::select! {
            msg = link.rx.recv() => {
                let Some(text) = msg else { break };
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    inner.on_message(p, s, v);
                }
            }
            _ = pings.tick() => ping(&link.tx),
            _ = presence.tick() => {
                if lock(&p.live).map(|l| l.playing).unwrap_or(false) {
                    inner.publish_presence();
                }
                // An offer the platform never took up is dropped.
                if let Ok(mut live) = lock(&p.live)
                    && live.incoming.as_ref().is_some_and(|i| now_ms() - i.received_local_ms > INCOMING_FOR_MS)
                {
                    live.incoming = None;
                }
            }
        }
    }
    if let Ok(mut live) = lock(&p.live) {
        live.reason = link
            .closed_because
            .lock()
            .ok()
            .and_then(|r| r.clone())
            .map(|r| format!("The companion server closed the connection ({r})"));
    }
}

fn command_wire(p: &Profile, cmd: &RemoteCommand) -> Result<Value, EncoreError> {
    let keys = |ids: &[i64]| -> Result<Vec<String>, EncoreError> {
        let store = lock(&p.store)?;
        Ok(ids
            .iter()
            .filter_map(|id| store.track(*id).ok().map(|t| t.song_key))
            .collect())
    };
    use RemoteCommandKind as K;
    let need = |what: &str| EncoreError::NotAllowed {
        reason: format!("This command needs {what}"),
    };
    Ok(match cmd.kind {
        K::Play => json!({ "kind": "play" }),
        K::Pause => json!({ "kind": "pause" }),
        K::Next => json!({ "kind": "next" }),
        K::Previous => json!({ "kind": "previous" }),
        K::SeekTo => json!({ "kind": "seekTo",
            "positionMs": cmd.position_ms.ok_or_else(|| need("position_ms"))? }),
        K::SetVolume => json!({ "kind": "setVolume",
            "level": cmd.level.ok_or_else(|| need("level"))?.clamp(0.0, 1.0) }),
        K::PlayNext => json!({ "kind": "playNext", "keys": keys(&cmd.song_ids)? }),
        K::AddToUpNext => json!({ "kind": "addToUpNext", "keys": keys(&cmd.song_ids)? }),
        K::Remove => json!({ "kind": "remove", "uid": cmd.uid.ok_or_else(|| need("uid"))? }),
        K::Move => json!({ "kind": "move", "uid": cmd.uid.ok_or_else(|| need("uid"))?,
            "toIndex": cmd.to_index.ok_or_else(|| need("to_index"))? }),
        K::JumpTo => json!({ "kind": "jumpTo", "uid": cmd.uid.ok_or_else(|| need("uid"))? }),
    })
}

fn remote_item(store: &Store, v: &Value) -> Option<RemoteQueueItem> {
    let song: PresenceSong = serde_json::from_value(v["song"].clone()).ok()?;
    Some(RemoteQueueItem {
        uid: v["uid"].as_i64()?,
        song: remote_song(store, &song),
        chosen_by: if v["chosenBy"] == "app" {
            ChosenBy::App
        } else {
            ChosenBy::User
        },
    })
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Board 12's output picker: this device and the profile's others, with what
    /// they play, and whether handoff is available (and why not). Synchronous.
    pub fn output_devices(&self) -> Result<OutputDevices, EncoreError> {
        let p = self.inner.profile()?;
        Ok(self.inner.output_list(&p))
    }

    /// Board 12, Hold menu: Rename. Any device of the profile. Async.
    pub async fn rename_device(&self, device_id: String, name: String) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let s = inner
                .live_session(&p)
                .ok_or_else(|| EncoreError::NotAllowed {
                    reason: "Connect a companion server first".into(),
                })?;
            sync_loop::signed(&inner, &s, |a| {
                let (s, id, name) = (s.clone(), device_id.clone(), name.clone());
                async move { s.client.rename_device(&a, &id, &name).await }
            })
            .await
            .map_err(sync_loop::into_error)?;
            Ok(())
        })
        .await
    }

    /// Move playback to another of your devices (board 12, item 5). Returns once it
    /// has started there (then fade out and pause: `HandoffConfirmed` says so too),
    /// or fails with why, and this device keeps playing. Async; up to 5 s.
    pub async fn hand_off_to(&self, device_id: String) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move { inner.push_to(device_id, new_id()).await })
            .await
    }

    /// The platform has loaded the song of `HandoffIncoming`: start it at the
    /// returned position now (the sender stops when this device confirms).
    pub fn handoff_ready(&self, transfer_id: String) -> Result<i64, EncoreError> {
        let p = self.inner.profile()?;
        let incoming = {
            let mut live = lock(&p.live)?;
            match live.incoming.take() {
                Some(i) if i.transfer_id == transfer_id => i,
                other => {
                    live.incoming = other;
                    return Err(EncoreError::NotFound {
                        reason: "That handoff has expired".into(),
                    });
                }
            }
        };
        {
            let mut pb = lock(&p.playback)?;
            pb.handoff_carry = Some((incoming.uid, incoming.snapshot.listened_ms));
        }
        let mut live = lock(&p.live)?;
        let now = now_ms();
        let position = start_position(
            &incoming.snapshot,
            live.clock.to_server(now),
            incoming.duration_ms,
        );
        live.position_ms = position;
        live.position_at_local_ms = now;
        live.playing = incoming.snapshot.playing;
        live.banner.dismiss(&incoming.from);
        live.relay(
            &incoming.from,
            &incoming.transfer_id,
            json!({ "kind": "accept", "transferId": incoming.transfer_id }),
        )
        .map_err(|reason| EncoreError::Unreachable { reason })?;
        drop(live);
        self.inner.publish_presence();
        Ok(position)
    }

    /// The platform couldn't load the song of `HandoffIncoming`: the sender keeps
    /// playing and shows `reason`.
    pub fn handoff_failed(&self, transfer_id: String, reason: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let mut live = lock(&p.live)?;
        if let Some(i) = live.incoming.take_if(|i| i.transfer_id == transfer_id) {
            let _ = live.relay(
                &i.from,
                &i.transfer_id,
                json!({ "kind": "decline", "transferId": i.transfer_id, "reason": reason }),
            );
        }
        Ok(())
    }

    /// "Continue" on the banner, or This device in the picker while another plays:
    /// pull playback here. Returns once `HandoffIncoming` has been sent (load it,
    /// then `handoff_ready`). Async; up to 6 s.
    pub async fn pull_from(&self, device_id: String) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            if lock(&p.playback).map(|pb| guest(&pb))? {
                return Err(EncoreError::NotAllowed {
                    reason: "Handoff isn't available in Guest mode".into(),
                });
            }
            let transfer_id = new_id();
            let (tx, rx) = oneshot::channel();
            {
                let mut live = lock(&p.live)?;
                live.banner.dismiss(&device_id);
                live.pulls.insert(transfer_id.clone(), tx);
                if let Err(reason) = live.relay(
                    &device_id,
                    &transfer_id,
                    json!({ "kind": "pullRequest", "transferId": transfer_id }),
                ) {
                    live.pulls.remove(&transfer_id);
                    return Err(EncoreError::Unreachable { reason });
                }
            }
            let got = tokio::time::timeout(PULL_WITHIN, rx).await;
            if let Ok(mut live) = lock(&p.live) {
                live.pulls.remove(&transfer_id);
            }
            match got {
                Ok(Ok(Ok(()))) => Ok(()),
                Ok(Ok(Err(reason))) => Err(EncoreError::Unreachable { reason }),
                _ => Err(EncoreError::Unreachable {
                    reason: "The other device didn't answer".into(),
                }),
            }
        })
        .await
    }

    /// The banner was swiped away: not again this session for that device.
    pub fn dismiss_continue(&self, device_id: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        lock(&p.live)?.banner.dismiss(&device_id);
        Ok(())
    }

    /// Control a playing device without moving the music. Async; up to 3 s.
    pub async fn remote_command(
        &self,
        device_id: String,
        command: RemoteCommand,
    ) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let cmd = command_wire(&p, &command)?;
            let reply = inner
                .ask(
                    &p,
                    &device_id,
                    |msg_id| json!({ "kind": "remote", "msgId": msg_id, "cmd": cmd }),
                )
                .await?;
            if reply["ok"].as_bool().unwrap_or(false) {
                Ok(())
            } else {
                Err(EncoreError::NotAllowed {
                    reason: reply["reason"]
                        .as_str()
                        .unwrap_or("The other device couldn't do that")
                        .to_owned(),
                })
            }
        })
        .await
    }

    /// The playing device's queue, for the queue sheet while controlling it. Empty
    /// under Private listening there. Async; up to 3 s.
    pub async fn remote_queue(&self, device_id: String) -> Result<RemoteQueue, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            let v = inner
                .ask(
                    &p,
                    &device_id,
                    |msg_id| json!({ "kind": "queueRequest", "msgId": msg_id }),
                )
                .await?;
            let store = lock(&p.store)?;
            let list = |k: &str| -> Vec<RemoteQueueItem> {
                v[k].as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|i| remote_item(&store, i))
                    .collect()
            };
            Ok(RemoteQueue {
                current: remote_item(&store, &v["current"]),
                up_next: list("upNext"),
                autoplay: list("autoplay"),
            })
        })
        .await
    }

    /// "Control only" (board 12, Hold menu) or the "Playing on…" capsule: send
    /// `RemoteState` whenever that device changes.
    pub fn control_only(&self, device_id: String) -> Result<Option<OutputDevice>, EncoreError> {
        let p = self.inner.profile()?;
        lock(&p.live)?.controlling = Some(device_id.clone());
        Ok(self.inner.device_row(&p, &device_id))
    }

    pub fn stop_remote_control(&self) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        lock(&p.live)?.controlling = None;
        Ok(())
    }

    /// The player paused (any reason). Presence shows it; nothing is logged.
    pub fn playback_paused(&self, position_ms: i64) -> Result<(), EncoreError> {
        self.inner.note_playback(Some(false), Some(position_ms));
        Ok(())
    }

    /// The player resumed.
    pub fn playback_resumed(&self, position_ms: i64) -> Result<(), EncoreError> {
        self.inner.note_playback(Some(true), Some(position_ms));
        Ok(())
    }

    /// The player's volume, 0 to 1 (for presence, remote control and handoff).
    pub fn set_volume_level(&self, level: f64) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        lock(&p.live)?.volume = level.clamp(0.0, 1.0);
        self.inner.publish_presence();
        Ok(())
    }
}
