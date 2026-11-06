# Third-party licences: core and server

What the core track's code depends on, and under which licence. Kept current with every dependency change (CLAUDE.md, quality bar). The platform tracks keep their own lists for what their apps add.

Rule: nothing linked into the apps may be copyleft stronger than MPL-2.0. The server may use AGPL-compatible licences. The licences of this project itself (MPL-2.0 apps, AGPL-3.0 server) are still a suggestion the owner has not confirmed.

## What was read and what was not

- **Licence texts read (6 November 2026):** UniFFI (MPL-2.0, `mozilla/uniffi-rs` LICENSE), flutter_rust_bridge (MIT, `fzyzcjy/flutter_rust_bridge` LICENSE), allo-isolate (Apache-2.0, its LICENSE file, because its Cargo.toml declares no licence), axum (MIT), tokio (MIT), serde (MIT or Apache-2.0).
- **Everything else:** the licence each crate declares in its Cargo.toml, as listed by `cargo tree`. Not yet checked against each licence text. Section 24 asks for a dependency and licence scan in CI that fails on a new, unreviewed licence; that is not built yet.
- **Choices made where a crate offers several licences:** MIT or Apache-2.0 everywhere. `r-efi` offers LGPL-2.1 as one of three options; MIT or Apache-2.0 is used.
- **MPL-2.0 in the apps:** the UniFFI runtime crates. MPL-2.0 is file-level copyleft and is the ceiling the plan allows.

## Compiled into the apps (encore-core and encore-dart, every target)
| Crate | Version | Licence (as declared by the crate) |
| --- | --- | --- |
| addr2line | 0.25.1 | Apache-2.0 OR MIT |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| allo-isolate | 0.1.27 | Apache-2.0 (no licence field in Cargo.toml; LICENSE file read) |
| android_logger | 0.15.1 | MIT OR Apache-2.0 |
| android_log-sys | 0.3.2 | MIT OR Apache-2.0 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| atomic | 0.5.3 | Apache-2.0/MIT |
| backtrace | 0.3.76 | MIT OR Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| camino | 1.2.6 | MIT OR Apache-2.0 |
| cargo_metadata | 0.23.1 | MIT |
| cargo-platform | 0.3.0 | MIT OR Apache-2.0 |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| console_error_panic_hook | 0.1.7 | Apache-2.0/MIT |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| dart-sys | 4.1.5 | MIT OR Apache-2.0 |
| dashmap | 5.5.3 | MIT |
| delegate-attr | 0.3.1 | MIT |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| env_filter | 0.1.4 | MIT OR Apache-2.0 |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| flutter_rust_bridge | 2.13.0 | MIT |
| flutter_rust_bridge_macros | 2.13.0 | MIT |
| fs-err | 3.3.2 | MIT OR Apache-2.0 |
| futures | 0.3.34 | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 |
| futures-io | 0.3.34 | MIT OR Apache-2.0 |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| gimli | 0.32.3 | MIT OR Apache-2.0 |
| hashbrown | 0.14.5 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| js-sys | 0.3.107 | MIT OR Apache-2.0 |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 |
| libc | 0.2.191 | MIT OR Apache-2.0 |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.35 | MIT OR Apache-2.0 |
| md-5 | 0.10.6 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| num_cpus | 1.17.0 | MIT OR Apache-2.0 |
| object | 0.37.3 | Apache-2.0 OR MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| oslog | 0.2.0 | MIT |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| redox_syscall | 0.5.18 | MIT |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| rustc-demangle | 0.1.28 | MIT/Apache-2.0 |
| rustix | 1.1.6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| semver | 1.0.28 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 |
| siphasher | 1.0.4 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.4 | MIT OR Apache-2.0 |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| threadpool | 1.8.1 | MIT/Apache-2.0 |
| tokio | 1.54.1 | MIT |
| toml | 1.2.0+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.2.0+spec-1.1.0 | MIT OR Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| uniffi | 0.32.2 | MPL-2.0 |
| uniffi_core | 0.32.2 | MPL-2.0 |
| uniffi_internal_macros | 0.32.2 | MPL-2.0 |
| uniffi_macros | 0.32.2 | MPL-2.0 |
| uniffi_meta | 0.32.2 | MPL-2.0 |
| uniffi_pipeline | 0.32.2 | MPL-2.0 |
| wasm-bindgen | 0.2.130 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.80 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.130 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.130 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.130 | MIT OR Apache-2.0 |
| web-sys | 0.3.107 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| winnow | 1.0.5 | MIT |
| zmij | 1.0.23 | MIT |

## Compiled into the companion server (encore-server)
| Crate | Version | Licence (as declared by the crate) |
| --- | --- | --- |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| axum | 0.8.9 | MIT |
| axum-core | 0.5.6 | MIT |
| bytes | 1.12.1 | MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| httpdate | 1.0.3 | MIT OR Apache-2.0 |
| hyper | 1.12.0 | MIT |
| hyper-util | 0.1.21 | MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| libc | 0.2.191 | MIT OR Apache-2.0 |
| matchit | 0.8.4 | MIT AND BSD-3-Clause |
| memchr | 2.8.3 | Unlicense OR MIT |
| mime | 0.3.18 | MIT OR Apache-2.0 |
| mio | 1.2.4 | MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.4 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| tokio | 1.54.1 | MIT |
| tokio-macros | 2.7.2 | MIT |
| tower | 0.5.3 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| zmij | 1.0.23 | MIT |

## Build and test tools (not shipped in any app or server image)

| Tool | Version | Licence | Where it runs |
| --- | --- | --- | --- |
| uniffi-bindgen (UniFFI) | 0.32.2 | MPL-2.0 | `make bindings`, `make check`, CI |
| flutter_rust_bridge_codegen | 2.13.0 | MIT | CI (`make check`, regenerate) |
| encore-tokens (this repository) | 0.1.0 | this project's licence | `make tokens`, `make check`, CI |
| serde_json (encore-tokens) | 1.0.154 | MIT or Apache-2.0 | `make tokens` |
| tower, http-body-util (server tests) | 0.5.3, 0.1 | MIT | `cargo test` in `server/` |
| JNA | 5.17.0 | Apache-2.0 or LGPL-2.1 (Apache-2.0 used) | CI Kotlin smoke test only. The Android app's own JNA dependency is the Android track's choice |
| Dart SDK, Swift and Kotlin compilers | runner's stable | BSD-3-Clause, Apache-2.0, Apache-2.0 | CI smoke tests only |
| GNU make (ezwinports build, owner's PC) | 4.4.1 | GPL-3.0-or-later | Runs the Makefiles; never linked or shipped |

Regenerate the two crate tables after a dependency change with
`cargo tree --locked -e normal --target all --prefix none -f "{p}|{l}"` in `core/` (for `-p encore-core -p encore-dart`) and in `server/`.
