//! Widget snapshots and `play_mix` (Phase 9, WP2; PLAN-UX section 20; board 34,
//! items 2, 3 and 10; docs/core/CORE-API.md, "Widgets").
//!
//! Home-screen widgets never open the database and never call the network: they
//! read one small file per profile that the core writes while the app runs,
//! `<data_dir>/widgets/<profile_id>/snapshot.json`, with cover thumbnails beside
//! it in `art/`. The rules:
//!
//! - The snapshot is built from local state only (the queue, the play log, the
//!   pinned mixes and today's stored set), never through `home()`, which may ask
//!   the companion server and rotates the set.
//! - Writes are atomic (a temporary file, then a rename) and debounced: a burst of
//!   changes within about a second is one write. Play state is written on start,
//!   pause, resume, seek and song change, never on progress ticks.
//! - Covers are about 256 px, fetched through the core's own client by the
//!   debounced writer (that is the app's network use, not the widget's), stored as
//!   the server sends them, and deleted once no snapshot names them.
//! - Each song, mix and album carries its album's palette (the colours the app draws
//!   that album's gradient from). Building a snapshot only reads palettes already in
//!   the database; the debounced writer makes the missing ones, once per cover
//!   (one small PNG, the same one `album_palette` uses), while the server is
//!   reachable, and the snapshot is written again with them. Palettes live in the
//!   database, never in the widget folder.
//! - Privacy: a profile with a PIN writes `locked` and Guest writes `guest`, both
//!   with no titles and no covers, at once (not after the debounce). Private
//!   listening and Learning off record no plays, so last played and the recent
//!   albums stay as they were; nothing plays "now" for the widget either. A
//!   removed profile's folder is deleted.
//!
//! Lock order: the write lock first, then the profile, the registry, playback,
//! the store and the live state, each taken on its own.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use encore_sources::MediaSource;
use encore_store::Store;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::profiles::SESSION_ENDED_KEY;
use super::{EncoreCore, Inner, Profile, checked_profile_id, lock, now_ms};
use crate::api::types::*;
use crate::playlog::ListeningMode as Mode;

/// The file format's version. A widget that finds a higher one says Open Encore.
pub(super) const SCHEMA: u32 = 1;
/// A burst of changes inside this is one write.
const DEBOUNCE: Duration = Duration::from_secs(1);
/// Cover thumbnails: widgets draw them at most about this big.
const COVER_PX: u32 = 256;
const SNAPSHOT: &str = "snapshot.json";
const ART: &str = "art";
/// The Recently played widget's 4×3 shows 8.
const RECENT_ALBUMS: usize = 8;
/// Plays read to find them (a long album run is one album).
const RECENT_PLAYS_READ: u32 = 300;
/// Pins are capped at 10 by `pin_mix`.
const MAX_PINNED: usize = 10;
/// Songs looked at for a mix's cover.
const MIX_COVER_LOOKS: usize = 12;
/// Palettes made in one debounced write (a long list of new albums takes a few).
const PALETTES_PER_ROUND: usize = 20;
/// An album whose palette failed isn't asked for again for this long.
const PALETTE_RETRY_MS: i64 = 10 * 60_000;

/// The writer's state, one per core (`Inner::widgets`).
#[derive(Debug, Default)]
pub(super) struct Widgets {
    /// Wakes the debounced writer (`start_writer`).
    wake: Arc<tokio::sync::Notify>,
    /// Something changed since the last debounced write.
    dirty: AtomicBool,
    /// One write at a time: the debounced writer, the immediate privacy writes and
    /// a profile's removal.
    write: Mutex<()>,
    /// The profile and the content last written, without its time: a snapshot
    /// that didn't change isn't written again (no disk write, no redraw).
    last: Mutex<Option<(String, Value)>>,
    /// Albums whose palette couldn't be made, and when: not asked for again for
    /// [`PALETTE_RETRY_MS`], so a server that refuses one cover isn't asked on every write.
    failed: Mutex<HashMap<i64, i64>>,
}

/// A cover the snapshot names but `art/` doesn't hold yet.
#[derive(Debug, Clone)]
struct Wanted {
    name: String,
    album_id: i64,
    source_id: i64,
    item_id: String,
}

