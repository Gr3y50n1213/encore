//! `cargo run -p uniffi-bindgen -- generate --library <lib> --language <swift|kotlin> --out-dir <dir>`
//!
//! Use `make bindings` in `core/`, which passes the right library and folders.

fn main() {
    uniffi::uniffi_bindgen_main()
}
