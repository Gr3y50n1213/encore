//! Where Emby differs from Jellyfin (ADR core-0022). The rest of the Jellyfin client
//! is used as it is; every difference here was seen on a real Emby 4.10.2.0 in CI.
//!
//! - Lyrics: Emby keeps a song's sidecar `.lrc` as a text subtitle stream on the
//!   song and serves it as JSON lines (`Stream.js`, start ticks per
//!   line); `/Audio/{id}/Lyrics` is read as a container there and fails.
//! - Playlist entries: Emby's `PlaylistItemId`s are slots, not names. They rise with
//!   the position: after a move the songs change slots (the first position has the
//!   lowest ID again), and after a removal the slots are renumbered to run on without
//!   a gap a second or so later. For a moment after a move, the listing still shows
//!   each song with its old ID, in the new order, while a removal already goes by the
//!   new ones (a removal by the listed ID took the wrong song in CI). So the mirror
//!   names an entry by its position and song (as for Subsonic), and every removal and
//!   move reads the playlist first, waits for a renumbering in progress to finish, finds
//!   the entry's position there, and takes the slot that position has: the IDs in
//!   rising order, whatever the listing pairs them with.
//! - Writes: a playlist is made from query parameters (a JSON body is a 500), added
//!   to at the end only (an insert is an add and then moves), renamed by saving the
//!   item itself, deleted as an item; favourites live under the user.
//! - Who a token belongs to: there is no `/Users/Me`. Emby records each device's
//!   session under the user whose token it sent, so `GET /Sessions` sent with the
//!   token shows this very client's session with that user.

use std::time::Duration;

use super::client::{JellyfinClient, RawLyrics};
use super::dto::{BaseItem, ItemsResult, PlaylistCreated, SessionInfo, SubtitleTrack};
use super::map;
use crate::error::{Result, SourceError};
use crate::model::{EditOutcome, PlaylistEntry, ServerEdit};
use crate::subsonic::client::resolve_entry;
use crate::transport::Method;

/// How long a removal waits for Emby to finish renumbering a playlist's slots, at
/// most (it took about eight seconds in CI).
const SETTLE_TRIES: u32 = 15;
const SETTLE_STEP: Duration = Duration::from_secs(1);

/// The stream index of a song's lyrics on Emby: its first text subtitle stream.
pub(crate) fn lyric_stream(item: &BaseItem) -> Option<i64> {
    let streams = item.media_streams.as_ref()?;
    let texts: Vec<_> = streams
        .iter()
        .filter(|s| {
            s.kind
                .as_deref()
                .is_some_and(|k| k.eq_ignore_ascii_case("subtitle"))
                && s.is_text_subtitle_stream != Some(false)
                && s.index.is_some()
        })
        .collect();
    texts
        .iter()
        .find(|s| {
            s.codec
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case("lrc"))
        })
        .or_else(|| texts.first())
        .and_then(|s| s.index)
}

/// Emby's lyric lines (`Stream.js`) as LRC, or as plain lines when none has a time.
/// None when there are no lines.
pub(crate) fn lrc_from_track(json: &str) -> Option<String> {
    let track: SubtitleTrack = serde_json::from_str(json.trim_start_matches('\u{feff}')).ok()?;
    let events: Vec<(i64, String)> = track
        .track_events
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| Some((e.start_position_ticks.unwrap_or(0).max(0), e.text?)))
        .collect();
    if events.iter().all(|(_, t)| t.trim().is_empty()) {
        return None;
    }
    let timed = events.iter().any(|(ticks, _)| *ticks > 0);
    let mut out = String::new();
    for (ticks, text) in &events {
        let line = text.replace(['\r', '\n'], " ");
        if timed {
            let cs = ticks / 100_000;
            out.push_str(&format!(
                "[{:02}:{:02}.{:02}]{line}\n",
                cs / 6000,
                (cs / 100) % 60,
                cs % 100
            ));
        } else {
            out.push_str(&line);
            out.push('\n');
        }
    }
    Some(out)
}

/// A playlist's songs as the mirror keeps them for Emby: named by position and song.
pub(crate) fn entries_of(items: &[BaseItem]) -> Vec<PlaylistEntry> {
    items
        .iter()
        .filter_map(|i| i.id.clone())
        .enumerate()
        .map(|(pos, song)| PlaylistEntry {
            entry_id: Some(crate::subsonic::map::entry_id(pos, &song)),
            track_item_id: song,
        })
        .collect()
}

