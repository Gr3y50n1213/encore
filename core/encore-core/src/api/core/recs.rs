//! Smart picks in the core (plan section 21; boards 15, 23, 37, 38;
//! docs/adr/core-0012): autoplay, radio, Smart shuffle, Why this song, feedback,
//! Not interested, the mixes and the Home shelves, the stats page, Reset and
//! Clear. The engine is `encore-recs`; this module feeds it from the profile's
//! store and turns its picks into songs.
//!
//! The model is rebuilt only when its inputs changed (a fingerprint of the play
//! log, likes, feedback, Not interested, the library and the epoch) or every 10
//! minutes for the time of day; picks read it, so the next-song decision is a
//! read, not a build.

use std::collections::HashMap;
use std::sync::Arc;

use encore_recs as recs;
use encore_store::{EventRecord, FeedbackRow, NotInterestedRow, Store};

use super::{EncoreCore, Inner, Profile, lock, now_ms};
use crate::api::types::*;
use crate::queue::{ChosenBy as QueueChosen, NewItem, StartedFrom as QueueFrom};

const MODEL_MAX_AGE_MS: i64 = 10 * 60_000;
const SESSION_GAP_MS: i64 = 30 * 60_000;
const MIX_SET_KEY: &str = "mixSet";
const PINNED_KEY: &str = "pinnedMixes";
const HIDDEN_KEY: &str = "hiddenMixes";
const TITLES_KEY: &str = "recentTitles";
const ADVENTURE_KEY: &str = "recs.adventurousness";

/// The model, the catalog it was built on, and song key to track ID.
pub(crate) type Built = (
    Arc<recs::Model>,
    Arc<recs::Catalog>,
    Arc<HashMap<String, i64>>,
);

/// The built model and what it was built from.
pub(crate) struct RecsCache {
    fingerprint: String,
    built_utc_ms: i64,
    learning: bool,
    model: Arc<recs::Model>,
    catalog: Arc<recs::Catalog>,
    /// Song key to a track row (the first with that key).
    ids: Arc<HashMap<String, i64>>,
}

impl std::fmt::Debug for RecsCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecsCache")
            .field("built_utc_ms", &self.built_utc_ms)
            .finish_non_exhaustive()
    }
}

pub(crate) fn params() -> recs::Params {
    recs::Params::default()
}

fn reason_of(r: &str) -> recs::NotInterestedReason {
    match r {
        "heardTooMuch" => recs::NotInterestedReason::HeardTooMuch,
        "wrongMoment" => recs::NotInterestedReason::WrongMoment,
        _ => recs::NotInterestedReason::NotMyTaste,
    }
}

fn reason_name(r: NotInterestedReason) -> &'static str {
    match r {
        NotInterestedReason::NotMyTaste => "notMyTaste",
        NotInterestedReason::HeardTooMuch => "heardTooMuch",
        NotInterestedReason::WrongMoment => "wrongMoment",
    }
}

fn api_reason(r: &str) -> NotInterestedReason {
    match r {
        "heardTooMuch" => NotInterestedReason::HeardTooMuch,
        "wrongMoment" => NotInterestedReason::WrongMoment,
        _ => NotInterestedReason::NotMyTaste,
    }
}

fn time_of_day(b: u8) -> TimeOfDay {
    match b {
        0 => TimeOfDay::Night,
        1 => TimeOfDay::Morning,
        2 => TimeOfDay::Afternoon,
        _ => TimeOfDay::Evening,
    }
}

fn mix_kind(s: recs::MixSlot) -> MixKind {
    match s {
        recs::MixSlot::Strongest => MixKind::Strongest,
        recs::MixSlot::Second => MixKind::Second,
        recs::MixSlot::TimeOfDay => MixKind::TimeOfDay,
        recs::MixSlot::Drift => MixKind::Drift,
        recs::MixSlot::Stretch => MixKind::Stretch,
        recs::MixSlot::Family => MixKind::Family,
        recs::MixSlot::Starter => MixKind::Starter,
    }
}

/// The engine's inputs from the store. `learn`: false gives the model only the
/// library (Guest and Learn-from-my-listening-off use the plain fallbacks).
fn inputs(
    store: &Store,
    learn: bool,
) -> Result<(recs::Catalog, recs::History, HashMap<String, i64>), EncoreError> {
    let rows = store.recs_songs()?;
    let mut ids = HashMap::new();
    let songs: Vec<recs::Song> = rows
        .into_iter()
        .map(|r| {
            ids.insert(r.song_key.clone(), r.track_id);
            recs::Song {
                key: r.song_key,
                title: r.title,
                artists: r.artists,
                album: r.album,
                album_artist: Some(r.album_artist),
                genres: r.genres,
                year: r.year,
                duration_ms: r.duration_ms,
                added_utc_ms: r.added_utc_ms,
                server_play_count: if learn { r.server_play_count } else { 0 },
                server_last_played_utc_ms: if learn {
                    r.server_last_played_utc_ms
                } else {
                    None
                },
                server_favorite: r.favorite,
                explicit: false,
            }
        })
        .collect();
    let catalog = recs::Catalog::new(songs);
    if !learn {
        return Ok((catalog, recs::History::default(), ids));
    }
    let plays = store
        .recs_plays()?
        .into_iter()
        .map(|p| recs::Play {
            outcome: p.signal.as_deref().and_then(recs::Outcome::from_signal),
            app_chosen: p.chosen_by == "app",
            strong_intent: matches!(p.started_from.as_str(), "search" | "queue" | "history"),
            song_key: p.song_key,
            started_utc_ms: p.started_utc_ms,
            utc_offset_minutes: p.utc_offset_minutes,
            listened_ms: p.listened_ms,
        })
        .collect();
    let history = recs::History {
        plays,
        likes: store.likes_with_times()?,
        added: store.playlist_adds()?,
        feedback: store
            .feedback_rows()?
            .into_iter()
            .map(|f| recs::Feedback {
                song_key: f.song_key,
                more: f.more,
                at_utc_ms: f.at_utc_ms,
                bucket: f.bucket,
            })
            .collect(),
        not_interested: store
            .not_interested_rows()?
            .into_iter()
            .filter(|r| !r.removed)
            .map(|r| recs::NotInterested {
                scope: if r.scope == "artist" {
                    recs::Scope::Artist(r.value)
                } else {
                    recs::Scope::Song(r.value)
                },
                reason: reason_of(&r.reason),
                at_utc_ms: r.at_utc_ms,
                bucket: r.bucket,
            })
            .collect(),
        epoch_start_utc_ms: store
            .state_number(encore_store::EPOCH_KEY)?
            .unwrap_or(i64::MIN),
    };
    Ok((catalog, history, ids))
}

