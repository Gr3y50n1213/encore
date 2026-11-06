//! The public API (docs/core/CORE-API.md, version 1). Swift and Kotlin reach it
//! through UniFFI; Dart through encore-dart, which wraps the same methods.

mod core;
mod types;

pub use self::core::EncoreCore;
pub use self::types::*;
