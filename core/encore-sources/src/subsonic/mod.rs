//! The OpenSubsonic adapter (ADR core-0021): Navidrome, gonic, Airsonic and every
//! other server that speaks the Subsonic API, with OpenSubsonic's additions where a
//! server offers them (`songLyrics`, `apiKeyAuthentication`). JSON only.
//!
//! The API reference is opensubsonic.github.io/docs/ (it was opensubsonic.netlify.app).

pub mod client;
pub(crate) mod dto;
pub(crate) mod map;
mod md5;
pub mod sync;

pub use client::{
    API_VERSION, SubsonicClient, SubsonicCredentials, SubsonicInfo, SubsonicUser, server_id_for,
};
pub use sync::{cursor_last_modified, library_changed, sync_library};

use crate::model::Capabilities;

/// What a Subsonic server offers Encore. Lyrics: `getLyrics` is in every Subsonic
/// API since 1.2.0, `getLyricsBySongId` where offered. Transcoding: `stream`'s
/// `maxBitRate` and `format`. Playlists: every edit is possible, moves included
/// (written back in order, see `SubsonicClient::edit`). No Quick Connect, and play
/// reporting stays with the companion server.
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
