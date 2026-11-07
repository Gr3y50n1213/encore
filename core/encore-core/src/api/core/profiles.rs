//! Profiles on one device and the switcher's state (board 14; plan section 6;
//! ADR core-0011): switching (with the queue kept per profile), PIN locks, names,
//! order, removal from this device, and Guest.
//!
//! The device registry (`device.sqlite`, outside every profile) keeps the order,
//! names, avatars, PIN hashes and the Guest session; nothing from anyone's
//! listening and no token.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use super::{EncoreCore, Inner, lock, now_ms, sync_loop};
use crate::api::types::*;
use crate::pin;
use crate::playlog::ListeningMode as LogMode;
use crate::tokens::tokens;

pub(super) const SESSION_ENDED_KEY: &str = "media.session_ended";
const LAST_ACTIVE: &str = "last_active";
const GUEST_STARTED: &str = "guest.started";
const GUEST_ACTIVITY: &str = "guest.activity";
const GUEST_PREVIOUS_MODE: &str = "guest.previous_mode";

fn rules() -> pin::Rules {
    let t = tokens();
    pin::Rules {
        wrong_tries: t.pin_wrong_tries,
        first_lock_ms: t.pin_first_lock_ms,
        max_lock_ms: t.pin_lock_max_ms,
    }
}

fn not_allowed(reason: impl Into<String>) -> EncoreError {
    EncoreError::NotAllowed {
        reason: reason.into(),
    }
}

/// Up to two letters: the first of the first two words ("Sam Lee" -> "SL").
pub(super) fn initials(name: &str) -> String {
    let letters: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if letters.is_empty() {
        "?".into()
    } else {
        letters
    }
}

/// A user image's address from what the registry keeps.
pub(super) fn avatar_url(base: &str, user_id: &str, tag: &str) -> String {
    let mut u = format!(
        "{}/Users/{user_id}/Images/Primary",
        base.trim_end_matches('/')
    );
    u.push_str(&format!(
        "?tag={tag}&fillWidth=256&fillHeight=256&quality=90"
    ));
    u
}

fn mode_name(m: LogMode) -> &'static str {
    match m {
        LogMode::Normal => "normal",
        LogMode::Private => "private",
        LogMode::Guest => "guest",
        LogMode::LearningOff => "learningOff",
    }
}

fn mode_from(s: &str) -> LogMode {
    match s {
        "private" => LogMode::Private,
        "learningOff" => LogMode::LearningOff,
        _ => LogMode::Normal,
    }
}

impl Inner {
    /// Whether `profile_id` may open now: no PIN, or unlocked just before (once).
    pub(super) fn take_unlock(&self, profile_id: &str) -> Result<(), EncoreError> {
        // An unlock is used by the next opening, PIN or not, so one left over from
        // a sign-in can't open the profile after a PIN is set.
        let unlocked = lock(&self.unlocked)?.remove(profile_id);
        if lock(&self.registry)?.pin(profile_id)?.is_none() {
            return Ok(());
        }
        if unlocked {
            Ok(())
        } else {
            Err(not_allowed("This profile is locked: unlock it first"))
        }
    }

    /// The queue as it stands, as song IDs and the index of the current one.
    pub(super) fn queue_songs(&self) -> Option<(Vec<i64>, usize)> {
        let p = self.profile().ok()?;
        let pb = lock(&p.playback).ok()?;
        let v = pb.queue.view();
        let mut ids: Vec<i64> = v.history.iter().map(|i| i.track_id).collect();
        let index = ids.len();
        ids.extend(v.current.map(|i| i.track_id));
        ids.extend(v.up_next.iter().map(|i| i.track_id));
        (!ids.is_empty() && v.current.is_some()).then_some((ids, index))
    }

