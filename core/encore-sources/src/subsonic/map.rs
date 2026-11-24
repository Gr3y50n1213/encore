//! Subsonic replies to the normalized model, and structured lyrics to LRC.
//!
//! - Durations are whole seconds (`Child.duration`); the model wants milliseconds.
//! - The MusicBrainz recording ID is `Child.musicBrainzId` (OpenSubsonic: "Track
//!   MusicBrainz ID"); Navidrome fills it from the recording ID tag.
//! - Artists: OpenSubsonic's `artists`/`albumArtists` lists when the server sends them,
//!   else the single `artist` name with `artistId`.
//! - `coverArt` is a cover art ID, not an item ID (OpenSubsonic's `getCoverArt`
//!   clarification). It is kept as the album's or playlist's `artwork_tag`: it changes
//!   when the cover does on servers that version it (Navidrome adds a timestamp), and
//!   it is what `cover_png` asks for.
//! - A playlist entry's ID is `"<position>:<song ID>"`, position counted from 0 in the
//!   playlist as the server has it now. Subsonic has no entry IDs and removes by
//!   position, so the position is what a removal needs; the song ID lets a stale
//!   position be noticed and the entry found again (`client::resolve_entry`).

use super::dto::{AlbumDto, ArtistDto, Child, Genre, PlaylistDto, StructuredLyrics};
use crate::jellyfin::time::parse_utc_ms;
use crate::model::{Album, Artist, ArtistCredit, Playlist, PlaylistEntry, Track};

fn clean(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn name_or_unknown(n: &Option<String>) -> String {
    clean(n).unwrap_or_else(|| "Unknown".to_owned())
}

fn small(n: Option<i64>) -> Option<u32> {
    n.and_then(|v| u32::try_from(v).ok()).filter(|v| *v > 0)
}

fn year(n: Option<i64>) -> Option<i32> {
    n.and_then(|v| i32::try_from(v).ok()).filter(|v| *v > 0)
}

fn genres(list: &[Genre], single: &Option<String>) -> Vec<String> {
    let many: Vec<String> = list.iter().filter_map(|g| clean(&g.name)).collect();
    if !many.is_empty() {
        return many;
    }
    clean(single).into_iter().collect()
}

fn credits(list: &[ArtistDto]) -> Vec<ArtistCredit> {
    list.iter()
        .filter_map(|a| {
            Some(ArtistCredit {
                source_item_id: clean(&a.id),
                name: clean(&a.name)?,
            })
        })
        .collect()
}

/// The linked list, else the one name with its ID.
fn credits_or(list: &[ArtistDto], name: &Option<String>, id: &Option<String>) -> Vec<ArtistCredit> {
    let linked = credits(list);
    if !linked.is_empty() {
        return linked;
    }
    clean(name)
        .map(|n| {
            vec![ArtistCredit {
                source_item_id: clean(id),
                name: n,
            }]
        })
        .unwrap_or_default()
}

/// A song. `album` is the album it was listed under, for the album artists a plain
/// Subsonic song does not carry.
pub(crate) fn to_track(c: &Child, album: Option<&AlbumDto>) -> Option<Track> {
    if c.is_dir == Some(true) {
        return None;
    }
    let id = clean(&c.id)?;
    let title = name_or_unknown(&c.title);
    let artists = credits_or(&c.artists, &c.artist, &c.artist_id);
    let mut album_artists = credits(&c.album_artists);
    if album_artists.is_empty()
        && let Some(a) = album
    {
        album_artists = album_artists_of(a);
    }
    let gain = c.replay_gain.as_ref();
    Some(Track {
        source_item_id: id,
        sort_title: clean(&c.sort_name).unwrap_or_else(|| title.clone()),
        title,
        album_item_id: clean(&c.album_id).or_else(|| album.and_then(|a| clean(&a.id))),
        album_title: clean(&c.album).or_else(|| album.and_then(|a| clean(&a.name))),
        artists,
        album_artists,
        disc_number: small(c.disc_number),
        track_number: small(c.track),
        duration_ms: c.duration.map(|s| s.max(0) as u64 * 1000).unwrap_or(0),
        year: year(c.year).or_else(|| album.and_then(|a| year(a.year))),
        container: clean(&c.suffix).map(|s| s.to_ascii_lowercase()),
        bitrate: small(c.bit_rate),
        mbid_recording: clean(&c.music_brainz_id),
        genres: genres(&c.genres, &c.genre),
        date_added_utc_ms: c.created.as_deref().and_then(parse_utc_ms),
        // Subsonic does not say whether a song has lyrics; asking is the only way to know.
        has_lyrics: true,
        track_gain_db: gain.and_then(|g| g.track_gain),
        album_gain_db: gain.and_then(|g| g.album_gain),
    })
}

fn album_artists_of(a: &AlbumDto) -> Vec<ArtistCredit> {
    credits_or(&a.artists, &a.artist, &a.artist_id)
}

pub(crate) fn to_album(a: &AlbumDto) -> Option<Album> {
    let id = clean(&a.id)?;
    let title = name_or_unknown(&a.name);
    let cover = clean(&a.cover_art);
    Some(Album {
        source_item_id: id,
        sort_title: clean(&a.sort_name).unwrap_or_else(|| title.clone()),
        title,
        album_artists: album_artists_of(a),
        year: year(a.year),
        mbid: clean(&a.music_brainz_id),
        genres: genres(&a.genres, &a.genre),
        date_added_utc_ms: a.created.as_deref().and_then(parse_utc_ms),
        has_artwork: cover.is_some(),
        artwork_tag: cover,
    })
}

pub(crate) fn to_artist(a: &ArtistDto) -> Option<Artist> {
    let id = clean(&a.id)?;
    let name = name_or_unknown(&a.name);
    Some(Artist {
        source_item_id: id,
        sort_name: clean(&a.sort_name).unwrap_or_else(|| name.clone()),
        name,
        mbid: clean(&a.music_brainz_id),
    })
}

/// The entry ID of the song at `position` (see the module notes).
pub(crate) fn entry_id(position: usize, song_id: &str) -> String {
    format!("{position}:{song_id}")
}

/// Split an entry ID back into its position and song ID.
pub(crate) fn parse_entry_id(entry_id: &str) -> Option<(usize, &str)> {
    let (pos, song) = entry_id.split_once(':')?;
    Some((pos.parse().ok()?, song))
}

pub(crate) fn to_entries(p: &PlaylistDto) -> Vec<PlaylistEntry> {
    p.entry
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let id = clean(&c.id)?;
            Some(PlaylistEntry {
                entry_id: Some(entry_id(i, &id)),
                track_item_id: id,
            })
        })
        .collect()
}

