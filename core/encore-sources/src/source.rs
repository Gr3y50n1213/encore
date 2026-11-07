//! The `MediaSource` trait (plan section 4). The core only talks to sources through
//! it, so a new server type is a new adapter and no UI change.
//!
//! Signing in is not part of the trait: every server type asks for different
//! credentials, so each adapter has its own connect flow and then hands the core a
//! `MediaSource` for the signed-in account.

use std::collections::HashSet;
use std::fmt;

use crate::error::Result;
use crate::jellyfin::{
    self, ArtworkInfo, JellyfinClient, RawLyrics, StreamInfo, StreamRequest, SyncProgress,
};
use crate::model::{
    Capabilities, Cursor, EditOutcome, LibraryDelta, PlaylistEntry, ServerEdit, SourceKind,
};
use crate::transport::BoxFuture;

pub trait MediaSource: Send + Sync + fmt::Debug {
    fn kind(&self) -> SourceKind;
    fn capabilities(&self) -> Capabilities;
    /// Everything new or changed since `since` (everything when None), and what was
    /// removed among `known_ids`.
    fn sync_library<'a>(
        &'a self,
        since: Option<&'a Cursor>,
        known_ids: &'a HashSet<String>,
        page_size: u32,
        progress: &'a (dyn Fn(SyncProgress) + Send + Sync),
    ) -> BoxFuture<'a, Result<LibraryDelta>>;
    fn stream_info(&self, item_id: &str, req: &StreamRequest) -> Result<StreamInfo>;
    fn artwork(&self, item_id: &str, size_px: u32) -> Result<ArtworkInfo>;
    fn lyrics<'a>(&'a self, item_id: &'a str) -> BoxFuture<'a, Result<Option<RawLyrics>>>;
    /// The cover as PNG for colour sampling, at most size_px on its longer side;
    /// None when there is no cover.
    fn cover_png<'a>(
        &'a self,
        item_id: &'a str,
        size_px: u32,
        tag: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<Vec<u8>>>>;
    /// Send one playlist or favourite write.
    fn edit<'a>(&'a self, edit: &'a ServerEdit) -> BoxFuture<'a, Result<EditOutcome>>;
    /// A playlist's songs as the server has them now, with their entry IDs.
    fn playlist_entries<'a>(
        &'a self,
        playlist_id: &'a str,
    ) -> BoxFuture<'a, Result<Vec<PlaylistEntry>>>;
}

/// A signed-in Jellyfin account.
#[derive(Debug)]
pub struct JellyfinSource {
    pub client: JellyfinClient,
    pub quick_connect: bool,
}

impl MediaSource for JellyfinSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Jellyfin
    }

    fn capabilities(&self) -> Capabilities {
        jellyfin::capabilities(self.quick_connect)
    }

    fn sync_library<'a>(
        &'a self,
        since: Option<&'a Cursor>,
        known_ids: &'a HashSet<String>,
        page_size: u32,
        progress: &'a (dyn Fn(SyncProgress) + Send + Sync),
    ) -> BoxFuture<'a, Result<LibraryDelta>> {
        Box::pin(jellyfin::sync_library(
            &self.client,
            since,
            known_ids,
            page_size,
            progress,
        ))
    }

    fn stream_info(&self, item_id: &str, req: &StreamRequest) -> Result<StreamInfo> {
        self.client.stream(item_id, req)
    }

    fn artwork(&self, item_id: &str, size_px: u32) -> Result<ArtworkInfo> {
        self.client.artwork(item_id, size_px)
    }

    fn lyrics<'a>(&'a self, item_id: &'a str) -> BoxFuture<'a, Result<Option<RawLyrics>>> {
        Box::pin(self.client.lyrics(item_id))
    }

    fn cover_png<'a>(
        &'a self,
        item_id: &'a str,
        size_px: u32,
        tag: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<Vec<u8>>>> {
        Box::pin(self.client.cover_png(item_id, size_px, tag))
    }

    fn edit<'a>(&'a self, edit: &'a ServerEdit) -> BoxFuture<'a, Result<EditOutcome>> {
        Box::pin(self.client.edit(edit))
    }

    fn playlist_entries<'a>(
        &'a self,
        playlist_id: &'a str,
    ) -> BoxFuture<'a, Result<Vec<PlaylistEntry>>> {
        Box::pin(async move {
            let page = self.client.playlist_items(playlist_id).await?;
            Ok(page
                .items
                .unwrap_or_default()
                .iter()
                .filter_map(jellyfin::map::to_playlist_entry)
                .collect())
        })
    }
}
