//! Handoff's rules without the network (plan section 11; board 12; ADR core-0016):
//! the snapshot a sender packages, the start position on the target, the clock
//! estimate from pings, and when the "Continue from…" banner may show. The live
//! link (`api/core/live.rs`) moves these over the companion server.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// One queue entry as it travels: by song key and the media server's item ID,
/// never this device's track ID (another device's mirror numbers songs its own way).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireEntry {
    pub key: String,
    pub item: String,
    pub chosen_by: String,
    pub started_from: String,
    pub context_id: Option<String>,
}

/// What a sender packages (section 11, step 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub history: Vec<WireEntry>,
    pub current: WireEntry,
    pub up_next: Vec<WireEntry>,
    pub autoplay: Vec<WireEntry>,
    pub shuffle: bool,
    /// "off", "all" or "one".
    pub repeat: String,
    pub autoplay_enabled: bool,
    pub radio_seed: Option<String>,
    /// Where the current song was, and when (the server's clock).
    pub position_ms: i64,
    pub at_server_ms: i64,
    pub playing: bool,
    /// 0 to 1.
    pub volume: f64,
    /// Listening time of the current song so far, carried on by the target.
    pub listened_ms: u64,
}

impl Snapshot {
    /// Every entry, for mapping.
    pub(crate) fn len(&self) -> usize {
        self.history.len() + 1 + self.up_next.len() + self.autoplay.len()
    }
}

/// Where the target starts: the sender's position plus the time since, by the
/// server's clock (which both measure against), within the song. A paused song
/// starts where it was.
pub(crate) fn start_position(s: &Snapshot, now_server_ms: i64, duration_ms: i64) -> i64 {
    let elapsed = if s.playing {
        (now_server_ms - s.at_server_ms).max(0)
    } else {
        0
    };
    let pos = s.position_ms.max(0) + elapsed;
    if duration_ms > 0 {
        pos.min(duration_ms)
    } else {
        pos
    }
}

/// This device's estimate of the server's clock, from ping round trips (the
/// sample with the shortest trip wins, as NTP does).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Clock {
    /// Server time minus local time.
    pub offset_ms: i64,
    pub rtt_ms: Option<i64>,
}

impl Clock {
    /// A ping sent at `sent_local`, answered with `server_ms`, received at `recv_local`.
    pub(crate) fn observe(&mut self, sent_local: i64, recv_local: i64, server_ms: i64) {
        let rtt = (recv_local - sent_local).max(0);
        if self.rtt_ms.is_none_or(|best| rtt <= best) {
            self.rtt_ms = Some(rtt);
            self.offset_ms = server_ms - (sent_local + rtt / 2);
        }
    }

    pub(crate) fn to_server(self, local_ms: i64) -> i64 {
        local_ms + self.offset_ms
    }
}

/// Board 12, item 1: the banner appears when this app opens while another of your
/// devices plays; never while this device plays; once per session per device;
/// never transfers by itself.
#[derive(Debug, Default)]
pub(crate) struct Banner {
    shown: HashSet<String>,
}

impl Banner {
    /// Whether to offer "Continue from `device`" now. Offering marks it shown.
    pub(crate) fn offer(&mut self, device: &str, it_plays: bool, this_plays: bool) -> bool {
        if !it_plays || this_plays || self.shown.contains(device) {
            return false;
        }
        self.shown.insert(device.to_owned());
        true
    }

    /// Swiped away (or tapped): not again this session.
    pub(crate) fn dismiss(&mut self, device: &str) {
        self.shown.insert(device.to_owned());
    }
}

/// A device's presence as the live link carries it (and the core publishes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Presence {
    /// "playing", "paused" or "idle".
    pub state: String,
    pub song: Option<PresenceSong>,
    pub position_ms: i64,
    pub at_server_ms: i64,
    pub volume: f64,
    /// Private listening: what plays isn't shared, even with your own devices.
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresenceSong {
    pub key: String,
    pub item: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: i64,
}

