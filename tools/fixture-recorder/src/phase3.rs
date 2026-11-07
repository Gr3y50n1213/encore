//! The Phase 3 part of the recording: lyrics, covers, playlist writes and a
//! favourite, in the one signed-in session the owner approves.
//!
//! Every write is undone before the session ends: the test playlist is deleted
//! (and checked gone) and the favourite is cleared (and checked), even when an
//! earlier step failed. Nothing here stops the run; each step says what happened,
//! and the notes go into RECORDING.md.

use std::path::Path;

use encore_sources::jellyfin::JellyfinClient;
use encore_sources::{EditOutcome, LibraryDelta, PlaylistEntry, ServerEdit};

/// What happened, for the console and RECORDING.md.
#[derive(Debug, Default)]
pub struct Notes {
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

impl Notes {
    fn note(&mut self, s: impl Into<String>) {
        let s = s.into();
        println!("  {s}");
        self.lines.push(s);
    }
    fn warn(&mut self, s: impl Into<String>) {
        let s = s.into();
        println!("  ! {s}");
        self.warnings.push(s);
    }
}

pub const TEST_PLAYLIST: &str = "Encore test - safe to delete";

async fn entries(client: &JellyfinClient, playlist: &str) -> Result<Vec<PlaylistEntry>, String> {
    let page = client
        .playlist_items(playlist)
        .await
        .map_err(|e| e.to_string())?;
    Ok(page
        .items
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| {
            Some(PlaylistEntry {
                entry_id: i.playlist_item_id,
                track_item_id: i.id?,
            })
        })
        .collect())
}

fn order(e: &[PlaylistEntry]) -> Vec<&str> {
    e.iter().map(|x| x.track_item_id.as_str()).collect()
}

/// PNG width and height from its header.
fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || !b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((w, h))
}

pub async fn run(client: &JellyfinClient, first: &LibraryDelta, covers_dir: &Path) -> Notes {
    let mut n = Notes::default();
    println!("\nPhase 3:");

    // Lyrics for every song that has them, up to 40.
    let with: Vec<&str> = first
        .tracks
        .iter()
        .filter(|t| t.has_lyrics)
        .take(40)
        .map(|t| t.source_item_id.as_str())
        .collect();
    let (mut found, mut cued) = (0, 0);
    for id in &with {
        match client.lyrics(id).await {
            Ok(Some(raw)) => {
                found += 1;
                if raw.body.contains("\"Position\"") {
                    cued += 1;
                }
            }
            Ok(None) => {}
            Err(e) => n.warn(format!("lyrics for one song: {e}")),
        }
    }
    n.note(format!(
        "Lyrics: {found} of {} songs marked as having lyrics returned them; {cued} with word cues.",
        with.len()
    ));

    // Covers as small PNGs, kept only on this computer.
    let _ = std::fs::create_dir_all(covers_dir);
    let albums: Vec<_> = first
        .albums
        .iter()
        .filter(|a| a.has_artwork)
        .take(40)
        .collect();
    let (mut png, mut largest, mut other) = (0, 0u32, 0);
    for (i, a) in albums.iter().enumerate() {
        match client
            .cover_png(&a.source_item_id, 64, a.artwork_tag.as_deref())
            .await
        {
            Ok(Some(bytes)) => match png_size(&bytes) {
                Some((w, h)) => {
                    png += 1;
                    largest = largest.max(w.max(h));
                    let _ = std::fs::write(covers_dir.join(format!("cover-{i:02}.png")), &bytes);
                }
                None => other += 1,
            },
            Ok(None) => {}
            Err(e) => n.warn(format!("one cover: {e}")),
        }
    }
    n.note(format!(
        "Covers: {png} of {} came back as PNG (largest side {largest} px); {other} in another format.",
        albums.len()
    ));
    if other > 0 {
        n.warn(
            "Some covers did not come back as PNG: the palette code would treat them as no cover",
        );
    }

    playlist_writes(client, first, &mut n).await;
    favourite(client, first, &mut n).await;
    n
}

