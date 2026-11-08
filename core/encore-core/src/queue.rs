//! The play queue (plan section 8, boards 11 and 42). Pure logic: no I/O, no clock.
//!
//! Three parts, like Apple Music: History (played, newest last), Up Next (what the
//! user chose: the rest of the album or playlist plus anything added) and Autoplay
//! (what the app picked; filled by the smart-picks engine from Phase 5).
//!
//! - Shuffle reorders Up Next only, never History, and turning it off restores the
//!   order the songs were added in.
//! - Repeat One replays a song when it ends by itself; skipping moves on.
//! - Repeat All starts the session's songs again after the last one.
//! - Previous restarts the current song after `previousButtonRestartAfterSeconds`;
//!   the swipe back always goes to the previous song.
//! - Every change can be undone once (the Undo toasts on board 11).

use serde::{Deserialize, Serialize};

/// Who chose a song. A skip of the app's pick is the strongest negative signal
/// (plan section 21), so this is stored with every play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChosenBy {
    User,
    App,
}

impl ChosenBy {
    pub fn as_str(self) -> &'static str {
        match self {
            ChosenBy::User => "user",
            ChosenBy::App => "app",
        }
    }
}

/// What started a song (plan section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartedFrom {
    Album,
    Playlist,
    Artist,
    Songs,
    Genre,
    Search,
    Radio,
    Mix,
    Autoplay,
    /// A suggestion Smart shuffle put in a shuffled list.
    SmartShuffle,
    Queue,
    History,
    Other,
}

impl StartedFrom {
    pub fn as_str(self) -> &'static str {
        match self {
            StartedFrom::Album => "album",
            StartedFrom::Playlist => "playlist",
            StartedFrom::Artist => "artist",
            StartedFrom::Songs => "songs",
            StartedFrom::Genre => "genre",
            StartedFrom::Search => "search",
            StartedFrom::Radio => "radio",
            StartedFrom::Mix => "mix",
            StartedFrom::Autoplay => "autoplay",
            StartedFrom::SmartShuffle => "smartShuffle",
            StartedFrom::Queue => "queue",
            StartedFrom::History => "history",
            StartedFrom::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// One song in the queue. `uid` tells two copies of the same song apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub uid: u64,
    /// The song's row in the profile store.
    pub track_id: i64,
    pub chosen_by: ChosenBy,
    pub started_from: StartedFrom,
    /// The album, playlist or mix it came from (the source's ID), if any.
    pub context_id: Option<String>,
    /// The order it was added in; unshuffling sorts by this.
    order: i64,
}

/// What the player should do after a Previous press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviousOutcome {
    /// Seek the current song back to 0:00.
    Restart,
    /// Play the new current song.
    Moved,
}

/// Why the queue moves forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The song played to its end (Repeat One replays it).
    Ended,
    /// The user pressed Next or swiped (Repeat One moves on).
    Skip,
}

/// A song to add, before it has a place in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    pub track_id: i64,
    pub chosen_by: ChosenBy,
    pub started_from: StartedFrom,
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct State {
    history: Vec<QueueItem>,
    current: Option<QueueItem>,
    up_next: Vec<QueueItem>,
    autoplay: Vec<QueueItem>,
    /// Songs of this listening session in their first order, for Repeat All.
    session: Vec<QueueItem>,
    shuffle: bool,
    repeat: RepeatMode,
    autoplay_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Queue {
    state: State,
    undo: Option<State>,
    next_uid: u64,
    next_order: i64,
    rng: SplitMix64,
    /// Offline (Phase 8): the songs that can play (downloaded or fully cached).
    /// Moving forward or back passes over the rest (board 11: "skipped
    /// automatically"); None plays everything. Not part of the undo state.
    playable: Option<std::collections::HashSet<i64>>,
}

/// A read-only view for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueView<'a> {
    pub history: &'a [QueueItem],
    pub current: Option<&'a QueueItem>,
    pub up_next: &'a [QueueItem],
    pub autoplay: &'a [QueueItem],
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub autoplay_enabled: bool,
}

