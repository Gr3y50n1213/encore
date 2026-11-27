//! What a music file says about itself: title, artists, album, numbers, year, genres,
//! length, MusicBrainz ID, lyrics and the embedded cover (mock-up 49, "What is read").
//!
//! MP3 (ID3v2), FLAC and Ogg (Vorbis comments), M4A (iTunes atoms) and WAV are read
//! through Symphonia's format readers; no audio is decoded. A file with no tags still
//! gives a usable song: its name comes from the file name ([`name_from_file`]).

use std::io::{Read, Seek};
use std::sync::Arc;

use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTag, StandardVisualKey, Tag, Visual};

/// Everything read from one file. Text is trimmed; empty values are `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalTags {
    pub title: Option<String>,
    /// In the file's order. A tag holding several names ("A; B", "A / B") is split.
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
    /// From the container, or counted from its packets when it doesn't say.
    pub duration_ms: Option<u64>,
    pub musicbrainz_recording_id: Option<String>,
    /// Plain or LRC text, as the tag holds it; the lyrics parser decides which.
    pub lyrics: Option<String>,
    /// The front cover, or else the first picture.
    pub cover: Option<Cover>,
}

/// An embedded picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Cover {
    /// For example `image/jpeg`; `None` when the file doesn't say.
    pub media_type: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TagError {
    /// Not a format Encore reads (or not audio at all).
    #[error("not a music file Encore can read ({0})")]
    Unsupported(String),
    /// Reading the file itself failed.
    #[error("couldn't read the file: {0}")]
    Io(String),
}

/// Read a file's tags. `extension` (without the dot) helps pick the format quickly.
pub fn read_tags<R>(reader: R, extension: Option<&str>) -> Result<LocalTags, TagError>
where
    R: Read + Seek + Send + Sync + 'static,
{
    let source = SeekableReader(reader);
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = extension {
        hint.with_extension(&ext.to_ascii_lowercase());
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| match e {
            SymphoniaError::IoError(io) if io.kind() != std::io::ErrorKind::UnexpectedEof => {
                TagError::Io(io.to_string())
            }
            e => TagError::Unsupported(e.to_string()),
        })?;
    let (track_id, mut duration_ms) = {
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| TagError::Unsupported("no audio track".into()))?;
        let ms = track
            .time_base
            .zip(track.duration)
            .and_then(|(tb, d)| tb.calc_time(track.start_ts.checked_add(d)?))
            .map(|t| (t.as_secs_f64() * 1000.0).round() as u64);
        (track.id, ms)
    };

    let mut tags = LocalTags::default();
    let mut visuals: Vec<Visual> = Vec::new();
    {
        let mut md = format.metadata();
        // Every revision, oldest first: a later one (the container's own tags after an
        // ID3v2 block in front, say) wins where both say something.
        let mut revisions = Vec::new();
        loop {
            if let Some(rev) = md.current() {
                revisions.push(rev.clone());
            }
            if md.pop().is_none() {
                break;
            }
        }
        for rev in revisions {
            let per_track = rev
                .per_track
                .iter()
                .filter(|t| t.track_id == u64::from(track_id))
                .flat_map(|t| t.metadata.tags.iter());
            for tag in rev.media.tags.iter().chain(per_track) {
                apply(&mut tags, tag);
            }
            visuals.extend(rev.media.visuals.iter().cloned());
            for t in rev
                .per_track
                .iter()
                .filter(|t| t.track_id == u64::from(track_id))
            {
                visuals.extend(t.metadata.visuals.iter().cloned());
            }
        }
    }
    tags.cover = pick_cover(visuals);
    if duration_ms.is_none() {
        duration_ms = counted_duration(format.as_mut(), track_id);
    }
    tags.duration_ms = duration_ms;
    Ok(tags)
}

