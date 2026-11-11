//! Downloads and offline (Phase 8; docs/adr/core-0017; boards 06, 08, 11, 13, 17,
//! 20). The core decides what to keep and fetches it itself, over the same client
//! the rest of the core uses (so a pinned self-signed server works); the platform
//! schedules `run_downloads` (a user-initiated job on Android, a background task
//! elsewhere) and reports the network with `set_connectivity`.
//!
//! - A profile keeps *collections* (a song, an album, a playlist, a mix, Liked
//!   songs, Smart downloads). Each holds songs, recomputed when the library, likes,
//!   playlists or mixes change; each wanted song is fetched once.
//! - Files live in `<data>/downloads`, named by a key made from the server, the
//!   song and the quality. Two profiles share a file; the device registry counts
//!   how many hold it (never which), and the file goes when the last lets go.
//! - Offline, only saved songs and songs the player has fully cached can play:
//!   the queue passes over the rest, Autoplay and radio pick from them, and a tap
//!   on another song is refused with a reason (board 35: shake, REJECT).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use encore_sources::jellyfin::{StreamCodec, StreamRequest};
use encore_sources::{MediaSource, SourceError};
use encore_store::Store;
use sha2::{Digest, Sha256};

use super::{EncoreCore, Inner, Profile, lock, now_ms};
use crate::api::types::*;
use crate::tokens::tokens;

const QUALITY_KEY: &str = "downloads.quality";
const WIFI_ONLY_KEY: &str = "downloads.wifi_only";
const LIMIT_KEY: &str = "downloads.limit_bytes";
const REMOVE_UNPLAYED_KEY: &str = "downloads.remove_unplayed";
const CACHE_LIMIT_KEY: &str = "downloads.cache_limit_bytes";
const LIKED_TITLE: &str = "Liked songs";
const SMART_TITLE: &str = "Smart downloads";
const DAY_MS: i64 = 86_400_000;

/// What the platform last said about the network (device-wide).
#[derive(Debug)]
pub(super) struct Connectivity {
    online: bool,
    network: Network,
    /// The player's fully cached songs, as `cache_key`s.
    cached_keys: Vec<String>,
    /// Those keys as the open profile's song IDs.
    cached_ids: HashSet<i64>,
}

impl Default for Connectivity {
    fn default() -> Self {
        // Until the platform says otherwise, assume a connection: nothing dims.
        Connectivity {
            online: true,
            network: Network::Wifi,
            cached_keys: Vec::new(),
            cached_ids: HashSet::new(),
        }
    }
}

/// The transfer in progress, if any (one at a time).
#[derive(Debug, Default)]
pub(super) struct Transfers {
    running: bool,
    cancel: Arc<AtomicBool>,
    active: Option<DownloadProgress>,
    /// Why the last run stopped, while songs still wait.
    last_stop: Option<DownloadStop>,
}

fn kind_name(k: DownloadKind) -> &'static str {
    match k {
        DownloadKind::Song => "song",
        DownloadKind::Album => "album",
        DownloadKind::Playlist => "playlist",
        DownloadKind::Mix => "mix",
        DownloadKind::LikedSongs => "liked",
        DownloadKind::Smart => "smart",
    }
}

fn kind_of(name: &str) -> Option<DownloadKind> {
    Some(match name {
        "song" => DownloadKind::Song,
        "album" => DownloadKind::Album,
        "playlist" => DownloadKind::Playlist,
        "mix" => DownloadKind::Mix,
        "liked" => DownloadKind::LikedSongs,
        "smart" => DownloadKind::Smart,
        _ => return None,
    })
}

/// A target's row key: (kind, item).
fn key_of(t: &DownloadTarget) -> Result<(&'static str, String), EncoreError> {
    let kind = kind_name(t.kind);
    let item = match t.kind {
        DownloadKind::Song | DownloadKind::Album | DownloadKind::Playlist => t.id.to_string(),
        DownloadKind::Mix => {
            t.mix_id
                .clone()
                .filter(|m| !m.is_empty())
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "That mix is no longer here".into(),
                })?
        }
        DownloadKind::LikedSongs | DownloadKind::Smart => String::new(),
    };
    Ok((kind, item))
}

fn target_of(kind: &str, item: &str) -> Option<DownloadTarget> {
    let kind = kind_of(kind)?;
    Some(match kind {
        DownloadKind::Mix => DownloadTarget {
            kind,
            id: 0,
            mix_id: Some(item.to_owned()),
        },
        DownloadKind::LikedSongs | DownloadKind::Smart => DownloadTarget {
            kind,
            id: 0,
            mix_id: None,
        },
        _ => DownloadTarget {
            kind,
            id: item.parse().ok()?,
            mix_id: None,
        },
    })
}

fn quality_name(q: StreamQuality) -> &'static str {
    match q {
        StreamQuality::Original => "original",
        StreamQuality::High => "high",
        StreamQuality::Normal => "normal",
        StreamQuality::DataSaver => "dataSaver",
    }
}

fn quality_of(name: &str) -> StreamQuality {
    match name {
        "high" => StreamQuality::High,
        "normal" => StreamQuality::Normal,
        "dataSaver" => StreamQuality::DataSaver,
        _ => StreamQuality::Original,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A song file's key: the server, the song and the quality. The quality's name
/// follows the hash, so a queued song is fetched at the quality it was asked at.
fn file_key(server_id: &str, item_id: &str, quality: StreamQuality) -> String {
    let q = quality_name(quality);
    let d = Sha256::digest(format!("{server_id}\u{1f}{item_id}\u{1f}{q}").as_bytes());
    format!("{}-{q}", hex(&d[..16]))
}

fn quality_in_key(key: &str) -> StreamQuality {
    quality_of(key.rsplit('-').next().unwrap_or_default())
}

fn cover_key(server_id: &str, item_id: &str, px: u32) -> String {
    let d = Sha256::digest(format!("{server_id}\u{1f}{item_id}\u{1f}cover-{px}").as_bytes());
    format!("{}-cover", hex(&d[..16]))
}

/// A file's extension from what the server said it is, else the song's container.
fn extension(content_type: Option<&str>, container: Option<&str>) -> String {
    let from_type = content_type.map(|c| {
        match c
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "audio/flac" | "audio/x-flac" => "flac",
            "audio/mpeg" | "audio/mp3" => "mp3",
            "audio/mp4" | "audio/x-m4a" | "audio/m4a" => "m4a",
            "audio/aac" | "audio/aacp" | "audio/x-aac" => "aac",
            "audio/ogg" | "application/ogg" => "ogg",
            "audio/opus" => "opus",
            "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => "wav",
            _ => "",
        }
    });
    match from_type {
        Some(e) if !e.is_empty() => e.to_owned(),
        _ => container
            .map(|c| c.to_ascii_lowercase())
            .filter(|c| {
                !c.is_empty() && c.len() <= 5 && c.chars().all(|ch| ch.is_ascii_alphanumeric())
            })
            .unwrap_or_else(|| "audio".to_owned()),
    }
}

/// What a song's file will roughly take, before the server says (for the limit
/// and the free-space check).
fn estimate_bytes(duration_ms: i64, container: Option<&str>, quality: StreamQuality) -> u64 {
    let d = &tokens().downloads;
    let t = tokens();
    let lossless = matches!(
        container.map(str::to_ascii_lowercase).as_deref(),
        Some("flac" | "wav" | "alac" | "aiff" | "ape" | "wv")
    );
    let kbps = match quality {
        StreamQuality::Original if lossless => d.estimate_lossless_kbps,
        StreamQuality::Original => d.estimate_lossy_kbps,
        StreamQuality::High => u64::from(t.high_kbps),
        StreamQuality::Normal => u64::from(t.normal_kbps),
        StreamQuality::DataSaver => u64::from(t.data_saver_kbps),
    };
    (duration_ms.max(0) as u64 / 1000) * kbps * 1000 / 8
}

/// One profile's view of who owns which `run_downloads`.
struct RunGuard<'a>(&'a Inner);

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut t) = lock(&self.0.transfers) {
            t.running = false;
            t.active = None;
        }
    }
}