impl Queue {
    /// An empty queue. `seed` drives shuffling, so tests can be exact.
    pub fn new(seed: u64) -> Queue {
        Queue {
            state: State {
                autoplay_enabled: true,
                ..State::default()
            },
            undo: None,
            next_uid: 1,
            next_order: 0,
            rng: SplitMix64(seed),
            playable: None,
        }
    }

    /// Offline: only these songs can play; None when online.
    pub fn set_playable(&mut self, playable: Option<std::collections::HashSet<i64>>) {
        self.playable = playable;
    }

    /// Whether this song can play now.
    pub fn can_play(&self, track_id: i64) -> bool {
        self.playable.as_ref().is_none_or(|p| p.contains(&track_id))
    }

    fn ok(&self, i: &QueueItem) -> bool {
        self.can_play(i.track_id)
    }

    pub fn view(&self) -> QueueView<'_> {
        QueueView {
            history: &self.state.history,
            current: self.state.current.as_ref(),
            up_next: &self.state.up_next,
            autoplay: &self.state.autoplay,
            shuffle: self.state.shuffle,
            repeat: self.state.repeat,
            autoplay_enabled: self.state.autoplay_enabled,
        }
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.state.current.as_ref()
    }

    fn make(&mut self, n: NewItem) -> QueueItem {
        let uid = self.next_uid;
        self.next_uid += 1;
        self.next_order += 1;
        QueueItem {
            uid,
            track_id: n.track_id,
            chosen_by: n.chosen_by,
            started_from: n.started_from,
            context_id: n.context_id,
            order: self.next_order,
        }
    }

    fn save_undo(&mut self) {
        self.undo = Some(self.state.clone());
    }

    /// Undo the last change (board 11's Undo toasts). Returns false if there is none.
    pub fn undo(&mut self) -> bool {
        match self.undo.take() {
            Some(s) => {
                self.state = s;
                true
            }
            None => false,
        }
    }

    /// Play a list (an album, a playlist, search results) from `start`. The current
    /// song moves to History; the rest of the list becomes Up Next.
    pub fn play(&mut self, items: Vec<NewItem>, start: usize) {
        if items.is_empty() {
            return;
        }
        self.save_undo();
        let start = start.min(items.len() - 1);
        let made: Vec<QueueItem> = items.into_iter().map(|n| self.make(n)).collect();
        if let Some(cur) = self.state.current.take() {
            self.state.history.push(cur);
        }
        self.state.session = made.clone();
        let mut rest: Vec<QueueItem> = made;
        let current = rest.remove(start);
        // Songs before the start belong to the session but play only on Repeat All.
        let after: Vec<QueueItem> = rest.split_off(start);
        self.state.current = Some(current);
        self.state.up_next = after;
        if self.state.shuffle {
            self.shuffle_up_next();
        }
    }

    /// Put a whole queue in place as another device had it (a handoff): History,
    /// the current song, Up Next and Autoplay in their order, with its shuffle,
    /// repeat and Autoplay settings. Up Next is not shuffled again: it arrives in
    /// the order it was playing in, which unshuffling then keeps.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        &mut self,
        history: Vec<NewItem>,
        current: NewItem,
        up_next: Vec<NewItem>,
        autoplay: Vec<NewItem>,
        shuffle: bool,
        repeat: RepeatMode,
        autoplay_enabled: bool,
    ) {
        self.save_undo();
        let history: Vec<QueueItem> = history.into_iter().map(|n| self.make(n)).collect();
        let current = self.make(current);
        let up_next: Vec<QueueItem> = up_next.into_iter().map(|n| self.make(n)).collect();
        let autoplay: Vec<QueueItem> = autoplay.into_iter().map(|n| self.make(n)).collect();
        let mut session = vec![current.clone()];
        session.extend(up_next.iter().cloned());
        self.state = State {
            history,
            current: Some(current),
            up_next,
            autoplay: if autoplay_enabled {
                autoplay
            } else {
                Vec::new()
            },
            session,
            shuffle,
            repeat,
            autoplay_enabled,
        };
    }

    /// Insert at the top of Up Next.
    pub fn play_next(&mut self, items: Vec<NewItem>) {
        self.save_undo();
        let first_order = self
            .state
            .up_next
            .iter()
            .map(|i| i.order)
            .min()
            .unwrap_or(self.next_order)
            - 1;
        let mut made: Vec<QueueItem> = items.into_iter().map(|n| self.make(n)).collect();
        // Keep the added songs in the order given, above everything already queued.
        let base = first_order - made.len() as i64;
        for (k, item) in made.iter_mut().enumerate() {
            item.order = base + k as i64;
        }
        self.state.up_next.splice(0..0, made);
    }

    /// Add to the end of Up Next.
    pub fn add_to_up_next(&mut self, items: Vec<NewItem>) {
        self.save_undo();
        let made: Vec<QueueItem> = items.into_iter().map(|n| self.make(n)).collect();
        self.state.up_next.extend(made);
    }

    /// Insert into Up Next at `index` (a drop at the line, board 43). At the top
    /// it is Play Next; past the end it is Add to Queue. The original order keeps
    /// the songs where they were dropped, so turning shuffle off leaves them there.
    pub fn insert_up_next(&mut self, items: Vec<NewItem>, index: usize) {
        let len = self.state.up_next.len();
        if index == 0 {
            return self.play_next(items);
        }
        if index >= len {
            return self.add_to_up_next(items);
        }
        self.save_undo();
        let n = items.len() as i64;
        let prev = self.state.up_next[index - 1].order;
        for i in self.state.up_next.iter_mut() {
            if i.order > prev {
                i.order += n;
            }
        }
        let mut made: Vec<QueueItem> = items.into_iter().map(|x| self.make(x)).collect();
        for (k, item) in made.iter_mut().enumerate() {
            item.order = prev + 1 + k as i64;
        }
        self.next_order += n;
        self.state.up_next.splice(index..index, made);
    }

    /// Replace the Autoplay part (the smart-picks engine calls this).
    pub fn set_autoplay(&mut self, items: Vec<NewItem>) {
        let made: Vec<QueueItem> = items.into_iter().map(|n| self.make(n)).collect();
        self.state.autoplay = made;
    }

    pub fn set_autoplay_enabled(&mut self, on: bool) {
        self.state.autoplay_enabled = on;
    }

    pub fn remove(&mut self, uid: u64) -> bool {
        let before = self.state.clone();
        let removed = remove_uid(&mut self.state.up_next, uid)
            .or_else(|| remove_uid(&mut self.state.autoplay, uid));
        if removed.is_some() {
            self.undo = Some(before);
        }
        removed.is_some()
    }

    /// Move a song to `to` in Up Next. An Autoplay song moved this way joins Up Next.
    pub fn move_item(&mut self, uid: u64, to: usize) -> bool {
        let before = self.state.clone();
        let item = match remove_uid(&mut self.state.up_next, uid)
            .or_else(|| remove_uid(&mut self.state.autoplay, uid))
        {
            Some(i) => i,
            None => return false,
        };
        let to = to.min(self.state.up_next.len());
        self.state.up_next.insert(to, item);
        // The order now follows the new position, so unshuffling keeps the move.
        renumber(&mut self.state.up_next);
        self.undo = Some(before);
        true
    }

    /// Empty Up Next. Autoplay stays (board 11, item 6).
    pub fn clear_up_next(&mut self) {
        self.save_undo();
        self.state.up_next.clear();
    }

    /// Jump to a song in Up Next or Autoplay. The songs passed over go to History.
    pub fn jump_to(&mut self, uid: u64) -> bool {
        let in_up_next = self.state.up_next.iter().position(|i| i.uid == uid);
        let in_auto = self.state.autoplay.iter().position(|i| i.uid == uid);
        if in_up_next.is_none() && in_auto.is_none() {
            return false;
        }
        self.save_undo();
        if let Some(cur) = self.state.current.take() {
            self.state.history.push(cur);
        }
        let target = if let Some(p) = in_up_next {
            let passed: Vec<QueueItem> = self.state.up_next.drain(..p).collect();
            self.state.history.extend(passed);
            self.state.up_next.remove(0)
        } else {
            let p = in_auto.unwrap_or(0);
            let passed: Vec<QueueItem> = self
                .state
                .up_next
                .drain(..)
                .chain(self.state.autoplay.drain(..p))
                .collect();
            self.state.history.extend(passed);
            self.state.autoplay.remove(0)
        };
        self.state.current = Some(target);
        true
    }

    /// Play a song from History again; the current song goes to History.
    pub fn replay_from_history(&mut self, uid: u64) -> bool {
        let Some(p) = self.state.history.iter().position(|i| i.uid == uid) else {
            return false;
        };
        self.save_undo();
        let again = self.state.history.remove(p);
        if let Some(cur) = self.state.current.take() {
            self.state.history.push(cur);
        }
        self.state.current = Some(again);
        true
    }

    /// The song that plays when the current one ends by itself, if any. This is what
    /// the player preloads.
    pub fn peek_after_end(&self) -> Option<&QueueItem> {
        if self.state.repeat == RepeatMode::One {
            return self.state.current.as_ref();
        }
        self.peek_next()
    }

    /// Where Next (or a swipe forward) would land; None is the end of the queue,
    /// where the UI shows the rubber band (board 42).
    pub fn peek_skip(&self) -> Option<&QueueItem> {
        self.peek_next()
    }

    /// Where a swipe back would land: the song before, or with Repeat All at the
    /// first song, the session's last. None is the start of the queue.
    pub fn peek_previous(&self) -> Option<&QueueItem> {
        self.state
            .history
            .iter()
            .rev()
            .find(|i| self.ok(i))
            .or_else(|| {
                (self.state.repeat == RepeatMode::All)
                    .then(|| self.state.session.iter().rev().find(|i| self.ok(i)))
                    .flatten()
            })
    }

    fn peek_next(&self) -> Option<&QueueItem> {
        if let Some(i) = self.state.up_next.iter().find(|i| self.ok(i)) {
            return Some(i);
        }
        if self.state.repeat == RepeatMode::All {
            return self.state.session.iter().find(|i| self.ok(i));
        }
        if self.state.autoplay_enabled {
            return self.state.autoplay.iter().find(|i| self.ok(i));
        }
        None
    }

    /// Move forward. Returns the new current song, or None at the end of the queue
    /// (the current song stays, as the "End of queue" bounce on board 42 shows).
    pub fn advance(&mut self, why: Advance) -> Option<&QueueItem> {
        if why == Advance::Ended && self.state.repeat == RepeatMode::One {
            return self.state.current.as_ref();
        }
        // Offline, songs that can't play are passed over: they go to History as
        // if jumped past (board 11). Online every song can play, so this is the
        // first of each part as before.
        let first_up_next = self.state.up_next.iter().position(|i| self.ok(i));
        let first_auto = self.state.autoplay.iter().position(|i| self.ok(i));
        let session_can = self.state.session.iter().any(|i| self.ok(i));
        let mut passed: Vec<QueueItem> = Vec::new();
        let next = if let Some(p) = first_up_next {
            passed.extend(self.state.up_next.drain(..p));
            Some(self.state.up_next.remove(0))
        } else if self.state.repeat == RepeatMode::All && session_can {
            passed.append(&mut self.state.up_next);
            // Start the session again with fresh copies.
            let again: Vec<QueueItem> = self
                .state
                .session
                .clone()
                .into_iter()
                .map(|i| {
                    self.make(NewItem {
                        track_id: i.track_id,
                        chosen_by: i.chosen_by,
                        started_from: i.started_from,
                        context_id: i.context_id,
                    })
                })
                .collect();
            self.state.up_next = again;
            if self.state.shuffle {
                self.shuffle_up_next();
            }
            let p = self
                .state
                .up_next
                .iter()
                .position(|i| self.ok(i))
                .unwrap_or(0);
            passed.extend(self.state.up_next.drain(..p));
            Some(self.state.up_next.remove(0))
        } else if self.state.autoplay_enabled
            && let Some(p) = first_auto
        {
            passed.append(&mut self.state.up_next);
            passed.extend(self.state.autoplay.drain(..p));
            Some(self.state.autoplay.remove(0))
        } else {
            None
        };
        let next = next?;
        self.undo = None;
        if let Some(cur) = self.state.current.take() {
            self.state.history.push(cur);
        }
        self.state.history.extend(passed);
        self.state.current = Some(next);
        self.state.current.as_ref()
    }

    /// Previous. `position_ms` is how far into the current song the player is;
    /// `restart_after_ms` is the 3 s rule. `always_back` is the swipe, which never
    /// restarts. At the first song it restarts (or wraps on Repeat All).
    pub fn previous(
        &mut self,
        position_ms: u64,
        restart_after_ms: u64,
        always_back: bool,
    ) -> PreviousOutcome {
        if !always_back && position_ms > restart_after_ms {
            return PreviousOutcome::Restart;
        }
        // Offline, songs that can't play are stepped over and stay ahead.
        let mut stepped: Vec<QueueItem> = Vec::new();
        while let Some(last) = self.state.history.last() {
            if self.ok(last) {
                break;
            }
            if let Some(i) = self.state.history.pop() {
                stepped.push(i);
            }
        }
        let popped = self.state.history.pop();
        if popped.is_none() && self.state.repeat != RepeatMode::All {
            // Nothing playable behind: put back what was stepped over.
            self.state.history.extend(stepped.into_iter().rev());
            return PreviousOutcome::Restart;
        }
        let prev = match popped {
            Some(p) => p,
            // The first song with Repeat All wraps to the session's last (board 42).
            None if self.state.repeat == RepeatMode::All => {
                match self
                    .state
                    .session
                    .iter()
                    .rev()
                    .find(|i| self.ok(i))
                    .cloned()
                {
                    Some(last) => self.make(NewItem {
                        track_id: last.track_id,
                        chosen_by: last.chosen_by,
                        started_from: last.started_from,
                        context_id: last.context_id,
                    }),
                    None => {
                        self.state.history.extend(stepped.into_iter().rev());
                        return PreviousOutcome::Restart;
                    }
                }
            }
            None => {
                self.state.history.extend(stepped.into_iter().rev());
                return PreviousOutcome::Restart;
            }
        };
        self.undo = None;
        if let Some(cur) = self.state.current.take() {
            self.state.up_next.insert(0, cur);
        }
        // What was stepped over goes back ahead, in its order.
        for i in stepped {
            self.state.up_next.insert(0, i);
        }
        self.state.current = Some(prev);
        PreviousOutcome::Moved
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.state.repeat = mode;
    }

    /// Turn shuffle on (Up Next is shuffled) or off (the original order returns).
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.state.shuffle {
            return;
        }
        self.state.shuffle = on;
        if on {
            self.shuffle_up_next();
        } else {
            self.state.up_next.sort_by_key(|i| i.order);
        }
    }

    fn shuffle_up_next(&mut self) {
        // Fisher-Yates.
        let n = self.state.up_next.len();
        for i in (1..n).rev() {
            let j = (self.rng.next() % (i as u64 + 1)) as usize;
            self.state.up_next.swap(i, j);
        }
    }
}

