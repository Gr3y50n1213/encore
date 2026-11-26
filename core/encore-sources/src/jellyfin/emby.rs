//! Where Emby differs from Jellyfin (ADR core-0022). The rest of the Jellyfin client
//! is used as it is; every difference here was seen on a real Emby 4.10.2.0 in CI.
//!
//! - Lyrics: Emby keeps a song's sidecar `.lrc` as a text subtitle stream on the song
//!   and serves it as JSON lines (`Stream.js`, start ticks per line);
//!   `/Audio/{id}/Lyrics` is read as a container name there and fails.
//! - Playlist entries: Emby's `PlaylistItemId`s can't be trusted to name an entry for
//!   long. After some moves Emby gives the entries new IDs by position, a moment later
//!   and not after every move, and its listing may go on pairing songs with the old
//!   ones meanwhile; after a removal it closes the gap in the IDs a second or so later.
//!   A removal by the listed ID took the wrong song in CI, more than once, while the
//!   order of the songs was always right. So the mirror names an entry by its position
//!   and song (as for Subsonic), and a removal, a move or an insert is done on the
//!   songs, not the IDs: the adapter reads the playlist, works out the new order of
//!   songs, adds that whole order at the end and then removes every entry that was
//!   there before, by the set of their IDs (the same set however Emby pairs them).
//!   It first waits, up to 15 seconds, for a gap left by an earlier removal to close,
//!   so no ID moves under it.
//! - Writes: a playlist is made from query parameters (a JSON body is a 500), added to
//!   at the end only, renamed by saving the item itself, deleted as an item;
//!   favourites live under the user.
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

/// How long a playlist edit waits for Emby to close the gap a removal left in the
/// entry IDs, at most (it took up to about eight seconds in CI).
const SETTLE_TRIES: u32 = 15;
const SETTLE_STEP: Duration = Duration::from_secs(1);

/// Songs and entry IDs per request, so a long playlist's query stays a sane length.
const CHUNK: usize = 200;

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

/// Whether the entry IDs run on without a gap, as they do once Emby has closed the
/// gap a removal left. IDs that aren't numbers have nothing to wait for.
fn settled(items: &[BaseItem]) -> bool {
    let mut ids: Vec<i64> = match items
        .iter()
        .map(|i| i.playlist_item_id.as_deref()?.parse().ok())
        .collect::<Option<Vec<i64>>>()
    {
        Some(s) => s,
        None => return true,
    };
    ids.sort_unstable();
    ids.windows(2).all(|w| w[1] == w[0] + 1)
}

