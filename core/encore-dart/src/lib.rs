//! Dart bindings for the Encore core (flutter_rust_bridge).
//!
//! `api/` is the surface; `frb_generated.rs` is written by
//! `flutter_rust_bridge_codegen` (run `make bindings` in `core/`) and never edited by hand.

pub mod api;

// Generated code: rustfmt leaves it as frb wrote it, so `make check` can compare it
// byte for byte with a fresh run, and the workspace's own lints do not apply to it.
#[rustfmt::skip]
#[allow(unreachable_pub, missing_debug_implementations)]
mod frb_generated;
