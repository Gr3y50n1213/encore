//! The local-first event log, merge rules (last writer wins per field, hybrid logical
//! clock) and the sync client.
//!
//! Plan section 5. The clock is here from Phase 2, because every event written from
//! the first playable build carries one; push, pull and merging arrive in Phase 4.

#![forbid(unsafe_code)]

pub mod hlc;

pub use hlc::{Hlc, HlcClock};
