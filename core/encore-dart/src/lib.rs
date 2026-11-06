//! Dart bindings for the Encore core (flutter_rust_bridge).
//!
//! `api/` is the surface; `frb_generated.rs` is written by
//! `flutter_rust_bridge_codegen` (run `make bindings` in `core/`) and never edited by hand.

pub mod api;
mod frb_generated;
