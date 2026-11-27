//! The music files on the device as a `MediaSource` (ADR core-0024). Its library is the
//! index the platform's looks through the chosen folders fill; a sync hands the whole
//! of it over. Songs play from their own address, covers are files beside the index,
//! lyrics come from the tags or a `.lrc` file. Nothing here touches a network, and no
//! file is ever changed.

use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::index::{LocalFile, LocalFolder, LocalIndex, album_id, artist_id, short_hash};
use super::tags::{Cover, read_tags};
use crate::error::{Result, SourceError};
use crate::jellyfin::{ArtworkInfo, RawLyrics, StreamInfo, StreamRequest, SyncProgress};
use crate::model::{
    Album, Artist, ArtistCredit, Capabilities, Cursor, EditOutcome, LibraryDelta, PlaylistEntry,
    ServerEdit, SourceKind, Track,
};
use crate::transport::{BoxFuture, Downloaded, Progress};

/// One file the platform found, for [`LocalSource::add_file`].
#[derive(Debug, Clone)]
pub struct FoundFile {
    pub folder_id: u32,
    /// The platform's address for the file (kept on the device only).
    pub uri: String,
    pub file_name: String,
    pub size: u64,
    pub modified_ms: i64,
    /// A path the core can open to read it (Android: `/proc/self/fd/<n>` of a
    /// descriptor the platform holds open until the call returns).
    pub read_path: String,
    /// A matching `.lrc` file's text, when there is one beside it.
    pub lrc: Option<String>,
    /// A path to `cover.jpg`, `folder.jpg` or the like beside it, when there is one.
    pub folder_cover_path: Option<String>,
}

/// What became of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Added {
    /// Read and in the library.
    Song,
    /// Not a music file Encore reads (or unreadable): left out.
    Skipped,
}

#[derive(Debug)]
pub struct LocalSource {
    dir: PathBuf,
    index: Arc<Mutex<LocalIndex>>,
}

const INDEX_FILE: &str = "local.json";
const COVERS_DIR: &str = "covers";
const EXTENSIONS: [&str; 9] = [
    "mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "wav", "alac",
];