async fn playlist_writes(client: &JellyfinClient, first: &LibraryDelta, n: &mut Notes) {
    let ids: Vec<String> = first
        .tracks
        .iter()
        .take(3)
        .map(|t| t.source_item_id.clone())
        .collect();
    if ids.len() < 3 {
        n.warn("fewer than 3 songs: the playlist writes were skipped");
        return;
    }
    let (a, b, c) = (ids[0].as_str(), ids[1].as_str(), ids[2].as_str());
    let created = client
        .edit(&ServerEdit::CreatePlaylist {
            name: TEST_PLAYLIST.into(),
            item_ids: vec![a.into(), b.into()],
        })
        .await;
    let pl = match created {
        Ok(EditOutcome::Created(id)) => {
            n.note("Playlist: created the test playlist with 2 songs.");
            id
        }
        Ok(EditOutcome::Done) => {
            n.warn("Playlist: the server made the playlist but gave no ID");
            return;
        }
        Err(e) => {
            n.warn(format!("Playlist: couldn't create the test playlist: {e}"));
            return;
        }
    };

    let steps: Result<(), String> = async {
        client
            .edit(&ServerEdit::AddToPlaylist {
                playlist_id: pl.clone(),
                item_ids: vec![c.into()],
                position: Some(1),
            })
            .await
            .map_err(|e| format!("add at a position: {e}"))?;
        let e1 = entries(client, &pl).await?;
        if order(&e1) == [a, c, b] {
            n.note("Playlist: inserting at position 1 put the song second.");
        } else {
            n.warn(format!(
                "Playlist: after inserting at position 1 the order is {:?}, not [a, c, b]",
                order(&e1)
            ));
        }
        if e1.iter().all(|e| e.entry_id.is_some()) {
            n.note("Playlist: every entry has its own entry ID.");
        } else {
            n.warn("Playlist: some entries have no entry ID");
        }
        // Move the first song to the end, by its entry ID.
        let entry_a = e1
            .iter()
            .find(|e| e.track_item_id == a)
            .and_then(|e| e.entry_id.clone())
            .ok_or("no entry ID for the first song")?;
        let moved = client
            .edit(&ServerEdit::MoveInPlaylist {
                playlist_id: pl.clone(),
                entry_id: entry_a.clone(),
                new_index: 2,
            })
            .await;
        let e2 = entries(client, &pl).await?;
        let want: Vec<&str> = order(&e1).into_iter().filter(|x| *x != a).chain([a]).collect();
        let mut now = e2.clone();
        if moved.is_ok() && order(&e2) == want {
            n.note("Playlist: Move takes the entry ID (PlaylistItemId).");
        } else {
            n.warn(format!(
                "Playlist: Move with the entry ID gave {:?} ({moved:?}); trying the song's own ID",
                order(&e2)
            ));
            let by_item = client
                .edit(&ServerEdit::MoveInPlaylist {
                    playlist_id: pl.clone(),
                    entry_id: a.into(),
                    new_index: 2,
                })
                .await;
            let e2b = entries(client, &pl).await?;
            if by_item.is_ok() && order(&e2b) == want {
                n.warn("Playlist: Move takes the song's item ID, not the entry ID: the core must change");
            } else {
                n.warn(format!("Playlist: Move did not work either way ({by_item:?})"));
            }
            now = e2b;
        }
        // Remove the added song by its entry ID (from the last read: no second read
        // in a row, so every reply in the recording stays distinct).
        let entry_c = now
            .iter()
            .find(|e| e.track_item_id == c)
            .and_then(|e| e.entry_id.clone())
            .ok_or("no entry ID for the added song")?;
        client
            .edit(&ServerEdit::RemoveFromPlaylist {
                playlist_id: pl.clone(),
                entry_ids: vec![entry_c],
            })
            .await
            .map_err(|e| format!("remove: {e}"))?;
        let e3 = entries(client, &pl).await?;
        if !order(&e3).contains(&c) && e3.len() == 2 {
            n.note("Playlist: removing by entry ID removed exactly that song.");
        } else {
            n.warn(format!("Playlist: after removing, the order is {:?}", order(&e3)));
        }
        client
            .edit(&ServerEdit::RenamePlaylist {
                playlist_id: pl.clone(),
                name: format!("{TEST_PLAYLIST} (renamed)"),
            })
            .await
            .map_err(|e| format!("rename: {e}"))?;
        n.note("Playlist: renamed.");
        Ok(())
    }
    .await;
    if let Err(e) = steps {
        n.warn(format!(
            "Playlist: a step failed ({e}); deleting the test playlist anyway"
        ));
    }

    // Always: delete it and check it is gone.
    match client
        .edit(&ServerEdit::DeletePlaylist {
            playlist_id: pl.clone(),
        })
        .await
    {
        Ok(_) => match client.playlist_items(&pl).await {
            Err(encore_sources::SourceError::NotFound) => {
                n.note("Playlist: deleted, and the server confirms it is gone.");
            }
            other => n.warn(format!(
                "Playlist: deleted, but reading it back gave {other:?}. Please check for \"{TEST_PLAYLIST}\" in Jellyfin"
            )),
        },
        Err(e) => n.warn(format!(
            "Playlist: COULDN'T DELETE \"{TEST_PLAYLIST}\" ({e}). Please delete it in Jellyfin; nothing else was changed."
        )),
    }
}

async fn favourite(client: &JellyfinClient, first: &LibraryDelta, n: &mut Notes) {
    let before = match client.favorite_ids(200).await {
        Ok(f) => f,
        Err(e) => {
            n.warn(format!(
                "Favourite: couldn't list favourites ({e}); skipped"
            ));
            return;
        }
    };
    n.note(format!(
        "Favourite: {} favourite songs before.",
        before.len()
    ));
    let Some(song) = first
        .tracks
        .iter()
        .map(|t| t.source_item_id.clone())
        .find(|id| !before.contains(id))
    else {
        n.warn("Favourite: every recorded song is already a favourite; skipped");
        return;
    };
    let set = client
        .edit(&ServerEdit::SetFavorite {
            item_id: song.clone(),
            favorite: true,
        })
        .await;
    match (&set, client.favorite_ids(200).await) {
        (Ok(_), Ok(now)) if now.contains(&song) => n.note("Favourite: set, and listed as one."),
        (r, listed) => n.warn(format!(
            "Favourite: set gave {r:?}; listed: {:?}",
            listed.map(|l| l.len())
        )),
    }
    // Always: clear it again and check.
    let cleared = client
        .edit(&ServerEdit::SetFavorite {
            item_id: song.clone(),
            favorite: false,
        })
        .await;
    match (&cleared, client.favorite_ids(200).await) {
        (Ok(_), Ok(now)) if !now.contains(&song) && now.len() == before.len() => {
            n.note("Favourite: cleared again; the favourites are as they were.")
        }
        (r, listed) => n.warn(format!(
            "Favourite: CLEARING gave {r:?}; listed: {:?}. Please check your favourites in Jellyfin",
            listed.map(|l| l.len())
        )),
    }
}