/// Covers while a snapshot is built: saved ones are named, missing ones are left
/// out and wanted (the debounced writer fetches them, then writes again).
struct Covers {
    saved: HashSet<String>,
    wanted: Vec<Wanted>,
    /// Albums whose palette is missing or was made for an older cover (the
    /// debounced writer makes them).
    palettes_wanted: Vec<i64>,
    /// Albums whose palette failed lately, with the time (`Widgets::failed`).
    failed: HashMap<i64, i64>,
    now: i64,
}

impl Covers {
    fn of(&mut self, store: &Store, album_id: Option<i64>) -> Option<String> {
        let w = cover_of(store, album_id?)?;
        if self.saved.contains(&w.name) {
            return Some(w.name);
        }
        if !self.wanted.iter().any(|x| x.name == w.name) {
            self.wanted.push(w);
        }
        None
    }

    /// An album's palette as the database has it. One made for an older cover still
    /// beats none; a missing or outdated one is noted for the writer. An album with no
    /// cover has no palette and nothing to fetch. Local reads only.
    fn palette(&mut self, store: &Store, album_id: Option<i64>) -> Option<WidgetPalette> {
        let id = album_id?;
        let (_, _, tag) = store.artwork_tag("album", id).ok()??;
        let tag = tag?;
        let cached = store.cached_palette("album", id).ok().flatten();
        let current = cached
            .as_ref()
            .is_some_and(|c| c.artwork_tag.as_ref() == Some(&tag));
        if !current {
            let failed_lately = self
                .failed
                .get(&id)
                .is_some_and(|at| self.now - at < PALETTE_RETRY_MS);
            if !failed_lately && !self.palettes_wanted.contains(&id) {
                self.palettes_wanted.push(id);
            }
        }
        cached.and_then(|c| widget_palette(c.palette_json.as_deref()?))
    }
}

/// A stored palette (the JSON `make_palette` saves) as the widget's record.
fn widget_palette(json: &str) -> Option<WidgetPalette> {
    let p: crate::palette::Palette = serde_json::from_str(json).ok()?;
    Some(ArtworkPalette::from(p).into())
}

/// An album's cover file: its source, its ID there, its artwork tag and the size,
/// hashed, so a changed cover gets a new file and the name says nothing of the
/// album. None when the album has no cover.
fn cover_of(store: &Store, album_id: i64) -> Option<Wanted> {
    let (source_id, item_id, tag) = store.artwork_tag("album", album_id).ok()??;
    let tag = tag?;
    let d =
        Sha256::digest(format!("{source_id}\u{1f}{item_id}\u{1f}{tag}\u{1f}{COVER_PX}").as_bytes());
    let hex: String = d[..16].iter().map(|b| format!("{b:02x}")).collect();
    Some(Wanted {
        name: format!("{hex}.img"),
        album_id,
        source_id,
        item_id,
    })
}

/// The library's song for a play's song key (the first copy).
fn first_track(store: &Store, song_key: &str) -> Option<encore_store::TrackRow> {
    let id = *store.track_ids_for_key(song_key).ok()?.first()?;
    store.track(id).ok()
}

fn widget_song(
    store: &Store,
    t: encore_store::TrackRow,
    now_playing: bool,
    covers: &mut Covers,
) -> WidgetSong {
    WidgetSong {
        cover: covers.of(store, t.album_id),
        palette: covers.palette(store, t.album_id),
        song_id: Some(t.id),
        title: t.title,
        artist: t.artist,
        album_id: t.album_id,
        album_title: t.album_title,
        duration_ms: t.duration_ms as i64,
        now_playing,
    }
}

/// The last song in the play log (this profile's, from any of its devices).
fn last_played(store: &Store, covers: &mut Covers) -> Result<Option<WidgetSong>, EncoreError> {
    let Some(e) = store.recent_play_events(1)?.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(match first_track(store, &e.song_key) {
        Some(t) => widget_song(store, t, false, covers),
        // It left the library: the log's own words, with no cover.
        None => WidgetSong {
            song_id: None,
            title: e.title,
            artist: e.artist,
            album_id: None,
            album_title: e.album,
            cover: None,
            palette: None,
            duration_ms: e.duration_ms as i64,
            now_playing: false,
        },
    }))
}