impl Inner {
    pub(super) fn downloads_dir(&self) -> PathBuf {
        Path::new(&self.config.data_dir).join("downloads")
    }

    fn partial_path(&self, key: &str) -> PathBuf {
        self.downloads_dir()
            .join(".partial")
            .join(format!("{key}.part"))
    }

    /// Whether this profile can keep downloads: not a Guest (it leaves nothing).
    fn can_download(&self, p: &Profile) -> Result<(), EncoreError> {
        if lock(&p.playback)?.log.mode() == crate::playlog::ListeningMode::Guest {
            return Err(EncoreError::NotAllowed {
                reason: "Guest can't download: nothing is saved in Guest mode".into(),
            });
        }
        Ok(())
    }

    fn download_quality(&self, store: &Store) -> Result<StreamQuality, EncoreError> {
        Ok(match store.setting(QUALITY_KEY)? {
            Some(q) => quality_of(&q),
            None => quality_of(&tokens().downloads.default_quality),
        })
    }

    fn wifi_only(&self, store: &Store) -> Result<bool, EncoreError> {
        Ok(match store.setting(WIFI_ONLY_KEY)?.as_deref() {
            Some("1") => true,
            Some("0") => false,
            _ => tokens().downloads.wifi_only_default,
        })
    }

    fn limit_bytes(&self, store: &Store) -> Result<Option<u64>, EncoreError> {
        Ok(store
            .setting(LIMIT_KEY)?
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|n| *n > 0))
    }

    pub(super) fn cache_limit(&self, store: &Store) -> Result<u64, EncoreError> {
        Ok(store
            .setting(CACHE_LIMIT_KEY)?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(tokens().downloads.cache_default_bytes))
    }

    /// The request the player or a download sends for a song at a quality.
    pub(super) fn stream_request(
        &self,
        store: &Store,
        quality: StreamQuality,
    ) -> Result<StreamRequest, EncoreError> {
        let t = tokens();
        let quality = match quality {
            StreamQuality::Original => encore_sources::jellyfin::StreamQuality::Original,
            StreamQuality::High => {
                encore_sources::jellyfin::StreamQuality::Capped { kbps: t.high_kbps }
            }
            StreamQuality::Normal => encore_sources::jellyfin::StreamQuality::Capped {
                kbps: t.normal_kbps,
            },
            StreamQuality::DataSaver => encore_sources::jellyfin::StreamQuality::Capped {
                kbps: t.data_saver_kbps,
            },
        };
        let codec = match store.setting("stream.codec")?.as_deref() {
            Some("opus") => StreamCodec::Opus,
            _ => StreamCodec::Aac,
        };
        let direct_play_containers: Vec<String> = match store.setting("stream.containers")? {
            Some(list) => list.split(',').map(str::to_owned).collect(),
            None => super::DEFAULT_CONTAINERS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        };
        Ok(StreamRequest {
            quality,
            codec,
            direct_play_containers,
        })
    }

    /// The saved file of a song, if it is on the device. A file that went missing
    /// is fetched again.
    pub(super) fn local_file(
        &self,
        store: &Store,
        song_id: i64,
    ) -> Result<Option<PathBuf>, EncoreError> {
        let Some(row) = store.download_song(song_id)? else {
            return Ok(None);
        };
        if row.state != "saved" {
            return Ok(None);
        }
        let file = lock(&self.registry)?.download_file(&row.file_key)?;
        let path = file.map(|f| self.downloads_dir().join(f.file_name));
        match path {
            Some(p) if p.is_file() => Ok(Some(p)),
            _ => {
                store.requeue_download(song_id)?;
                Ok(None)
            }
        }
    }

    /// A downloaded album's kept cover, if any.
    pub(super) fn local_cover(
        &self,
        store: &Store,
        album_id: i64,
    ) -> Result<Option<PathBuf>, EncoreError> {
        let Some(key) = store.download_cover(album_id)? else {
            return Ok(None);
        };
        let file = lock(&self.registry)?.download_file(&key)?;
        Ok(file
            .map(|f| self.downloads_dir().join(f.file_name))
            .filter(|p| p.is_file()))
    }

    /// The rolling cache's name for a song at a quality (no token in it).
    pub(super) fn cache_key(
        &self,
        store: &Store,
        track: &encore_store::TrackRow,
        q: StreamQuality,
    ) -> String {
        let server = store
            .source(track.source_id)
            .map(|s| s.server_id)
            .unwrap_or_default();
        format!("{server}/{}/{}", track.source_item_id, quality_name(q))
    }

    /// Offline: the songs that can play (saved and fully cached); None online.
    /// The lock order is the store before connectivity: read connectivity on its
    /// own, then the store.
    fn offline_cached(&self) -> Result<Option<HashSet<i64>>, EncoreError> {
        let c = lock(&self.connectivity)?;
        Ok((!c.online).then(|| c.cached_ids.clone()))
    }

    pub(super) fn playable_now(&self, p: &Profile) -> Result<Option<HashSet<i64>>, EncoreError> {
        let Some(cached) = self.offline_cached()? else {
            return Ok(None);
        };
        let mut set = lock(&p.store)?.saved_download_ids()?;
        set.extend(cached);
        Ok(Some(set))
    }

    /// Offline, the song keys that can play, for smart picks' `only`. `store`
    /// is already held by the caller.
    pub(super) fn playable_keys(&self, store: &Store) -> Result<Option<Vec<String>>, EncoreError> {
        let Some(cached) = self.offline_cached()? else {
            return Ok(None);
        };
        let mut ids = store.saved_download_ids()?;
        ids.extend(cached);
        Ok(Some(
            ids.into_iter()
                .filter_map(|id| store.track(id).ok().map(|t| t.song_key))
                .collect(),
        ))
    }

    /// Hand the queue what can play now, and tell the app (the queue's next
    /// song may have changed).
    pub(super) fn refresh_playable(&self, p: &Profile) -> Result<(), EncoreError> {
        let playable = self.playable_now(p)?;
        let changed = {
            let mut pb = lock(&p.playback)?;
            let before = pb.queue.peek_skip().map(|i| i.uid);
            pb.queue.set_playable(playable);
            pb.queue.peek_skip().map(|i| i.uid) != before
        };
        if changed {
            self.with_queue(|_, _| {})?;
        }
        Ok(())
    }

    /// Offline: a list to play keeps only what can play, and a tap on a song that
    /// can't is refused with the reason (board 06, item 5; board 35's shake).
    pub(super) fn offline_list(
        &self,
        p: &Profile,
        ids: &[i64],
        start: usize,
    ) -> Result<(Vec<i64>, u32), EncoreError> {
        let Some(set) = self.playable_now(p)? else {
            return Ok((ids.to_vec(), start as u32));
        };
        let start = start.min(ids.len().saturating_sub(1));
        if let Some(first) = ids.get(start)
            && !set.contains(first)
        {
            return Err(not_downloaded());
        }
        let kept: Vec<i64> = ids.iter().copied().filter(|id| set.contains(id)).collect();
        let new_start = ids[..start].iter().filter(|id| set.contains(id)).count();
        Ok((kept, new_start as u32))
    }

    /// Offline: songs added to the queue keep only what can play.
    pub(super) fn offline_songs(&self, p: &Profile, ids: &[i64]) -> Result<Vec<i64>, EncoreError> {
        let Some(set) = self.playable_now(p)? else {
            return Ok(ids.to_vec());
        };
        let kept: Vec<i64> = ids.iter().copied().filter(|id| set.contains(id)).collect();
        if kept.is_empty() && !ids.is_empty() {
            return Err(not_downloaded());
        }
        Ok(kept)
    }

    /// The cache keys the platform reported, as this profile's song IDs.
    fn cached_ids_of(&self, store: &Store, keys: &[String]) -> HashSet<i64> {
        let sources: HashMap<String, i64> = store
            .sources()
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.server_id, s.id))
            .collect();
        keys.iter()
            .filter_map(|k| {
                let mut parts = k.splitn(3, '/');
                let (server, item) = (parts.next()?, parts.next()?);
                let source = sources.get(server)?;
                store
                    .track_by_item(*source, item)
                    .ok()
                    .flatten()
                    .map(|t| t.id)
            })
            .collect()
    }

    /// Re-read the cached keys for the profile that just opened.
    pub(super) fn connectivity_for_profile(&self, p: &Profile) -> Result<(), EncoreError> {
        let keys = lock(&self.connectivity)?.cached_keys.clone();
        let ids = self.cached_ids_of(&*lock(&p.store)?, &keys);
        lock(&self.connectivity)?.cached_ids = ids;
        self.refresh_playable(p)
    }

    fn emit_downloads_changed(&self) {
        self.emit(CoreEvent::of(EventKind::DownloadsChanged));
    }

    /// The songs a collection holds now. None: it no longer exists (an album or
    /// playlist left the library, a mix was replaced).
    fn members_of(
        &self,
        store: &Store,
        kind: &str,
        item: &str,
        mix_ids: Option<&HashMap<String, i64>>,
    ) -> Result<Option<Vec<i64>>, EncoreError> {
        let id = || item.parse::<i64>().unwrap_or(-1);
        Ok(match kind {
            "song" => store.track(id()).ok().map(|t| vec![t.id]),
            "album" => match store.album(id()) {
                Ok(_) => Some(
                    store
                        .album_tracks(id())?
                        .into_iter()
                        .map(|t| t.id)
                        .collect(),
                ),
                Err(_) => None,
            },
            "playlist" => match store.playlist_record(id())? {
                Some(rec) => Some(
                    store
                        .playlist_items(id())?
                        .into_iter()
                        .filter_map(|r| {
                            store
                                .track_by_item(rec.source_id, &r.track_item_id)
                                .ok()
                                .flatten()
                        })
                        .map(|t| t.id)
                        .collect(),
                ),
                None => None,
            },
            "liked" => Some(
                store
                    .liked_songs(0, u32::MAX)?
                    .into_iter()
                    .map(|t| t.id)
                    .collect(),
            ),
            "mix" => {
                let Some(ids) = mix_ids else {
                    return Ok(Some(store.download_members(kind, item)?));
                };
                self.find_mix(store, item).map(|(m, _, _)| {
                    m.songs
                        .iter()
                        .filter_map(|s| ids.get(&s.song_key).copied())
                        .collect()
                })
            }
            // Smart downloads are chosen by `refresh_smart_downloads`.
            _ => Some(store.download_members(kind, item)?),
        })
    }

    /// A collection's title as the library names it now.
    fn title_of(&self, store: &Store, kind: &str, item: &str, mix_title: Option<String>) -> String {
        let id = item.parse::<i64>().unwrap_or(-1);
        match kind {
            "song" => store.track(id).map(|t| t.title).unwrap_or_default(),
            "album" => store.album(id).map(|a| a.title).unwrap_or_default(),
            "playlist" => store.playlist(id).map(|p| p.name).unwrap_or_default(),
            "liked" => LIKED_TITLE.to_owned(),
            "smart" => SMART_TITLE.to_owned(),
            _ => mix_title.unwrap_or_default(),
        }
    }

    /// Bring every collection's songs up to date (or only `kinds`), want the new
    /// songs and let go of the ones no collection holds any more. Emits
    /// `DownloadsChanged` when something changed.
    pub(super) fn reconcile_downloads(
        &self,
        p: &Profile,
        kinds: Option<&[&str]>,
    ) -> Result<bool, EncoreError> {
        let wants = |k: &str| kinds.is_none_or(|ks| ks.contains(&k));
        let has_mix = {
            let store = lock(&p.store)?;
            store
                .download_collections()?
                .iter()
                .any(|c| c.kind == "mix" && wants("mix"))
        };
        // The smart-picks model reads the store itself: before taking its lock.
        let mix_ids = if has_mix {
            Some(self.recs_model(p)?.2)
        } else {
            None
        };
        let mut changed = false;
        let store = lock(&p.store)?;
        for c in store.download_collections()? {
            if !wants(&c.kind) {
                continue;
            }
            match self.members_of(&store, &c.kind, &c.item, mix_ids.as_deref())? {
                Some(members) => {
                    if store.download_members(&c.kind, &c.item)? != members {
                        store.set_download_members(&c.kind, &c.item, &members)?;
                        changed = true;
                    }
                    let title = self.title_of(&store, &c.kind, &c.item, Some(c.title.clone()));
                    if !title.is_empty() && title != c.title {
                        store.upsert_download_collection(
                            &c.kind,
                            &c.item,
                            &title,
                            c.auto,
                            now_ms(),
                        )?;
                        changed = true;
                    }
                }
                None => {
                    store.remove_download_collection(&c.kind, &c.item)?;
                    changed = true;
                }
            }
        }
        changed |= self.settle_songs(&store)?;
        drop(store);
        if changed {
            self.refresh_playable(p)?;
            self.emit_downloads_changed();
        }
        Ok(changed)
    }

    /// Want every song a collection holds, let go of the rest, and keep covers
    /// only for albums with a saved song. True when something changed.
    fn settle_songs(&self, store: &Store) -> Result<bool, EncoreError> {
        let wanted = store.download_wanted()?;
        let rows = store.download_songs()?;
        let have: HashSet<i64> = rows.iter().map(|r| r.track_id).collect();
        let mut changed = false;
        let quality = self.download_quality(store)?;
        let now = now_ms();
        for id in wanted.difference(&have) {
            let Ok(t) = store.track(*id) else { continue };
            let server = store
                .source(t.source_id)
                .map(|s| s.server_id)
                .unwrap_or_default();
            store.queue_download_song(t.id, &file_key(&server, &t.source_item_id, quality), now)?;
            changed = true;
        }
        for r in rows.iter().filter(|r| !wanted.contains(&r.track_id)) {
            self.release_song(store, r)?;
            changed = true;
        }
        // Covers follow the albums that still have a saved song.
        let saved = store.saved_download_ids()?;
        let albums: HashSet<i64> = saved
            .iter()
            .filter_map(|id| store.track(*id).ok().and_then(|t| t.album_id))
            .collect();
        for album in store.download_cover_albums()? {
            if !albums.contains(&album)
                && let Some(key) = store.remove_download_cover(album)?
            {
                self.release_file(&key)?;
                changed = true;
            }
        }
        Ok(changed)
    }

    /// Let go of one song's download: its row, its file (when no other profile
    /// holds it) and a part fetched so far.
    fn release_song(
        &self,
        store: &Store,
        row: &encore_store::DownloadSongRow,
    ) -> Result<(), EncoreError> {
        store.remove_download_song(row.track_id)?;
        if row.state == "saved" {
            self.release_file(&row.file_key)?;
        }
        let _ = std::fs::remove_file(self.partial_path(&row.file_key));
        Ok(())
    }

    fn release_file(&self, key: &str) -> Result<(), EncoreError> {
        if let Some(name) = lock(&self.registry)?.release_download_file(key)? {
            let _ = std::fs::remove_file(self.downloads_dir().join(name));
        }
        Ok(())
    }

    /// Recount who holds each file from every profile's own list, and delete
    /// files no one holds (after a profile is removed, or "Remove all"). The
    /// registry's counts are only a fast path; this is the truth.
    pub(super) fn collect_download_garbage(&self) -> Result<(), EncoreError> {
        let mut held: HashMap<String, i64> = HashMap::new();
        let mut queued: HashSet<String> = HashSet::new();
        if let Ok(dirs) = std::fs::read_dir(self.profiles_dir()) {
            for d in dirs.flatten() {
                let db = d.path().join("library.sqlite");
                if !db.is_file() {
                    continue;
                }
                let Ok(store) = Store::open(&db) else {
                    continue;
                };
                for k in store.download_file_keys().unwrap_or_default() {
                    *held.entry(k).or_default() += 1;
                }
                for r in store.download_songs().unwrap_or_default() {
                    if r.state == "queued" {
                        queued.insert(r.file_key);
                    }
                }
            }
        }
        let reg = lock(&self.registry)?;
        let mut known: HashSet<String> = HashSet::new();
        for f in reg.download_files()? {
            let n = held.get(&f.file_key).copied().unwrap_or(0);
            if n != f.refs {
                reg.set_download_refs(&f.file_key, n)?;
            }
            if n == 0 {
                let _ = std::fs::remove_file(self.downloads_dir().join(&f.file_name));
            } else {
                known.insert(f.file_name);
            }
        }
        drop(reg);
        // Files on disk that nothing names (a crash between writing and
        // recording), and parts nobody is waiting for.
        if let Ok(files) = std::fs::read_dir(self.downloads_dir()) {
            for f in files.flatten() {
                let path = f.path();
                let name = f.file_name().to_string_lossy().into_owned();
                if path.is_file() && !known.contains(&name) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        if let Ok(parts) = std::fs::read_dir(self.downloads_dir().join(".partial")) {
            for f in parts.flatten() {
                let name = f.file_name().to_string_lossy().into_owned();
                let key = name.trim_end_matches(".part");
                if !queued.contains(key) {
                    let _ = std::fs::remove_file(f.path());
                }
            }
        }
        Ok(())
    }

    /// The first collection holding a song: the one its progress is shown under.
    fn collection_of(&self, store: &Store, track_id: i64) -> Option<(String, String, String)> {
        store
            .download_collections()
            .ok()?
            .into_iter()
            .find(|c| {
                store
                    .download_members(&c.kind, &c.item)
                    .ok()
                    .is_some_and(|m| m.contains(&track_id))
            })
            .map(|c| (c.kind, c.item, c.title))
    }

    /// Every collection, with how far it got.
    fn download_items(
        &self,
        store: &Store,
        active_song: Option<i64>,
    ) -> Result<Vec<DownloadedItem>, EncoreError> {
        let counts: HashMap<(String, String), encore_store::DownloadCounts> =
            store.download_counts()?.into_iter().collect();
        let errors: HashMap<i64, String> = store
            .download_songs()?
            .into_iter()
            .filter(|r| r.state == "failed")
            .filter_map(|r| r.error.map(|e| (r.track_id, e)))
            .collect();
        let mut out = Vec::new();
        for c in store.download_collections()? {
            let Some(target) = target_of(&c.kind, &c.item) else {
                continue;
            };
            let n = counts
                .get(&(c.kind.clone(), c.item.clone()))
                .cloned()
                .unwrap_or_default();
            let members = store.download_members(&c.kind, &c.item)?;
            let downloading = active_song.is_some_and(|a| members.contains(&a));
            let state = if n.total > 0 && n.saved == n.total {
                DownloadState::Saved
            } else if c.paused {
                DownloadState::Paused
            } else if downloading {
                DownloadState::Downloading
            } else if n.queued == 0 && n.failed > 0 {
                DownloadState::Failed
            } else {
                DownloadState::Waiting
            };
            let id = c.item.parse::<i64>().unwrap_or(0);
            let subtitle = match c.kind.as_str() {
                "album" => store.album(id).map(|a| a.artist).unwrap_or_default(),
                "song" => store.track(id).map(|t| t.artist).unwrap_or_default(),
                _ => String::new(),
            };
            out.push(DownloadedItem {
                target,
                title: c.title,
                subtitle,
                state,
                saved: n.saved,
                total: n.total,
                failed: n.failed,
                bytes: n.bytes.max(0) as u64,
                auto: c.auto,
                added_utc_ms: c.added_utc_ms,
                error: members.iter().find_map(|m| errors.get(m).cloned()),
            });
        }
        Ok(out)
    }

    fn settings_of(&self, store: &Store) -> Result<DownloadSettings, EncoreError> {
        let has = |kind: &str| -> Result<bool, EncoreError> {
            Ok(store.download_collection(kind, "")?.is_some())
        };
        Ok(DownloadSettings {
            quality: self.download_quality(store)?,
            wifi_only: self.wifi_only(store)?,
            limit_bytes: self.limit_bytes(store)?,
            remove_unplayed: store.setting(REMOVE_UNPLAYED_KEY)?.as_deref() == Some("1"),
            liked_songs: has("liked")?,
            smart_downloads: has("smart")?,
            cache_limit_bytes: self.cache_limit(store)?,
        })
    }

    /// Record the "downloaded" signal for songs the person chose (not Liked songs
    /// or Smart downloads, which the app keeps by itself). Nothing under Private
    /// listening, Guest or Learn from my listening off.
    fn note_download_signal(&self, p: &Profile, members: &[i64]) -> Result<(), EncoreError> {
        use crate::playlog::ListeningMode as M;
        if matches!(
            lock(&p.playback)?.log.mode(),
            M::Guest | M::LearningOff | M::Private
        ) {
            return Ok(());
        }
        let store = lock(&p.store)?;
        let keys: Vec<String> = members
            .iter()
            .filter_map(|id| store.track(*id).ok().map(|t| t.song_key))
            .collect();
        store.add_download_signals(&keys, now_ms())?;
        Ok(())
    }

    /// Smart downloads (section 21): pinned mixes' songs, taken in turn from each
    /// mix, inside the budget.
    fn choose_smart(&self, p: &Profile) -> Result<u32, EncoreError> {
        let ids = self.recs_model(p)?.2;
        let store = lock(&p.store)?;
        if store.download_collection("smart", "")?.is_none() {
            return Ok(0);
        }
        let quality = self.download_quality(&store)?;
        let budget = tokens().downloads.smart_budget_bytes;
        let lists: Vec<Vec<i64>> = self
            .pinned(&store)
            .iter()
            .map(|m| {
                m.songs
                    .iter()
                    .filter_map(|s| ids.get(&s.song_key).copied())
                    .collect()
            })
            .collect();
        let mut chosen: Vec<i64> = Vec::new();
        let mut seen = HashSet::new();
        let mut used = 0u64;
        let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
        'outer: for i in 0..longest {
            for list in &lists {
                let Some(&id) = list.get(i) else { continue };
                if !seen.insert(id) {
                    continue;
                }
                let size = match store.download_song(id)? {
                    Some(r) if r.state == "saved" => r.bytes.max(0) as u64,
                    _ => store
                        .track(id)
                        .map(|t| {
                            estimate_bytes(t.duration_ms as i64, t.container.as_deref(), quality)
                        })
                        .unwrap_or(0),
                };
                if used + size > budget {
                    break 'outer;
                }
                used += size;
                chosen.push(id);
            }
        }
        store.set_download_members("smart", "", &chosen)?;
        Ok(chosen.len() as u32)
    }
}

