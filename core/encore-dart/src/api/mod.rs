//! Everything under `api` is what flutter_rust_bridge exposes to Dart.
//! Each function forwards to `encore-core`, so Dart, Swift and Kotlin get the
//! same behaviour from the same code.

pub mod core;
pub mod hello;
