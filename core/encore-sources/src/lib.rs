//! The `MediaSource` adapters, the normalized library model and the song key.
//!
//! Plan section 4. The UI never talks to a media server; it talks to the core, which
//! talks to an adapter here. Jellyfin is the first; OpenSubsonic, Plex, Emby and local
//! files follow with no UI change.
//!
//! Every adapter runs on a `Transport`, so its tests replay recorded HTTP fixtures
//! (`tools/fixtures/`) and need no live server.

#![forbid(unsafe_code)]

pub mod error;
pub mod http;
pub mod jellyfin;
pub mod model;
pub mod ranged;
pub mod song_key;
pub mod source;
pub mod transport;
pub mod ws;

pub use error::{Result, SourceError, Unreachable};
pub use model::*;
pub use source::{JellyfinSource, MediaSource};
pub use transport::{FixtureExchange, FixtureTransport, Transport};
