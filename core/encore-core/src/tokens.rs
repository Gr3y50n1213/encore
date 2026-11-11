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
    pub search_lyrics: u32,
    pub recent_searches_max: u32,
    pub typo_max_edits: u32,
    pub lyrics_gap_ms: i64,
    pub lyrics_last_line_ms: i64,
    pub lyrics_refresh_after_ms: i64,
    pub palette: crate::palette::Settings,
    pub signal: crate::signal::Rules,
    pub undo_ms: i64,
    pub pin_min_digits: usize,
    pub pin_max_digits: usize,
    pub pin_wrong_tries: u32,
    pub pin_first_lock_ms: i64,
    pub pin_lock_max_ms: i64,
    pub guest_idle_ms: i64,
    pub switch_flush: std::time::Duration,
    pub private_ends_after_ms: i64,
    pub confirm_over_songs: u32,
    pub downloads: Downloads,
    /// The sleep timer's fade (Phase 9; `motion.timing.sleepTimerFadeOutMs`).
    pub sleep_fade_ms: i64,
}

/// Downloads and the rolling cache (Phase 8; `downloads` in tokens.json).
#[derive(Debug, Clone, PartialEq)]
pub struct Downloads {
    pub default_quality: String,
    pub wifi_only_default: bool,
    pub limit_options_gb: Vec<u64>,
    pub remove_unplayed_after_days: u32,
    pub keep_free_bytes: u64,
    pub max_attempts: u32,
    pub progress_every_ms: u64,
    pub cover_size_px: u32,
    pub estimate_lossless_kbps: u64,
    pub estimate_lossy_kbps: u64,
    pub cache_options_bytes: Vec<u64>,
    pub cache_default_bytes: u64,
    pub smart_budget_bytes: u64,
}

const MB: u64 = 1_000_000;
const GB: u64 = 1_000_000_000;

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

fn real(v: &Value, path: &str) -> f64 {
    at(v, path)
        .as_f64()
        .unwrap_or_else(|| panic!("{path} is not a number"))
}