fn apply(tags: &mut LocalTags, tag: &Tag) {
    let Some(std) = &tag.std else { return };
    match std {
        StandardTag::TrackTitle(v) => set(&mut tags.title, v),
        StandardTag::Artist(v) => {
            for name in split_names(v) {
                if !tags.artists.contains(&name) {
                    tags.artists.push(name);
                }
            }
        }
        StandardTag::Album(v) => set(&mut tags.album, v),
        StandardTag::AlbumArtist(v) => set(&mut tags.album_artist, v),
        StandardTag::TrackNumber(n) => {
            tags.track_number = u32::try_from(*n).ok().filter(|n| *n > 0)
        }
        StandardTag::DiscNumber(n) => tags.disc_number = u32::try_from(*n).ok().filter(|n| *n > 0),
        StandardTag::RecordingYear(y) | StandardTag::ReleaseYear(y) => {
            if tags.year.is_none() {
                tags.year = Some(i32::from(*y));
            }
        }
        StandardTag::RecordingDate(v)
        | StandardTag::ReleaseDate(v)
        | StandardTag::OriginalRecordingDate(v) => {
            if tags.year.is_none() {
                tags.year = year_of(v);
            }
        }
        StandardTag::Genre(v) => {
            for g in split_names(v) {
                if !tags.genres.contains(&g) {
                    tags.genres.push(g);
                }
            }
        }
        StandardTag::MusicBrainzRecordingId(v) | StandardTag::MusicBrainzTrackId(v) => {
            if tags.musicbrainz_recording_id.is_none() {
                set(&mut tags.musicbrainz_recording_id, v);
            }
        }
        StandardTag::Lyrics(v) => set(&mut tags.lyrics, v),
        _ => {}
    }
}

fn set(slot: &mut Option<String>, value: &Arc<String>) {
    let v = value.trim();
    if !v.is_empty() {
        *slot = Some(v.to_owned());
    }
}

