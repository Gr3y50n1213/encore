//! The device's own music files as a source (Phase 12 stage 4, ADR core-0024,
//! mock-up 49). The platform finds the files in the folders the listener chose and
//! hands each to `local_add_file`; the core reads its tags and keeps an index; a look
//! ends with `local_finish_look`, which brings the library up to date.

use std::sync::Arc;

use encore_sources::SourceKind;
use encore_sources::local::{Added, FoundFile, LocalSource};
use encore_store::{NewSource, Store};

use super::{EncoreCore, Inner, Profile, credential_key, lock, now_ms, profile_id};
use crate::api::types::*;
use crate::source_any::AnySource;

/// What the Servers page calls the device's music.
const LOCAL_NAME: &str = "On this phone";

impl Inner {
    /// The open profile's device music: its source ID and the source, if it has one.
    fn local_source(&self, p: &Profile) -> Result<Option<(i64, Arc<AnySource>)>, EncoreError> {
        Ok(lock(&p.sources)?
            .iter()
            .find(|(_, s)| s.local().is_some())
            .map(|(id, s)| (*id, s.clone())))
    }

    /// The device music source, made the first time a folder is added.
    fn local_source_or_new(
        &self,
        p: &Profile,
        dir: &std::path::Path,
    ) -> Result<(i64, Arc<AnySource>), EncoreError> {
        if let Some(found) = self.local_source(p)? {
            return Ok(found);
        }
        let source_id = add_local_record(&*lock(&p.store)?)?;
        let source = Arc::new(AnySource::Local(LocalSource::open(dir)?));
        lock(&p.sources)?.insert(source_id, source.clone());
        Ok((source_id, source))
    }
}

fn add_local_record(store: &Store) -> Result<i64, EncoreError> {
    Ok(store.add_source(&NewSource {
        kind: SourceKind::Local.as_str(),
        name: LOCAL_NAME,
        base_url: "",
        server_id: "device",
        server_version: None,
        user_id: "local",
        user_name: "",
        pinned_cert_sha256: None,
        created_utc_ms: now_ms(),
    })?)
}

fn local_of(source: &AnySource) -> Result<&LocalSource, EncoreError> {
    source.local().ok_or_else(|| EncoreError::NotFound {
        reason: "This profile has no music on this device".into(),
    })
}

