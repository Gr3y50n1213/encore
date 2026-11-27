//! Music files on the device (Phase 12 stage 4, mock-up 49).
//!
//! The platform finds the files (on Android, in the folders the listener picked) and
//! hands each one to the core as a stream; nothing here walks a file system or holds a
//! path, so the same code serves Android's content addresses, iOS bookmarks and desktop
//! folders. This module reads what a file says about itself: its tags, its cover and its
//! lyrics, with Symphonia (already in the workspace for audio analysis, MPL-2.0).

pub mod tags;

pub use tags::{Cover, LocalTags, TagError, name_from_file, read_tags};