/// Whether a file name looks like music Encore reads.
pub fn is_music_file(file_name: &str) -> bool {
    file_name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

impl LocalSource {
    /// The source whose index lives in `dir` (made if missing).
    pub fn open(dir: &Path) -> Result<LocalSource> {
        std::fs::create_dir_all(dir.join(COVERS_DIR)).map_err(storage)?;
        let index = LocalIndex::load(&dir.join(INDEX_FILE)).map_err(storage)?;
        Ok(LocalSource {
            dir: dir.to_owned(),
            index: Arc::new(Mutex::new(index)),
        })
    }

    fn with<R>(&self, f: impl FnOnce(&mut LocalIndex) -> R) -> Result<R> {
        let mut ix = self
            .index
            .lock()
            .map_err(|_| SourceError::Storage("the local index is unavailable".into()))?;
        Ok(f(&mut ix))
    }

    fn save(&self) -> Result<()> {
        let ix = self
            .index
            .lock()
            .map_err(|_| SourceError::Storage("the local index is unavailable".into()))?;
        ix.save(&self.dir.join(INDEX_FILE)).map_err(storage)
    }

    pub fn folders(&self) -> Result<Vec<(LocalFolder, u32)>> {
        self.with(|ix| {
            ix.folders
                .iter()
                .map(|f| {
                    let n = ix.files.values().filter(|x| x.folder_id == f.id).count();
                    (f.clone(), u32::try_from(n).unwrap_or(u32::MAX))
                })
                .collect()
        })
    }

    pub fn add_folder(&self, uri: &str, name: &str) -> Result<LocalFolder> {
        let f = self.with(|ix| ix.add_folder(uri, name))?;
        self.save()?;
        Ok(f)
    }

    pub fn remove_folder(&self, folder_id: u32) -> Result<bool> {
        let removed = self.with(|ix| ix.remove_folder(folder_id))?;
        self.save()?;
        Ok(removed)
    }

    pub fn set_available(&self, folder_id: u32, available: bool) -> Result<()> {
        self.with(|ix| ix.set_available(folder_id, available))?;
        self.save()
    }

    /// The files known in a folder: address, size and time.
    pub fn known_files(&self, folder_id: u32) -> Result<Vec<(String, u64, i64)>> {
        self.with(|ix| {
            ix.known(folder_id)
                .into_iter()
                .map(|f| (f.uri.clone(), f.size, f.modified_ms))
                .collect()
        })
    }

    /// Read one file into the index. An unchanged file isn't read again. Not saved
    /// until [`LocalSource::finish_look`].
    pub fn add_file(&self, file: &FoundFile, now_ms: i64) -> Result<Added> {
        if self.with(|ix| ix.is_current(&file.uri, file.size, file.modified_ms))? {
            return Ok(Added::Song);
        }
        if !is_music_file(&file.file_name) {
            return Ok(Added::Skipped);
        }
        let ext = file
            .file_name
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase());
        let reader = match std::fs::File::open(&file.read_path) {
            Ok(r) => r,
            Err(_) => return Ok(Added::Skipped),
        };
        let mut tags = match read_tags(reader, ext.as_deref()) {
            Ok(t) => t,
            Err(_) => return Ok(Added::Skipped),
        };
        let cover = match tags.cover.take() {
            Some(c) => Some(self.keep_cover(&c)?),
            None => match &file.folder_cover_path {
                Some(p) => self.keep_folder_cover(p)?,
                None => None,
            },
        };
        self.with(|ix| {
            ix.put(
                file.folder_id,
                &file.uri,
                &file.file_name,
                file.size,
                file.modified_ms,
                now_ms,
                ext,
                tags,
                file.lrc.clone(),
                cover,
            )
        })?;
        Ok(Added::Song)
    }

    /// After a look through a folder: files not seen are gone; the index is saved.
    /// Returns how many were forgotten.
    pub fn finish_look(&self, folder_id: u32, seen: &[String]) -> Result<usize> {
        let gone = self.with(|ix| ix.forget_unseen(folder_id, seen))?;
        self.save()?;
        self.tidy_covers()?;
        Ok(gone)
    }

    /// Each song's key (`Track::song_key`) with the address it plays from, so a server's
    /// copy of the same song plays from the device (ADR core-0024).
    pub fn song_keys(&self) -> Result<std::collections::HashMap<String, String>> {
        let delta = self.delta(&HashSet::new())?;
        let uris: std::collections::HashMap<String, String> = self.with(|ix| {
            ix.files
                .iter()
                .map(|(id, f)| (id.clone(), f.uri.clone()))
                .collect()
        })?;
        Ok(delta
            .tracks
            .iter()
            .filter_map(|t| {
                uris.get(&t.source_item_id)
                    .map(|u| (t.song_key(), u.clone()))
            })
            .collect())
    }

    /// The address a song plays from.
    pub fn uri_of(&self, item_id: &str) -> Result<Option<String>> {
        self.with(|ix| ix.files.get(item_id).map(|f| f.uri.clone()))
    }

    fn keep_cover(&self, cover: &Cover) -> Result<String> {
        let ext = match cover.media_type.as_deref() {
            Some("image/png") => "png",
            Some("image/webp") => "webp",
            _ => "jpg",
        };
        let name = format!(
            "{}.{ext}",
            short_hash("cover", &String::from_utf8_lossy(&cover.data))
        );
        let path = self.dir.join(COVERS_DIR).join(&name);
        if !path.exists() {
            std::fs::write(&path, &cover.data).map_err(storage)?;
        }
        Ok(name)
    }

    fn keep_folder_cover(&self, path: &str) -> Result<Option<String>> {
        let mut data = Vec::new();
        let Ok(mut f) = std::fs::File::open(path) else {
            return Ok(None);
        };
        // A folder picture bigger than 10 MB isn't a cover.
        if f.by_ref().take(10 << 20).read_to_end(&mut data).is_err() || data.is_empty() {
            return Ok(None);
        }
        let media_type = if data.starts_with(b"\x89PNG") {
            "image/png"
        } else {
            "image/jpeg"
        };
        self.keep_cover(&Cover {
            media_type: Some(media_type.into()),
            data,
        })
        .map(Some)
    }

    /// Covers no file uses any more go.
    fn tidy_covers(&self) -> Result<()> {
        let used: HashSet<String> =
            self.with(|ix| ix.files.values().filter_map(|f| f.cover.clone()).collect())?;
        if let Ok(entries) = std::fs::read_dir(self.dir.join(COVERS_DIR)) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !used.contains(&name) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    fn cover_path(&self, item_id: &str) -> Result<Option<PathBuf>> {
        let name = self.with(|ix| {
            if let Some(f) = ix.files.get(item_id) {
                return f.cover.clone();
            }
            // An album: the cover of its first song that has one.
            ix.files
                .values()
                .filter(|f| album_of(f).as_deref() == Some(item_id))
                .find_map(|f| f.cover.clone())
        })?;
        Ok(name.map(|n| self.dir.join(COVERS_DIR).join(n)))
    }

    fn delta(&self, known_ids: &HashSet<String>) -> Result<LibraryDelta> {
        self.with(|ix| {
            let mut artists: BTreeMap<String, Artist> = BTreeMap::new();
            let mut albums: BTreeMap<String, Album> = BTreeMap::new();
            let mut tracks = Vec::new();
            let mut credit = |name: &str| {
                let id = artist_id(name);
                artists.entry(id.clone()).or_insert_with(|| Artist {
                    source_item_id: id.clone(),
                    name: name.to_owned(),
                    sort_name: name.to_owned(),
                    mbid: None,
                });
                ArtistCredit {
                    source_item_id: Some(id),
                    name: name.to_owned(),
                }
            };
            for (id, f) in &ix.files {
                let artist_credits: Vec<ArtistCredit> =
                    f.artists.iter().map(|a| credit(a)).collect();
                let album_artist_name = album_artist_of(f);
                let album_artists = vec![credit(&album_artist_name)];
                let album_item_id = album_of(f);
                if let (Some(aid), Some(title)) = (&album_item_id, &f.album) {
                    let a = albums.entry(aid.clone()).or_insert_with(|| Album {
                        source_item_id: aid.clone(),
                        title: title.clone(),
                        sort_title: title.clone(),
                        album_artists: album_artists.clone(),
                        year: f.year,
                        mbid: None,
                        genres: f.genres.clone(),
                        date_added_utc_ms: Some(f.added_ms),
                        has_artwork: false,
                        artwork_tag: None,
                    });
                    a.date_added_utc_ms = a
                        .date_added_utc_ms
                        .min(Some(f.added_ms))
                        .or(Some(f.added_ms));
                    if a.year.is_none() {
                        a.year = f.year;
                    }
                    if !a.has_artwork
                        && let Some(c) = &f.cover
                    {
                        a.has_artwork = true;
                        a.artwork_tag = Some(c.clone());
                    }
                }
                tracks.push(Track {
                    source_item_id: id.clone(),
                    title: f.title.clone(),
                    sort_title: f.title.clone(),
                    album_item_id,
                    album_title: f.album.clone(),
                    artists: artist_credits,
                    album_artists,
                    disc_number: f.disc_number,
                    track_number: f.track_number,
                    duration_ms: f.duration_ms,
                    year: f.year,
                    container: f.container.clone(),
                    bitrate: None,
                    mbid_recording: f.mbid_recording.clone(),
                    genres: f.genres.clone(),
                    date_added_utc_ms: Some(f.added_ms),
                    has_lyrics: f.lyrics.is_some(),
                    track_gain_db: None,
                    album_gain_db: None,
                });
            }
            let present: HashSet<&String> = artists
                .keys()
                .chain(albums.keys())
                .chain(tracks.iter().map(|t| &t.source_item_id))
                .collect();
            let removed_item_ids = known_ids
                .iter()
                .filter(|id| !present.contains(id))
                .cloned()
                .collect();
            LibraryDelta {
                full: true,
                artists: artists.into_values().collect(),
                albums: albums.into_values().collect(),
                tracks,
                playlists: Vec::new(),
                playlists_complete: true,
                removed_item_ids,
                favorite_item_ids: None,
                play_counts: None,
                cursor: None,
            }
        })
    }
}

fn album_artist_of(f: &LocalFile) -> String {
    f.album_artist
        .clone()
        .or_else(|| f.artists.first().cloned())
        .unwrap_or_else(|| "Unknown artist".to_owned())
}

fn album_of(f: &LocalFile) -> Option<String> {
    f.album.as_ref().map(|t| album_id(&album_artist_of(f), t))
}

fn storage(e: std::io::Error) -> SourceError {
    SourceError::Storage(e.to_string())
}

fn not_here() -> SourceError {
    SourceError::Protocol("music on this device has no server to change".into())
}

fn file_url(path: &Path) -> Result<String> {
    url::Url::from_file_path(path)
        .map(|u| u.to_string())
        .map_err(|_| SourceError::Storage("the cover's path isn't absolute".into()))
}

fn lyrics_format(body: &str) -> &'static str {
    let timed = body.lines().any(|l| {
        l.trim_start().starts_with('[') && l.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
    });
    if timed { "lrc" } else { "plain" }
}