/// A file descriptor the platform hands over, now owned (and closed) here.
#[cfg(unix)]
fn adopt_fd(fd: i32) -> Result<std::fs::File, EncoreError> {
    use std::os::fd::FromRawFd;
    if fd < 0 {
        return Err(EncoreError::Storage {
            reason: "not an open file".into(),
        });
    }
    // SAFETY: the caller hands over an open descriptor it no longer uses (Android's
    // `ParcelFileDescriptor.detachFd()`); this `File` becomes its only owner and closes it.
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

#[cfg(not(unix))]
fn adopt_fd(_fd: i32) -> Result<std::fs::File, EncoreError> {
    Err(EncoreError::Storage {
        reason: "file descriptors are handed over only on Android and Apple platforms".into(),
    })
}

fn info(f: encore_sources::local::LocalFolder, song_count: u32) -> LocalFolderInfo {
    LocalFolderInfo {
        folder_id: f.id,
        uri: f.uri,
        name: f.name,
        song_count,
        available: f.available,
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl EncoreCore {
    /// The folders of the device's music in the open profile (mock-up 49, screen 2);
    /// empty when there are none.
    pub fn local_folders(&self) -> Result<Vec<LocalFolderInfo>, EncoreError> {
        let p = self.inner.profile()?;
        let Some((_, source)) = self.inner.local_source(&p)? else {
            return Ok(Vec::new());
        };
        Ok(local_of(&source)?
            .folders()?
            .into_iter()
            .map(|(f, n)| info(f, n))
            .collect())
    }

    /// Add a folder the listener chose (`uri` is the platform's address for it, kept on
    /// this device only). The first one makes the profile's device music source. Then
    /// look through it: `local_known_files`, `local_add_file` for each file, and
    /// `local_finish_look`.
    pub fn add_local_folder(
        &self,
        uri: String,
        name: String,
    ) -> Result<LocalFolderInfo, EncoreError> {
        let p = self.inner.profile()?;
        let (_, source) = self
            .inner
            .local_source_or_new(&p, &self.inner.local_dir(&p.id))?;
        let local = local_of(&source)?;
        let f = local.add_folder(&uri, &name)?;
        let n = local
            .folders()?
            .into_iter()
            .find(|(x, _)| x.id == f.id)
            .map(|(_, n)| n)
            .unwrap_or(0);
        Ok(info(f, n))
    }

    /// Stop using a folder: its songs leave the library, the files stay. With no folder
    /// left, the device music source goes too.
    pub async fn remove_local_folder(&self, folder_id: u32) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        let Some((source_id, source)) = self.inner.local_source(&p)? else {
            return Ok(());
        };
        let local = local_of(&source)?;
        local.remove_folder(folder_id)?;
        if local.folders()?.is_empty() {
            self.remove_server(source_id)?;
            let _ = std::fs::remove_dir_all(self.inner.local_dir(&p.id));
            return Ok(());
        }
        self.local_sync(source_id, source).await
    }

    /// Whether the platform can reach a folder now (an SD card taken out: "Not available
    /// right now"). Its songs stay listed.
    pub fn set_local_folder_available(
        &self,
        folder_id: u32,
        available: bool,
    ) -> Result<(), EncoreError> {
        let p = self.inner.profile()?;
        if let Some((_, source)) = self.inner.local_source(&p)? {
            local_of(&source)?.set_available(folder_id, available)?;
        }
        Ok(())
    }

    /// The files already known in a folder, so the platform can hand over only new or
    /// changed ones (any it hands over unchanged are skipped anyway).
    pub fn local_known_files(&self, folder_id: u32) -> Result<Vec<LocalKnownFile>, EncoreError> {
        let p = self.inner.profile()?;
        let Some((_, source)) = self.inner.local_source(&p)? else {
            return Ok(Vec::new());
        };
        Ok(local_of(&source)?
            .known_files(folder_id)?
            .into_iter()
            .map(|(uri, size, modified_ms)| LocalKnownFile {
                uri,
                size,
                modified_ms,
            })
            .collect())
    }

    /// Read one file into the device's index. True when it is a song; false when it
    /// isn't music Encore reads. Call from a background thread: it reads the file.
    pub fn local_add_file(&self, file: LocalFoundFile) -> Result<bool, EncoreError> {
        let p = self.inner.profile()?;
        let Some((_, source)) = self.inner.local_source(&p)? else {
            return Err(EncoreError::NotFound {
                reason: "Add a folder first".into(),
            });
        };
        // Descriptors are taken over first, so they are closed whatever happens next.
        let reader = file.read_fd.map(adopt_fd).transpose()?;
        let cover = file.folder_cover_fd.map(adopt_fd).transpose()?;
        let found = FoundFile {
            folder_id: file.folder_id,
            uri: file.uri,
            file_name: file.file_name,
            size: file.size,
            modified_ms: file.modified_ms,
            read_path: file.read_path,
            lrc: file.lrc,
            folder_cover_path: file.folder_cover_path,
        };
        let local = local_of(&source)?;
        let added = match reader {
            Some(r) => local.add_opened(&found, r, cover, now_ms())?,
            None => local.add_file(&found, now_ms())?,
        };
        Ok(added == Added::Song)
    }

    /// The look through a folder is done: `seen` is every file address found in it this
    /// time (files not in it are gone). The library is brought up to date. Async.
    pub async fn local_finish_look(
        &self,
        folder_id: u32,
        seen: Vec<String>,
    ) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let Some((source_id, source)) = self.inner.local_source(&p)? else {
            return Ok(0);
        };
        local_of(&source)?.finish_look(folder_id, &seen)?;
        self.local_sync(source_id, source).await?;
        Ok(lock(&p.store)?.track_count(source_id)?)
    }

    /// Bring the library up to date during a long look, so songs appear as they are
    /// found (mock-up 49, screen 3). Nothing is forgotten until `local_finish_look`.
    /// Async.
    pub async fn local_refresh(&self) -> Result<u32, EncoreError> {
        let p = self.inner.profile()?;
        let Some((source_id, source)) = self.inner.local_source(&p)? else {
            return Ok(0);
        };
        self.local_sync(source_id, source).await?;
        Ok(lock(&p.store)?.track_count(source_id)?)
    }

    /// A profile with only the device's music, for someone with no server (mock-up 49,
    /// screen 5). It opens at once; add a folder next.
    pub fn create_device_profile(&self, name: String) -> Result<SignedIn, EncoreError> {
        let user_id = ulid::Ulid::generate().to_string();
        let id = profile_id("device", &user_id);
        let path = self.inner.store_path(&id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| EncoreError::Storage {
                reason: format!("couldn't create the profile folder ({e})"),
            })?;
        }
        let source_id = add_local_record(&Store::open(&path)?)?;
        lock(&self.inner.unlocked)?.insert(id.clone());
        self.inner.open_profile(&id, Vec::new())?;
        let name = name.trim();
        let user_name = if name.is_empty() { "Me" } else { name }.to_owned();
        lock(&self.inner.registry)?.set_display_name(&id, Some(&user_name))?;
        Ok(SignedIn {
            credential_key: credential_key(&id, source_id),
            profile_id: id,
            source_id,
            token: String::new(),
            user_name,
        })
    }
}

impl EncoreCore {
    /// Bring the library up to date with the device's index: the device's songs, minus
    /// those a server also has (listed once, as the server's).
    async fn local_sync(&self, source_id: i64, source: Arc<AnySource>) -> Result<(), EncoreError> {
        let inner = self.inner.clone();
        self.spawn(async move {
            let p = inner.profile()?;
            crate::library::sync_source(
                &p.store,
                source_id,
                source.as_ref(),
                inner.sync_page_size,
                now_ms(),
                &|_| {},
            )
            .await
            .map_err(EncoreError::from)?;
            lock(&p.store)?.drop_device_twins()?;
            let _ = inner.reconcile_downloads(&p, None);
            inner.refresh_playable(&p)?;
            inner.widgets_soon();
            Ok(())
        })
        .await
    }
}