/// `user`: the signed-in user's name. Subsonic lets only the owner change a playlist,
/// and OpenSubsonic adds `readonly` for the rest.
pub(crate) fn to_playlist(p: &PlaylistDto, user: &str) -> Option<Playlist> {
    let owner_ok = match clean(&p.owner) {
        Some(owner) if !user.is_empty() => owner.eq_ignore_ascii_case(user),
        _ => true,
    };
    Some(Playlist {
        source_item_id: clean(&p.id)?,
        name: name_or_unknown(&p.name),
        date_added_utc_ms: p.created.as_deref().and_then(parse_utc_ms),
        entries: to_entries(p),
        artwork_tag: clean(&p.cover_art),
        can_edit: owner_ok && p.readonly != Some(true),
    })
}

/// The set to show when a song has several: synced before unsynced, else the first.
pub(crate) fn pick_lyrics(sets: &[StructuredLyrics]) -> Option<&StructuredLyrics> {
    let has_text = |l: &&StructuredLyrics| {
        l.line
            .iter()
            .any(|x| x.value.as_deref().is_some_and(|v| !v.trim().is_empty()))
    };
    sets.iter()
        .filter(has_text)
        .find(|l| l.synced == Some(true))
        .or_else(|| sets.iter().find(has_text))
}

/// Structured lyrics as LRC text (synced) or plain lines (unsynced), the forms the
/// lyrics crate reads. The offset becomes LRC's `[offset:]` tag: both mean "positive
/// shows the lyrics sooner" (OpenSubsonic `structuredLyrics.offset`, and the LRC
/// convention the lyrics crate follows). Returns the format name and the text.
pub(crate) fn lyrics_text(l: &StructuredLyrics) -> (&'static str, String) {
    let one_line = |v: &Option<String>| {
        v.as_deref()
            .unwrap_or("")
            .replace(['\r', '\n'], " ")
            .trim()
            .to_owned()
    };
    let synced = l.synced == Some(true) && l.line.iter().any(|x| x.start.is_some());
    if !synced {
        let lines: Vec<String> = l.line.iter().map(|x| one_line(&x.value)).collect();
        return ("plain", lines.join("\n"));
    }
    let mut out = String::new();
    if let Some(lang) = clean(&l.lang).filter(|c| c != "und" && c != "xxx") {
        out.push_str(&format!("[la:{lang}]\n"));
    }
    if let Some(offset) = l.offset.filter(|o| *o != 0) {
        out.push_str(&format!("[offset:{offset}]\n"));
    }
    for x in &l.line {
        let ms = x.start.unwrap_or(0).max(0);
        out.push_str(&format!(
            "[{:02}:{:02}.{:03}]{}\n",
            ms / 60_000,
            (ms / 1000) % 60,
            ms % 1000,
            one_line(&x.value)
        ));
    }
    ("lrc", out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subsonic::dto::LyricLine;

    fn child(json: &str) -> Child {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_plain_subsonic_song_takes_its_album_artist_from_the_album() {
        let album: AlbumDto = serde_json::from_str(
            r#"{"id":"al1","name":"Quiet Rooms","artist":"Paper Harbour","artistId":"ar1","year":2021}"#,
        )
        .unwrap();
        let t = to_track(
            &child(r#"{"id":"s1","title":"Low Lantern","artist":"Paper Harbour feat. Wren","duration":201,"suffix":"FLAC"}"#),
            Some(&album),
        )
        .unwrap();
        assert_eq!(t.album_item_id.as_deref(), Some("al1"));
        assert_eq!(t.album_title.as_deref(), Some("Quiet Rooms"));
        assert_eq!(t.album_artists[0].name, "Paper Harbour");
        assert_eq!(t.album_artists[0].source_item_id.as_deref(), Some("ar1"));
        assert_eq!(t.artists[0].source_item_id, None);
        assert_eq!(t.duration_ms, 201_000);
        assert_eq!(t.year, Some(2021));
        assert_eq!(t.container.as_deref(), Some("flac"));
        assert!(to_track(&child(r#"{"id":"d","isDir":true}"#), None).is_none());
        assert!(to_track(&child(r#"{"title":"no id"}"#), None).is_none());
    }

    #[test]
    fn entry_ids_round_trip() {
        assert_eq!(entry_id(3, "s:9"), "3:s:9");
        assert_eq!(parse_entry_id("3:s:9"), Some((3, "s:9")));
        assert_eq!(parse_entry_id("x:s1"), None);
        assert_eq!(parse_entry_id("nothing"), None);
    }

    #[test]
    fn someone_elses_playlist_is_read_only() {
        let p: PlaylistDto =
            serde_json::from_str(r#"{"id":"p1","name":"Shared","owner":"someone"}"#).unwrap();
        assert!(!to_playlist(&p, "listener").unwrap().can_edit);
        assert!(to_playlist(&p, "Someone").unwrap().can_edit);
        let r: PlaylistDto =
            serde_json::from_str(r#"{"id":"p1","name":"Smart","readonly":true}"#).unwrap();
        assert!(!to_playlist(&r, "listener").unwrap().can_edit);
    }

    #[test]
    fn synced_lyrics_become_lrc_with_the_offset() {
        let l = StructuredLyrics {
            lang: Some("eng".into()),
            offset: Some(-250),
            synced: Some(true),
            line: vec![
                LyricLine {
                    start: Some(0),
                    value: Some("First light".into()),
                },
                LyricLine {
                    start: Some(83_456),
                    value: Some("on the\nwater".into()),
                },
            ],
        };
        let (format, text) = lyrics_text(&l);
        assert_eq!(format, "lrc");
        assert_eq!(
            text,
            "[la:eng]\n[offset:-250]\n[00:00.000]First light\n[01:23.456]on the water\n"
        );
        let plain = StructuredLyrics {
            synced: Some(false),
            ..l
        };
        assert_eq!(
            lyrics_text(&plain),
            ("plain", "First light\non the water".to_owned())
        );
    }
}