/// The run's shared counters, for progress.
struct RunCounters {
    done: u32,
    last_emit: Option<Instant>,
}

impl EncoreCore {
    async fn run_inner(inner: Arc<Inner>, free_bytes: u64) -> Result<DownloadRun, EncoreError> {
        let Ok(p) = inner.profile() else {
            return Ok(DownloadRun {
                saved: 0,
                failed: 0,
                remaining: 0,
                stop: DownloadStop::NoProfile,
            });
        };
        if inner.can_download(&p).is_err() {
            return Ok(DownloadRun {
                saved: 0,
                failed: 0,
                remaining: 0,
                stop: DownloadStop::NoProfile,
            });
        }
        let cancel = {
            let mut t = lock(&inner.transfers)?;
            if t.running {
                None
            } else {
                t.running = true;
                t.cancel = Arc::new(AtomicBool::new(false));
                Some(t.cancel.clone())
            }
        };
        let Some(cancel) = cancel else {
            return Ok(DownloadRun {
                saved: 0,
                failed: 0,
                remaining: lock(&p.store)?.download_queued_count()?,
                stop: DownloadStop::AlreadyRunning,
            });
        };
        let _guard = RunGuard(&inner);
        std::fs::create_dir_all(inner.downloads_dir().join(".partial")).map_err(|e| {
            EncoreError::Storage {
                reason: format!("couldn't make the downloads folder ({e})"),
            }
        })?;
        inner.reconcile_downloads(&p, None)?;
        let d = &tokens().downloads;
        let mut saved = 0u32;
        let mut failed = 0u32;
        let mut written = 0u64;
        let mut tried: HashSet<i64> = HashSet::new();
        let counters = Arc::new(Mutex::new(RunCounters {
            done: 0,
            last_emit: None,
        }));
        let stop = loop {
            if cancel.load(Ordering::SeqCst) {
                break DownloadStop::Stopped;
            }
            // The profile switched: this run was for the other one.
            match inner.profile() {
                Ok(now) if Arc::ptr_eq(&now, &p) => {}
                _ => break DownloadStop::Stopped,
            }
            let (online, network) = {
                let c = lock(&inner.connectivity)?;
                (c.online, c.network)
            };
            if !online {
                break DownloadStop::Offline;
            }
            let (next, quality_now, wifi_only, limit, used) = {
                let store = lock(&p.store)?;
                let next = store
                    .download_candidates(64)?
                    .into_iter()
                    .find(|(id, _)| !tried.contains(id));
                (
                    next,
                    inner.download_quality(&store)?,
                    inner.wifi_only(&store)?,
                    inner.limit_bytes(&store)?,
                    store.download_bytes()?.max(0) as u64,
                )
            };
            let _ = quality_now;
            let Some((track_id, key)) = next else {
                break DownloadStop::Done;
            };
            if wifi_only && network == Network::Cellular {
                break DownloadStop::WaitingForWifi;
            }
            let quality = quality_in_key(&key);
            let (track, item, source) = {
                let store = lock(&p.store)?;
                let Ok(track) = store.track(track_id) else {
                    // It left the library: let it go.
                    if let Some(row) = store.download_song(track_id)? {
                        inner.release_song(&store, &row)?;
                    }
                    continue;
                };
                let item = inner.collection_of(&store, track_id);
                (track.clone(), item, inner.source(&p, track.source_id))
            };
            let source = match source {
                Ok(s) => s,
                Err(_) => break DownloadStop::SignInNeeded,
            };
            // Another profile has it already: hold it too.
            let existing = lock(&inner.registry)?.download_file(&key)?;
            if let Some(f) = existing
                && inner.downloads_dir().join(&f.file_name).is_file()
            {
                lock(&inner.registry)?.hold_download_file(&key, &f.file_name, f.bytes)?;
                lock(&p.store)?.set_download_saved(track_id, f.bytes, now_ms())?;
                saved += 1;
                if let Ok(mut c) = counters.lock() {
                    c.done += 1;
                }
                Self::keep_cover(&inner, &p, &track, source.as_ref()).await;
                inner.refresh_playable(&p)?;
                inner.emit_downloads_changed();
                continue;
            }
            let estimate = estimate_bytes(
                track.duration_ms as i64,
                track.container.as_deref(),
                quality,
            );
            if let Some(limit) = limit
                && used + estimate > limit
            {
                break DownloadStop::StorageLimit;
            }
            if free_bytes > 0 && free_bytes.saturating_sub(written) < estimate + d.keep_free_bytes {
                break DownloadStop::LowSpace;
            }
            let part = inner.partial_path(&key);
            let resume_from = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
            let request = inner.stream_request(&*lock(&p.store)?, quality)?;
            let (item_kind, item_ref, item_title) =
                item.unwrap_or_else(|| ("song".into(), track_id.to_string(), track.title.clone()));
            let (item_saved, item_total) = {
                let store = lock(&p.store)?;
                let counts: HashMap<(String, String), encore_store::DownloadCounts> =
                    store.download_counts()?.into_iter().collect();
                let n = counts
                    .get(&(item_kind.clone(), item_ref.clone()))
                    .cloned()
                    .unwrap_or_default();
                (n.saved, n.total)
            };
            let songs_total = {
                let done = counters.lock().map(|c| c.done).unwrap_or(0);
                done + lock(&p.store)?.download_queued_count()?
            };
            let progress_inner = inner.clone();
            let progress_counters = counters.clone();
            let song_title = track.title.clone();
            let every = std::time::Duration::from_millis(d.progress_every_ms);
            let progress = move |bytes: u64, total: Option<u64>| {
                let Ok(mut c) = progress_counters.lock() else {
                    return;
                };
                let now = Instant::now();
                let due = c.last_emit.is_none_or(|t| now.duration_since(t) >= every)
                    || total.is_some_and(|t| bytes >= t);
                if !due {
                    return;
                }
                c.last_emit = Some(now);
                let info = DownloadProgress {
                    item_title: item_title.clone(),
                    item_saved,
                    item_total,
                    song_id: track_id,
                    song_title: song_title.clone(),
                    songs_done: c.done,
                    songs_total,
                    bytes_done: bytes,
                    bytes_total: total,
                };
                drop(c);
                if let Ok(mut t) = progress_inner.transfers.lock() {
                    t.active = Some(info.clone());
                }
                let mut e = CoreEvent::of(EventKind::DownloadProgress);
                e.download = Some(info);
                progress_inner.emit(e);
            };
            // The first moment this song moves: its item shows the ring.
            progress(resume_from, None);
            inner.emit_downloads_changed();
            let got = source
                .download(
                    &track.source_item_id,
                    &request,
                    &part,
                    resume_from,
                    &progress,
                    &cancel,
                )
                .await;
            match got {
                Ok(dl) => {
                    let complete = dl.expected_bytes.is_none_or(|e| e == dl.bytes) && dl.bytes > 0;
                    if !complete {
                        let _ = std::fs::remove_file(&part);
                        let give_up =
                            Self::count_failure(&p, track_id, "The download was cut short")?;
                        tried.insert(track_id);
                        if give_up {
                            failed += 1;
                        }
                        continue;
                    }
                    let ext = extension(dl.content_type.as_deref(), track.container.as_deref());
                    let name = format!("{key}.{ext}");
                    let dest = inner.downloads_dir().join(&name);
                    let _ = std::fs::remove_file(&dest);
                    if let Err(e) = std::fs::rename(&part, &dest) {
                        let _ = std::fs::remove_file(&part);
                        return Err(EncoreError::Storage {
                            reason: format!("couldn't keep the downloaded file ({e})"),
                        });
                    }
                    written += dl.bytes;
                    lock(&inner.registry)?.hold_download_file(&key, &name, dl.bytes as i64)?;
                    lock(&p.store)?.set_download_saved(track_id, dl.bytes as i64, now_ms())?;
                    saved += 1;
                    if let Ok(mut c) = counters.lock() {
                        c.done += 1;
                    }
                    Self::keep_cover(&inner, &p, &track, source.as_ref()).await;
                    inner.refresh_playable(&p)?;
                    inner.emit_downloads_changed();
                }
                Err(SourceError::Cancelled) => break DownloadStop::Stopped,
                Err(SourceError::SessionExpired) => break DownloadStop::SignInNeeded,
                Err(SourceError::Unreachable(_)) => break DownloadStop::ServerUnreachable,
                Err(SourceError::Storage(_)) => break DownloadStop::LowSpace,
                Err(SourceError::NotFound) => {
                    let _ = std::fs::remove_file(&part);
                    lock(&p.store)?.set_download_attempt_failed(
                        track_id,
                        "This song is no longer on the server",
                        true,
                    )?;
                    tried.insert(track_id);
                    failed += 1;
                    inner.emit_downloads_changed();
                }
                Err(e) => {
                    let give_up = Self::count_failure(&p, track_id, &e.to_string())?;
                    tried.insert(track_id);
                    if give_up {
                        failed += 1;
                    }
                    inner.emit_downloads_changed();
                }
            }
        };
        let remaining = lock(&p.store)?.download_queued_count()?;
        if let Ok(mut t) = lock(&inner.transfers) {
            t.active = None;
            t.last_stop = (remaining > 0 && stop != DownloadStop::Done).then_some(stop);
        }
        inner.emit_downloads_changed();
        Ok(DownloadRun {
            saved,
            failed,
            remaining,
            stop,
        })
    }

