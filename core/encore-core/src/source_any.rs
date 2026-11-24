//! A signed-in media server of any kind (ADR core-0021): the profile's sources are these,
//! so the rest of the core never names Jellyfin. Everything a source does goes through
//! `MediaSource`; the few things that are only sometimes possible (sound analysis's ranged
//! reads, the avatar, signing out on the server, the companion's sign-in) are here, each
//! answering for its kind.

use std::collections::HashSet;

use encore_sources::jellyfin::SyncProgress;
use encore_sources::ranged::RangeReader;
use encore_sources::transport::{BoxFuture, Downloaded, Progress};
use encore_sources::{
    Capabilities, Cursor, EditOutcome, JellyfinSource, LibraryDelta, MediaSource, PlaylistEntry,
    Result, ServerEdit, SourceKind, SubsonicSource,
};

#[derive(Debug)]
pub enum AnySource {
    Jellyfin(JellyfinSource),
    /// Navidrome and other OpenSubsonic servers (ADR core-0021).
    Subsonic(SubsonicSource),
}

/// What the companion needs to check who a device is (core-0009, core-0021 point 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionCredential {
    /// A Jellyfin access token.
    Jellyfin(String),
    /// A Subsonic saved sign-in (`SubsonicCredentials::to_token_string`): user name and salted
    /// token or API key, never a password.
    Subsonic(String),
}

impl CompanionCredential {
    /// What `/auth/exchange` takes as `media_token`: the companion tells the kinds apart.
    pub fn media_token(&self) -> &str {
        match self {
            CompanionCredential::Jellyfin(t) | CompanionCredential::Subsonic(t) => t,
        }
    }
}

impl AnySource {
    fn inner(&self) -> &dyn MediaSource {
        match self {
            AnySource::Jellyfin(s) => s,
            AnySource::Subsonic(s) => s,
        }
    }

    /// Ranged reads of a song's original file, for sound analysis.
    pub fn range_reader(
        &self,
        item_id: &str,
        runtime: tokio::runtime::Handle,
    ) -> Result<RangeReader> {
        match self {
            AnySource::Jellyfin(s) => s.client.range_reader(item_id, runtime),
            AnySource::Subsonic(s) => s.client.range_reader(item_id, runtime),
        }
    }

    /// The signed-in user's picture for the profile switcher: the server's address, the
    /// user's ID and the picture's tag; None when the user has none.
    pub async fn avatar(&self) -> Result<Option<(String, String, String)>> {
        match self {
            AnySource::Jellyfin(s) => {
                let me = s.client.me().await?;
                Ok(me
                    .image_tag
                    .map(|tag| (s.client.base_url().as_str().to_owned(), me.user_id, tag)))
            }
            // The switcher's picture is Jellyfin's (address, user, tag); a Subsonic
            // picture is a signed address, not kept in the registry. The initial shows.
            AnySource::Subsonic(_) => Ok(None),
        }
    }

    /// End the session on the server where the protocol has one.
    pub async fn sign_out(&self) -> Result<()> {
        match self {
            AnySource::Jellyfin(s) => s.client.sign_out().await,
            // Subsonic has no session to end: forgetting the token here is signing out.
            AnySource::Subsonic(_) => Ok(()),
        }
    }

    /// The sign-in the companion checks, if this source can give one.
    pub fn companion_credential(&self) -> Option<CompanionCredential> {
        match self {
            AnySource::Jellyfin(s) => s
                .client
                .token_for_companion_sign_in()
                .map(|t| CompanionCredential::Jellyfin(t.to_owned())),
            AnySource::Subsonic(s) => s
                .client
                .credentials()
                .map(|c| CompanionCredential::Subsonic(c.to_token_string())),
        }
    }
}

impl MediaSource for AnySource {
    fn kind(&self) -> SourceKind {
        self.inner().kind()
    }
    fn capabilities(&self) -> Capabilities {
        self.inner().capabilities()
    }
    fn sync_library<'a>(
        &'a self,
        since: Option<&'a Cursor>,
        known_ids: &'a HashSet<String>,
        page_size: u32,
        progress: &'a (dyn Fn(SyncProgress) + Send + Sync),
    ) -> BoxFuture<'a, Result<LibraryDelta>> {
        self.inner()
            .sync_library(since, known_ids, page_size, progress)
    }
    fn stream_info(
        &self,
        item_id: &str,
        req: &encore_sources::jellyfin::StreamRequest,
    ) -> Result<encore_sources::jellyfin::StreamInfo> {
        self.inner().stream_info(item_id, req)
    }
    fn artwork(
        &self,
        item_id: &str,
        size_px: u32,
    ) -> Result<encore_sources::jellyfin::ArtworkInfo> {
        self.inner().artwork(item_id, size_px)
    }
    fn lyrics<'a>(
        &'a self,
        item_id: &'a str,
    ) -> BoxFuture<'a, Result<Option<encore_sources::jellyfin::RawLyrics>>> {
        self.inner().lyrics(item_id)
    }
    fn cover_png<'a>(
        &'a self,
        item_id: &'a str,
        size_px: u32,
        tag: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<Vec<u8>>>> {
        self.inner().cover_png(item_id, size_px, tag)
    }
    fn edit<'a>(&'a self, edit: &'a ServerEdit) -> BoxFuture<'a, Result<EditOutcome>> {
        self.inner().edit(edit)
    }
    fn playlist_entries<'a>(
        &'a self,
        playlist_id: &'a str,
    ) -> BoxFuture<'a, Result<Vec<PlaylistEntry>>> {
        self.inner().playlist_entries(playlist_id)
    }
    fn download<'a>(
        &'a self,
        item_id: &'a str,
        req: &'a encore_sources::jellyfin::StreamRequest,
        to: &'a std::path::Path,
        resume_from: u64,
        progress: Progress<'a>,
        cancel: &'a std::sync::atomic::AtomicBool,
    ) -> BoxFuture<'a, Result<Downloaded>> {
        self.inner()
            .download(item_id, req, to, resume_from, progress, cancel)
    }
    fn artwork_bytes<'a>(
        &'a self,
        item_id: &'a str,
        size_px: u32,
    ) -> BoxFuture<'a, Result<Option<Vec<u8>>>> {
        self.inner().artwork_bytes(item_id, size_px)
    }
}
