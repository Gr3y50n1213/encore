//! Phase 5's recording (`--phase5`): what "your own history" is made of, read-only
//! (the owner's answer, 7 November 2026; docs/adr/core-0014).
//!
//! 1. The whole library with Jellyfin's own user data (play counts, last played,
//!    favourites), through the adapter's library sync.
//! 2. The installed plugins, and if Playback Reporting is one, its play log for
//!    this user's audio (a `SELECT` only).
//! 3. Jellyfin's "similar items" for five songs.
//!
//! The history itself is written only to `tools/target/private-history/`
//! (git-ignored, on this PC): song and artist names hashed with a key that is not
//! kept, times and durations as they are, so the replay can score the engine on
//! the owner's real listening. The repository gets only the shapes of the
//! replies (three rows each, values scrubbed) and summary counts.

use std::collections::HashMap;
use std::path::Path;

use encore_sources::LibraryDelta;
use encore_sources::jellyfin::JellyfinClient;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug, Default)]
pub struct Notes {
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    /// The plugin's rows, if it is installed: (item ID, played at, seconds).
    pub plugin_plays: Option<Vec<(String, String, f64)>>,
}

const PLUGIN: &str = "Playback Reporting";

pub async fn run(client: &JellyfinClient, user_id: &str, library: &LibraryDelta) -> Notes {
    let mut n = Notes::default();
    let counted = library.play_counts.as_ref().map_or(0, Vec::len);
    let plays: u64 = library
        .play_counts
        .iter()
        .flatten()
        .map(|(_, c, _)| u64::from(*c))
        .sum();
    n.lines.push(format!(
        "Library: {} songs; {counted} of them played in some app, {plays} plays in all (Jellyfin's own counts).",
        library.tracks.len()
    ));
    match client.plugin_names().await {
        Ok(names) => {
            let has = names.iter().any(|x| x.contains(PLUGIN));
            n.lines.push(format!(
                "Plugins: {} installed; Playback Reporting {}.",
                names.len(),
                if has { "is one of them" } else { "is not" }
            ));
            if has {
                let q = format!(
                    "SELECT ItemId, DateCreated, PlayDuration FROM PlaybackActivity \
                     WHERE UserId = '{user_id}' AND ItemType = 'Audio' ORDER BY DateCreated"
                );
                match client.playback_reporting_query(&q).await {
                    Ok(v) => {
                        let rows: Vec<(String, String, f64)> = v["results"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|r| {
                                let r = r.as_array()?;
                                let secs = r
                                    .get(2)?
                                    .as_str()
                                    .and_then(|s| s.parse().ok())
                                    .or(r.get(2)?.as_f64())?;
                                Some((
                                    r.first()?.as_str()?.to_owned(),
                                    r.get(1)?.as_str()?.to_owned(),
                                    secs,
                                ))
                            })
                            .collect();
                        n.lines.push(format!(
                            "Playback Reporting: {} audio plays with their durations.",
                            rows.len()
                        ));
                        n.plugin_plays = Some(rows);
                    }
                    Err(e) => n
                        .warnings
                        .push(format!("The Playback Reporting query failed: {e}")),
                }
            }
        }
        Err(e) => n.warnings.push(format!(
            "The plugin list couldn't be read (an administrator's token is needed): {e}"
        )),
    }
    // Similar items for the five most played songs (or the first five).
    let mut by_plays: Vec<(u32, &str)> = library
        .play_counts
        .iter()
        .flatten()
        .map(|(id, c, _)| (*c, id.as_str()))
        .collect();
    by_plays.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let mut seeds: Vec<String> = by_plays
        .iter()
        .take(5)
        .map(|(_, id)| (*id).to_owned())
        .collect();
    if seeds.is_empty() {
        seeds = library
            .tracks
            .iter()
            .take(5)
            .map(|t| t.source_item_id.clone())
            .collect();
    }
    let mut found = 0;
    for id in &seeds {
        match client.similar(id, 10).await {
            Ok(list) => found += list.len(),
            Err(e) => n.warnings.push(format!("Similar items failed: {e}")),
        }
    }
    n.lines
        .push(format!("Similar items: {found} for {} songs.", seeds.len()));
    n
}

