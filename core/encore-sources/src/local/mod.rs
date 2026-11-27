//! Music files on the device (Phase 12 stage 4, mock-up 49).
//!
//! The platform finds the files (on Android, in the folders the listener picked) and
//! hands each one over with a path the core can open for that call; nothing here walks
//! a file system, so the same code serves Android's content addresses, iOS bookmarks
//! and desktop folders. `tags` reads what a file says about itself (its tags, cover and
//! lyrics) with Symphonia (already in the workspace for audio analysis, MPL-2.0);
//! `index` keeps what was read; `source` is the library a sync hands to the store.

pub mod index;
pub mod source;
pub mod tags;

pub use index::{LocalFile, LocalFolder, LocalIndex};
pub use source::{Added, FoundFile, LocalSource, is_music_file};
pub use tags::{Cover, LocalTags, TagError, name_from_file, read_tags};