impl Inner {
    fn learns(&self, p: &Profile) -> Result<bool, EncoreError> {
        use crate::playlog::ListeningMode as M;
        Ok(!matches!(
            lock(&p.playback)?.log.mode(),
            M::Guest | M::LearningOff
        ))
    }

    fn utc_offset(&self, p: &Profile) -> i32 {
        lock(&p.playback)
            .map(|pb| pb.utc_offset_minutes)
            .unwrap_or(0)
    }

    /// The model, rebuilt when its inputs changed or it is 10 minutes old.
    pub(super) fn recs_model(&self, p: &Profile) -> Result<Built, EncoreError> {
        let learn = self.learns(p)?;
        let now = now_ms();
        let fingerprint = format!("{}|{learn}", lock(&p.store)?.recs_fingerprint()?);
        if let Some(c) = lock(&p.recs)?.as_ref()
            && c.fingerprint == fingerprint
            && c.learning == learn
            && now - c.built_utc_ms < MODEL_MAX_AGE_MS
        {
            return Ok((c.model.clone(), c.catalog.clone(), c.ids.clone()));
        }
        let (catalog, history, ids) = inputs(&*lock(&p.store)?, learn)?;
        let model = recs::Model::build(&history, &catalog, &params(), now, self.utc_offset(p));
        let cache = RecsCache {
            fingerprint,
            built_utc_ms: now,
            learning: learn,
            model: Arc::new(model),
            catalog: Arc::new(catalog),
            ids: Arc::new(ids),
        };
        let out = (
            cache.model.clone(),
            cache.catalog.clone(),
            cache.ids.clone(),
        );
        *lock(&p.recs)? = Some(cache);
        Ok(out)
    }

    fn adventurousness_percent(&self, p: &Profile, plays: u32) -> Result<u32, EncoreError> {
        // Exploration is fixed at Balanced while still learning (section 21).
        let setting = lock(&p.store)?.setting(ADVENTURE_KEY)?;
        let pct = match setting.as_deref() {
            _ if plays < params().full_from_plays => 10,
            Some("familiar") => 5,
            Some("adventurous") => 20,
            _ => 10,
        };
        Ok(pct)
    }

    fn key_of(store: &Store, track_id: i64) -> Option<String> {
        store.track(track_id).ok().map(|t| t.song_key)
    }

    /// What the moment looks like for the engine: the current song and the last 5
    /// finished as seeds, everything queued excluded, and this session's skips and
    /// likes (the in-session adjustment), read from the log.
    fn context(&self, p: &Profile, model: &recs::Model) -> Result<recs::Context, EncoreError> {
        // Playback before store, as everywhere else (the lock order).
        let pb = lock(&p.playback)?;
        let store = lock(&p.store)?;
        let v = pb.queue.view();
        let mut seeds = Vec::new();
        if let Some(c) = v.current.and_then(|c| Self::key_of(&store, c.track_id)) {
            seeds.push(c);
        }
        if let Some(r) = &pb.radio_seed {
            seeds.retain(|k| k != r);
            seeds.insert(0, r.clone());
        }
        let mut exclude: Vec<String> = Vec::new();
        for item in v
            .history
            .iter()
            .rev()
            .take(50)
            .chain(v.up_next)
            .chain(v.autoplay)
        {
            if let Some(k) = Self::key_of(&store, item.track_id) {
                exclude.push(k);
            }
        }
        // This session: plays since the last 30-minute gap.
        let now = now_ms();
        let recent = store.recent_play_events(60)?;
        let mut session = Vec::new();
        let mut last = now;
        for e in &recent {
            if last - e.started_utc_ms > SESSION_GAP_MS {
                break;
            }
            last = e.started_utc_ms;
            session.push(e);
        }
        let mut finished = 0;
        for e in &session {
            if e.signal.as_deref() == Some("finished") && finished < 5 {
                if !seeds.contains(&e.song_key) {
                    seeds.push(e.song_key.clone());
                }
                finished += 1;
            }
        }
        let early_skips_in_a_row = session
            .iter()
            .take_while(|e| e.chosen_by == "app" && e.signal.as_deref() == Some("skippedEarly"))
            .count() as u32;
        let pushed_away = session
            .iter()
            .filter(|e| e.chosen_by == "app" && e.signal.as_deref() == Some("skippedEarly"))
            .map(|e| e.song_key.clone())
            .collect();
        let since = session.last().map_or(now, |e| e.started_utc_ms);
        let pulled_toward = store
            .likes_with_times()?
            .into_iter()
            .filter(|(_, t)| *t >= since)
            .max_by_key(|(_, t)| *t)
            .map(|(k, _)| k);
        drop(store);
        drop(pb);
        Ok(recs::Context {
            seeds,
            exclude,
            early_skips_in_a_row,
            pushed_away,
            pulled_toward,
            exploration_percent: self.adventurousness_percent(p, model.plays_counted)?,
            only: None,
            hide_explicit: false,
            seed: now as u64,
        })
    }