    /// One more failed try; true when it gave up (after `downloads.maxAttempts`).
    fn count_failure(p: &Profile, track_id: i64, reason: &str) -> Result<bool, EncoreError> {
        let store = lock(&p.store)?;
        let attempts = store
            .download_song(track_id)?
            .map(|r| r.attempts)
            .unwrap_or(0)
            + 1;
        let give_up = attempts >= tokens().downloads.max_attempts;
        store.set_download_attempt_failed(track_id, reason, give_up)?;
        Ok(give_up)
    }

    /// Keep the album's cover with its first saved song (best effort: a song
    /// plays offline without it).
    async fn keep_cover(
        inner: &Arc<Inner>,
        p: &Arc<Profile>,
        track: &encore_store::TrackRow,
        source: &dyn MediaSource,
    ) {
        let Some(album_id) = track.album_id else {
            return;
        };
        let found = (|| -> Result<Option<(String, String)>, EncoreError> {
            let store = lock(&p.store)?;
            if store.download_cover(album_id)?.is_some() {
                return Ok(None);
            }
            let album = store.album(album_id)?;
            if !album.has_artwork {
                return Ok(None);
            }
            let server = store.source(album.source_id)?.server_id;
            Ok(Some((
                cover_key(
                    &server,
                    &album.source_item_id,
                    tokens().downloads.cover_size_px,
                ),
                album.source_item_id,
            )))
        })();
        let Ok(Some((key, item))) = found else { return };
        let existing = lock(&inner.registry)
            .ok()
            .and_then(|r| r.download_file(&key).ok().flatten());
        let (name, bytes) = match existing {
            Some(f) if inner.downloads_dir().join(&f.file_name).is_file() => (f.file_name, f.bytes),
            _ => {
                let Ok(Some(data)) = source
                    .artwork_bytes(&item, tokens().downloads.cover_size_px)
                    .await
                else {
                    return;
                };
                let name = format!("{key}.img");
                if std::fs::write(inner.downloads_dir().join(&name), &data).is_err() {
                    return;
                }
                (name, data.len() as i64)
            }
        };
        let held =
            lock(&inner.registry).and_then(|r| Ok(r.hold_download_file(&key, &name, bytes)?));
        if held.is_ok()
            && let Ok(store) = lock(&p.store)
        {
            let _ = store.set_download_cover(album_id, &key, bytes);
        }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Keep a song, album, playlist or mix on this device (board 13, item 3;
    /// board 08). Its songs wait for `run_downloads`. Liked songs and Smart
    /// downloads are turned on in `set_download_settings`.
    pub fn download(&self, target: DownloadTarget) -> Result<DownloadedItem, EncoreError> {
        let p = self.inner.profile()?;
        self.inner.can_download(&p)?;
        if matches!(target.kind, DownloadKind::Smart) {
            return Err(EncoreError::NotAllowed {
                reason: "Smart downloads are turned on in Settings".into(),
            });
        }
        let (kind, item) = key_of(&target)?;
        // A downloaded mix is pinned, so it keeps its songs and its page (board 23,
        // item 4) instead of being replaced at 6:00.
        if kind == "mix" {
            let pinned = {
                let store = lock(&p.store)?;
                self.inner.pinned(&store).iter().any(|m| m.id == item)
            };
            if !pinned {
                self.pin_mix(item.clone(), true)?;
            }
        }
        let mix_ids = if kind == "mix" {
            Some(self.inner.recs_model(&p)?.2)
        } else {
            None
        };
        let members = {
            let store = lock(&p.store)?;
            let mix_title = if kind == "mix" {
                self.inner.find_mix(&store, &item).map(|(m, _, _)| m.title)
            } else {
                None
            };
            let members = self
                .inner
                .members_of(&store, kind, &item, mix_ids.as_deref())?
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "That isn't in your library any more".into(),
                })?;
            let title = self.inner.title_of(&store, kind, &item, mix_title);
            store.upsert_download_collection(
                kind,
                &item,
                &title,
                matches!(target.kind, DownloadKind::LikedSongs),
                now_ms(),
            )?;
            store.set_download_paused(kind, &item, false)?;
            store.set_download_members(kind, &item, &members)?;
            self.inner.settle_songs(&store)?;
            members
        };
        if !matches!(target.kind, DownloadKind::LikedSongs) {
            self.inner.note_download_signal(&p, &members)?;
        }
        self.inner.refresh_playable(&p)?;
        self.inner.emit_downloads_changed();
        let active = lock(&self.inner.transfers)?
            .active
            .as_ref()
            .map(|a| a.song_id);
        let store = lock(&p.store)?;
        self.inner
            .download_items(&store, active)?
            .into_iter()
            .find(|i| {
                key_of(&i.target)
                    .ok()
                    .is_some_and(|(k, it)| k == kind && it == item)
            })
            .ok_or_else(|| EncoreError::internal("download"))
    }

