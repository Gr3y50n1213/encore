# ADR core-0002: Where the generated bindings and token constants live

Date: 6 November 2026
Status: accepted

## Context
Phase 1 (docs/ROADMAP.md) asks the core track to generate UniFFI bindings for Swift and Kotlin, flutter_rust_bridge bindings for Dart, and token constants for all three. The platform tracks "use the generated code only" and never edit `core/`, while the core track never edits `apple/`, `android/` or `flutter/`. Plan section 3 lists eight crates, with no place for binding glue.

## Decision
- Generated code is committed under `core/bindings/`, which the core track owns:
  - `swift/`: `EncoreCore.swift`, `EncoreCoreFFI.h`, `EncoreCoreFFI.modulemap` (UniFFI)
  - `kotlin/uk/gcartee/encore/core/encore_core.kt` (UniFFI, loads `encore_core` through JNA)
  - `dart/`: the `encore_core` Dart package (flutter_rust_bridge output in `lib/src/rust/`, the token constants in `lib/encore_tokens.dart`)
  - `tokens/`: `EncoreTokens.swift`, `EncoreTokens.kt`
- Two crates are added beside the eight: `encore-dart` (the flutter_rust_bridge crate; it wraps `encore-core` and holds no logic) and `uniffi-bindgen` (runs the pinned UniFFI generator). UniFFI stays inside `encore-core`, as section 3 says, behind a default `uniffi` feature that `encore-dart` and the server turn off.
- `make check` regenerates every file and fails if anything differs from what is committed. CI also compiles and runs each binding on Linux (Swift, Kotlin on a JVM with JNA, Dart).
- The Dart bindings are generated in CI (`core.yml`, run manually with "regenerate"), because flutter_rust_bridge needs a Dart SDK and the core track's PC has none. Dart formatting and `dart fix` are off, so the output does not depend on the Dart SDK version.

## Alternatives considered
- Generating the bindings inside each platform's build: every track would need the generator versions, and a platform build could drift from the contract.
- Writing the Dart output into `flutter/`: that folder belongs to the Windows and Linux track.
- Putting the token constants in `design/`: possible (the core track may change `design/`), but keeping every generated file in one folder makes "what the core hands over" one path.

## Consequences
- Each platform track copies or references `core/bindings/<language>/` and compiles the Rust core for its own targets. Swift and Kotlin must load a library built from the same commit, because UniFFI checks API checksums at start-up.
- A change to the public API means `make bindings` plus a commit of `core/bindings/`, and a note in `docs/core/CORE-API.md`.
- To undo: generated code can move, but every platform track then changes its import paths, so it needs a request to each track.
