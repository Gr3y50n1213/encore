//! The `MediaSource` trait, the normalized library model and one adapter per media
//! server (Jellyfin first; OpenSubsonic, Plex, Emby and local files later).
//!
//! Plan section 4. The UI never talks to a media server; it talks to the core, which
//! talks to a `MediaSource`.

#![forbid(unsafe_code)]

pub mod model;
pub mod song_key;

pub use model::*;