    /// Stop keeping it (board 13: Remove download). A song that is also part of a
    /// downloaded album or playlist stays, and the reason says which.
    pub fn remove_download(&self, target: DownloadTarget) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let (kind, item) = key_of(&target)?;
        let still_in = {
            let store = lock(&p.store)?;
            store.remove_download_collection(kind, &item)?;
            self.inner.settle_songs(&store)?;
            if kind == "song" {
                self.inner
                    .collection_of(&store, target.id)
                    .map(|(_, _, title)| title)
            } else {
                None
            }
        };
        self.inner.refresh_playable(&p)?;
        self.inner.emit_downloads_changed();
        match still_in {
            // A song kept as part of an album or playlist stays with it.
            Some(title) => Err(EncoreError::NotAllowed {
                reason: format!("It's saved as part of {title}. Remove that download instead"),
            }),
            None => Ok(()),
        }
    }

    /// Board 03: "clear downloads" (after a confirm sheet). Returns how many
    /// collections went.
    pub fn remove_all_downloads(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        if let Ok(t) = lock(&self.inner.transfers) {
            t.cancel.store(true, Ordering::SeqCst);
        }
        let n = {
            let store = lock(&p.store)?;
            let all = store.download_collections()?;
            for c in &all {
                store.remove_download_collection(&c.kind, &c.item)?;
            }
            self.inner.settle_songs(&store)?;
            all.len() as u32
        };
        self.inner.collect_download_garbage()?;
        self.inner.refresh_playable(&p)?;
        self.inner.emit_downloads_changed();
        Ok(n)
    }

    /// Pause or resume one item's downloads (board 06, item 5: tap the ring).
    pub fn pause_download(&self, target: DownloadTarget, paused: bool) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let (kind, item) = key_of(&target)?;
        lock(&p.store)?.set_download_paused(kind, &item, paused)?;
        self.inner.emit_downloads_changed();
        Ok(())
    }

    /// Try every failed song again (board 06, item 5: tap the warning dot).
    pub fn retry_failed_downloads(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let n = lock(&p.store)?.retry_failed_downloads()?;
        self.inner.emit_downloads_changed();
        Ok(n)
    }

    /// Badges, rings and what dims offline, for every list (cheap).
    pub fn download_marks(&self) -> Result<DownloadMarks, EncoreError> {
        let p = self.inner.profile()?;
        let (online, cached) = {
            let c = lock(&self.inner.connectivity)?;
            (c.online, c.cached_ids.clone())
        };
        let active = lock(&self.inner.transfers)?
            .active
            .as_ref()
            .map(|a| a.song_id);
        let store = lock(&p.store)?;
        let mut saved = Vec::new();
        let mut pending = Vec::new();
        let mut failed = Vec::new();
        for r in store.download_songs()? {
            match r.state.as_str() {
                "saved" => saved.push(r.track_id),
                "failed" => failed.push(r.track_id),
                _ => pending.push(r.track_id),
            }
        }
        let mut playable: Vec<i64> = saved.clone();
        playable.extend(cached.iter().copied().filter(|id| !saved.contains(id)));
        Ok(DownloadMarks {
            items: self.inner.download_items(&store, active)?,
            saved_song_ids: saved,
            pending_song_ids: pending,
            failed_song_ids: failed,
            playable_offline_song_ids: playable,
            online,
            active_song_id: active,
        })
    }

    /// The Downloads page (board 17, item 3).
    pub fn downloads_overview(&self) -> Result<DownloadsOverview, EncoreError> {
        let p = self.inner.profile()?;
        let (active, stop) = {
            let t = lock(&self.inner.transfers)?;
            (t.active.clone(), t.last_stop)
        };
        let store = lock(&p.store)?;
        let songs = store.download_songs()?;
        Ok(DownloadsOverview {
            used_bytes: store.download_bytes()?.max(0) as u64,
            song_count: songs.iter().filter(|r| r.state == "saved").count() as u32,
            items: self
                .inner
                .download_items(&store, active.as_ref().map(|a| a.song_id))?,
            queued: store.download_queued_count()?,
            failed: songs.iter().filter(|r| r.state == "failed").count() as u32,
            active,
            stop,
            settings: self.inner.settings_of(&store)?,
        })
    }

    pub fn download_settings(&self) -> Result<DownloadSettings, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        self.inner.settings_of(&store)
    }

    /// Change the Downloads settings (changes apply at once, board 17). Liked
    /// songs and Smart downloads add or remove their collections; a new quality
    /// applies to songs downloaded from now on.
    pub fn set_download_settings(&self, settings: DownloadSettings) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        if settings.liked_songs || settings.smart_downloads {
            self.inner.can_download(&p)?;
        }
        let quality = match settings.quality {
            StreamQuality::DataSaver => StreamQuality::Normal,
            q => q,
        };
        let smart_turned_on = {
            let store = lock(&p.store)?;
            store.set_setting(QUALITY_KEY, quality_name(quality))?;
            store.set_setting(WIFI_ONLY_KEY, if settings.wifi_only { "1" } else { "0" })?;
            store.set_setting(
                LIMIT_KEY,
                &settings
                    .limit_bytes
                    .filter(|n| *n > 0)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            )?;
            store.set_setting(
                REMOVE_UNPLAYED_KEY,
                if settings.remove_unplayed { "1" } else { "0" },
            )?;
            store.set_setting(CACHE_LIMIT_KEY, &settings.cache_limit_bytes.to_string())?;
            let now = now_ms();
            if settings.liked_songs {
                store.upsert_download_collection("liked", "", LIKED_TITLE, true, now)?;
            } else {
                store.remove_download_collection("liked", "")?;
            }
            let had_smart = store.download_collection("smart", "")?.is_some();
            if settings.smart_downloads {
                store.upsert_download_collection("smart", "", SMART_TITLE, true, now)?;
            } else {
                store.remove_download_collection("smart", "")?;
            }
            settings.smart_downloads && !had_smart
        };
        if smart_turned_on {
            self.inner.choose_smart(&p)?;
        }
        self.inner.reconcile_downloads(&p, None)?;
        self.inner.emit_downloads_changed();
        Ok(())
    }

    /// Fetch what waits, one song at a time, until done or blocked (Wi-Fi only,
    /// the limit, low space, offline, sign-in). `free_bytes` is the free space on
    /// the device now (0 if unknown); `network` is the current one. Call it from
    /// the platform's background job; `stop_downloads` ends it, keeping what was
    /// fetched. Async.
    pub async fn run_downloads(
        &self,
        network: Network,
        free_bytes: u64,
    ) -> Result<DownloadRun, EncoreError> {
        if let Ok(mut c) = lock(&self.inner.connectivity) {
            c.network = network;
        }
        let inner = self.inner.clone();
        self.spawn(async move { Self::run_inner(inner, free_bytes).await })
            .await
    }

    /// End the run in progress after the chunk it is writing (the job was
    /// stopped). The part fetched so far is kept and resumed next time.
    pub fn stop_downloads(&self) {
        if let Ok(t) = lock(&self.inner.transfers) {
            t.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Smart downloads' nightly pick (section 21): call before `run_downloads`
    /// from the nightly job. Returns how many songs it holds.
    pub fn refresh_smart_downloads(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let n = self.inner.choose_smart(&p)?;
        self.inner.reconcile_downloads(&p, Some(&["smart"]))?;
        Ok(n)
    }

    /// "Remove downloads not played for 30 days" (board 17, item 3), per album,
    /// playlist, mix or song, never Liked songs or Smart downloads. Call it at
    /// launch and from the jobs; it does nothing when the setting is off.
    pub fn remove_unplayed_downloads(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let n = {
            let store = lock(&p.store)?;
            if store.setting(REMOVE_UNPLAYED_KEY)?.as_deref() != Some("1") {
                return Ok(0);
            }
            let cutoff =
                now_ms() - i64::from(tokens().downloads.remove_unplayed_after_days) * DAY_MS;
            let mut n = 0;
            for c in store.download_collections()? {
                if c.auto {
                    continue;
                }
                let last = store
                    .download_last_played(&c.kind, &c.item)?
                    .unwrap_or(c.added_utc_ms)
                    .max(c.added_utc_ms);
                if last < cutoff {
                    store.remove_download_collection(&c.kind, &c.item)?;
                    n += 1;
                }
            }
            if n > 0 {
                self.inner.settle_songs(&store)?;
            }
            n
        };
        if n > 0 {
            self.inner.refresh_playable(&p)?;
            self.inner.emit_downloads_changed();
        }
        Ok(n)
    }

    /// What the network is now (call on every change, and at start). Offline,
    /// only saved songs and the player's fully cached ones (`cached_keys`, the
    /// `StreamInfo.cache_key`s it holds whole) can play: the queue passes over the
    /// rest and taps on them are refused. Works with no profile open.
    pub fn set_connectivity(
        &self,
        online: bool,
        network: Network,
        cached_keys: Vec<String>,
    ) -> Result<(), EncoreError> {
        let profile = self.inner.profile().ok();
        let ids = match &profile {
            Some(p) => self.inner.cached_ids_of(&*lock(&p.store)?, &cached_keys),
            None => HashSet::new(),
        };
        let changed = {
            let mut c = lock(&self.inner.connectivity)?;
            let changed = c.online != online || c.cached_ids != ids;
            c.online = online;
            c.network = network;
            c.cached_keys = cached_keys;
            c.cached_ids = ids;
            changed
        };
        if let Some(p) = profile {
            self.inner.refresh_playable(&p)?;
        }
        if changed {
            self.inner.emit_downloads_changed();
            // Phase 9, WP2: the widgets' "Server not reachable" follows.
            self.inner.widgets_soon();
        }
        Ok(())
    }

    /// Board 07's Downloaded scope: the same search over what can play offline.
    pub fn search_downloaded(&self, query: String) -> Result<SearchResults, EncoreError> {
        let mut r = self.search(query)?;
        let p = self.inner.profile()?;
        let cached = lock(&self.inner.connectivity)?.cached_ids.clone();
        let saved = lock(&p.store)?.saved_download_ids()?;
        let playable: HashSet<i64> = saved.union(&cached).copied().collect();
        let store = lock(&p.store)?;
        let albums: HashSet<i64> = playable
            .iter()
            .filter_map(|id| store.track(*id).ok().and_then(|t| t.album_id))
            .collect();
        let playlists: HashSet<i64> = store
            .download_collections()?
            .into_iter()
            .filter(|c| c.kind == "playlist")
            .filter_map(|c| c.item.parse().ok())
            .collect();
        let artists: HashSet<String> = playable
            .iter()
            .filter_map(|id| store.track(*id).ok().map(|t| t.artist.to_lowercase()))
            .collect();
        drop(store);
        r.songs.retain(|s| playable.contains(&s.id));
        r.albums.retain(|a| albums.contains(&a.id));
        r.playlists.retain(|pl| playlists.contains(&pl.id));
        r.artists
            .retain(|a| artists.iter().any(|n| n.contains(&a.name.to_lowercase())));
        r.lyrics.retain(|l| playable.contains(&l.song.id));
        let keep_top = r.top.as_ref().is_none_or(|t| {
            t.song.as_ref().is_some_and(|s| playable.contains(&s.id))
                || t.album.as_ref().is_some_and(|a| albums.contains(&a.id))
                || t.playlist
                    .as_ref()
                    .is_some_and(|pl| playlists.contains(&pl.id))
                || t.artist
                    .as_ref()
                    .is_some_and(|a| artists.iter().any(|n| n.contains(&a.name.to_lowercase())))
        });
        if !keep_top {
            r.top = None;
        }
        Ok(r)
    }
}

/// Board 06, item 5: "Not downloaded", with what to do about it.
pub(super) fn not_downloaded() -> EncoreError {
    EncoreError::NotAllowed {
        reason: "Not downloaded. It plays when you're back online".into(),
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn keys_carry_their_quality_and_never_a_token() {
        let k = file_key("srv", "item", StreamQuality::High);
        assert!(k.ends_with("-high"));
        assert_eq!(quality_in_key(&k), StreamQuality::High);
        assert_ne!(k, file_key("srv", "item", StreamQuality::Original));
        assert_ne!(k, file_key("other", "item", StreamQuality::High));
        assert!(cover_key("srv", "album", 1200).ends_with("-cover"));
    }

    #[test]
    fn extensions_come_from_the_reply_then_the_container() {
        assert_eq!(extension(Some("audio/flac"), None), "flac");
        assert_eq!(
            extension(Some("audio/mpeg; charset=x"), Some("flac")),
            "mp3"
        );
        assert_eq!(
            extension(Some("application/octet-stream"), Some("M4A")),
            "m4a"
        );
        assert_eq!(
            extension(None, Some("../evil")),
            "audio",
            "a container is never a path"
        );
        assert_eq!(extension(None, None), "audio");
    }

    #[test]
    fn estimates_follow_the_quality() {
        let flac = estimate_bytes(240_000, Some("flac"), StreamQuality::Original);
        let mp3 = estimate_bytes(240_000, Some("mp3"), StreamQuality::Original);
        let normal = estimate_bytes(240_000, Some("flac"), StreamQuality::Normal);
        assert!(flac > mp3 && mp3 > normal, "{flac} {mp3} {normal}");
        assert_eq!(normal, 240 * 160 * 1000 / 8);
    }

    #[test]
    fn targets_round_trip_through_their_row_keys() {
        for t in [
            DownloadTarget {
                kind: DownloadKind::Album,
                id: 7,
                mix_id: None,
            },
            DownloadTarget {
                kind: DownloadKind::Mix,
                id: 0,
                mix_id: Some("m1".into()),
            },
            DownloadTarget {
                kind: DownloadKind::LikedSongs,
                id: 0,
                mix_id: None,
            },
        ] {
            let (k, i) = key_of(&t).unwrap();
            assert_eq!(target_of(k, &i), Some(t));
        }
        assert!(
            key_of(&DownloadTarget {
                kind: DownloadKind::Mix,
                id: 0,
                mix_id: None
            })
            .is_err()
        );
    }
}