impl crate::source::MediaSource for LocalSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Local
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            lyrics: true,
            transcoding: false,
            playlists_writable: false,
            quick_connect: false,
            playback_reporting: false,
        }
    }

    fn sync_library<'a>(
        &'a self,
        _since: Option<&'a Cursor>,
        known_ids: &'a HashSet<String>,
        _page_size: u32,
        progress: &'a (dyn Fn(SyncProgress) + Send + Sync),
    ) -> BoxFuture<'a, Result<LibraryDelta>> {
        Box::pin(async move {
            let delta = self.delta(known_ids)?;
            let n = delta.tracks.len() as u64;
            progress(SyncProgress { done: n, total: n });
            Ok(delta)
        })
    }

    fn stream_info(&self, item_id: &str, _req: &StreamRequest) -> Result<StreamInfo> {
        let uri = self.uri_of(item_id)?.ok_or(SourceError::NotFound)?;
        Ok(StreamInfo {
            url: uri.clone(),
            url_with_api_key: uri,
            headers: Vec::new(),
            may_transcode: false,
        })
    }

    fn artwork(&self, item_id: &str, _size_px: u32) -> Result<ArtworkInfo> {
        let path = self.cover_path(item_id)?.ok_or(SourceError::NotFound)?;
        Ok(ArtworkInfo {
            url: file_url(&path)?,
            headers: Vec::new(),
        })
    }

    fn lyrics<'a>(&'a self, item_id: &'a str) -> BoxFuture<'a, Result<Option<RawLyrics>>> {
        Box::pin(async move {
            let body = self.with(|ix| ix.files.get(item_id).and_then(|f| f.lyrics.clone()))?;
            Ok(body.map(|body| RawLyrics {
                format: lyrics_format(&body).into(),
                body,
            }))
        })
    }

    fn cover_png<'a>(
        &'a self,
        _item_id: &'a str,
        _size_px: u32,
        _tag: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<Vec<u8>>>> {
        // Covers here are mostly JPEG and the core decodes PNG only: the platform
        // samples their colours from the picture it shows.
        Box::pin(async move { Ok(None) })
    }

    fn edit<'a>(&'a self, _edit: &'a ServerEdit) -> BoxFuture<'a, Result<EditOutcome>> {
        Box::pin(async move { Err(not_here()) })
    }

    fn playlist_entries<'a>(
        &'a self,
        _playlist_id: &'a str,
    ) -> BoxFuture<'a, Result<Vec<PlaylistEntry>>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn download<'a>(
        &'a self,
        _item_id: &'a str,
        _req: &'a StreamRequest,
        _to: &'a Path,
        _resume_from: u64,
        _progress: Progress<'a>,
        _cancel: &'a std::sync::atomic::AtomicBool,
    ) -> BoxFuture<'a, Result<Downloaded>> {
        // Already on the device: nothing to download.
        Box::pin(async move { Err(not_here()) })
    }

    fn artwork_bytes<'a>(
        &'a self,
        item_id: &'a str,
        _size_px: u32,
    ) -> BoxFuture<'a, Result<Option<Vec<u8>>>> {
        Box::pin(async move {
            match self.cover_path(item_id)? {
                Some(p) => Ok(std::fs::read(p).ok()),
                None => Ok(None),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MediaSource;

    fn fixture(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/local")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    fn found(folder_id: u32, name: &str, uri: &str) -> FoundFile {
        FoundFile {
            folder_id,
            uri: uri.into(),
            file_name: name.into(),
            size: 1,
            modified_ms: 1,
            read_path: fixture(name),
            lrc: None,
            folder_cover_path: None,
        }
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("encore-local-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[tokio::test]
    async fn a_look_becomes_a_library() {
        let dir = temp("library");
        let src = LocalSource::open(&dir).unwrap();
        let f = src.add_folder("content://tree/Music", "Music").unwrap();
        for (name, uri) in [
            ("tagged.mp3", "content://m/1"),
            ("tagged.flac", "content://m/2"),
            ("untagged.mp3", "content://m/3"),
        ] {
            assert_eq!(
                src.add_file(&found(f.id, name, uri), 100).unwrap(),
                Added::Song
            );
        }
        assert_eq!(
            src.add_file(&found(f.id, "notes.txt", "content://m/4"), 100)
                .unwrap(),
            Added::Skipped
        );
        src.finish_look(
            f.id,
            &[
                "content://m/1".into(),
                "content://m/2".into(),
                "content://m/3".into(),
            ],
        )
        .unwrap();

        let delta = src
            .sync_library(None, &HashSet::new(), 100, &|_| {})
            .await
            .unwrap();
        assert!(delta.full);
        assert_eq!(delta.tracks.len(), 3);
        // Both tagged files are on "Test Tones" by Encore Fixtures (the FLAC has no
        // album artist, so its first artist stands in): one album.
        assert_eq!(delta.albums.len(), 1);
        let mp3 = delta
            .tracks
            .iter()
            .find(|t| t.title == "Morning Tone")
            .unwrap();
        assert_eq!(mp3.artists.len(), 2);
        let album = delta
            .albums
            .iter()
            .find(|a| Some(&a.source_item_id) == mp3.album_item_id.as_ref())
            .unwrap();
        assert!(album.has_artwork);
        // The untagged one is named from its file and has no album.
        let untagged = delta.tracks.iter().find(|t| t.title == "untagged").unwrap();
        assert_eq!(untagged.album_item_id, None);
        assert!(untagged.duration_ms > 900);

        // Plays from its own address; the cover is a file; the FLAC's lyrics are LRC.
        let stream = src
            .stream_info(
                &mp3.source_item_id,
                &StreamRequest {
                    quality: crate::jellyfin::StreamQuality::Original,
                    codec: crate::jellyfin::StreamCodec::Aac,
                    direct_play_containers: Vec::new(),
                },
            )
            .unwrap();
        assert_eq!(stream.url, "content://m/1");
        assert!(stream.headers.is_empty());
        let art = src.artwork(album.source_item_id.as_str(), 300).unwrap();
        assert!(art.url.starts_with("file:"), "{}", art.url);
        let flac = delta
            .tracks
            .iter()
            .find(|t| t.title == "Evening Tone")
            .unwrap();
        assert!(flac.has_lyrics);
        let lyrics = src.lyrics(&flac.source_item_id).await.unwrap().unwrap();
        assert_eq!(lyrics.format, "lrc");

        // A file that's gone leaves, and the next sync says so.
        let known: HashSet<String> = delta
            .tracks
            .iter()
            .map(|t| t.source_item_id.clone())
            .collect();
        src.finish_look(f.id, &["content://m/1".into(), "content://m/3".into()])
            .unwrap();
        let again = LocalSource::open(&dir).unwrap();
        let delta2 = again
            .sync_library(None, &known, 100, &|_| {})
            .await
            .unwrap();
        assert_eq!(delta2.tracks.len(), 2);
        assert!(delta2.removed_item_ids.contains(&flac.source_item_id));
        // Removing the folder empties the library; the files are untouched.
        assert!(again.remove_folder(f.id).unwrap());
        assert!(
            again
                .sync_library(None, &known, 100, &|_| {})
                .await
                .unwrap()
                .tracks
                .is_empty()
        );
        assert!(std::path::Path::new(&fixture("tagged.mp3")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn music_files_by_name() {
        assert!(is_music_file("a.MP3"));
        assert!(is_music_file("b.flac"));
        assert!(!is_music_file("cover.jpg"));
        assert!(!is_music_file("lyrics.lrc"));
        assert!(!is_music_file("noext"));
    }
}
