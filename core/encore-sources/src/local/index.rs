//! What Encore knows about the music files on the device (ADR core-0024): the folders
//! the listener chose, and for each file found in them its address, size, time and
//! tags. Kept as one JSON file in the profile's folder, with the covers beside it, so a
//! later look reads only new or changed files.
//!
//! Addresses (on Android, `content://` addresses of the chosen folders' documents) stay
//! on the device: library IDs are hashes of them, so nothing that syncs names a folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::tags::{LocalTags, name_from_file};

/// A folder the listener chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFolder {
    pub id: u32,
    /// The platform's address for it (Android: the tree's `content://` address).
    pub uri: String,
    /// What to call it ("Music", "Download/Albums").
    pub name: String,
    /// False while the platform can't reach it (an SD card taken out).
    #[serde(default = "yes")]
    pub available: bool,
}

fn yes() -> bool {
    true
}

/// One music file and what it says about itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalFile {
    pub folder_id: u32,
    pub uri: String,
    pub file_name: String,
    pub size: u64,
    pub modified_ms: i64,
    /// When Encore first found it.
    pub added_ms: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
    pub duration_ms: u64,
    pub mbid_recording: Option<String>,
    pub container: Option<String>,
    /// The tag's lyrics, else a matching `.lrc` file's.
    pub lyrics: Option<String>,
    /// The cover's file name in the covers folder.
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalIndex {
    pub folders: Vec<LocalFolder>,
    /// By library ID ([`file_id`]).
    pub files: BTreeMap<String, LocalFile>,
    #[serde(default)]
    next_folder_id: u32,
}

/// The library ID of a file: a hash of its address.
pub fn file_id(uri: &str) -> String {
    format!("t{}", short_hash("file", uri))
}

/// The library ID of an album: its artist and title, ignoring case.
pub fn album_id(album_artist: &str, title: &str) -> String {
    format!(
        "a{}",
        short_hash(
            "album",
            &format!("{}\u{1f}{}", fold(album_artist), fold(title))
        )
    )
}

/// The library ID of an artist: the name, ignoring case.
pub fn artist_id(name: &str) -> String {
    format!("r{}", short_hash("artist", &fold(name)))
}

pub(crate) fn short_hash(label: &str, value: &str) -> String {
    let d = Sha256::new()
        .chain_update(label.as_bytes())
        .chain_update([0u8])
        .chain_update(value.as_bytes())
        .finalize();
    d[..12].iter().map(|b| format!("{b:02x}")).collect()
}

fn fold(s: &str) -> String {
    s.trim().to_lowercase()
}

