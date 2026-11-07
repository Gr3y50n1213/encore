# ADR android-0001: The Android project, and how it builds and uses the core

Date: 7 November 2026
Status: accepted (Phase 1, Android track)

## Context
Phase 1 of the Android track creates `android/`, compiles the Rust core for Android inside it, shows a screen whose text comes from the core, and makes `make check-android` and its CI job real (docs/ROADMAP.md, docs/android/PLAN-ANDROID.md section 3). The rules: Kotlin and Jetpack Compose with Material 3 Expressive; Android 9 minimum, API 36 target; the core's generated bindings used as they are; design values from the generated token constants; 16 KB page alignment.

## Decision
- **One Gradle project, one app module** (`android/app`), Kotlin DSL, a version catalog (`android/gradle/libs.versions.toml`) with exact versions, and the Gradle wrapper (9.6.0, its distribution pinned by SHA-256; the wrapper jar matches Gradle's published checksum, and CI's `setup-gradle` validates it again).
- **Identifiers:** application ID `uk.gcartee.encore` (ADR core-0001's natural choice; it becomes permanent at the first Play upload), `.debug` added for debug builds so they never replace a release install. Kotlin package `uk.gcartee.encore.app`.
- **Versions** (checked on Google Maven, Maven Central and developer.android.com on 7 November 2026): Android Gradle plugin 9.5.1 (needs Gradle 9.6 and JDK 17, builds up to API 37, has Kotlin built in, so no `kotlin-android` plugin), Kotlin's Compose compiler plugin 2.4.21, Compose BOM 2026.09.00, **Material 3 1.5.0-rc01** (see below), activity-compose 1.13.0, core-ktx 1.19.1, JNA 5.17.0 (`@aar`) and kotlinx-coroutines 1.11.0 (the versions core CI tests the bindings with), NDK 28.2.13676358 (AGP 9.5's default; r28 aligns to 16 KB).
- **Material 3 1.5.0-rc01, not the BOM's 1.4.0.** The 1.5.0 release notes promote the expressive APIs this plan needs: the motion scheme (alpha15), the expressive theme and wavy progress (alpha18), toggle buttons (alpha19) and button groups (alpha22). `MaterialShapes` is still experimental there. Move to 1.5.0 when it is final.
- **The core is built by Gradle**, inside `android/app/build/` (its own Cargo target folder, so nothing is written in `core/`): task `buildRustCore` runs `cargo rustc --locked --release -p encore-core --target <triple> --crate-type cdylib` for each ABI in `encore.abis` with the NDK's clang at API 28 and the 16 KB linker flags, exactly as core CI does, and the result is added to the variant's `jniLibs`. `-Pencore.jniLibs=<folder>` uses prebuilt libraries instead (CI's `encore-core-android` artifact), for a quick local build; they must be from the same commit as the bindings.
- **The generated code is used where it lies**: `core/bindings/kotlin`, `core/bindings/kotlin-android` (`EncoreTls`) and `core/bindings/tokens` are added as Kotlin source folders of the variant. Nothing is copied, so a core change reaches the app on the next build.
- **The product name** comes from the core's one constant: the build reads `APP_NAME` from `core/encore-core/src/brand.rs` into the `app_name` string resource (the launcher label), and the screen calls `appName()`.
- **Certificate checks** (request core-0001): `EncoreTls.initialize(applicationContext)` in `Application.onCreate`, and the verifier's Kotlin part `org.rustls:rustls-platform-verifier`. Its project publishes it only as a Maven folder in its own GitHub repository, so that repository is declared with `exclusiveContent` for that one module and nothing else. A build task fails when its version differs from `rustls-platform-verifier-android` in `core/Cargo.lock`.
- **Release builds** are shrunk with R8, with keep rules for JNA, the UniFFI bindings and the verifier. No release key exists yet; `-Pencore.signReleaseWithDebugKey=true` signs one with the debug key for trying on an emulator or phone only.
- **Backups:** `allowBackup="false"` and data extraction rules that exclude everything: profile databases hold listening history (plan section 12), tokens go in the Keystore.
- **CI** (`.github/workflows/android.yml`): `make check-android` on Ubuntu (lint, unit tests, debug and release builds for all four ABIs, `zipalign -c -P 16`), then emulators at API 28 and 36 run the instrumented tests and start the R8 release build, reading the core's sentence from the screen.

## Alternatives considered
- **cargo-ndk or the Mozilla rust-android-gradle plugin:** each is one more outside tool for what a short Gradle task does with the NDK already installed; the task mirrors core CI's commands line for line.
- **Copying the bindings into `android/`:** two copies drift, and UniFFI refuses a library whose checksums differ from the bindings.
- **Material 3 1.4.0 (the BOM's stable):** predates the expressive APIs becoming stable; building on them would mean opting in to experimental APIs everywhere and moving later anyway.
- **Vendoring the verifier's `.aar`:** a binary in the repository with no upstream check; the exclusive Maven source plus the version check is cleaner.

## Consequences
- A full local build compiles the Rust core per ABI; on the owner's two-core PC use `make check ABIS=arm64-v8a`, or prebuilt libraries. CI builds all four.
- When core moves to a new rustls-platform-verifier, the Android build fails until `rustlsPlatformVerifier` is updated: on purpose.
- Gradle's dependency verification (checksums for every artifact) is not on yet: it needs metadata generated on Linux as well as Windows. To do before the first release.
