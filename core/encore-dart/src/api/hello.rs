//! Phase 1 hello-world calls. See `docs/core/CORE-API.md`.

/// A greeting written by the core. Same as `encore_core::hello`.
#[flutter_rust_bridge::frb(sync)]
pub fn hello(name: String) -> String {
    encore_core::hello(name)
}

/// The core's version. Same as `encore_core::core_version`.
#[flutter_rust_bridge::frb(sync)]
pub fn core_version() -> String {
    encore_core::core_version()
}

/// The product name. Same as `encore_core::app_name`.
#[flutter_rust_bridge::frb(sync)]
pub fn app_name() -> String {
    encore_core::app_name()
}

#[cfg(test)]
mod tests {
    #[test]
    fn forwards_to_the_core() {
        assert_eq!(super::hello("Sam".into()), encore_core::hello("Sam".into()));
        assert_eq!(super::core_version(), encore_core::core_version());
        assert_eq!(super::app_name(), encore_core::app_name());
    }
}