impl LocalIndex {
    pub fn load(path: &Path) -> std::io::Result<LocalIndex> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LocalIndex::default()),
            Err(e) => Err(e),
        }
    }

    /// Written whole to a new file, then moved over the old one.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let part: PathBuf = path.with_extension("json.part");
        std::fs::write(&part, bytes)?;
        std::fs::rename(&part, path)
    }

    /// Add a folder, or return the one already there with this address.
    pub fn add_folder(&mut self, uri: &str, name: &str) -> LocalFolder {
        if let Some(f) = self.folders.iter_mut().find(|f| f.uri == uri) {
            f.name = name.to_owned();
            f.available = true;
            return f.clone();
        }
        self.next_folder_id = self
            .next_folder_id
            .max(self.folders.iter().map(|f| f.id).max().unwrap_or(0))
            + 1;
        let f = LocalFolder {
            id: self.next_folder_id,
            uri: uri.to_owned(),
            name: name.to_owned(),
            available: true,
        };
        self.folders.push(f.clone());
        f
    }

    /// Stop using a folder: its files leave the library (they stay on the device).
    pub fn remove_folder(&mut self, folder_id: u32) -> bool {
        let before = self.folders.len();
        self.folders.retain(|f| f.id != folder_id);
        self.files.retain(|_, f| f.folder_id != folder_id);
        self.folders.len() != before
    }

    pub fn set_available(&mut self, folder_id: u32, available: bool) {
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == folder_id) {
            f.available = available;
        }
    }

    /// The files known in a folder, for the platform to skip unchanged ones.
    pub fn known(&self, folder_id: u32) -> Vec<&LocalFile> {
        self.files
            .values()
            .filter(|f| f.folder_id == folder_id)
            .collect()
    }

    /// Whether this file is already known with this size and time.
    pub fn is_current(&self, uri: &str, size: u64, modified_ms: i64) -> bool {
        self.files
            .get(&file_id(uri))
            .is_some_and(|f| f.size == size && f.modified_ms == modified_ms)
    }

    /// After a look through a folder: the files not seen in it are gone.
    pub fn forget_unseen(&mut self, folder_id: u32, seen: &[String]) -> usize {
        let seen: std::collections::HashSet<String> = seen.iter().map(|u| file_id(u)).collect();
        let before = self.files.len();
        self.files
            .retain(|id, f| f.folder_id != folder_id || seen.contains(id));
        before - self.files.len()
    }

    /// Put a file's tags in the index (replacing what was known of it).
    #[allow(clippy::too_many_arguments)]
    pub fn put(
        &mut self,
        folder_id: u32,
        uri: &str,
        file_name: &str,
        size: u64,
        modified_ms: i64,
        now_ms: i64,
        container: Option<String>,
        tags: LocalTags,
        lyrics_file: Option<String>,
        cover: Option<String>,
    ) -> String {
        let id = file_id(uri);
        let added_ms = self.files.get(&id).map(|f| f.added_ms).unwrap_or(now_ms);
        let (name_title, name_artist) = name_from_file(file_name);
        let mut artists = tags.artists;
        if artists.is_empty() {
            artists.extend(tags.album_artist.clone().or(name_artist));
        }
        let file = LocalFile {
            folder_id,
            uri: uri.to_owned(),
            file_name: file_name.to_owned(),
            size,
            modified_ms,
            added_ms,
            title: tags.title.unwrap_or(name_title),
            artists,
            album: tags.album,
            album_artist: tags.album_artist,
            track_number: tags.track_number,
            disc_number: tags.disc_number,
            year: tags.year,
            genres: tags.genres,
            duration_ms: tags.duration_ms.unwrap_or(0),
            mbid_recording: tags.musicbrainz_recording_id,
            container,
            lyrics: tags.lyrics.or(lyrics_file),
            cover,
        };
        self.files.insert(id.clone(), file);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(title: &str) -> LocalTags {
        LocalTags {
            title: Some(title.into()),
            artists: vec!["A".into()],
            album: Some("Al".into()),
            duration_ms: Some(1000),
            ..Default::default()
        }
    }

    #[test]
    fn folders_files_and_what_a_look_leaves() {
        let mut ix = LocalIndex::default();
        let music = ix.add_folder("content://tree/Music", "Music");
        let dl = ix.add_folder("content://tree/Download", "Download");
        assert_eq!(ix.add_folder("content://tree/Music", "Music").id, music.id);
        assert_ne!(music.id, dl.id);
        ix.put(
            music.id,
            "content://m/1",
            "1.mp3",
            10,
            5,
            100,
            None,
            tags("One"),
            None,
            None,
        );
        ix.put(
            music.id,
            "content://m/2",
            "2.mp3",
            10,
            5,
            100,
            None,
            tags("Two"),
            None,
            None,
        );
        ix.put(
            dl.id,
            "content://d/3",
            "3.mp3",
            10,
            5,
            100,
            None,
            tags("Three"),
            None,
            None,
        );
        assert!(ix.is_current("content://m/1", 10, 5));
        assert!(!ix.is_current("content://m/1", 11, 5));
        // A second look finds only the first file in Music.
        assert_eq!(ix.forget_unseen(music.id, &["content://m/1".into()]), 1);
        assert_eq!(ix.files.len(), 2);
        // A file seen again keeps when it was first found.
        ix.put(
            music.id,
            "content://m/1",
            "1.mp3",
            11,
            6,
            900,
            None,
            tags("One"),
            None,
            None,
        );
        assert_eq!(ix.files[&file_id("content://m/1")].added_ms, 100);
        assert!(ix.remove_folder(dl.id));
        assert_eq!(ix.files.len(), 1);
    }

    #[test]
    fn a_file_without_tags_is_named_from_its_file_name() {
        let mut ix = LocalIndex::default();
        let f = ix.add_folder("content://tree/Music", "Music");
        let id = ix.put(
            f.id,
            "content://m/9",
            "07 - Some Band - A Song.mp3",
            1,
            1,
            1,
            None,
            LocalTags::default(),
            Some("[00:01.00]la".into()),
            None,
        );
        let file = &ix.files[&id];
        assert_eq!(file.title, "A Song");
        assert_eq!(file.artists, vec!["Some Band".to_owned()]);
        assert_eq!(file.lyrics.as_deref(), Some("[00:01.00]la"));
    }

    #[test]
    fn ids_hide_the_address_and_ignore_case() {
        let id = file_id("content://tree/Music/My%20Folder/song.mp3");
        assert!(!id.contains("Music") && !id.contains("song"));
        assert_eq!(album_id("Band", "Album"), album_id(" band ", "ALBUM"));
        assert_eq!(artist_id("Band"), artist_id("BAND"));
    }

    #[test]
    fn the_index_survives_a_save() {
        let dir = std::env::temp_dir().join(format!("encore-local-ix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("local.json");
        let mut ix = LocalIndex::default();
        let f = ix.add_folder("content://tree/Music", "Music");
        ix.put(
            f.id,
            "content://m/1",
            "1.mp3",
            1,
            1,
            1,
            None,
            tags("One"),
            None,
            None,
        );
        ix.save(&path).unwrap();
        let back = LocalIndex::load(&path).unwrap();
        assert_eq!(back.folders, ix.folders);
        assert_eq!(back.files, ix.files);
        assert!(
            LocalIndex::load(&dir.join("none.json"))
                .unwrap()
                .files
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