impl Presence {
    /// The position now, by the server's clock.
    pub(crate) fn position_at(&self, now_server_ms: i64) -> i64 {
        let pos = if self.state == "playing" {
            self.position_ms + (now_server_ms - self.at_server_ms).max(0)
        } else {
            self.position_ms
        };
        match &self.song {
            Some(s) if s.duration_ms > 0 => pos.clamp(0, s.duration_ms),
            _ => pos.max(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(k: &str) -> WireEntry {
        WireEntry {
            key: k.into(),
            item: format!("item-{k}"),
            chosen_by: "user".into(),
            started_from: "album".into(),
            context_id: None,
        }
    }

    fn snap(playing: bool) -> Snapshot {
        Snapshot {
            history: vec![entry("a")],
            current: entry("b"),
            up_next: vec![entry("c"), entry("d")],
            autoplay: vec![entry("e")],
            shuffle: false,
            repeat: "off".into(),
            autoplay_enabled: true,
            radio_seed: None,
            position_ms: 61_000,
            at_server_ms: 1_000_000,
            playing,
            volume: 0.8,
            listened_ms: 60_000,
        }
    }

    #[test]
    fn the_target_starts_where_the_sender_is_now() {
        // 350 ms after the snapshot (by the server's clock) the song is 350 ms on.
        assert_eq!(start_position(&snap(true), 1_000_350, 200_000), 61_350);
        // Paused: exactly where it was, however long it took.
        assert_eq!(start_position(&snap(false), 1_090_000, 200_000), 61_000);
        // Never past the end, never before the start (clock skew).
        assert_eq!(start_position(&snap(true), 9_000_000, 200_000), 200_000);
        assert_eq!(start_position(&snap(true), 999_000, 200_000), 61_000);
    }

    #[test]
    fn the_snapshot_travels_as_camel_case_json_and_back() {
        let s = snap(true);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["upNext"][1]["key"], "d");
        assert_eq!(v["atServerMs"], 1_000_000);
        assert_eq!(v["current"]["chosenBy"], "user");
        assert_eq!(serde_json::from_value::<Snapshot>(v).unwrap(), s);
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn the_clock_keeps_the_shortest_round_trip() {
        let mut c = Clock::default();
        // Local 1000, server says 5050 at the middle of a 100 ms trip: offset 4000.
        c.observe(1000, 1100, 5050);
        assert_eq!((c.offset_ms, c.rtt_ms), (4000, Some(100)));
        // A slower trip says less: ignored.
        c.observe(2000, 2600, 6900);
        assert_eq!(c.offset_ms, 4000);
        // A quicker one wins.
        c.observe(3000, 3020, 7012);
        assert_eq!((c.offset_ms, c.rtt_ms), (4002, Some(20)));
        assert_eq!(c.to_server(10_000), 14_002);
    }

    #[test]
    fn the_banner_follows_board_12() {
        let mut b = Banner::default();
        assert!(!b.offer("iphone", false, false), "nothing plays there");
        assert!(
            !b.offer("iphone", true, true),
            "never while this device plays"
        );
        assert!(b.offer("iphone", true, false));
        assert!(
            !b.offer("iphone", true, false),
            "once per session per device"
        );
        assert!(b.offer("pixel", true, false), "each device once");
        b.dismiss("mac");
        assert!(!b.offer("mac", true, false), "dismissed");
    }

    #[test]
    fn presence_moves_on_while_playing() {
        let p = Presence {
            state: "playing".into(),
            song: Some(PresenceSong {
                key: "k".into(),
                item: "i".into(),
                title: "T".into(),
                artist: "A".into(),
                album: None,
                duration_ms: 100_000,
            }),
            position_ms: 10_000,
            at_server_ms: 50_000,
            volume: 1.0,
            private: false,
        };
        assert_eq!(p.position_at(52_500), 12_500);
        assert_eq!(p.position_at(900_000), 100_000);
        let paused = Presence {
            state: "paused".into(),
            ..p
        };
        assert_eq!(paused.position_at(900_000), 10_000);
    }
}
