//! The Jellyfin adapter (plan section 4), written against Jellyfin 12.0's API and
//! supporting 10.10 and later.

pub mod address;
pub mod auth;
pub mod client;
pub mod dto;
pub(crate) mod map;
pub mod sync;
pub mod time;

pub use address::{AddressPlan, AddressWarning, plan_address};
pub use auth::DeviceInfo;
pub use client::{
    ArtworkInfo, JellyfinClient, Me, QuickConnectCode, RawLyrics, ServerInfo, Session, StreamCodec,
    StreamInfo, StreamQuality, StreamRequest, probe,
};
pub use sync::{SyncProgress, sync_library};

use crate::model::Capabilities;

/// What a Jellyfin server offers Encore.
pub fn capabilities(quick_connect: bool) -> Capabilities {
    Capabilities {
        lyrics: true,
        transcoding: true,
        playlists_writable: true,
        quick_connect,
        // Off by default; the companion server keeps the real history (section 4).
        playback_reporting: false,
    }
}

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod recorded_tests;