    fn items(ids: &HashMap<String, i64>, picks: &[recs::Pick], from: QueueFrom) -> Vec<NewItem> {
        picks
            .iter()
            .filter_map(|x| ids.get(&x.song_key))
            .map(|&id| NewItem {
                track_id: id,
                chosen_by: QueueChosen::App,
                started_from: from,
                context_id: None,
            })
            .collect()
    }

    /// Fill the Autoplay section (board 37: 10 songs, refilled when fewer than 5
    /// are upcoming; touches only the Autoplay section). `force` refills anyway.
    pub(super) fn refill_autoplay(&self, force: bool) -> Result<Option<QueueState>, EncoreError> {
        let p = self.profile()?;
        {
            let pb = lock(&p.playback)?;
            let v = pb.queue.view();
            if !v.autoplay_enabled || v.current.is_none() {
                return Ok(None);
            }
            if !force && v.up_next.len() + v.autoplay.len() >= 5 {
                return Ok(None);
            }
        }
        let (model, catalog, ids) = self.recs_model(&p)?;
        let ctx = self.context(&p, &model)?;
        let radio = lock(&p.playback)?.radio_seed.clone();
        let picks = match &radio {
            Some(seed) => recs::radio(&model, &catalog, seed, &ctx, 10, &params()),
            None => recs::autoplay(&model, &catalog, &ctx, 10, &params()),
        };
        let from = if radio.is_some() {
            QueueFrom::Radio
        } else {
            QueueFrom::Autoplay
        };
        let items = Self::items(&ids, &picks, from);
        let (_, state) = self.with_queue(|pb, _| pb.queue.set_autoplay(items))?;
        Ok(Some(state))
    }

    fn pick_reason(
        &self,
        store: &Store,
        ids: &HashMap<String, i64>,
        r: &recs::Reason,
    ) -> PickReason {
        let song_of = |k: &str| {
            ids.get(k)
                .and_then(|id| store.track(*id).ok())
                .map(Song::from)
        };
        let mut out = PickReason {
            kind: ReasonKind::YouLikedIt,
            song: None,
            count: None,
            of: None,
            days: None,
            time_of_day: None,
            artist: None,
            genre: None,
        };
        match r {
            recs::Reason::FinishedOften {
                song_key,
                finished,
                of,
            } => {
                out.kind = ReasonKind::FinishedOften;
                out.song = song_of(song_key);
                out.count = Some(*finished);
                out.of = Some(*of);
            }
            recs::Reason::PlayedTogether { song_key } => {
                out.kind = ReasonKind::PlayedTogether;
                out.song = song_of(song_key);
            }
            recs::Reason::TimeOfDay { bucket } => {
                out.kind = ReasonKind::TimeOfDay;
                out.time_of_day = Some(time_of_day(*bucket));
            }
            recs::Reason::YouLikedIt => out.kind = ReasonKind::YouLikedIt,
            recs::Reason::Rediscover { days } => {
                out.kind = ReasonKind::Rediscover;
                out.days = Some(*days);
            }
            recs::Reason::SameArtist { artist } => {
                out.kind = ReasonKind::SameArtist;
                out.artist = Some(artist.clone());
            }
            recs::Reason::SameGenre { genre } => {
                out.kind = ReasonKind::SameGenre;
                out.genre = Some(genre.clone());
            }
            recs::Reason::PlayedElsewhere { count } => {
                out.kind = ReasonKind::PlayedElsewhere;
                out.count = Some(*count);
            }
            recs::Reason::NewToYou => out.kind = ReasonKind::NewToYou,
            recs::Reason::RecentlyAdded => out.kind = ReasonKind::RecentlyAdded,
        }
        out
    }

    fn record_event(
        &self,
        p: &Profile,
        kind: &str,
        payload: serde_json::Value,
    ) -> Result<(), EncoreError> {
        let now = now_ms();
        let hlc = lock(&p.edits_clock)?.tick(now);
        lock(&p.store)?.insert_event(&EventRecord {
            id: crate::edits::new_id(),
            device_id: self.config.device_id.clone(),
            hlc: hlc.to_string(),
            kind: kind.to_owned(),
            payload: payload.to_string(),
            created_utc_ms: now,
        })?;
        Ok(())
    }

    fn bucket_now(&self, p: &Profile) -> u8 {
        recs::bucket_of(now_ms(), self.utc_offset(p))
    }

    // ------------------------------------------------------------- mixes

    fn local_day(&self, p: &Profile, t: i64) -> i64 {
        (t + i64::from(self.utc_offset(p)) * 60_000).div_euclid(recs::DAY_MS)
    }

    fn stored<T: serde::de::DeserializeOwned>(store: &Store, key: &str) -> Option<T> {
        store
            .recs_state(key)
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
    }

    fn store_json<T: serde::Serialize>(store: &Store, key: &str, v: &T) -> Result<(), EncoreError> {
        let json = serde_json::to_string(v).map_err(EncoreError::internal)?;
        Ok(store.set_recs_state(key, Some(&json))?)
    }

