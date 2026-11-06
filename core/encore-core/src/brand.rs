//! The product name, kept in one place.
//!
//! "Encore" is a placeholder codename (docs/DECISIONS.md, section 4). Renaming the
//! product is a change to this file only. Crate, module and package names that
//! contain "encore" are internal identifiers, not the product name; see
//! `docs/adr/core-0001-names-and-identifiers.md`.

/// The name people see. Never put another product's or server's brand here.
pub const APP_NAME: &str = "Encore";
