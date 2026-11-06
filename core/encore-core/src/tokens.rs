//! Starting values from `design/tokens.json`, read when the crate is built, so the
//! core uses the same numbers the platforms see (core/CLAUDE.md: numbers live in
//! tokens or config, never inline).

use std::sync::OnceLock;

use serde_json::Value;

const TOKENS: &str = include_str!("../../../design/tokens.json");

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub preload_before_end_ms: u64,
    pub checkpoint_seconds: u64,
    pub sync_page_size: u32,
    pub https_ports: Vec<u16>,
    pub http_port: u16,
    pub probe_timeout_seconds: u64,
    pub quick_connect_expiry_seconds: u64,
    pub previous_restart_after_ms: u64,
    pub high_kbps: u32,
    pub normal_kbps: u32,
    pub data_saver_kbps: u32,
    pub default_wifi: String,
    pub default_cellular: String,
    pub search_songs: u32,
    pub search_albums: u32,
    pub search_artists: u32,
    pub search_playlists: u32,
    pub recent_searches_max: u32,
    pub typo_max_edits: u32,
}

fn at<'a>(v: &'a Value, path: &str) -> &'a Value {
    let found = path.split('.').fold(v, |cur, key| &cur[key]);
    assert!(!found.is_null(), "design/tokens.json has no {path}");
    found
}

fn num(v: &Value, path: &str) -> u64 {
    at(v, path)
        .as_f64()
        .unwrap_or_else(|| panic!("{path} is not a number")) as u64
}

fn text(v: &Value, path: &str) -> String {
    at(v, path)
        .as_str()
        .unwrap_or_else(|| panic!("{path} is not text"))
        .to_owned()
}

fn parse() -> Tokens {
    let v: Value = serde_json::from_str(TOKENS).expect("design/tokens.json is valid JSON");
    Tokens {
        preload_before_end_ms: num(&v, "playback.preloadBeforeEndSeconds") * 1000,
        checkpoint_seconds: num(&v, "playLog.checkpointSeconds"),
        sync_page_size: num(&v, "library.syncPageSize") as u32,
        https_ports: at(&v, "library.probe.httpsPorts")
            .as_array()
            .expect("httpsPorts is a list")
            .iter()
            .filter_map(Value::as_u64)
            .map(|p| p as u16)
            .collect(),
        http_port: num(&v, "library.probe.httpPort") as u16,
        probe_timeout_seconds: num(&v, "library.probe.timeoutSeconds"),
        quick_connect_expiry_seconds: num(&v, "library.quickConnectExpirySeconds"),
        previous_restart_after_ms: num(
            &v,
            "gestures.playerSwipe.previousButtonRestartAfterSeconds",
        ) * 1000,
        high_kbps: num(&v, "playback.quality.highKbps") as u32,
        normal_kbps: num(&v, "playback.quality.normalKbps") as u32,
        data_saver_kbps: num(&v, "playback.quality.dataSaverKbps") as u32,
        default_wifi: text(&v, "playback.quality.defaultWifi"),
        default_cellular: text(&v, "playback.quality.defaultCellular"),
        search_songs: num(&v, "search.limits.songs") as u32,
        search_albums: num(&v, "search.limits.albums") as u32,
        search_artists: num(&v, "search.limits.artists") as u32,
        search_playlists: num(&v, "search.limits.playlists") as u32,
        recent_searches_max: num(&v, "search.recentSearchesMax") as u32,
        typo_max_edits: num(&v, "search.typoMaxEdits") as u32,
    }
}

/// The parsed tokens (parsed once).
pub fn tokens() -> &'static Tokens {
    static T: OnceLock<Tokens> = OnceLock::new();
    T.get_or_init(parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_value_the_core_uses_is_in_tokens_json() {
        let t = tokens();
        assert_eq!(t.preload_before_end_ms, 20_000);
        assert_eq!(t.checkpoint_seconds, 10);
        assert_eq!(t.previous_restart_after_ms, 3_000);
        assert_eq!(t.https_ports, [443, 8920]);
        assert_eq!(
            (t.high_kbps, t.normal_kbps, t.data_saver_kbps),
            (320, 160, 96)
        );
        assert_eq!(t.search_songs, 20);
    }
}