    /// Today's mix set: built on this device at the first request after 06:00 local
    /// time, after 25 or more new plays, or on Regenerate; never replaced while one
    /// of its mixes is playing.
    pub(super) fn mix_set(&self, regenerate: bool) -> Result<recs::MixSet, EncoreError> {
        let p = self.profile()?;
        let (model, catalog, _) = self.recs_model(&p)?;
        let now = now_ms();
        let offset = self.utc_offset(&p);
        let today = self.local_day(&p, now);
        let local_hour = (now + i64::from(offset) * 60_000).rem_euclid(recs::DAY_MS) / 3_600_000;
        // What is playing, read before the store lock (playback comes first).
        let playing_context = lock(&p.playback)?
            .queue
            .current()
            .and_then(|c| c.context_id.clone());
        let store = lock(&p.store)?;
        let old: Option<recs::MixSet> = Self::stored(&store, MIX_SET_KEY);
        if let Some(old) = &old {
            let playing = playing_context
                .as_ref()
                .is_some_and(|id| old.mixes.iter().any(|m| &m.id == id));
            let due_by_day = old.day < today && (local_hour >= 6 || old.day < today - 1);
            let due_by_plays = model.plays_counted >= old.plays_counted + 25;
            let changed_stage = old.learning != (model.plays_counted < params().full_from_plays);
            if playing || !(regenerate || due_by_day || due_by_plays || changed_stage) {
                return Ok(old.clone());
            }
        }
        let hidden: Vec<String> = Self::stored(&store, HIDDEN_KEY).unwrap_or_default();
        let recent: Vec<(i64, String)> = Self::stored(&store, TITLES_KEY).unwrap_or_default();
        let recent_titles: Vec<String> = recent
            .iter()
            .filter(|(d, _)| today - d <= 14)
            .map(|(_, t)| t.clone())
            .collect();
        let set = recs::build_mixes(
            &model,
            &catalog,
            &params(),
            &recs::Previous {
                set: old.as_ref(),
                hidden: &hidden,
                recent_titles: &recent_titles,
                regenerate,
            },
        );
        Self::store_json(&store, MIX_SET_KEY, &set)?;
        let mut titles: Vec<(i64, String)> = recent
            .into_iter()
            .filter(|(d, _)| today - d <= 14)
            .collect();
        for m in &set.mixes {
            if !titles.iter().any(|(_, t)| *t == m.title) {
                titles.push((today, m.title.clone()));
            }
        }
        Self::store_json(&store, TITLES_KEY, &titles)?;
        Ok(set)
    }

    fn pinned(&self, store: &Store) -> Vec<recs::Mix> {
        Self::stored(store, PINNED_KEY).unwrap_or_default()
    }

    fn summary(
        &self,
        ids: &HashMap<String, i64>,
        store: &Store,
        m: &recs::Mix,
        pinned: bool,
        made: i64,
    ) -> MixSummary {
        let mut covers = Vec::new();
        let mut albums = Vec::new();
        for s in &m.songs {
            if let Some(id) = ids.get(&s.song_key)
                && let Ok(t) = store.track(*id)
            {
                if !albums.contains(&t.album_title) {
                    albums.push(t.album_title.clone());
                    covers.push(*id);
                }
                if covers.len() == 4 {
                    break;
                }
            }
        }
        MixSummary {
            mix_id: m.id.clone(),
            kind: mix_kind(m.slot),
            title: m.title.clone(),
            subtitle: m.subtitle.clone(),
            cover_song_ids: covers,
            song_count: m
                .songs
                .iter()
                .filter(|s| ids.contains_key(&s.song_key))
                .count() as u32,
            pinned,
            made_utc_ms: made,
        }
    }