/// The slots in position order: the IDs the listing shows, in rising order. None
/// when they aren't all numbers (then the listing's own pairing is used).
fn slots_in_order(items: &[BaseItem]) -> Option<Vec<String>> {
    let mut ids: Vec<(i64, String)> = items
        .iter()
        .map(|i| {
            let s = i.playlist_item_id.clone()?;
            Some((s.parse().ok()?, s))
        })
        .collect::<Option<_>>()?;
    ids.sort();
    Some(ids.into_iter().map(|(_, s)| s).collect())
}

/// Whether the slots run on without a gap, as they do once Emby has renumbered them.
fn settled(items: &[BaseItem]) -> bool {
    let mut slots: Vec<i64> = match items
        .iter()
        .map(|i| i.playlist_item_id.as_deref()?.parse().ok())
        .collect::<Option<Vec<i64>>>()
    {
        Some(s) => s,
        // Not numbers: nothing to wait for.
        None => return true,
    };
    slots.sort_unstable();
    slots.windows(2).all(|w| w[1] == w[0] + 1)
}

impl JellyfinClient {
    /// Lyrics on Emby: the song's lyrics stream, as LRC (or plain lines).
    pub(super) async fn emby_lyrics(&self, item_id: &str) -> Result<Option<RawLyrics>> {
        let user = self.user_id()?.to_owned();
        let page: ItemsResult = self
            .get_json(
                "/Items",
                &[
                    ("userId", user.as_str()),
                    ("ids", item_id),
                    ("fields", "MediaStreams,MediaSources"),
                    ("enableImages", "false"),
                    ("enableUserData", "false"),
                ],
            )
            .await?;
        let Some(item) = page.items.unwrap_or_default().into_iter().next() else {
            return Ok(None);
        };
        let Some(index) = lyric_stream(&item) else {
            return Ok(None);
        };
        let source = item
            .media_sources
            .iter()
            .flatten()
            .find_map(|s| s.id.clone())
            .unwrap_or_else(|| format!("mediasource_{item_id}"));
        let resp = self
            .send(
                Method::Get,
                self.url(
                    &format!("/Items/{item_id}/{source}/Subtitles/{index}/Stream.js"),
                    &[],
                )?,
                None,
            )
            .await?;
        match Self::check(resp, SourceError::SessionExpired) {
            Ok(r) => Ok(lrc_from_track(&r.text()).map(|body| RawLyrics {
                format: "lrc".into(),
                body,
            })),
            Err(SourceError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The user Emby records for this client's own session, made by the request
    /// itself with the token: the token's owner. Two different users there means
    /// another check with the same device name ran at the same moment: try again.
    pub(super) async fn emby_token_owner(&self) -> Result<String> {
        let sessions: Vec<SessionInfo> = self.get_json("/Sessions", &[]).await?;
        let device = self.device();
        let mut users: Vec<String> = sessions
            .into_iter()
            .filter(|s| {
                s.device_id.as_deref() == Some(device.device_id.as_str())
                    && s.client.as_deref() == Some(device.client.as_str())
            })
            .filter_map(|s| s.user_id.filter(|u| !u.is_empty()))
            .collect();
        users.sort();
        users.dedup();
        match users.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(SourceError::Protocol(
                "Emby didn't list this sign-in's session".into(),
            )),
            _ => Err(SourceError::Protocol(
                "Emby listed this device under two users; try again".into(),
            )),
        }
    }

    /// A playlist's entries as Emby lists them now.
    async fn emby_playlist_now(&self, playlist_id: &str) -> Result<Vec<BaseItem>> {
        Ok(self
            .playlist_items(playlist_id)
            .await?
            .items
            .unwrap_or_default())
    }

    /// The playlist once Emby has finished renumbering its slots (or after the wait).
    async fn emby_playlist_settled(&self, playlist_id: &str) -> Result<Vec<BaseItem>> {
        let mut items = self.emby_playlist_now(playlist_id).await?;
        for _ in 0..SETTLE_TRIES {
            if settled(&items) {
                break;
            }
            tokio::time::sleep(self.settle_step()).await;
            items = self.emby_playlist_now(playlist_id).await?;
        }
        Ok(items)
    }

    /// A playlist's songs for the mirror (`MediaSource::playlist_entries`).
    pub async fn emby_playlist_entries(&self, playlist_id: &str) -> Result<Vec<PlaylistEntry>> {
        Ok(entries_of(&self.emby_playlist_now(playlist_id).await?))
    }

    fn settle_step(&self) -> Duration {
        if cfg!(test) {
            Duration::from_millis(1)
        } else {
            SETTLE_STEP
        }
    }

    /// Where an entry of the mirror is now: its slot ID and index. Not found: Emby
    /// lost it already (removed elsewhere), so a removal is done and a move is moot.
    fn emby_slot(entry_id: &str, items: &[BaseItem], taken: &[usize]) -> Option<(usize, String)> {
        let now = entries_of(items);
        let at = resolve_entry(entry_id, &now, taken)?;
        let slot = match slots_in_order(items) {
            Some(slots) => slots.get(at)?.clone(),
            None => items.get(at)?.playlist_item_id.clone()?,
        };
        Some((at, slot))
    }

    async fn emby_write(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<serde_json::Value>,
    ) -> Result<crate::transport::Response> {
        let resp = self.send(method, self.url(path, query)?, body).await?;
        // 403: not this user's to change (a playlist someone else owns).
        if resp.status == 403 {
            return Err(SourceError::Server {
                status: 403,
                message: resp.text().chars().take(500).collect(),
            });
        }
        Self::check(resp, SourceError::SessionExpired)
    }

    /// Move the entry in `slot` to position `to`.
    async fn emby_move(&self, playlist_id: &str, slot: &str, to: usize) -> Result<()> {
        self.emby_write(
            Method::Post,
            &format!("/Playlists/{playlist_id}/Items/{slot}/Move/{to}"),
            &[],
            None,
        )
        .await
        .map(|_| ())
    }

    /// One playlist or favourite write on Emby (`JellyfinClient::edit` hands it here).
    pub(super) async fn emby_edit(&self, edit: &ServerEdit) -> Result<EditOutcome> {
        let user = self.user_id()?.to_owned();
        match edit {
            ServerEdit::CreatePlaylist { name, item_ids } => {
                let ids = item_ids.join(",");
                let mut q = vec![
                    ("name", name.as_str()),
                    ("mediaType", "Audio"),
                    ("userId", user.as_str()),
                ];
                if !ids.is_empty() {
                    q.push(("ids", ids.as_str()));
                }
                let resp = self
                    .emby_write(Method::Post, "/Playlists", &q, None)
                    .await?;
                let created: PlaylistCreated = resp.json()?;
                Ok(EditOutcome::Created(created.id.ok_or_else(|| {
                    SourceError::Protocol("no ID for the new playlist".into())
                })?))
            }
            ServerEdit::AddToPlaylist {
                playlist_id,
                item_ids,
                position,
            } => {
                let before = match position {
                    Some(_) => self.emby_playlist_settled(playlist_id).await?.len(),
                    None => 0,
                };
                let ids = item_ids.join(",");
                self.emby_write(
                    Method::Post,
                    &format!("/Playlists/{playlist_id}/Items"),
                    &[("ids", ids.as_str()), ("userId", user.as_str())],
                    None,
                )
                .await?;
                // Emby adds at the end only: move the new songs up, one by one.
                if let Some(p) = *position {
                    let to = (p as usize).min(before);
                    for i in 0..item_ids.len() {
                        let items = self.emby_playlist_settled(playlist_id).await?;
                        let slot = match slots_in_order(&items) {
                            Some(slots) => slots.get(before + i).cloned(),
                            None => items
                                .get(before + i)
                                .and_then(|it| it.playlist_item_id.clone()),
                        };
                        let Some(slot) = slot else {
                            break;
                        };
                        if before + i != to + i {
                            self.emby_move(playlist_id, &slot, to + i).await?;
                        }
                    }
                }
                Ok(EditOutcome::Done)
            }
            ServerEdit::RemoveFromPlaylist {
                playlist_id,
                entry_ids,
            } => {
                let items = self.emby_playlist_settled(playlist_id).await?;
                let mut taken = Vec::new();
                let mut slots = Vec::new();
                for e in entry_ids {
                    if let Some((at, slot)) = Self::emby_slot(e, &items, &taken) {
                        taken.push(at);
                        slots.push(slot);
                    }
                }
                if !slots.is_empty() {
                    let ids = slots.join(",");
                    self.emby_write(
                        Method::Delete,
                        &format!("/Playlists/{playlist_id}/Items"),
                        &[("entryIds", ids.as_str())],
                        None,
                    )
                    .await?;
                }
                Ok(EditOutcome::Done)
            }
            ServerEdit::MoveInPlaylist {
                playlist_id,
                entry_id,
                new_index,
            } => {
                let items = self.emby_playlist_settled(playlist_id).await?;
                if let Some((at, slot)) = Self::emby_slot(entry_id, &items, &[]) {
                    let to = (*new_index as usize).min(items.len().saturating_sub(1));
                    if at != to {
                        self.emby_move(playlist_id, &slot, to).await?;
                    }
                }
                Ok(EditOutcome::Done)
            }
            ServerEdit::RenamePlaylist { playlist_id, name } => {
                // No rename call: the item is saved back with its new name.
                let mut item: serde_json::Value = self
                    .get_json(&format!("/Users/{user}/Items/{playlist_id}"), &[])
                    .await?;
                item["Name"] = serde_json::Value::String(name.clone());
                self.emby_write(
                    Method::Post,
                    &format!("/Items/{playlist_id}"),
                    &[],
                    Some(item),
                )
                .await?;
                Ok(EditOutcome::Done)
            }
            ServerEdit::DeletePlaylist { playlist_id } => {
                self.emby_write(Method::Delete, &format!("/Items/{playlist_id}"), &[], None)
                    .await?;
                Ok(EditOutcome::Done)
            }
            ServerEdit::SetFavorite { item_id, favorite } => {
                self.emby_write(
                    if *favorite {
                        Method::Post
                    } else {
                        Method::Delete
                    },
                    &format!("/Users/{user}/FavoriteItems/{item_id}"),
                    &[],
                    None,
                )
                .await?;
                Ok(EditOutcome::Done)
            }
        }
    }
}

/// Emby's songs to the model: as Jellyfin's, with lyrics known from the streams.
pub(crate) fn to_track(item: &BaseItem) -> Option<crate::model::Track> {
    let mut t = map::to_track(item)?;
    t.has_lyrics |= lyric_stream(item).is_some();
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyric_lines_become_lrc_and_untimed_lines_stay_plain() {
        let js = "\u{feff}{\"TrackEvents\":[{\"Text\":\"A line to sing\",\"StartPositionTicks\":10000000,\"EndPositionTicks\":50000000},{\"Text\":\"Another line\",\"StartPositionTicks\":652300000}]}";
        assert_eq!(
            lrc_from_track(js).unwrap(),
            "[00:01.00]A line to sing\n[01:05.23]Another line\n"
        );
        let plain = r#"{"TrackEvents":[{"Text":"One"},{"Text":"Two","StartPositionTicks":0}]}"#;
        assert_eq!(lrc_from_track(plain).unwrap(), "One\nTwo\n");
        assert_eq!(lrc_from_track(r#"{"TrackEvents":[]}"#), None);
        assert_eq!(lrc_from_track("not json"), None);
    }

    #[test]
    fn the_lyrics_stream_is_the_first_text_subtitle_preferring_lrc() {
        let item: BaseItem = serde_json::from_str(
            r#"{"Id":"12","MediaStreams":[{"Type":"Audio","Codec":"mp3","Index":0},
                {"Type":"Subtitle","Codec":"srt","Index":1,"IsTextSubtitleStream":true},
                {"Type":"Subtitle","Codec":"lrc","Index":2,"IsTextSubtitleStream":true}]}"#,
        )
        .unwrap();
        assert_eq!(lyric_stream(&item), Some(2));
        let none: BaseItem =
            serde_json::from_str(r#"{"Id":"13","MediaStreams":[{"Type":"Audio","Index":0}]}"#)
                .unwrap();
        assert_eq!(lyric_stream(&none), None);
        assert!(to_track(&item).unwrap().has_lyrics);
        assert!(!to_track(&none).unwrap().has_lyrics);
    }

    #[test]
    fn slots_settle_when_they_run_on() {
        let items = |ids: &[&str]| -> Vec<BaseItem> {
            ids.iter()
                .map(|i| BaseItem {
                    id: Some(format!("s{i}")),
                    playlist_item_id: Some((*i).to_owned()),
                    ..Default::default()
                })
                .collect()
        };
        assert!(settled(&items(&["5", "6", "7"])));
        assert!(!settled(&items(&["5", "6", "8"])));
        assert!(settled(&items(&[])));
        // Just after a move: the new order, each song still with its old ID. The slots
        // are the IDs in rising order, so position 0 is slot 1.
        let stale = items(&["3", "1", "2"]);
        assert!(settled(&stale));
        assert_eq!(slots_in_order(&stale).unwrap(), ["1", "2", "3"]);
        let (at, slot) =
            JellyfinClient::emby_slot(&crate::subsonic::map::entry_id(0, "s3"), &stale, &[])
                .unwrap();
        assert_eq!((at, slot.as_str()), (0, "1"));
    }
}
