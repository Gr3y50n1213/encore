//! The Plex adapter (ADR core-0023): a Plex Media Server's music libraries, reached
//! with one server's access token that plex.tv hands over at sign-in (the PIN link,
//! `tv`). JSON only (`Accept: application/json`).

pub mod client;
pub(crate) mod dto;
pub(crate) mod map;
pub mod sync;
pub mod tv;

pub use client::{PlexClient, PlexCredentials, PlexInfo};
pub use sync::{library_changed, sync_library};
pub use tv::{
    AccountToken, PlexAccount, PlexConnection, PlexIdentity, PlexPin, PlexServerResource, PlexTv,
    client_identifier,
};

use crate::model::Capabilities;

/// What a Plex server offers Encore. Lyrics: the lyric streams of each track.
/// Transcoding: the universal music transcoder under a cap. Playlists: every edit
/// (Plex has entry IDs and moves). No Quick Connect (the plex.tv link is its own flow,
/// told apart by `ServerKind::Plex`), and play reporting stays with the companion.
pub fn capabilities() -> Capabilities {
    Capabilities {
        lyrics: true,
        transcoding: true,
        playlists_writable: true,
        quick_connect: false,
        playback_reporting: false,
    }
}

#[cfg(test)]
mod fixture_tests;