    fn find_mix(&self, store: &Store, mix_id: &str) -> Option<(recs::Mix, bool, i64)> {
        if let Some(m) = self.pinned(store).into_iter().find(|m| m.id == mix_id) {
            return Some((m, true, 0));
        }
        let set: recs::MixSet = Self::stored(store, MIX_SET_KEY)?;
        let made = set.made_utc_ms;
        set.mixes
            .into_iter()
            .find(|m| m.id == mix_id)
            .map(|m| (m, false, made))
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Fill Autoplay now with 10 songs seeded by the current song and the last ones
    /// finished (the core also does this by itself when fewer than 5 are
    /// upcoming). Only the Autoplay section changes; songs the user queued are
    /// never touched.
    pub fn refresh_autoplay(&self) -> Result<QueueState, EncoreError> {
        match self.inner.refill_autoplay(true)? {
            Some(s) => Ok(s),
            None => self.queue(),
        }
    }

    /// Radio from a song, album, artist or playlist (section 21): it plays the
    /// seed, then Autoplay keeps the seed for the whole session.
    pub fn start_radio(&self, seed: RadioSeed, id: i64) -> Result<QueueState, EncoreError> {
        let p = self.inner.profile()?;
        let songs: Vec<i64> = {
            let store = lock(&p.store)?;
            match seed {
                RadioSeed::Song => vec![id],
                RadioSeed::Album => store.album_tracks(id)?.into_iter().map(|t| t.id).collect(),
                RadioSeed::Artist => store
                    .artist_tracks(
                        id,
                        encore_store::Page {
                            offset: 0,
                            limit: 50,
                        },
                    )?
                    .into_iter()
                    .map(|t| t.id)
                    .collect(),
                RadioSeed::Playlist => {
                    let source = store
                        .playlist_record(id)?
                        .ok_or_else(|| EncoreError::NotFound {
                            reason: "That playlist isn't in your library".into(),
                        })?
                        .source_id;
                    store
                        .playlist_items(id)?
                        .into_iter()
                        .filter_map(|r| {
                            store.track_by_item(source, &r.track_item_id).ok().flatten()
                        })
                        .map(|t| t.id)
                        .collect()
                }
            }
        };
        let first = *songs.first().ok_or_else(|| EncoreError::NotFound {
            reason: "Nothing to start a radio from".into(),
        })?;
        let key = Inner::key_of(&*lock(&p.store)?, first);
        self.play(
            vec![first],
            0,
            StartedFrom::Radio,
            Some(format!("radio-{id}")),
        )?;
        lock(&p.playback)?.radio_seed = key;
        self.refresh_autoplay()
    }

    /// Smart shuffle (section 21, New): the list shuffled, with one suggestion
    /// after every 4 songs. Suggestions have `chosen_by` App (show the spark mark;
    /// a swipe removes one).
    pub fn smart_shuffle(
        &self,
        song_ids: Vec<i64>,
        started_from: StartedFrom,
        context_id: Option<String>,
    ) -> Result<QueueState, EncoreError> {
        let p = self.inner.profile()?;
        let (model, catalog, ids) = self.inner.recs_model(&p)?;
        let keys: Vec<String> = {
            let store = lock(&p.store)?;
            song_ids
                .iter()
                .filter_map(|id| Inner::key_of(&store, *id))
                .collect()
        };
        let ctx = self.inner.context(&p, &model)?;
        let order = recs::smart_shuffle(&model, &catalog, &keys, &ctx, &params());
        let items: Vec<NewItem> = order
            .iter()
            .filter_map(|(k, suggested)| {
                ids.get(k).map(|&id| NewItem {
                    track_id: id,
                    chosen_by: if *suggested {
                        QueueChosen::App
                    } else {
                        QueueChosen::User
                    },
                    started_from: if *suggested {
                        QueueFrom::SmartShuffle
                    } else {
                        started_from.into()
                    },
                    context_id: context_id.clone(),
                })
            })
            .collect();
        lock(&p.playback)?.radio_seed = None;
        Ok(self.inner.with_queue(|pb, _| pb.queue.play(items, 0))?.1)
    }

    /// Why this song (board 38): one or two reasons and up to two "sounds like".
    pub fn why_this_song(&self, song_id: i64) -> Result<WhyThisSong, EncoreError> {
        let p = self.inner.profile()?;
        let (model, catalog, ids) = self.inner.recs_model(&p)?;
        let key =
            Inner::key_of(&*lock(&p.store)?, song_id).ok_or_else(|| EncoreError::NotFound {
                reason: "That song isn't in your library".into(),
            })?;
        let ctx = self.inner.context(&p, &model)?;
        let reasons = recs::why(&model, &catalog, &key, &ctx);
        let alike = recs::sounds_like(&model, &catalog, &key, 2);
        let store = lock(&p.store)?;
        Ok(WhyThisSong {
            reasons: reasons
                .iter()
                .map(|r| self.inner.pick_reason(&store, &ids, r))
                .collect(),
            sounds_like: alike
                .iter()
                .filter_map(|k| ids.get(k).and_then(|id| store.track(*id).ok()))
                .map(Song::from)
                .collect(),
        })
    }

    /// More like this (+1.5 for the song and its neighbours here; not a like) or
    /// Fewer like this (-1). Show a one-line confirmation.
    pub fn more_like_this(&self, song_id: i64) -> Result<(), EncoreError> {
        self.feedback(song_id, true)
    }

    pub fn fewer_like_this(&self, song_id: i64) -> Result<(), EncoreError> {
        self.feedback(song_id, false)
    }

    /// Not interested in a song (board 38): it leaves every smart surface at once
    /// but still plays when picked. Undo for 5 s with `remove_not_interested`.
    pub fn not_interested_song(
        &self,
        song_id: i64,
        reason: NotInterestedReason,
    ) -> Result<NotInterestedItem, EncoreError> {
        let p = self.inner.profile()?;
        let (key, title) = {
            let store = lock(&p.store)?;
            let t = store.track(song_id).map_err(|_| EncoreError::NotFound {
                reason: "That song isn't in your library".into(),
            })?;
            (t.song_key.clone(), t.title.clone())
        };
        self.put_not_interested("song", &key, &title, reason, false)?;
        Ok(NotInterestedItem {
            song_id: Some(song_id),
            artist: None,
            label: title,
            reason,
            added_utc_ms: now_ms(),
        })
    }

    /// Not interested in an artist: none of their songs are suggested.
    pub fn not_interested_artist(
        &self,
        artist: String,
        reason: NotInterestedReason,
    ) -> Result<NotInterestedItem, EncoreError> {
        self.put_not_interested("artist", &artist, &artist, reason, false)?;
        Ok(NotInterestedItem {
            song_id: None,
            artist: Some(artist.clone()),
            label: artist,
            reason,
            added_utc_ms: now_ms(),
        })
    }

    /// Undo, or remove from Settings' list.
    pub fn remove_not_interested(&self, item: NotInterestedItem) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let (scope, value) = match (&item.song_id, &item.artist) {
            (Some(id), _) => (
                "song",
                Inner::key_of(&*lock(&p.store)?, *id).unwrap_or_default(),
            ),
            (None, Some(a)) => ("artist", a.clone()),
            _ => return Ok(()),
        };
        self.put_not_interested(scope, &value, &item.label, item.reason, true)
    }

