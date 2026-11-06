# Encore core API (the contract)

**Version 0** (Phase 1, 6 November 2026). Core crate version `0.1.0`.

This file is the only thing the platform tracks may rely on. Every public call, event and type of the core is listed here before a platform uses it. A change that breaks a platform gets a line under "Changes" and a request in `docs/requests/` that says what to change.

## Changes

| API version | Phase | What changed |
| --- | --- | --- |
| 0 | 1 | First version: `hello`, `core_version`, `app_name`; token constants |

## How every platform gets the core

| | Apple (Swift) | Android (Kotlin) | Windows and Linux (Dart) |
| --- | --- | --- | --- |
| Generated code | `core/bindings/swift/`: `EncoreCore.swift`, `EncoreCoreFFI.h`, `EncoreCoreFFI.modulemap` | `core/bindings/kotlin/uk/gcartee/encore/core/encore_core.kt` | The Dart package at `core/bindings/dart/` (`encore_core`) |
| Generator | UniFFI 0.32.2 | UniFFI 0.32.2 | flutter_rust_bridge 2.13.0 |
| Rust crate to build | `encore-core` (`core/encore-core`) | `encore-core` | `encore-dart` (`core/encore-dart`), which wraps `encore-core` |
| Library to build | static library `libencore_core.a` for each Apple target, packaged as an XCFramework by the Apple track | `libencore_core.so` for each Android ABI, in `jniLibs/<abi>/` | `encore_dart.dll` (Windows), `libencore_dart.so` (Linux) |
| Runtime dependency | none | JNA (`net.java.dev.jna:jna`, the `@aar` build on Android). The Android track chooses and licence-checks its version; CI tests with 5.17.0 | `flutter_rust_bridge` 2.13.0 (Dart package, exact version) |
| Package or module | Swift module `EncoreCore` (the FFI module is `EncoreCoreFFI`) | `uk.gcartee.encore.core` | `package:encore_core/encore_core.dart` |
| Token constants | `core/bindings/tokens/EncoreTokens.swift` | `core/bindings/tokens/EncoreTokens.kt` (package `uk.gcartee.encore.tokens`) | `package:encore_core/encore_tokens.dart` |

Rules for every platform:

- Build the library from the **same commit** as the generated code you use. UniFFI checks API checksums when the library loads and fails if they differ.
- Never edit generated files. When something is missing, write a request (`docs/requests/`).
- Each platform builds the core for its own targets inside its own folder, with `cargo build --release` from `core/` (`--target <triple>`). The Rust source stays in `core/`.
- Regenerate everything with `make bindings tokens` in `core/`. The Dart bindings need flutter_rust_bridge_codegen 2.13.0 and a Dart SDK; without them, run the `core` workflow by hand with "regenerate" ticked and take the files from its artifact.

## Calls

All three Phase 1 calls are synchronous, pure, allocation-only and safe from any thread, including the main thread. None of them can fail.

### `hello(name)`

A greeting written by the core, so the first screen on every platform shows text that comes from Rust.

| | |
| --- | --- |
| Input | `name`: string. Surrounding whitespace is trimmed. |
| Output | string. `"Hello, <name>, from the Encore core."`, or `"Hello from the Encore core."` when `name` is empty or only whitespace. The product name in it comes from `app_name()`. |
| Errors | none |
| Thread | any; synchronous |
| Added | API 0, Phase 1 |

### `core_version()`

| | |
| --- | --- |
| Input | none |
| Output | string: the core crate's version, e.g. `"0.1.0"` |
| Errors | none |
| Thread | any; synchronous |
| Added | API 0, Phase 1 |

### `app_name()`

The product name, kept in one place (`core/encore-core/src/brand.rs`), so no UI types it. Today `"Encore"`, a placeholder (ADR core-0001).

| | |
| --- | --- |
| Input | none |
| Output | string |
| Errors | none |
| Thread | any; synchronous |
| Added | API 0, Phase 1 |

## Calling them

**Swift** (add the three files from `core/bindings/swift/` to the target, link the XCFramework):

```swift
import EncoreCore   // or nothing, if EncoreCore.swift is compiled into the app module

let text = hello(name: "Sam")        // "Hello, Sam, from the Encore core."
let version = coreVersion()          // "0.1.0"
let name = appName()                 // "Encore"
```

**Kotlin** (add `encore_core.kt`, put `libencore_core.so` in `jniLibs/<abi>/`, depend on JNA):

```kotlin
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.core.hello

val text = hello("Sam")              // "Hello, Sam, from the Encore core."
val version = coreVersion()
val name = appName()
```

**Dart** (depend on the package by path, ship `encore_dart.dll` or `libencore_dart.so` beside the app):

```dart
import 'package:encore_core/encore_core.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

Future<void> main() async {
  await RustLib.init(externalLibrary: ExternalLibrary.open('<path to encore_dart library>'));
  final text = hello(name: 'Sam');   // "Hello, Sam, from the Encore core."
  final version = coreVersion();
  final name = appName();
}
```

`RustLib.init()` without arguments uses flutter_rust_bridge's default loader, which looks for the library under its default name; the Windows and Linux track decides how to bundle and load it.

## Token constants

Generated from `design/tokens.json` by `tools/encore-tokens` (`make tokens` in `core/`). Never type a colour, size or duration; read it from here. Keys keep their tokens.json names. Groups become nested types, and `$source` and `$status` notes become doc comments.

| Token | Swift | Kotlin | Dart |
| --- | --- | --- | --- |
| `breakpoints.railWidth` | `EncoreTokens.Breakpoints.railWidth` | `EncoreTokens.Breakpoints.railWidth` | `EncoreTokens.breakpoints.railWidth` |
| `motion.timing.reduceMotionCrossfadeMs` | `EncoreTokens.Motion.Timing.reduceMotionCrossfadeMs` | `EncoreTokens.Motion.Timing.reduceMotionCrossfadeMs` | `EncoreTokens.motion.timing.reduceMotionCrossfadeMs` |
| `smartPicks.explorationPercent.default` | ``EncoreTokens.SmartPicks.ExplorationPercent.`default` `` | `EncoreTokens.SmartPicks.ExplorationPercent.default` | `EncoreTokens.smartPicks.explorationPercent.defaultValue` |

Types: whole numbers are `Int` (Swift and Kotlin; `Long` in Kotlin above 32 bits) or `int`; decimals are `Double` or `double`; text is `String`; lists are arrays or `List`. A list of objects (for example `albumGradient.exampleAlbums`) becomes a list of small records named `<Key>Item`. A Dart name that is a reserved word gets `Value` at the end.

Not in the constants yet: the colour palette and the type scale (`$status: define-in-phase-1`). They are added after the owner approves the proposal.

## Proven so far

- Rust unit tests for every call (`cargo test` in `core/`).
- CI (`.github/workflows/core.yml`) compiles each binding and calls all three functions from Swift, from Kotlin on a JVM with JNA, and from Dart, on Linux. These prove the bindings, not the platforms: the Apple, Android and Windows and Linux tracks prove them on their own devices in their Phase 1.
- CI's `apple` job, on a macOS runner, builds `libencore_core.a` for iOS (arm64), the iOS Simulator (arm64 and x86_64) and macOS (arm64 and x86_64) with the plan's minimums (iOS 17, macOS 14). It links the Swift bindings for all five in Swift 6 language mode and calls the core on macOS and in an iOS Simulator. The iOS device build is linked but not run, because CI has no iPhone. It also packages an XCFramework of the three platforms and uploads it as `encore-core-apple` for 14 days, so the Apple track can try it before setting up its own Xcode build.