fn widget_mix(store: &Store, m: &encore_recs::Mix, pinned: bool, covers: &mut Covers) -> WidgetMix {
    // The cover of the first of its songs whose album has one.
    let album = m
        .songs
        .iter()
        .take(MIX_COVER_LOOKS)
        .filter_map(|s| first_track(store, &s.song_key)?.album_id)
        .find(|a| cover_of(store, *a).is_some());
    WidgetMix {
        mix_id: m.id.clone(),
        title: m.title.clone(),
        cover: covers.of(store, album),
        // The colours of the album the cover comes from, so the tint matches the cover.
        palette: covers.palette(store, album),
        pinned,
    }
}

/// A palette as the file holds it (null until it is made).
fn palette_value(p: &Option<WidgetPalette>) -> Value {
    match p {
        Some(p) => json!({
            "scheme": match p.scheme {
                PaletteScheme::Dark => "dark",
                PaletteScheme::Light => "light",
            },
            "base": p.base,
            "stops": p.stops,
            "text": p.text,
        }),
        None => Value::Null,
    }
}

/// Write `name` in `dir` atomically: a temporary file beside it, then a rename, so
/// a widget never reads half a file.
fn write_atomically(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{name}.{}.tmp", ulid::Ulid::generate()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dir.join(name)).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The covers saved in a profile's `art/` folder (not the temporary files).
fn saved_art(dir: &Path) -> HashSet<String> {
    std::fs::read_dir(dir.join(ART))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

/// Every cover file a snapshot names.
fn referenced(s: &WidgetSnapshot) -> HashSet<String> {
    let song = s.song.iter().filter_map(|x| x.cover.clone());
    let mixes = s
        .pinned_mixes
        .iter()
        .chain(s.next_up.iter())
        .filter_map(|m| m.cover.clone());
    let albums = s.recent_albums.iter().filter_map(|a| a.cover.clone());
    song.chain(mixes).chain(albums).collect()
}

/// Delete the covers the snapshot no longer names (all of them under Locked and
/// Guest), and the folder once it is empty.
fn prune_art(dir: &Path, keep: &HashSet<String>) {
    let art = dir.join(ART);
    let Ok(entries) = std::fs::read_dir(&art) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !keep.contains(&name) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    if keep.is_empty() {
        let _ = std::fs::remove_dir(&art);
    }
}

/// Covers a round fetched for a profile that is no longer open: no snapshot names
/// them (they were missing when the round began), so they go.
fn discard(fetched: &[PathBuf]) {
    for f in fetched {
        let _ = std::fs::remove_file(f);
    }
    if let Some(art) = fetched.first().and_then(|f| f.parent()) {
        // Only when empty: the profile's own covers stay.
        let _ = std::fs::remove_dir(art);
        if let Some(dir) = art.parent() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

fn privacy_name(p: WidgetPrivacy) -> &'static str {
    match p {
        WidgetPrivacy::Normal => "normal",
        WidgetPrivacy::Locked => "locked",
        WidgetPrivacy::Guest => "guest",
    }
}

/// The file format, schema 1 (CORE-API.md, "Widgets"). Keys are camelCase; covers
/// are file names in `art/` beside the file; `recap` is reserved for Phase 11 and
/// always null.
pub(super) fn snapshot_json(s: &WidgetSnapshot) -> Value {
    let mix = |m: &WidgetMix| {
        json!({
            "mixId": m.mix_id,
            "title": m.title,
            "cover": m.cover,
            "palette": palette_value(&m.palette),
            "pinned": m.pinned,
        })
    };
    json!({
        "schema": s.schema,
        "profileId": s.profile_id,
        "privacy": privacy_name(s.privacy),
        "song": s.song.as_ref().map(|x| json!({
            "songId": x.song_id,
            "title": x.title,
            "artist": x.artist,
            "albumId": x.album_id,
            "albumTitle": x.album_title,
            "cover": x.cover,
            "palette": palette_value(&x.palette),
            "durationMs": x.duration_ms,
            "nowPlaying": x.now_playing,
        })),
        "isPlaying": s.is_playing,
        "positionMs": s.position_ms,
        "positionAtUtcMs": s.position_at_utc_ms,
        "pinnedMixes": s.pinned_mixes.iter().map(mix).collect::<Vec<Value>>(),
        "nextUp": s.next_up.as_ref().map(mix),
        "recentAlbums": s.recent_albums.iter().map(|a| json!({
            "albumId": a.album_id,
            "title": a.title,
            "artist": a.artist,
            "cover": a.cover,
            "palette": palette_value(&a.palette),
        })).collect::<Vec<Value>>(),
        "reachable": s.reachable,
        "writtenAtUtcMs": s.written_at_utc_ms,
        "recap": Value::Null,
    })
}

/// Start the debounced writer on the core's runtime (from `EncoreCore::build`).
/// It holds the core weakly and ends with it.
pub(super) fn start_writer(core: &EncoreCore) {
    let Some(rt) = core.rt.as_ref() else { return };
    let weak = Arc::downgrade(&core.inner);
    let wake = core.inner.widgets.wake.clone();
    rt.spawn(async move {
        loop {
            wake.notified().await;
            // Everything that changes within this second goes in one write.
            tokio::time::sleep(DEBOUNCE).await;
            let Some(inner) = weak.upgrade() else { return };
            inner.widgets_round().await;
        }
    });
}

impl Inner {
    fn widget_dir(&self, profile_id: &str) -> PathBuf {
        Path::new(&self.config.data_dir)
            .join("widgets")
            .join(profile_id)
    }

    /// Something a widget shows changed: the open profile's snapshot is written
    /// within about a second. Cheap; safe anywhere, holding any lock.
    pub(super) fn widgets_soon(&self) {
        self.widgets.dirty.store(true, Ordering::SeqCst);
        self.widgets.wake.notify_one();
    }

    /// Write now, without the debounce: a privacy change (a PIN set, Guest, the
    /// history cleared) reaches the file before anything else can read it. Missing
    /// covers follow in the debounced write. Call it holding no lock.
    pub(super) fn widgets_now(&self) {
        let _ = self.write_snapshot(None, false);
        self.widgets_soon();
    }

    /// The open profile is closing (a switch, a removal, a restore): write it as
    /// not playing, since it stops here. Call it holding no lock.
    pub(super) fn widgets_closing(&self) {
        let _ = self.write_snapshot(None, true);
    }

    /// A profile left this device: its snapshot and covers go, and the app hears
    /// of it, so that profile's widgets say "Profile removed".
    pub(super) fn remove_widgets(&self, profile_id: &str) {
        let Ok(id) = checked_profile_id(profile_id) else {
            return;
        };
        let one = lock(&self.widgets.write);
        let _ = std::fs::remove_dir_all(self.widget_dir(id));
        drop(one);
        let mut e = CoreEvent::of(EventKind::WidgetSnapshotChanged);
        e.widget_profile_id = Some(id.to_owned());
        self.emit(e);
    }

    /// What a profile's widgets may show: Locked (a PIN) before Guest.
    fn widget_privacy(&self, p: &Profile) -> Result<WidgetPrivacy, EncoreError> {
        if lock(&self.registry)?.pin(&p.id)?.is_some() {
            return Ok(WidgetPrivacy::Locked);
        }
        Ok(if lock(&p.playback)?.log.mode() == Mode::Guest {
            WidgetPrivacy::Guest
        } else {
            WidgetPrivacy::Normal
        })
    }

    /// Whether the media server can be reached, as far as this device knows: the
    /// platform says it's online, a source is signed in, and the server hasn't
    /// ended the sign-in. No request is made.
    fn widgets_reachable(&self, p: &Profile) -> bool {
        // `playable_now` is None online; it reads connectivity, then the store.
        let online = matches!(self.playable_now(p), Ok(None));
        let signed_in = lock(&p.sources).is_ok_and(|s| !s.is_empty());
        let ended = lock(&p.store)
            .ok()
            .and_then(|s| s.setting(SESSION_ENDED_KEY).ok().flatten())
            .is_some_and(|v| v == "1");
        online && signed_in && !ended
    }

    /// Next up: the first mix of today's stored set, in the set's order, that no
    /// play was started from since the set was made and that isn't playing now;
    /// when every one was played, the set's first. None without a set, and under
    /// Learning off (Home shows no set then either). Never builds a set.
    pub(super) fn next_up_mix(
        &self,
        store: &Store,
        mode: Mode,
        playing_context: Option<&str>,
    ) -> Result<Option<encore_recs::Mix>, EncoreError> {
        if mode == Mode::LearningOff {
            return Ok(None);
        }
        let Some(set) = Inner::stored_mix_set(store) else {
            return Ok(None);
        };
        let played: HashSet<String> = store
            .mixes_played_since(set.made_utc_ms)?
            .into_iter()
            .collect();
        let fresh = set
            .mixes
            .iter()
            .find(|m| !played.contains(&m.id) && playing_context != Some(m.id.as_str()));
        Ok(match fresh {
            Some(m) => Some(m.clone()),
            None => set.mixes.first().cloned(),
        })
    }

    /// The open profile's snapshot from local state, with the covers already
    /// saved and the palettes already made, then the covers it lacks and the
    /// albums whose palette it lacks. `closing`: nothing plays any more. No
    /// network: the writer fetches what is missing.
    fn build_snapshot(
        &self,
        p: &Profile,
        closing: bool,
    ) -> Result<(WidgetSnapshot, Vec<Wanted>, Vec<i64>), EncoreError> {
        let now = now_ms();
        let privacy = self.widget_privacy(p)?;
        let reachable = self.widgets_reachable(p);
        let mut snap = WidgetSnapshot {
            schema: SCHEMA,
            profile_id: p.id.clone(),
            privacy,
            song: None,
            is_playing: false,
            position_ms: 0,
            // 0 while nothing plays now (so an idle snapshot stays the same).
            position_at_utc_ms: 0,
            pinned_mixes: Vec::new(),
            next_up: None,
            recent_albums: Vec::new(),
            reachable,
            written_at_utc_ms: now,
        };
        if privacy != WidgetPrivacy::Normal {
            // Locked and Guest: nothing of the profile at all, no palettes either.
            return Ok((snap, Vec::new(), Vec::new()));
        }
        let mut covers = Covers {
            saved: saved_art(&self.widget_dir(&p.id)),
            wanted: Vec::new(),
            palettes_wanted: Vec::new(),
            failed: lock(&self.widgets.failed)?.clone(),
            now,
        };
        // Playback before the store (the lock order), each on its own.
        let (mode, playing_track, playing_context) = {
            let pb = lock(&p.playback)?;
            let mode = pb.log.mode();
            let current = pb.queue.current();
            // Now playing is the current song of a play the log is recording, so
            // Private listening, Learning off and Guest never show here.
            let track = match (pb.playing_uid, current, pb.log.current_play_id()) {
                (Some(uid), Some(c), Some(_)) if c.uid == uid && mode == Mode::Normal => {
                    Some(c.track_id)
                }
                _ => None,
            };
            (mode, track, current.and_then(|c| c.context_id.clone()))
        };
        let (playing, position, at) = lock(&p.live)?.play_state();
        let store = lock(&p.store)?;
        match playing_track.and_then(|id| store.track(id).ok()) {
            Some(t) => {
                // Closing, it stops here: it becomes the last song played.
                snap.song = Some(widget_song(&store, t, !closing, &mut covers));
                if !closing {
                    snap.is_playing = playing;
                    snap.position_ms = position;
                    snap.position_at_utc_ms = if at > 0 { at } else { now };
                }
            }
            None => snap.song = last_played(&store, &mut covers)?,
        }
        let mut seen = HashSet::new();
        for e in store.recent_play_events(RECENT_PLAYS_READ)? {
            if snap.recent_albums.len() >= RECENT_ALBUMS {
                break;
            }
            let Some(album_id) = first_track(&store, &e.song_key).and_then(|t| t.album_id) else {
                continue;
            };
            if !seen.insert(album_id) {
                continue;
            }
            let Ok(a) = store.album(album_id) else {
                continue;
            };
            snap.recent_albums.push(WidgetAlbum {
                album_id,
                cover: covers.of(&store, Some(album_id)),
                palette: covers.palette(&store, Some(album_id)),
                title: a.title,
                artist: a.artist,
            });
        }
        for m in self.pinned(&store).iter().take(MAX_PINNED) {
            let tile = widget_mix(&store, m, true, &mut covers);
            snap.pinned_mixes.push(tile);
        }
        if let Some(m) = self.next_up_mix(&store, mode, playing_context.as_deref())? {
            let pinned = snap.pinned_mixes.iter().any(|x| x.mix_id == m.id);
            snap.next_up = Some(widget_mix(&store, &m, pinned, &mut covers));
        }
        drop(store);
        Ok((snap, covers.wanted, covers.palettes_wanted))
    }

    /// Build and write the open profile's snapshot now, prune the covers it no
    /// longer names, and tell the app; a snapshot that only differs in its time
    /// isn't written again. `only`: skip it when another profile is open by now.
    /// Holds the write lock while it builds, so a privacy change written
    /// meanwhile can't be overwritten by an older state.
    fn write_snapshot(
        &self,
        only: Option<&str>,
        closing: bool,
    ) -> Result<Option<WidgetSnapshot>, EncoreError> {
        let (snap, written) = {
            let _one = lock(&self.widgets.write)?;
            let p = self.profile()?;
            if only.is_some_and(|id| id != p.id) {
                return Ok(None);
            }
            let (snap, _, _) = self.build_snapshot(&p, closing)?;
            let dir = self.widget_dir(&p.id);
            let json = snapshot_json(&snap);
            let mut content = json.clone();
            if let Some(o) = content.as_object_mut() {
                o.remove("writtenAtUtcMs");
            }
            let unchanged = dir.join(SNAPSHOT).is_file()
                && lock(&self.widgets.last)?
                    .as_ref()
                    .is_some_and(|(id, v)| *id == p.id && *v == content);
            if !unchanged {
                write_atomically(&dir, SNAPSHOT, json.to_string().as_bytes()).map_err(|e| {
                    EncoreError::Storage {
                        reason: format!("couldn't write the widget snapshot ({e})"),
                    }
                })?;
                *lock(&self.widgets.last)? = Some((p.id.clone(), content));
            }
            prune_art(&dir, &referenced(&snap));
            (snap, !unchanged)
        };
        if written {
            let mut e = CoreEvent::of(EventKind::WidgetSnapshotChanged);
            e.widget_profile_id = Some(snap.profile_id.clone());
            self.emit(e);
        }
        Ok(Some(snap))
    }

    /// Whether `p` is still the open profile and may still show covers.
    fn still_showing(&self, p: &Arc<Profile>) -> bool {
        let open = self.profile().is_ok_and(|q| Arc::ptr_eq(&q, p));
        open && self
            .widget_privacy(p)
            .is_ok_and(|v| v == WidgetPrivacy::Normal)
    }

    /// A downloaded album's kept cover (1200 px, Phase 8), for when the server
    /// can't be asked.
    fn kept_cover(&self, p: &Profile, album_id: i64) -> Option<Vec<u8>> {
        let path = {
            let store = lock(&p.store).ok()?;
            self.local_cover(&store, album_id).ok().flatten()?
        };
        std::fs::read(path).ok()
    }

    /// Fetch the covers a snapshot lacks, through the core's own client (pinned
    /// self-signed servers work), and save them atomically in `art/`; offline, a
    /// downloaded album's kept cover is copied instead. Best effort: a cover that
    /// fails is simply left out. Returns the files written.
    async fn fetch_covers(&self, p: &Arc<Profile>, wanted: Vec<Wanted>) -> Vec<PathBuf> {
        let mut fetched = Vec::new();
        if wanted.is_empty() {
            return fetched;
        }
        let reachable = self.widgets_reachable(p);
        let art = self.widget_dir(&p.id).join(ART);
        for w in wanted {
            // Stop at once if the profile closed, got a PIN or went Guest.
            if !self.still_showing(p) {
                break;
            }
            let from_server = if reachable
                && let Ok(source) = self.source(p, w.source_id)
                && let Ok(Some(b)) = source.artwork_bytes(&w.item_id, COVER_PX).await
            {
                Some(b)
            } else {
                None
            };
            // Offline, or the server had none: a downloaded album's kept cover.
            let Some(bytes) = from_server.or_else(|| self.kept_cover(p, w.album_id)) else {
                continue;
            };
            if !self.still_showing(p) {
                break;
            }
            if write_atomically(&art, &w.name, &bytes).is_ok() {
                fetched.push(art.join(&w.name));
            }
        }
        fetched
    }

    /// Make the album palettes a snapshot lacks: `make_palette`'s work (one small
    /// PNG through the core's own client, kept in the database for good, so the
    /// app's `album_palette` is free afterwards). Only while the server is
    /// reachable, at most [`PALETTES_PER_ROUND`] a round, and only while this
    /// profile may still show them; an album that fails is left alone for
    /// [`PALETTE_RETRY_MS`]. Nothing is written to the widget folder.
    async fn fetch_palettes(&self, p: &Arc<Profile>, wanted: Vec<i64>) {
        if wanted.is_empty() || !self.widgets_reachable(p) {
            return;
        }
        for id in wanted.into_iter().take(PALETTES_PER_ROUND) {
            if !self.still_showing(p) {
                break;
            }
            if self.make_palette(p, "album", id).await.is_err()
                && let Ok(mut failed) = lock(&self.widgets.failed)
            {
                failed.insert(id, now_ms());
            }
        }
    }

    /// One debounced write: fetch the covers and palettes the snapshot lacks,
    /// then write it.
    async fn widgets_round(self: &Arc<Self>) {
        if !self.widgets.dirty.swap(false, Ordering::SeqCst) {
            return;
        }
        let Ok(p) = self.profile() else { return };
        let (wanted, palettes) = match self.build_snapshot(&p, false) {
            Ok((_, wanted, palettes)) => (wanted, palettes),
            Err(_) => return,
        };
        let fetched = self.fetch_covers(&p, wanted).await;
        self.fetch_palettes(&p, palettes).await;
        if !matches!(self.write_snapshot(Some(&p.id), false), Ok(Some(_))) {
            discard(&fetched);
        }
    }

    /// Tests: one debounced write now, whatever the timer says.
    #[cfg(test)]
    pub(super) async fn widgets_flush(self: &Arc<Self>) {
        self.widgets.dirty.store(true, Ordering::SeqCst);
        self.widgets_round().await;
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// The open profile's widget snapshot, built now from local state: the same
    /// thing `<data_dir>/widgets/<profile_id>/snapshot.json` holds (the file may
    /// lag by about a second, and names only covers already saved). No network.
    /// There is no call for another profile's: the core never reads a profile that
    /// isn't open. Synchronous.
    pub fn widget_snapshot(&self) -> Result<WidgetSnapshot, EncoreError> {
        let p = self.inner.profile()?;
        Ok(self.inner.build_snapshot(&p, false)?.0)
    }

    /// Start a mix from a widget, the Quick Settings tile or a shortcut: the same
    /// as the app's Play on a mix card (`mix(id).songs` played from the first, as
    /// `StartedFrom::Mix` with the mix's ID as context, shuffle off).
    ///
    /// `MixRef { kind: Id, mix_id }` plays that mix, pinned or in today's set. When
    /// it is gone (the daily set rotated, it was hidden or unpinned) it falls back
    /// to Next up.
    ///
    /// **Next up** is the first mix of today's set, in the set's order, that no
    /// play was started from since the set was made and that isn't the one
    /// playing now; when every mix was played, the set's first. "Today's set" is
    /// the one Home last built or fetched: this call never builds one and never
    /// calls the network. Under Learning off there is no set (as on Home). With no
    /// Next up, the first pinned mix plays; with none either, `NotFound`.
    /// Offline, only songs that can play join the queue (as `play`). Synchronous.
    pub fn play_mix(&self, mix: MixRef) -> Result<QueueState, EncoreError> {
        let p = self.inner.profile()?;
        let asked = match mix.kind {
            MixRefKind::Id => mix.mix_id.filter(|id| !id.is_empty()),
            MixRefKind::NextUp => None,
        };
        // Playback before the store (the lock order).
        let (mode, playing_context) = {
            let pb = lock(&p.playback)?;
            (
                pb.log.mode(),
                pb.queue.current().and_then(|c| c.context_id.clone()),
            )
        };
        let mix_id = {
            let store = lock(&p.store)?;
            match asked {
                Some(id) if self.inner.find_mix(&store, &id).is_some() => id,
                _ => match self
                    .inner
                    .next_up_mix(&store, mode, playing_context.as_deref())?
                {
                    Some(m) => m.id,
                    None => self
                        .inner
                        .pinned(&store)
                        .first()
                        .map(|m| m.id.clone())
                        .ok_or_else(|| EncoreError::NotFound {
                            reason: "No mix yet: mixes build as you listen".into(),
                        })?,
                },
            }
        };
        let ids: Vec<i64> = self
            .mix(mix_id.clone())?
            .songs
            .iter()
            .map(|s| s.song.id)
            .collect();
        if ids.is_empty() {
            return Err(EncoreError::NotFound {
                reason: "That mix has no songs in your library".into(),
            });
        }
        self.play(ids, 0, StartedFrom::Mix, Some(mix_id))?;
        self.set_shuffle(false)
    }
}