    /// Settings, Not interested.
    pub fn not_interested_list(&self) -> Result<Vec<NotInterestedItem>, EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        Ok(store
            .not_interested_rows()?
            .into_iter()
            .filter(|r| !r.removed)
            .map(|r| NotInterestedItem {
                song_id: (r.scope == "song")
                    .then(|| store.track_ids_for_key(&r.value).ok()?.first().copied())
                    .flatten(),
                artist: (r.scope == "artist").then(|| r.value.clone()),
                label: r.label,
                reason: api_reason(&r.reason),
                added_utc_ms: r.at_utc_ms,
            })
            .collect())
    }

    /// Clear list (Not interested has its own; Reset suggestions leaves it).
    pub fn clear_not_interested(&self) -> Result<(), EncoreError> {
        for item in self.not_interested_list()? {
            self.remove_not_interested(item)?;
        }
        Ok(())
    }

    /// How adventurous (board 38); applies from the next refill.
    pub fn set_adventurousness(&self, level: Adventurousness) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let v = match level {
            Adventurousness::Familiar => "familiar",
            Adventurousness::Balanced => "balanced",
            Adventurousness::Adventurous => "adventurous",
        };
        Ok(lock(&p.store)?.set_setting(ADVENTURE_KEY, v)?)
    }

    pub fn adventurousness(&self) -> Result<Adventurousness, EncoreError> {
        let p = self.inner.profile()?;
        Ok(match lock(&p.store)?.setting(ADVENTURE_KEY)?.as_deref() {
            Some("familiar") => Adventurousness::Familiar,
            Some("adventurous") => Adventurousness::Adventurous,
            _ => Adventurousness::Balanced,
        })
    }

    /// Reset suggestions (board 38, note 5): a new learning epoch. Plays and
    /// feedback before now stop counting; likes, playlists, history and the Not
    /// interested list stay. Syncs as a tombstone no device can undo.
    pub fn reset_suggestions(&self) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let now = now_ms();
        lock(&p.store)?.raise_state(encore_store::EPOCH_KEY, now)?;
        lock(&p.store)?.set_recs_state(MIX_SET_KEY, None)?;
        self.inner.record_event(
            &p,
            "resetSuggestions",
            serde_json::json!({ "epochUtcMs": now }),
        )?;
        *lock(&p.recs)? = None;
        self.send_soon();
        Ok(())
    }

    /// Clear listening history (board 38, note 6): every play and lyric tap so
    /// far is deleted, on this device, the companion server and every device of
    /// the profile; none can bring them back. The yearly recap starts again today.
    pub fn clear_listening_history(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let now = now_ms();
        let n = {
            let store = lock(&p.store)?;
            store.raise_state(encore_store::CLEARED_KEY, now)?;
            let n = store.clear_history_before(now)?;
            store.set_recs_state(MIX_SET_KEY, None)?;
            n
        };
        self.inner.record_event(
            &p,
            "clearHistory",
            serde_json::json!({ "beforeUtcMs": now }),
        )?;
        *lock(&p.recs)? = None;
        self.send_soon();
        Ok(n)
    }

    /// Home (the surfaces table of section 21): Made for you, Rediscover, Recently
    /// played, Deep Cuts, Recently added and Most played. Builds today's mixes when
    /// due (up to `smartPicks.budgets.mixBuildSeconds`). Async.
    pub async fn home(&self) -> Result<Home, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let from_server = server_set(&inner, false).await;
            tokio::task::block_in_place(|| home_of(&inner, false, from_server))
        })
        .await
    }

    /// A new set now (Regenerate). Async.
    pub async fn regenerate_mixes(&self) -> Result<Home, EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let from_server = server_set(&inner, true).await;
            tokio::task::block_in_place(|| home_of(&inner, true, from_server))
        })
        .await
    }

    /// A mix's page (board 15), each song with one reason.
    pub fn mix(&self, mix_id: String) -> Result<MixPage, EncoreError> {
        let p = self.inner.profile()?;
        let (_, _, ids) = self.inner.recs_model(&p)?;
        let store = lock(&p.store)?;
        let (m, pinned, made) =
            self.inner
                .find_mix(&store, &mix_id)
                .ok_or_else(|| EncoreError::NotFound {
                    reason: "That mix is no longer here".into(),
                })?;
        Ok(MixPage {
            summary: self.inner.summary(&ids, &store, &m, pinned, made),
            songs: m
                .songs
                .iter()
                .filter_map(|s| {
                    let t = store.track(*ids.get(&s.song_key)?).ok()?;
                    Some(MixSong {
                        song: Song::from(t),
                        reason: s
                            .reasons
                            .first()
                            .map(|r| self.inner.pick_reason(&store, &ids, r)),
                        new_to_you: s.exploration,
                    })
                })
                .collect(),
        })
    }

    /// Pin (kept past tomorrow; up to 10) or unpin a mix.
    pub fn pin_mix(&self, mix_id: String, pinned: bool) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        let mut list = self.inner.pinned(&store);
        list.retain(|m| m.id != mix_id);
        if pinned {
            if list.len() >= 10 {
                return Err(EncoreError::NotAllowed {
                    reason: "Up to 10 mixes can be pinned: unpin one first".into(),
                });
            }
            let (m, _, _) =
                self.inner
                    .find_mix(&store, &mix_id)
                    .ok_or_else(|| EncoreError::NotFound {
                        reason: "That mix is no longer here".into(),
                    })?;
            list.insert(0, m);
        }
        Inner::store_json(&store, PINNED_KEY, &list)
    }

    /// Hide a mix from today's set (Undo: `unhide_mix`).
    pub fn hide_mix(&self, mix_id: String) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let store = lock(&p.store)?;
        let mut hidden: Vec<String> = Inner::stored(&store, HIDDEN_KEY).unwrap_or_default();
        if !hidden.contains(&mix_id) {
            hidden.push(mix_id.clone());
        }
        hidden.retain(|h| h.starts_with("mix-"));
        Inner::store_json(&store, HIDDEN_KEY, &hidden)?;
        if let Some(mut set) = Inner::stored::<recs::MixSet>(&store, MIX_SET_KEY) {
            set.mixes.retain(|m| m.id != mix_id);
            Inner::store_json(&store, MIX_SET_KEY, &set)?;
        }
        Ok(())
    }

    /// Not in this mix (board 15, item 5): the song leaves the mix and the mix's
    /// mood learns it doesn't fit (Fewer like this at this time of day).
    pub fn not_in_this_mix(&self, mix_id: String, song_id: i64) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        {
            let store = lock(&p.store)?;
            let key = Inner::key_of(&store, song_id).unwrap_or_default();
            let mut pinned = self.inner.pinned(&store);
            for m in pinned.iter_mut().filter(|m| m.id == mix_id) {
                m.songs.retain(|s| s.song_key != key);
            }
            Inner::store_json(&store, PINNED_KEY, &pinned)?;
            if let Some(mut set) = Inner::stored::<recs::MixSet>(&store, MIX_SET_KEY) {
                for m in set.mixes.iter_mut().filter(|m| m.id == mix_id) {
                    m.songs.retain(|s| s.song_key != key);
                }
                Inner::store_json(&store, MIX_SET_KEY, &set)?;
            }
        }
        self.feedback(song_id, false)
    }

    /// Save as playlist (board 15): a normal playlist with the mix's songs and
    /// title, written to the media server like any other.
    pub fn save_mix_as_playlist(&self, mix_id: String) -> Result<EditResult, EncoreError> {
        let page = self.mix(mix_id)?;
        let ids: Vec<i64> = page.songs.iter().map(|s| s.song.id).collect();
        let made = self.add_to(
            DropTarget {
                kind: DropTargetKind::NewPlaylist,
                id: None,
            },
            ids,
            None,
            true,
        )?;
        if let Some(pl) = made.playlist_id {
            self.rename_playlist(pl, page.summary.title)?;
        }
        Ok(made)
    }

    /// How suggestions are doing (board 38, note 7): local, the last 4 weeks.
    pub fn suggestion_stats(&self) -> Result<SuggestionStats, EncoreError> {
        let p = self.inner.profile()?;
        let (_, catalog, _) = self.inner.recs_model(&p)?;
        let plays: Vec<(recs::Play, String)> = lock(&p.store)?
            .recs_plays()?
            .into_iter()
            .map(|r| {
                (
                    recs::Play {
                        outcome: r.signal.as_deref().and_then(recs::Outcome::from_signal),
                        app_chosen: r.chosen_by == "app",
                        strong_intent: false,
                        song_key: r.song_key,
                        started_utc_ms: r.started_utc_ms,
                        utc_offset_minutes: r.utc_offset_minutes,
                        listened_ms: r.listened_ms,
                    },
                    r.started_from,
                )
            })
            .collect();
        let s = recs::stats(&plays, &catalog, now_ms());
        Ok(SuggestionStats {
            since_utc_ms: s.since_utc_ms,
            shelves: s
                .shelves
                .into_iter()
                .map(|x| ShelfStat {
                    shelf: x.shelf,
                    plays: x.plays,
                    early_skips_per_1000: x.early_skips_per_1000,
                    finished_per_1000: x.finished_per_1000,
                    artists_per_20_x10: x.artists_per_20_x10,
                    repeats_per_1000: x.repeats_per_1000,
                })
                .collect(),
        })
    }
}

