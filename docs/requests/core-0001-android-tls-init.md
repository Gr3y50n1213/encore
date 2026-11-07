# Request: initialize the certificate verifier on Android

- **From:** core
- **To:** android
- **Status:** answered
- **Date:** 6 November 2026
- **Phase:** 2

## What I need

The core checks server certificates with the operating system's verifier (`rustls-platform-verifier` 0.7.1, MIT or Apache-2.0), so a private CA the user installed is trusted the same way the rest of Android trusts it. On Android that verifier calls into the JVM, and it needs two things that only the Android app can provide:

1. **Its Kotlin component in the app build.** The crate ships a small Android library (a local Maven repository inside the crate's source folder, `rustls-platform-verifier-android`). The Android track adds it to the Gradle build. The crate's README, "Android" section, describes the Gradle lines.
2. **One init call before the first connection**, with an Android `Context` (the Application context is fine), from the app's startup code.

For item 2 the core exports one JNI function, and the Kotlin side is ready to copy: `core/bindings/kotlin-android/uk/gcartee/encore/core/EncoreTls.kt`.

```kotlin
package uk.gcartee.encore.core

object EncoreTls {
    init { System.loadLibrary("encore_core") }
    @JvmStatic external fun initialize(context: android.content.Context)
}
```

Call `EncoreTls.initialize(applicationContext)` once, early in `Application.onCreate`. It calls `rustls_platform_verifier::android::init_with_env` inside the core; calling it again is harmless. Until it runs, an https connection fails with "Android certificate checks are not set up: call EncoreTls.initialize(context) when the app starts" (an `Unreachable` error). It never crashes the app, and plain http on the local network keeps working.

## Why

Plan section 12: "TLS for every connection outside the home network." Board 21, item 3: "A self-signed certificate asks once to trust its fingerprint." Using the platform verifier keeps Android's own trust store and user-installed CAs in charge. A bundled list of public roots would refuse a private CA.

## What I did meanwhile

The owner left the choice to the core track on 6 November 2026, on condition that it breaks nothing elsewhere. The core added:
- the `jni` crate 0.22.4 (MIT or Apache-2.0), built for Android only, and the same version the verifier already uses, so there is one copy;
- the JNI function;
- the guard described above.

Other platforms build exactly as before. CI's `android` job builds all four ABIs and checks that `Java_uk_gcartee_encore_core_EncoreTls_initialize` is exported. Nothing has run on an Android device yet.

What is left is the Android track's: add the verifier's Kotlin component to the Gradle build, copy `EncoreTls.kt`, and call it at start-up.

## Answer

Done by the Android track, 7 November 2026 (commit 0993cba, Phase 1):
- `org.rustls:rustls-platform-verifier` 0.2.0 is in the Gradle build, from its project's Maven folder declared with `exclusiveContent` for that module only (`android/settings.gradle.kts`). A build task fails if its version differs from `rustls-platform-verifier-android` in `core/Cargo.lock`. R8 keeps `org.rustls.platformverifier.**`.
- `EncoreTls.kt` is used where it lies (`core/bindings/kotlin-android` is a source folder of the app, not a copy), and `EncoreApp.onCreate` calls `EncoreTls.initialize(applicationContext)` before anything else.
- It ran on the API 28 and API 36 emulators at every app start in CI run 42414284761, with no crash. An https sign-in through it is still to be tried on a phone against an https server (the CI recording is plain http on loopback). Self-signed servers: see docs/android/REVIEW-PHASE2.md, item 4.
