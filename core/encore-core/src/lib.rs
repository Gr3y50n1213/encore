//! The public API of the Encore core.
//!
//! Every platform calls the core through generated bindings only: UniFFI for Swift
//! and Kotlin (this crate), flutter_rust_bridge for Dart (`encore-dart`, which wraps
//! these same functions). `docs/core/CORE-API.md` is the contract; anything public
//! here is listed there first.
//!
//! Phase 1 offers only the hello-world calls that prove the bindings work.

pub mod brand;
pub mod library;
pub mod playlog;
pub mod queue;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

/// A greeting written by the core, so every platform's first screen shows text
/// that comes from Rust.
///
/// A blank `name` (empty or only whitespace) gives the greeting without a name.
/// Surrounding whitespace is trimmed.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn hello(name: String) -> String {
    let name = name.trim();
    if name.is_empty() {
        format!("Hello from the {} core.", brand::APP_NAME)
    } else {
        format!("Hello, {name}, from the {} core.", brand::APP_NAME)
    }
}

/// The core's version, for About screens and bug reports.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// The product name from [`brand::APP_NAME`], so no UI has to type it.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn app_name() -> String {
    brand::APP_NAME.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_with_a_name() {
        assert_eq!(hello("Sam".into()), "Hello, Sam, from the Encore core.");
    }

    #[test]
    fn hello_trims_the_name() {
        assert_eq!(
            hello("  Sam \n".into()),
            "Hello, Sam, from the Encore core."
        );
    }

    #[test]
    fn hello_without_a_name() {
        assert_eq!(hello(String::new()), "Hello from the Encore core.");
        assert_eq!(hello(" \t ".into()), "Hello from the Encore core.");
    }

    #[test]
    fn hello_keeps_non_latin_names() {
        assert_eq!(hello("सैम".into()), "Hello, सैम, from the Encore core.");
        assert_eq!(hello("سام".into()), "Hello, سام, from the Encore core.");
    }

    #[test]
    fn version_matches_the_crate() {
        assert_eq!(core_version(), env!("CARGO_PKG_VERSION"));
        assert!(!core_version().is_empty());
    }

    #[test]
    fn app_name_comes_from_the_brand_constant() {
        assert_eq!(app_name(), brand::APP_NAME);
    }
}