impl EncoreCore {
    fn feedback(&self, song_id: i64, more: bool) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        if !self.inner.learns(&p)? {
            return Err(EncoreError::NotAllowed {
                reason: "Suggestions aren't learning right now".into(),
            });
        }
        let key =
            Inner::key_of(&*lock(&p.store)?, song_id).ok_or_else(|| EncoreError::NotFound {
                reason: "That song isn't in your library".into(),
            })?;
        let bucket = self.inner.bucket_now(&p);
        let id = crate::edits::new_id();
        lock(&p.store)?.add_feedback(&FeedbackRow {
            id: id.clone(),
            song_key: key.clone(),
            more,
            bucket,
            at_utc_ms: now_ms(),
        })?;
        self.inner.record_event(
            &p,
            "feedback",
            serde_json::json!({ "songKey": key, "more": more, "bucket": bucket }),
        )?;
        self.send_soon();
        Ok(())
    }

    fn put_not_interested(
        &self,
        scope: &str,
        value: &str,
        label: &str,
        reason: NotInterestedReason,
        removed: bool,
    ) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let now = now_ms();
        let hlc = lock(&p.edits_clock)?.tick(now).to_string();
        let bucket = self.inner.bucket_now(&p);
        lock(&p.store)?.put_not_interested(&NotInterestedRow {
            scope: scope.to_owned(),
            value: value.to_owned(),
            label: label.to_owned(),
            reason: reason_name(reason).to_owned(),
            bucket,
            at_utc_ms: now,
            hlc: hlc.clone(),
            device_id: self.inner.config.device_id.clone(),
            removed,
        })?;
        // The event carries the same clock as the row, so every device ends the same.
        lock(&p.store)?.insert_event(&EventRecord {
            id: crate::edits::new_id(),
            device_id: self.inner.config.device_id.clone(),
            hlc,
            kind: "notInterested".into(),
            payload: serde_json::json!({ "scope": scope, "value": value, "label": label,
                "reason": reason_name(reason), "bucket": bucket, "removed": removed })
            .to_string(),
            created_utc_ms: now,
        })?;
        // Upcoming autoplay songs it covers go at once.
        let _ = self.inner.refill_autoplay(true);
        self.send_soon();
        Ok(())
    }
}