fn remove_uid(list: &mut Vec<QueueItem>, uid: u64) -> Option<QueueItem> {
    let p = list.iter().position(|i| i.uid == uid)?;
    Some(list.remove(p))
}

fn renumber(list: &mut [QueueItem]) {
    let base = list.iter().map(|i| i.order).min().unwrap_or(0);
    for (k, item) in list.iter_mut().enumerate() {
        item.order = base + k as i64;
    }
}

/// Small, fast, seedable generator (Steele et al., SplitMix64). Not for secrets.
#[derive(Debug, Clone)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(ids: &[i64]) -> Vec<NewItem> {
        ids.iter()
            .map(|&id| NewItem {
                track_id: id,
                chosen_by: ChosenBy::User,
                started_from: StartedFrom::Album,
                context_id: Some("al1".into()),
            })
            .collect()
    }

    fn picks(ids: &[i64]) -> Vec<NewItem> {
        ids.iter()
            .map(|&id| NewItem {
                track_id: id,
                chosen_by: ChosenBy::App,
                started_from: StartedFrom::Autoplay,
                context_id: None,
            })
            .collect()
    }

    fn ids(list: &[QueueItem]) -> Vec<i64> {
        list.iter().map(|i| i.track_id).collect()
    }

    fn cur(q: &Queue) -> Option<i64> {
        q.current().map(|i| i.track_id)
    }

    /// Offline (Phase 8, board 11): songs that can't play are passed over, forward
    /// and back, and land in History or stay ahead in their order; online nothing
    /// changes.
    #[test]
    fn offline_the_queue_passes_over_songs_that_cannot_play() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3, 4, 5]), 0);
        q.set_autoplay(picks(&[90, 91]));
        q.set_playable(Some([1, 3, 5, 91].into_iter().collect()));
        assert_eq!(q.peek_skip().map(|i| i.track_id), Some(3));
        assert_eq!(q.advance(Advance::Skip).map(|i| i.track_id), Some(3));
        assert_eq!(ids(q.view().history), [1, 2], "2 was passed over");
        assert_eq!(q.advance(Advance::Ended).map(|i| i.track_id), Some(5));
        assert_eq!(ids(q.view().history), [1, 2, 3, 4]);
        // Up Next is spent: Autoplay's first playable pick.
        assert_eq!(q.advance(Advance::Skip).map(|i| i.track_id), Some(91));
        assert!(
            q.advance(Advance::Skip).is_none(),
            "nothing left that can play"
        );
        assert_eq!(cur(&q), Some(91));
        // Back steps over what can't play and keeps it ahead.
        assert_eq!(q.peek_previous().map(|i| i.track_id), Some(5));
        assert_eq!(q.previous(0, 3000, true), PreviousOutcome::Moved);
        assert_eq!(cur(&q), Some(5));
        assert_eq!(q.previous(0, 3000, true), PreviousOutcome::Moved);
        assert_eq!(cur(&q), Some(3));
        assert_eq!(ids(q.view().up_next)[..2], [4, 5], "4 stays ahead of 5");
        // Online again, every song plays.
        q.set_playable(None);
        assert_eq!(q.peek_skip().map(|i| i.track_id), Some(4));
    }

    #[test]
    fn offline_with_nothing_playable_behind_previous_restarts() {
        let mut q = Queue::new(1);
        q.play(album(&[7, 8]), 0);
        q.advance(Advance::Skip);
        q.set_playable(Some([8].into_iter().collect()));
        let history = ids(q.view().history);
        assert_eq!(q.previous(0, 3000, true), PreviousOutcome::Restart);
        assert_eq!(ids(q.view().history), history, "nothing lost");
    }
    #[test]
    fn playing_an_album_from_the_middle() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3, 4]), 1);
        assert_eq!(cur(&q), Some(2));
        assert_eq!(ids(q.view().up_next), [3, 4]);
        assert!(q.view().history.is_empty());
    }

    #[test]
    fn songs_move_through_history_up_next_and_autoplay() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2]), 0);
        q.set_autoplay(picks(&[90, 91]));
        assert_eq!(q.advance(Advance::Ended).map(|i| i.track_id), Some(2));
        assert_eq!(q.advance(Advance::Ended).map(|i| i.track_id), Some(90));
        assert_eq!(q.current().unwrap().chosen_by, ChosenBy::App);
        assert_eq!(ids(q.view().history), [1, 2]);
        assert_eq!(ids(q.view().autoplay), [91]);
    }

    #[test]
    fn the_end_of_the_queue_keeps_the_current_song() {
        let mut q = Queue::new(1);
        q.play(album(&[1]), 0);
        q.set_autoplay_enabled(false);
        assert!(q.advance(Advance::Skip).is_none());
        assert_eq!(cur(&q), Some(1));
    }

    #[test]
    fn play_next_goes_on_top_and_add_goes_to_the_end() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 0);
        q.play_next(album(&[10, 11]));
        q.add_to_up_next(album(&[20]));
        assert_eq!(ids(q.view().up_next), [10, 11, 2, 3, 20]);
    }

    #[test]
    fn shuffle_touches_up_next_only_and_comes_back_in_order() {
        let mut q = Queue::new(42);
        q.play(album(&[1, 2, 3, 4, 5, 6, 7, 8]), 0);
        q.advance(Advance::Ended);
        q.set_shuffle(true);
        assert_eq!(ids(q.view().history), [1]);
        assert_eq!(cur(&q), Some(2));
        let shuffled = ids(q.view().up_next);
        assert_ne!(shuffled, [3, 4, 5, 6, 7, 8]);
        let mut sorted = shuffled.clone();
        sorted.sort();
        assert_eq!(sorted, [3, 4, 5, 6, 7, 8]);
        q.set_shuffle(false);
        assert_eq!(ids(q.view().up_next), [3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn shuffle_is_the_same_for_the_same_seed() {
        let order = |seed| {
            let mut q = Queue::new(seed);
            q.set_shuffle(true);
            q.play(album(&[1, 2, 3, 4, 5, 6]), 0);
            ids(q.view().up_next)
        };
        assert_eq!(order(7), order(7));
        assert_ne!(order(7), order(8));
    }

    #[test]
    fn repeat_one_replays_on_end_but_skip_moves_on() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 0);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.peek_after_end().map(|i| i.track_id), Some(1));
        assert_eq!(q.advance(Advance::Ended).map(|i| i.track_id), Some(1));
        assert_eq!(q.advance(Advance::Skip).map(|i| i.track_id), Some(2));
        assert_eq!(q.advance(Advance::Ended).map(|i| i.track_id), Some(2));
    }

    #[test]
    fn repeat_all_starts_the_session_again() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 1);
        q.set_repeat(RepeatMode::All);
        q.advance(Advance::Ended); // 3
        assert_eq!(q.peek_after_end().map(|i| i.track_id), Some(1));
        assert_eq!(q.advance(Advance::Ended).map(|i| i.track_id), Some(1));
        assert_eq!(ids(q.view().up_next), [2, 3]);
    }

    #[test]
    fn a_drop_at_the_line_stays_there_through_shuffle() {
        let mut q = Queue::new(3);
        q.play(album(&[1, 2, 3, 4]), 0);
        q.insert_up_next(album(&[8, 9]), 1);
        assert_eq!(ids(q.view().up_next), [2, 8, 9, 3, 4]);
        q.set_shuffle(true);
        q.set_shuffle(false);
        assert_eq!(ids(q.view().up_next), [2, 8, 9, 3, 4]);
        q.insert_up_next(album(&[7]), 0);
        q.insert_up_next(album(&[6]), 99);
        assert_eq!(ids(q.view().up_next), [7, 2, 8, 9, 3, 4, 6]);
        assert!(q.undo());
        assert_eq!(ids(q.view().up_next), [7, 2, 8, 9, 3, 4]);
    }

    /// Board 42: the targets a swipe shows, the end of the queue, and the wrap.
    #[test]
    fn swipe_targets_and_the_ends_of_the_queue() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 0);
        q.set_autoplay_enabled(false);
        assert_eq!(q.peek_skip().map(|i| i.track_id), Some(2));
        assert_eq!(q.peek_previous(), None, "the first song: the rubber band");
        q.advance(Advance::Skip);
        q.advance(Advance::Skip);
        assert_eq!(
            q.peek_skip(),
            None,
            "the last song, no autoplay: the rubber band"
        );
        assert_eq!(q.peek_previous().map(|i| i.track_id), Some(2));

        // Repeat All: both ends wrap.
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 0);
        q.set_autoplay_enabled(false);
        q.set_repeat(RepeatMode::All);
        assert_eq!(q.peek_previous().map(|i| i.track_id), Some(3));
        assert_eq!(q.previous(0, 3_000, true), PreviousOutcome::Moved);
        assert_eq!(cur(&q), Some(3), "swiping back from the first song wraps");
        assert_eq!(
            ids(q.view().up_next),
            [1, 2, 3],
            "the song left is next again"
        );
        q.advance(Advance::Skip);
        q.advance(Advance::Skip);
        q.advance(Advance::Skip);
        assert_eq!(q.peek_skip().map(|i| i.track_id), Some(1));
    }

    #[test]
    fn previous_restarts_after_three_seconds_but_the_swipe_goes_back() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 0);
        q.advance(Advance::Skip);
        assert_eq!(q.previous(5_000, 3_000, false), PreviousOutcome::Restart);
        assert_eq!(cur(&q), Some(2));
        assert_eq!(q.previous(5_000, 3_000, true), PreviousOutcome::Moved);
        assert_eq!(cur(&q), Some(1));
        assert_eq!(ids(q.view().up_next), [2, 3]);
        // At the first song there is nothing behind: restart.
        assert_eq!(q.previous(1_000, 3_000, false), PreviousOutcome::Restart);
        assert_eq!(cur(&q), Some(1));
    }

    #[test]
    fn jumping_ahead_puts_the_passed_songs_in_history_and_can_be_undone() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3, 4]), 0);
        let target = q.view().up_next[2].uid; // song 4
        assert!(q.jump_to(target));
        assert_eq!(cur(&q), Some(4));
        assert_eq!(ids(q.view().history), [1, 2, 3]);
        assert!(q.undo());
        assert_eq!(cur(&q), Some(1));
        assert_eq!(ids(q.view().up_next), [2, 3, 4]);
        assert!(!q.undo());
    }

    #[test]
    fn remove_move_and_clear_with_undo() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3, 4]), 0);
        q.set_autoplay(picks(&[90]));
        let three = q.view().up_next[1].uid;
        assert!(q.remove(three));
        assert_eq!(ids(q.view().up_next), [2, 4]);
        assert!(q.undo());
        assert_eq!(ids(q.view().up_next), [2, 3, 4]);
        let pick = q.view().autoplay[0].uid;
        assert!(q.move_item(pick, 0)); // an autoplay song moved joins Up Next
        assert_eq!(ids(q.view().up_next), [90, 2, 3, 4]);
        assert!(q.view().autoplay.is_empty());
        q.set_autoplay(picks(&[91]));
        q.clear_up_next();
        assert!(q.view().up_next.is_empty());
        assert_eq!(ids(q.view().autoplay), [91]);
        assert!(!q.remove(12345));
    }

    #[test]
    fn a_moved_song_keeps_its_place_after_unshuffling() {
        let mut q = Queue::new(3);
        q.play(album(&[1, 2, 3, 4]), 0);
        let four = q.view().up_next[2].uid;
        q.move_item(four, 0);
        q.set_shuffle(true);
        q.set_shuffle(false);
        assert_eq!(ids(q.view().up_next), [4, 2, 3]);
    }

    #[test]
    fn replaying_from_history() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 2, 3]), 0);
        q.advance(Advance::Ended);
        let first = q.view().history[0].uid;
        assert!(q.replay_from_history(first));
        assert_eq!(cur(&q), Some(1));
        assert_eq!(ids(q.view().history), [2]);
    }

    #[test]
    fn every_queue_entry_has_its_own_uid() {
        let mut q = Queue::new(1);
        q.play(album(&[1, 1, 1]), 0);
        let mut uids: Vec<u64> = q.view().up_next.iter().map(|i| i.uid).collect();
        uids.push(q.current().unwrap().uid);
        uids.sort();
        uids.dedup();
        assert_eq!(uids.len(), 3);
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;

    fn n(id: i64) -> NewItem {
        NewItem {
            track_id: id,
            chosen_by: ChosenBy::User,
            started_from: StartedFrom::Album,
            context_id: None,
        }
    }

    #[test]
    fn a_handed_off_queue_arrives_as_it_was() {
        let mut q = Queue::new(7);
        q.play(vec![n(99)], 0);
        q.restore(
            vec![n(1)],
            n(2),
            vec![n(4), n(3)],
            vec![n(5)],
            true,
            RepeatMode::All,
            true,
        );
        let v = q.view();
        let ids = |items: &[QueueItem]| items.iter().map(|i| i.track_id).collect::<Vec<_>>();
        assert_eq!(ids(v.history), vec![1]);
        assert_eq!(v.current.map(|i| i.track_id), Some(2));
        assert_eq!(ids(v.up_next), vec![4, 3], "not shuffled again");
        assert_eq!(ids(v.autoplay), vec![5]);
        assert!(v.shuffle && v.autoplay_enabled);
        assert_eq!(v.repeat, RepeatMode::All);
        // Unshuffling keeps the order it arrived in.
        q.set_shuffle(false);
        assert_eq!(ids(q.view().up_next), vec![4, 3]);
        // Autoplay off: no Autoplay section.
        q.restore(
            vec![],
            n(2),
            vec![],
            vec![n(5)],
            false,
            RepeatMode::Off,
            false,
        );
        assert!(q.view().autoplay.is_empty());
        // Undo puts back the queue that was here.
        assert!(q.undo());
        assert_eq!(q.view().current.map(|i| i.track_id), Some(2));
    }
}