/// "A; B", "A / B" and ID3v2.4's NUL-separated lists become separate names. A slash
/// with no spaces stays ("AC/DC").
fn split_names(value: &str) -> Vec<String> {
    value
        .split(['\0', ';'])
        .flat_map(|part| part.split(" / "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The year from "2019", "2019-05-17" or "2019-05-17T00:00:00".
fn year_of(date: &str) -> Option<i32> {
    let digits: String = date
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.len() == 4 {
        digits.parse().ok().filter(|y| (1000..=9999).contains(y))
    } else {
        None
    }
}

fn pick_cover(visuals: Vec<Visual>) -> Option<Cover> {
    let front = visuals
        .iter()
        .position(|v| v.usage == Some(StandardVisualKey::FrontCover))
        .unwrap_or(0);
    let v = visuals.into_iter().nth(front)?;
    (!v.data.is_empty()).then(|| Cover {
        media_type: v.media_type,
        data: v.data.into_vec(),
    })
}

/// The length from the last packet's end, for a file whose header doesn't say (an MP3
/// with no Xing or VBRI frame). Packets are read, never decoded.
fn counted_duration(format: &mut dyn FormatReader, track_id: u32) -> Option<u64> {
    let time_base = format
        .tracks()
        .iter()
        .find(|t| t.id == track_id)?
        .time_base?;
    let mut end = None;
    while let Ok(Some(packet)) = format.next_packet() {
        if packet.track_id == track_id {
            end = packet.pts.checked_add(packet.dur).or(end);
        }
    }
    let t = time_base.calc_time(end?)?;
    Some((t.as_secs_f64() * 1000.0).round() as u64)
}

/// A song's title and artist from its file name, for a file with no tags:
/// "03 - Artist - Title.mp3", "Artist - Title.flac", "03. Title.m4a" or "Title.ogg".
pub fn name_from_file(file_name: &str) -> (String, Option<String>) {
    let stem = match file_name.rfind('.') {
        Some(i) if i > 0 => &file_name[..i],
        _ => file_name,
    };
    // A leading track number: "03 ", "03 - ", "03. ", "3-".
    let digits = stem.chars().take_while(char::is_ascii_digit).count();
    let rest = if (1..=3).contains(&digits) {
        let after = &stem[digits..];
        let trimmed = after.trim_start_matches([' ', '.', '-', '_']);
        if trimmed.len() < after.len() && !trimmed.is_empty() {
            trimmed
        } else {
            stem
        }
    } else {
        stem
    };
    let rest = rest.replace('_', " ");
    match rest.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            (title.trim().to_owned(), Some(artist.trim().to_owned()))
        }
        _ => (rest.trim().to_owned(), None),
    }
}

/// Symphonia wants to know whether a source can seek and how long it is.
struct SeekableReader<R>(R);

impl<R: Read> Read for SeekableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<R: Seek> Seek for SeekableReader<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl<R: Read + Seek + Send + Sync> symphonia::core::io::MediaSource for SeekableReader<R> {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::fs::File {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/local")
            .join(name);
        std::fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn an_mp3_with_id3_tags_and_a_cover() {
        let t = read_tags(fixture("tagged.mp3"), Some("mp3")).unwrap();
        assert_eq!(t.title.as_deref(), Some("Morning Tone"));
        assert_eq!(
            t.artists,
            vec!["Encore Fixtures".to_owned(), "Second Voice".to_owned()]
        );
        assert_eq!(t.album.as_deref(), Some("Test Tones"));
        assert_eq!(t.album_artist.as_deref(), Some("Encore Fixtures"));
        assert_eq!(t.track_number, Some(3));
        assert_eq!(t.disc_number, Some(1));
        assert_eq!(t.year, Some(2019));
        assert_eq!(t.genres, vec!["Ambient".to_owned()]);
        let ms = t.duration_ms.unwrap();
        assert!((900..=1200).contains(&ms), "{ms}");
        let cover = t.cover.expect("the embedded cover");
        assert_eq!(&cover.data[..4], b"\x89PNG");
    }

    #[test]
    fn a_flac_with_vorbis_comments_lyrics_and_an_id() {
        let t = read_tags(fixture("tagged.flac"), Some("flac")).unwrap();
        assert_eq!(t.title.as_deref(), Some("Evening Tone"));
        assert_eq!(t.artists, vec!["Encore Fixtures".to_owned()]);
        assert_eq!(t.album.as_deref(), Some("Test Tones"));
        assert_eq!(t.track_number, Some(4));
        assert_eq!(t.year, Some(2020));
        assert_eq!(
            t.musicbrainz_recording_id.as_deref(),
            Some("00000000-0000-4000-8000-000000000004")
        );
        let lyrics = t.lyrics.expect("lyrics");
        assert!(lyrics.contains("[00:00.50]First line"), "{lyrics}");
        let ms = t.duration_ms.unwrap();
        assert!((900..=1100).contains(&ms), "{ms}");
        assert!(t.cover.is_some(), "the FLAC picture block");
    }

    #[test]
    fn an_m4a_with_itunes_tags() {
        let t = read_tags(fixture("tagged.m4a"), Some("m4a")).unwrap();
        assert_eq!(t.title.as_deref(), Some("Noon Tone"));
        assert_eq!(t.artists, vec!["Encore Fixtures".to_owned()]);
        assert_eq!(t.album.as_deref(), Some("Test Tones"));
        assert_eq!(t.track_number, Some(5));
        let ms = t.duration_ms.unwrap();
        assert!((900..=1200).contains(&ms), "{ms}");
    }

    #[test]
    fn an_ogg_vorbis_file() {
        let t = read_tags(fixture("tagged.ogg"), Some("ogg")).unwrap();
        assert_eq!(t.title.as_deref(), Some("Night Tone"));
        assert_eq!(t.artists, vec!["Encore Fixtures".to_owned()]);
        assert_eq!(t.genres, vec!["Ambient".to_owned(), "Drone".to_owned()]);
    }

    #[test]
    fn a_file_with_no_tags_still_has_a_length() {
        let t = read_tags(fixture("untagged.mp3"), Some("mp3")).unwrap();
        assert_eq!(t.title, None);
        assert!(t.artists.is_empty());
        let ms = t.duration_ms.expect("counted from the packets");
        assert!((900..=1200).contains(&ms), "{ms}");
    }

    #[test]
    fn something_that_isnt_music_is_refused() {
        let bytes = std::io::Cursor::new(b"not a music file at all, just words".to_vec());
        assert!(matches!(
            read_tags(bytes, Some("mp3")),
            Err(TagError::Unsupported(_))
        ));
    }

    #[test]
    fn names_come_from_file_names() {
        assert_eq!(
            name_from_file("03 - Some Artist - A Song.mp3"),
            ("A Song".into(), Some("Some Artist".into()))
        );
        assert_eq!(
            name_from_file("Some Artist - A Song.flac"),
            ("A Song".into(), Some("Some Artist".into()))
        );
        assert_eq!(name_from_file("03. A Song.m4a"), ("A Song".into(), None));
        assert_eq!(name_from_file("A_Song.ogg"), ("A Song".into(), None));
        assert_eq!(name_from_file("1979.mp3"), ("1979".into(), None));
        assert_eq!(name_from_file("track"), ("track".into(), None));
    }

    #[test]
    fn several_names_in_one_tag_are_split() {
        assert_eq!(split_names("A; B"), vec!["A", "B"]);
        assert_eq!(split_names("A / B"), vec!["A", "B"]);
        assert_eq!(split_names("AC/DC"), vec!["AC/DC"]);
        assert_eq!(split_names("A\0B"), vec!["A", "B"]);
        assert_eq!(year_of("2019-05-17"), Some(2019));
        assert_eq!(year_of("19"), None);
    }
}