fn home_of(
    inner: &Inner,
    regenerate: bool,
    from_server: Option<recs::MixSet>,
) -> Result<Home, EncoreError> {
    use crate::playlog::ListeningMode as M;
    let p = inner.profile()?;
    let mode = lock(&p.playback)?.log.mode();
    let (model, catalog, ids) = inner.recs_model(&p)?;
    let from_companion = from_server.is_some() && mode != M::LearningOff;
    let set = if mode == M::LearningOff {
        None
    } else if let Some(set) = from_server {
        Some(set)
    } else {
        Some(inner.mix_set(regenerate)?)
    };
    let shelves = recs::build_shelves(&model, &catalog, &params());
    let store = lock(&p.store)?;
    let song = |k: &str| {
        ids.get(k)
            .and_then(|id| store.track(*id).ok())
            .map(Song::from)
    };
    let pinned = inner.pinned(&store);
    let now = now_ms();
    let made = set.as_ref().map(|s| s.made_utc_ms);
    let mut mixes: Vec<MixSummary> = pinned
        .iter()
        .map(|m| inner.summary(&ids, &store, m, true, 0))
        .collect();
    if let Some(s) = &set {
        for m in &s.mixes {
            if !pinned.iter().any(|x| x.id == m.id) {
                mixes.push(inner.summary(&ids, &store, m, false, s.made_utc_ms));
            }
        }
    }
    let p_full = params().full_from_plays;
    let state = match (&set, mode) {
        (None, _) => MixState::LearningOff,
        (_, M::Private | M::Guest) => MixState::Private,
        (Some(s), _) if s.learning => MixState::NotEnoughPlays,
        (Some(s), _) if now - s.made_utc_ms > 48 * 3_600_000 => MixState::OutOfDate,
        _ => MixState::Fresh,
    };
    let mut recently_played = Vec::new();
    for e in store.recent_play_events(100)? {
        if recently_played.len() >= 12 {
            break;
        }
        if let Some(s) = song(&e.song_key)
            && !recently_played.iter().any(|x: &Song| x.id == s.id)
        {
            recently_played.push(s);
        }
    }
    let recently_added = store
        .tracks(
            encore_store::TrackSort::RecentlyAdded,
            encore_store::Page {
                offset: 0,
                limit: 12,
            },
        )?
        .into_iter()
        .map(Song::from)
        .collect();
    let most_played: Vec<Song> = store
        .most_played(12)?
        .into_iter()
        .filter_map(|id| store.track(id).ok())
        .map(Song::from)
        .collect();
    Ok(Home {
        made_for_you: MixShelf {
            state,
            mixes,
            plays_so_far: model.plays_counted.min(p_full),
            plays_needed: p_full,
            made_utc_ms: made,
            from_companion,
        },
        rediscover: shelves
            .rediscover
            .iter()
            .filter_map(|x| song(&x.song_key))
            .collect(),
        recently_played,
        deep_cuts: shelves
            .deep_cuts
            .iter()
            .filter_map(|x| song(&x.song_key))
            .take(12)
            .collect(),
        recently_added,
        most_played,
    })
}

/// The profile's song list for the companion server, and its hash (SHA-256 of
/// the JSON, so an unchanged list isn't sent again).
fn catalog_payload(store: &Store) -> Result<(serde_json::Value, String), EncoreError> {
    use sha2::{Digest, Sha256};
    let (catalog, _, _) = inputs(store, true)?;
    let songs = serde_json::to_value(catalog.songs()).map_err(EncoreError::internal)?;
    let hash: String = Sha256::digest(songs.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((songs, hash))
}

/// Send the song list to the companion server if it doesn't have this one.
pub(super) async fn upload_catalog(
    inner: &Inner,
    p: &Profile,
    s: &super::sync_loop::Session,
) -> Result<bool, crate::companion::CompanionError> {
    let local = |e: EncoreError| crate::companion::CompanionError::Refused(e.to_string());
    let (songs, hash) = {
        let store = lock(&p.store).map_err(local)?;
        catalog_payload(&store).map_err(local)?
    };
    let held =
        super::sync_loop::signed(inner, s, |a| async move { s.client.catalog_hash(&a).await })
            .await?;
    if held.as_deref() == Some(hash.as_str()) {
        return Ok(false);
    }
    let (songs, hash) = (&songs, &hash);
    super::sync_loop::signed(inner, s, |a| async move {
        s.client.put_catalog(&a, hash, songs).await
    })
    .await?;
    Ok(true)
}

/// Today's set from the companion server, when the profile has one and it
/// answers; None otherwise (the device builds it itself).
async fn server_set(inner: &Arc<Inner>, regenerate: bool) -> Option<recs::MixSet> {
    use crate::playlog::ListeningMode as M;
    let p = inner.profile().ok()?;
    if lock(&p.playback).ok()?.log.mode() == M::LearningOff {
        return None;
    }
    let s = lock(&p.companion).ok()?.clone()?;
    let (offset, playing) = {
        let pb = lock(&p.playback).ok()?;
        (
            pb.utc_offset_minutes,
            pb.queue.current().and_then(|c| c.context_id.clone()),
        )
    };
    let (hidden, titles) = {
        let store = lock(&p.store).ok()?;
        let hidden: Vec<String> = Inner::stored(&store, HIDDEN_KEY).unwrap_or_default();
        let titles: Vec<(i64, String)> = Inner::stored(&store, TITLES_KEY).unwrap_or_default();
        (
            hidden,
            titles.into_iter().map(|(_, t)| t).collect::<Vec<String>>(),
        )
    };
    let body = serde_json::json!({ "utcOffsetMinutes": offset, "hidden": hidden,
        "recentTitles": titles, "regenerate": regenerate, "playingMixId": playing });
    let ask = || {
        let (s, body) = (&s, &body);
        super::sync_loop::signed(inner, s, move |a| async move {
            s.client.recs_home(&a, body).await
        })
    };
    let reply = match ask().await {
        // No song list there yet: send it, then ask again.
        Err(crate::companion::CompanionError::Refused(_)) => {
            upload_catalog(inner, &p, &s).await.ok()?;
            ask().await.ok()?
        }
        other => other.ok()?,
    };
    let set: recs::MixSet = serde_json::from_value(reply).ok()?;
    if let Ok(store) = lock(&p.store) {
        let _ = Inner::store_json(&store, MIX_SET_KEY, &set);
    }
    Some(set)
}