/// A short hash with a key that is not kept: the same value always gives the same
/// hash within one run, and nothing outside it can reverse it.
fn hasher(key: [u8; 32]) -> impl Fn(&str) -> String {
    move |s: &str| {
        let d = Sha256::new()
            .chain_update(key)
            .chain_update(s.as_bytes())
            .finalize();
        d.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }
}

/// The private dataset for the local replay: the library (hashed names, real
/// genres, years and lengths) and the plays (Jellyfin's counts, and the plugin's
/// per-play log when there is one). Written to `dir` only.
pub fn write_private(
    dir: &Path,
    key: [u8; 32],
    library: &LibraryDelta,
    notes: &Notes,
) -> std::io::Result<()> {
    let h = hasher(key);
    let counts: HashMap<&str, (u32, Option<i64>)> = library
        .play_counts
        .iter()
        .flatten()
        .map(|(id, c, last)| (id.as_str(), (*c, *last)))
        .collect();
    let favorites: std::collections::HashSet<&str> = library
        .favorite_item_ids
        .iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let songs: Vec<Value> = library
        .tracks
        .iter()
        .map(|t| {
            let (count, last) = counts
                .get(t.source_item_id.as_str())
                .copied()
                .unwrap_or((0, None));
            json!({
                "key": h(&t.source_item_id),
                "title": h(&t.title),
                "artists": t.artists.iter().map(|a| h(&a.name)).collect::<Vec<_>>(),
                "album": t.album_title.as_deref().map(&h),
                "album_artist": t.album_artists.first().map(|a| h(&a.name)),
                "genres": t.genres,
                "year": t.year,
                "duration_ms": t.duration_ms,
                "added_utc_ms": t.date_added_utc_ms,
                "server_play_count": count,
                "server_last_played_utc_ms": last,
                "server_favorite": favorites.contains(t.source_item_id.as_str()),
                "explicit": false,
            })
        })
        .collect();
    let plays: Vec<Value> = notes
        .plugin_plays
        .iter()
        .flatten()
        .map(|(id, at, secs)| json!({ "key": h(id), "at": at, "seconds": secs }))
        .collect();
    std::fs::create_dir_all(dir)?;
    let data =
        json!({ "songs": songs, "pluginPlays": plays, "hasPlugin": notes.plugin_plays.is_some() });
    std::fs::write(dir.join("history.json"), serde_json::to_string(&data)?)?;
    std::fs::write(
        dir.join("README.txt"),
        "The owner's own listening history, recorded read-only by fixture-recorder --phase5.\n\
         Names are hashed with a key that was not kept. Never commit this folder (tools/target is git-ignored).\n",
    )
}

/// The committed fixture keeps three rows of each reply, with the listening values
/// scrubbed (counts and dates are the owner's history).
pub fn scrub(path: &str, body: &mut Value) {
    if let Some(items) = body.get_mut("Items").and_then(Value::as_array_mut) {
        items.truncate(3);
        for i in items.iter_mut() {
            if let Some(u) = i.get_mut("UserData") {
                u["PlayCount"] = json!(1);
                u["LastPlayedDate"] = json!("2026-01-01T00:00:00.0000000Z");
            }
        }
        if body.get("TotalRecordCount").is_some() {
            body["TotalRecordCount"] = json!(body["Items"].as_array().map_or(0, Vec::len));
        }
    }
    if path.ends_with("submit_custom_query")
        && let Some(rows) = body.get_mut("results").and_then(Value::as_array_mut)
    {
        rows.truncate(3);
        for r in rows.iter_mut() {
            if let Some(r) = r.as_array_mut() {
                if r.len() > 1 {
                    r[1] = json!("2026-01-01 00:00:00.0000000");
                }
                if r.len() > 2 {
                    r[2] = json!("180");
                }
            }
        }
    }
}