/// The playlist's songs after one edit, from its songs now. None when the edit
/// changes nothing (an entry that is gone already, a move to where it is).
pub(crate) fn songs_after(edit: &ServerEdit, items: &[BaseItem]) -> Option<Vec<String>> {
    let now = entries_of(items);
    let mut songs: Vec<String> = now.iter().map(|e| e.track_item_id.clone()).collect();
    match edit {
        ServerEdit::RemoveFromPlaylist { entry_ids, .. } => {
            let mut taken = Vec::new();
            for e in entry_ids {
                if let Some(at) = resolve_entry(e, &now, &taken) {
                    taken.push(at);
                }
            }
            if taken.is_empty() {
                return None;
            }
            Some(
                songs
                    .into_iter()
                    .enumerate()
                    .filter(|(i, _)| !taken.contains(i))
                    .map(|(_, s)| s)
                    .collect(),
            )
        }
        ServerEdit::MoveInPlaylist {
            entry_id,
            new_index,
            ..
        } => {
            let at = resolve_entry(entry_id, &now, &[])?;
            let to = (*new_index as usize).min(songs.len().saturating_sub(1));
            if at == to {
                return None;
            }
            let song = songs.remove(at);
            songs.insert(to, song);
            Some(songs)
        }
        ServerEdit::AddToPlaylist {
            item_ids, position, ..
        } => {
            let at = position.map_or(songs.len(), |p| (p as usize).min(songs.len()));
            songs.splice(at..at, item_ids.iter().cloned());
            Some(songs)
        }
        _ => None,
    }
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
        // For the error only: which devices Emby listed (IDs and app names, no users).
        let seen: Vec<String> = sessions
            .iter()
            .map(|s| {
                format!(
                    "{}/{}",
                    s.device_id.as_deref().unwrap_or("?"),
                    s.client.as_deref().unwrap_or("?")
                )
            })
            .collect();
        // By the device ID. The client name is compared only to choose between users,
        // as Emby may keep it as sent, percent-encoded (CI, 26 November 2026).
        let ours: Vec<SessionInfo> = sessions
            .into_iter()
            .filter(|s| s.device_id.as_deref() == Some(device.device_id.as_str()))
            .collect();
        let users_of = |only_client: bool| {
            let encoded = super::auth::encode(&device.client);
            let mut users: Vec<String> = ours
                .iter()
                .filter(|s| {
                    !only_client
                        || s.client.as_deref() == Some(device.client.as_str())
                        || s.client.as_deref() == Some(encoded.as_str())
                })
                .filter_map(|s| s.user_id.clone().filter(|u| !u.is_empty()))
                .collect();
            users.sort();
            users.dedup();
            users
        };
        let mut users = users_of(false);
        if users.len() > 1 {
            users = users_of(true);
        }
        match users.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(SourceError::Protocol(format!(
                "Emby didn't list this sign-in's session (it listed {})",
                seen.join(", ")
            ))),
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

    /// The playlist once a gap from an earlier removal has closed (or after the wait:
    /// a gap still there then is the playlist's own, and stays).
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

    /// Add songs at the end, in order.
    async fn emby_append(&self, playlist_id: &str, songs: &[String]) -> Result<()> {
        let user = self.user_id()?.to_owned();
        for chunk in songs.chunks(CHUNK) {
            let ids = chunk.join(",");
            self.emby_write(
                Method::Post,
                &format!("/Playlists/{playlist_id}/Items"),
                &[("ids", ids.as_str()), ("userId", user.as_str())],
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// Make the playlist hold `songs`, in that order: add them all at the end, then
    /// remove every entry that was there before (`items`), by their IDs.
    async fn emby_rewrite(
        &self,
        playlist_id: &str,
        items: &[BaseItem],
        songs: &[String],
    ) -> Result<()> {
        let before: Vec<String> = items
            .iter()
            .filter_map(|i| i.playlist_item_id.clone())
            .collect();
        if before.len() != items.len() {
            return Err(SourceError::Protocol(
                "Emby listed a playlist entry without its ID".into(),
            ));
        }
        self.emby_append(playlist_id, songs).await?;
        for chunk in before.chunks(CHUNK) {
            let ids = chunk.join(",");
            self.emby_write(
                Method::Delete,
                &format!("/Playlists/{playlist_id}/Items"),
                &[("entryIds", ids.as_str())],
                None,
            )
            .await?;
        }
        Ok(())
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
            // At the end: a plain add.
            ServerEdit::AddToPlaylist {
                playlist_id,
                item_ids,
                position: None,
            } => {
                self.emby_append(playlist_id, item_ids).await?;
                Ok(EditOutcome::Done)
            }
            ServerEdit::AddToPlaylist { playlist_id, .. }
            | ServerEdit::RemoveFromPlaylist { playlist_id, .. }
            | ServerEdit::MoveInPlaylist { playlist_id, .. } => {
                let items = self.emby_playlist_settled(playlist_id).await?;
                // None: already as asked (removed or moved there elsewhere).
                if let Some(songs) = songs_after(edit, &items) {
                    self.emby_rewrite(playlist_id, &items, &songs).await?;
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

    fn items(songs: &[&str], ids: &[&str]) -> Vec<BaseItem> {
        songs
            .iter()
            .zip(ids)
            .map(|(s, i)| BaseItem {
                id: Some((*s).to_owned()),
                playlist_item_id: Some((*i).to_owned()),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn ids_settle_when_they_run_on() {
        assert!(settled(&items(&["a", "b", "c"], &["5", "6", "7"])));
        assert!(!settled(&items(&["a", "b", "c"], &["5", "6", "8"])));
        // Just after a move the pairing may be old, but the set runs on.
        assert!(settled(&items(&["c", "a", "b"], &["3", "1", "2"])));
        assert!(settled(&[]));
    }

    #[test]
    fn each_edit_is_worked_out_on_the_songs() {
        let now = items(&["c", "a", "b"], &["3", "1", "2"]);
        let edit = |e: ServerEdit| songs_after(&e, &now);
        let id = crate::subsonic::map::entry_id;
        // The mirror still says "a" is first (before an earlier move): the same song
        // nearest to there is the one meant.
        assert_eq!(
            edit(ServerEdit::RemoveFromPlaylist {
                playlist_id: "p".into(),
                entry_ids: vec![id(0, "a")],
            }),
            Some(vec!["c".to_owned(), "b".to_owned()])
        );
        assert_eq!(
            edit(ServerEdit::RemoveFromPlaylist {
                playlist_id: "p".into(),
                entry_ids: vec![id(1, "gone")],
            }),
            None
        );
        assert_eq!(
            edit(ServerEdit::MoveInPlaylist {
                playlist_id: "p".into(),
                entry_id: id(2, "b"),
                new_index: 0,
            }),
            Some(vec!["b".to_owned(), "c".to_owned(), "a".to_owned()])
        );
        assert_eq!(
            edit(ServerEdit::MoveInPlaylist {
                playlist_id: "p".into(),
                entry_id: id(0, "c"),
                new_index: 0,
            }),
            None
        );
        assert_eq!(
            edit(ServerEdit::AddToPlaylist {
                playlist_id: "p".into(),
                item_ids: vec!["d".into(), "e".into()],
                position: Some(1),
            }),
            Some(["c", "d", "e", "a", "b"].map(str::to_owned).to_vec())
        );
    }
}