fn list(v: &Value, path: &str) -> Vec<u64> {
    at(v, path)
        .as_array()
        .unwrap_or_else(|| panic!("{path} is not a list"))
        .iter()
        .map(|n| {
            n.as_f64()
                .unwrap_or_else(|| panic!("{path} holds a non-number")) as u64
        })
        .collect()
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
        search_lyrics: num(&v, "search.limits.lyrics") as u32,
        recent_searches_max: num(&v, "search.recentSearchesMax") as u32,
        typo_max_edits: num(&v, "search.typoMaxEdits") as u32,
        lyrics_gap_ms: num(&v, "lyrics.instrumentalGapSeconds") as i64 * 1000,
        lyrics_last_line_ms: num(&v, "lyrics.lastLineSeconds") as i64 * 1000,
        lyrics_refresh_after_ms: num(&v, "lyrics.refreshAfterDays") as i64 * 86_400_000,
        palette: crate::palette::Settings {
            sample_size_px: num(&v, "albumGradient.sampleSizePx") as u32,
            clusters: num(&v, "albumGradient.clusters") as usize,
            stops: num(&v, "albumGradient.stops") as usize,
            rank_chroma_weight: real(&v, "albumGradient.rankChromaWeight"),
            duplicate_distance: real(&v, "albumGradient.duplicateDistance"),
            light_art_min_lightness: real(&v, "albumGradient.lightArtMinLightness"),
            dark_base_lightness: real(&v, "albumGradient.darkBaseLightness"),
            dark_base_max_chroma: real(&v, "albumGradient.darkBaseMaxChroma"),
            light_base_lightness: real(&v, "albumGradient.lightBaseLightness"),
            light_base_max_chroma: real(&v, "albumGradient.lightBaseMaxChroma"),
            light_field_min_lightness: real(&v, "albumGradient.lightFieldMinLightness"),
            light_field_max_chroma: real(&v, "albumGradient.lightFieldMaxChroma"),
            max_iterations: num(&v, "albumGradient.kmeansMaxIterations") as usize,
            text_contrast_min: real(&v, "albumGradient.textContrastMin"),
            light_art_text_contrast_min: real(&v, "albumGradient.lightArtTextContrastMin"),
            lightness_step: real(&v, "albumGradient.contrastLightnessStep"),
        },
        undo_ms: num(&v, "gestures.dragAndDrop.shortcutUndoMs") as i64,
        pin_min_digits: num(&v, "profiles.pinMinDigits") as usize,
        pin_max_digits: num(&v, "profiles.pinMaxDigits") as usize,
        pin_wrong_tries: num(&v, "profiles.pinWrongTriesBeforeLock") as u32,
        pin_first_lock_ms: num(&v, "profiles.pinFirstLockSeconds") as i64 * 1000,
        pin_lock_max_ms: num(&v, "profiles.pinLockMaxSeconds") as i64 * 1000,
        guest_idle_ms: num(&v, "profiles.guestIdleHours") as i64 * 3_600_000,
        switch_flush: std::time::Duration::from_secs(num(&v, "profiles.switchFlushSeconds")),
        private_ends_after_ms: num(&v, "smartPicks.privateListeningEndsAfterHoursSilent") as i64
            * 3_600_000,
        confirm_over_songs: num(&v, "gestures.dragAndDrop.confirmOverSongs") as u32,
        downloads: Downloads {
            default_quality: text(&v, "downloads.defaultQuality"),
            wifi_only_default: at(&v, "downloads.wifiOnlyDefault")
                .as_bool()
                .expect("downloads.wifiOnlyDefault is true or false"),
            limit_options_gb: list(&v, "downloads.limitOptionsGB"),
            remove_unplayed_after_days: num(&v, "downloads.removeUnplayedAfterDays") as u32,
            keep_free_bytes: num(&v, "downloads.keepFreeMB") * MB,
            max_attempts: num(&v, "downloads.maxAttempts") as u32,
            progress_every_ms: num(&v, "downloads.progressEveryMs"),
            cover_size_px: num(&v, "downloads.coverSizePx") as u32,
            estimate_lossless_kbps: num(&v, "downloads.estimateLosslessKbps"),
            estimate_lossy_kbps: num(&v, "downloads.estimateLossyKbps"),
            cache_options_bytes: list(&v, "downloads.cacheOptionsMB")
                .into_iter()
                .map(|m| m * MB)
                .collect(),
            cache_default_bytes: num(&v, "downloads.cacheDefaultMB") * MB,
            smart_budget_bytes: num(&v, "smartPicks.mixes.smartDownloadsDefaultBudgetGB") * GB,
        },
        signal: crate::signal::Rules {
            early_skip_ms: num(&v, "smartPicks.earlySkipBoundarySeconds") * 1000,
            late_skip_percent: num(&v, "smartPicks.lateSkipPercent"),
            accident_ms: num(&v, "smartPicks.accidentSeconds") * 1000,
            interruption_window_ms: num(&v, "smartPicks.interruptionWindowSeconds") as i64 * 1000,
            too_soon_ms: num(&v, "smartPicks.tooSoonHours") as i64 * 3_600_000,
            coalesce_ms: num(&v, "gestures.playerSwipe.coalesceMs") as i64,
        },
        sleep_fade_ms: num(&v, "motion.timing.sleepTimerFadeOutMs") as i64,
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
        let d = &t.downloads;
        assert_eq!(d.default_quality, "original");
        assert!(d.wifi_only_default);
        assert_eq!(d.limit_options_gb, [2, 5, 10, 20, 50]);
        assert_eq!((d.keep_free_bytes, d.max_attempts), (500_000_000, 3));
        assert_eq!(d.cache_default_bytes, 1_000_000_000);
        assert_eq!(d.smart_budget_bytes, 2_000_000_000);
    }
}