    /// Put a queue in place without playing it (it loads paused).
    pub(super) fn load_queue(&self, ids: &[i64], index: usize) -> Result<(), EncoreError> {
        let p = self.profile()?;
        let known: Vec<i64> = {
            let store = lock(&p.store)?;
            ids.iter()
                .copied()
                .filter(|id| store.track(*id).is_ok())
                .collect()
        };
        if known.is_empty() {
            return Ok(());
        }
        let index = index.min(known.len() - 1);
        let items = Inner::new_items(&known, StartedFrom::Queue, None);
        let mut pb = lock(&p.playback)?;
        pb.queue.play(items, index);
        Ok(())
    }

    /// Keep the open profile's queue for next time (called when it closes).
    pub(super) fn save_queue(&self) -> Result<(), EncoreError> {
        let Some((ids, index)) = self.queue_songs() else {
            return Ok(());
        };
        let p = self.profile()?;
        let json = serde_json::json!({ "songIds": ids, "index": index }).to_string();
        lock(&p.store)?.save_queue(&json, now_ms())?;
        Ok(())
    }

    /// Load the queue the open profile had last time, paused.
    pub(super) fn restore_queue(&self) -> Result<(), EncoreError> {
        let p = self.profile()?;
        let saved = lock(&p.store)?.saved_queue()?;
        let Some(v) = saved.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) else {
            return Ok(());
        };
        let ids: Vec<i64> = v["songIds"]
            .as_array()
            .map(|a| a.iter().filter_map(|i| i.as_i64()).collect())
            .unwrap_or_default();
        let index = v["index"].as_u64().unwrap_or(0) as usize;
        self.load_queue(&ids, index)
    }

    /// After a profile opened: its own queue comes back, paused. A Guest session
    /// doesn't outlive a profile opening (its queue never meets the next listener).
    pub(super) fn after_open(&self) -> Result<(), EncoreError> {
        if lock(&self.registry)?.state(GUEST_STARTED)?.is_some() {
            return self.end_guest_now();
        }
        self.restore_queue()
    }

    /// Before a profile closes: keep its queue (not Guest's).
    pub(super) fn before_close(&self) {
        let guest = lock(&self.registry)
            .ok()
            .and_then(|r| r.state(GUEST_STARTED).ok().flatten())
            .is_some();
        if !guest {
            let _ = self.save_queue();
        }
    }

    /// The Guest session, if one is going.
    fn guest(&self) -> Result<Option<GuestSession>, EncoreError> {
        let r = lock(&self.registry)?;
        let started = r.state(GUEST_STARTED)?.and_then(|s| s.parse().ok());
        let Some(started_utc_ms) = started else {
            return Ok(None);
        };
        let activity: i64 = r
            .state(GUEST_ACTIVITY)?
            .and_then(|s| s.parse().ok())
            .unwrap_or(started_utc_ms);
        Ok(Some(GuestSession {
            started_utc_ms,
            ends_if_idle_utc_ms: activity + tokens().guest_idle_ms,
        }))
    }

    fn end_guest_now(&self) -> Result<(), EncoreError> {
        let previous = {
            let r = lock(&self.registry)?;
            let prev = r.state(GUEST_PREVIOUS_MODE)?;
            r.set_state(GUEST_STARTED, None)?;
            r.set_state(GUEST_ACTIVITY, None)?;
            r.set_state(GUEST_PREVIOUS_MODE, None)?;
            prev
        };
        if let Ok(p) = self.profile() {
            {
                let mut pb = lock(&p.playback)?;
                let store = lock(&p.store)?;
                let listened = pb.last_listened_ms;
                let mode = mode_from(previous.as_deref().unwrap_or("normal"));
                pb.log.set_mode(&store, mode, listened, now_ms())?;
                pb.queue = crate::queue::Queue::new(now_ms() as u64);
            }
            self.restore_queue()?;
        }
        Ok(())
    }

    /// Something played: a Guest session isn't idle. An idle one ends here, so the
    /// next listener never finds the guest's queue (board 14: "after 4 hours idle").
    pub(super) fn guest_tick(&self, activity: bool) -> Result<(), EncoreError> {
        let Some(g) = self.guest()? else {
            return Ok(());
        };
        let now = now_ms();
        if now > g.ends_if_idle_utc_ms {
            return self.end_guest_now();
        }
        if activity && now - (g.ends_if_idle_utc_ms - tokens().guest_idle_ms) > 60_000 {
            lock(&self.registry)?.set_state(GUEST_ACTIVITY, Some(&now.to_string()))?;
        }
        Ok(())
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// Switch to another profile on this device (board 14, item 2). A locked one
    /// must be unlocked first (`unlock_profile`). This profile's unsent changes are
    /// sent first (a few seconds at most; anything left goes next time), its queue
    /// is kept, and the other profile opens with its own queue, paused. With
    /// `keep_playing` the current queue moves over instead ("Keep playing"): the
    /// player keeps going and reports `playback_started` again, so the rest of the
    /// song counts for the new profile. `tokens` are the new profile's keychain
    /// entries. Async.
    pub async fn switch_profile(
        &self,
        profile_id: String,
        tokens_in: Vec<SourceToken>,
        keep_playing: bool,
    ) -> Result<QueueState, EncoreError> {
        super::checked_profile_id(&profile_id)?;
        if lock(&self.inner.registry)?.pin(&profile_id)?.is_some()
            && !lock(&self.inner.unlocked)?.contains(&profile_id)
        {
            return Err(not_allowed("This profile is locked: unlock it first"));
        }
        let inner = self.inner.clone();
        let carried = if keep_playing {
            inner.queue_songs()
        } else {
            None
        };
        // Send what's waiting before closing.
        let flush = {
            let p = inner.profile().ok();
            p.and_then(|p| {
                let s = lock(&p.companion).ok()?.clone()?;
                Some((p, s))
            })
        };
        if let Some((p, s)) = flush {
            let inner2 = inner.clone();
            let _ = self
                .spawn(async move {
                    let _ = tokio::time::timeout(
                        tokens().switch_flush,
                        sync_loop::push_all(&inner2, &p, &s),
                    )
                    .await;
                    Ok(())
                })
                .await;
        }
        self.open_profile(profile_id, tokens_in)?;
        if let Some((ids, index)) = carried {
            let p = self.inner.profile()?;
            lock(&p.playback)?.queue = crate::queue::Queue::new(now_ms() as u64);
            self.inner.load_queue(&ids, index)?;
        }
        self.queue()
    }

    /// Unlock a profile for the next `switch_profile` or `open_profile`. `pin` None
    /// means the app's biometric check (Face ID, Touch ID, Windows Hello) passed;
    /// after the device's biometrics changed, the PIN is needed once. Five wrong
    /// PINs lock it for 30 s, then longer; the reason says how long.
    pub fn unlock_profile(
        &self,
        profile_id: String,
        pin_in: Option<String>,
    ) -> Result<(), EncoreError> {
        let reg = lock(&self.inner.registry)?;
        let Some(rec) = reg.pin(&profile_id)? else {
            return Ok(());
        };
        let now = now_ms();
        if rec.locked_until_utc_ms > now {
            let s = (rec.locked_until_utc_ms - now + 999) / 1000;
            return Err(not_allowed(format!(
                "Too many wrong tries. Try again in {s} s."
            )));
        }
        match pin_in {
            None if rec.required_once => {
                return Err(not_allowed(
                    "Enter the PIN once: this device's Face ID or fingerprints changed",
                ));
            }
            None => {}
            Some(p) => {
                if !pin::same(&pin::hash(&rec.salt, &p), &rec.hash) {
                    let st = rules().after_wrong(
                        pin::TryState {
                            failed: rec.failed,
                            locked_until_utc_ms: rec.locked_until_utc_ms,
                            lockouts: rec.lockouts,
                        },
                        now,
                    );
                    reg.set_pin_state(
                        &profile_id,
                        st.failed,
                        st.locked_until_utc_ms,
                        st.lockouts,
                        rec.required_once,
                    )?;
                    return Err(not_allowed(if st.locked_until_utc_ms > now {
                        format!(
                            "Wrong PIN. Too many wrong tries: try again in {} s.",
                            (st.locked_until_utc_ms - now + 999) / 1000
                        )
                    } else {
                        "Wrong PIN".to_owned()
                    }));
                }
                reg.set_pin_state(&profile_id, 0, 0, 0, false)?;
            }
        }
        lock(&self.inner.unlocked)?.insert(profile_id);
        Ok(())
    }

    /// Lock the open profile with a PIN of `profiles.pinMinDigits` to
    /// `pinMaxDigits` digits (Manage profiles). It is a screen lock, not encryption.
    pub fn set_profile_pin(&self, profile_id: String, new_pin: String) -> Result<(), EncoreError> {
        self.only_open(&profile_id)?;
        let t = tokens();
        if !pin::valid(&new_pin, t.pin_min_digits, t.pin_max_digits) {
            return Err(not_allowed(format!(
                "A PIN is {} to {} digits",
                t.pin_min_digits, t.pin_max_digits
            )));
        }
        let salt = pin::new_salt();
        let reg = lock(&self.inner.registry)?;
        reg.ensure(&profile_id, now_ms())?;
        reg.set_pin(&profile_id, &salt, &pin::hash(&salt, &new_pin))?;
        Ok(())
    }

    /// Remove the open profile's PIN.
    pub fn remove_profile_pin(&self, profile_id: String) -> Result<(), EncoreError> {
        self.only_open(&profile_id)?;
        Ok(lock(&self.inner.registry)?.clear_pin(&profile_id)?)
    }

    /// The device's biometrics changed: every PIN is needed once before biometric
    /// unlock works again (board 14, States).
    pub fn biometrics_changed(&self) -> Result<(), EncoreError> {
        Ok(lock(&self.inner.registry)?.require_pins_once()?)
    }

    /// Rename a profile on this device (None: back to the media-server user name).
    pub fn rename_profile(
        &self,
        profile_id: String,
        name: Option<String>,
    ) -> Result<(), EncoreError> {
        let name = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        if name.as_ref().is_some_and(|n| n.chars().count() > 40) {
            return Err(not_allowed("A profile name is at most 40 characters"));
        }
        let reg = lock(&self.inner.registry)?;
        reg.ensure(&profile_id, now_ms())?;
        Ok(reg.set_display_name(&profile_id, name.as_deref())?)
    }

    /// The switcher's order (Manage profiles); profiles not named keep their place
    /// after these.
    pub fn reorder_profiles(&self, profile_ids: Vec<String>) -> Result<(), EncoreError> {
        Ok(lock(&self.inner.registry)?.reorder(&profile_ids)?)
    }

    /// Remove a profile from this device only (board 14, item 3): its folder here
    /// goes; its data on the companion server stays. Not deleting: that is
    /// `delete_profile_everywhere`. Delete its keychain entries after this.
    pub fn remove_profile_from_device(&self, profile_id: String) -> Result<(), EncoreError> {
        super::checked_profile_id(&profile_id)?;
        if self.current_profile().as_deref() == Some(profile_id.as_str())
            && let Some(p) = self.inner.take_profile()?
        {
            for _ in 0..100 {
                if Arc::strong_count(&p) == 1 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let dir = self.inner.profiles_dir().join(&profile_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| EncoreError::Storage {
                reason: format!("couldn't remove the profile from this device ({e})"),
            })?;
        }
        let reg = lock(&self.inner.registry)?;
        reg.remove(&profile_id)?;
        if reg.state(LAST_ACTIVE)?.as_deref() == Some(profile_id.as_str()) {
            reg.set_state(LAST_ACTIVE, None)?;
        }
        Ok(())
    }

    /// The profile that was open last, to open at launch.
    pub fn last_active_profile(&self) -> Result<Option<String>, EncoreError> {
        Ok(lock(&self.inner.registry)?.state(LAST_ACTIVE)?)
    }

    /// Start Guest on this device (board 14, item 5): nothing is recorded,
    /// recommended or synced, likes and playlists can't be saved, and the queue
    /// starts empty. The open profile's own queue is kept and comes back when Guest
    /// ends. Show "Guest, nothing is saved".
    pub fn start_guest(&self) -> Result<QueueState, EncoreError> {
        let p = self.inner.profile()?;
        if self.inner.guest()?.is_some() {
            return self.queue();
        }
        self.inner.save_queue()?;
        let now = now_ms();
        {
            let mut pb = lock(&p.playback)?;
            let store = lock(&p.store)?;
            let previous = mode_name(pb.log.mode());
            let reg = lock(&self.inner.registry)?;
            reg.set_state(GUEST_PREVIOUS_MODE, Some(previous))?;
            reg.set_state(GUEST_STARTED, Some(&now.to_string()))?;
            reg.set_state(GUEST_ACTIVITY, Some(&now.to_string()))?;
            let listened = pb.last_listened_ms;
            pb.log.set_mode(&store, LogMode::Guest, listened, now)?;
            pb.queue = crate::queue::Queue::new(now as u64);
        }
        self.queue()
    }

    /// End Guest: the profile's own listening mode and queue come back (paused).
    pub fn end_guest(&self) -> Result<QueueState, EncoreError> {
        self.inner.end_guest_now()?;
        self.queue()
    }

    /// The Guest session, if one is going (it may have just ended for being idle).
    pub fn guest_session(&self) -> Result<Option<GuestSession>, EncoreError> {
        self.inner.guest_tick(false)?;
        self.inner.guest()
    }
}

impl EncoreCore {
    fn only_open(&self, profile_id: &str) -> Result<(), EncoreError> {
        if self.current_profile().as_deref() == Some(profile_id) {
            Ok(())
        } else {
            Err(not_allowed("Open the profile first"))
        }
    }

    /// Order `profiles()` the user's way and fill the switcher's fields.
    pub(super) fn arrange_profiles(
        &self,
        mut list: Vec<ProfileSummary>,
        needs_sign_in: &HashSet<String>,
    ) -> Result<Vec<ProfileSummary>, EncoreError> {
        let reg = lock(&self.inner.registry)?;
        for p in &list {
            reg.ensure(&p.profile_id, now_ms())?;
        }
        let order = reg.profiles()?;
        let current = self.current_profile();
        for p in &mut list {
            let entry = order.iter().find(|o| o.profile_id == p.profile_id);
            if let Some(name) = entry.and_then(|e| e.display_name.clone()) {
                p.display_name = name;
            }
            p.initials = initials(&p.display_name);
            p.avatar_url = entry
                .and_then(|e| e.avatar.clone())
                .map(|(b, u, t)| avatar_url(&b, &u, &t));
            p.locked = reg.pin(&p.profile_id)?.is_some();
            p.active = current.as_deref() == Some(p.profile_id.as_str());
            p.needs_sign_in = needs_sign_in.contains(&p.profile_id);
        }
        let rank = |id: &str| {
            order
                .iter()
                .position(|o| o.profile_id == id)
                .unwrap_or(usize::MAX)
        };
        // The open profile first (board 14, item 1), then the user's order.
        list.sort_by_key(|p| (!p.active, rank(&p.profile_id)));
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_and_avatar_addresses() {
        assert_eq!(initials("Sam Lee"), "SL");
        assert_eq!(initials("sam"), "S");
        assert_eq!(initials("  élodie  marie anne "), "ÉM");
        assert_eq!(initials("(kids) room"), "KR");
        assert_eq!(initials(""), "?");
        assert_eq!(
            avatar_url("http://j:8096/", "u1", "t"),
            "http://j:8096/Users/u1/Images/Primary?tag=t&fillWidth=256&fillHeight=256&quality=90"
        );
    }
}
