# Request: initialize the certificate verifier on Android

- **From:** core
- **To:** android (and the owner, for one dependency)
- **Status:** open
- **Date:** 6 November 2026
- **Phase:** 2

## What I need

The core checks server certificates with the operating system's verifier (`rustls-platform-verifier` 0.7.1, MIT or Apache-2.0), so a private CA the user installed is trusted the same way the rest of Android trusts it. On Android that verifier calls into the JVM, and it needs two things that only the Android app can provide:

1. **Its Kotlin component in the app build.** The crate ships a small Android library (a local Maven repository inside the crate's source folder, `rustls-platform-verifier-android`). The Android track adds it to the Gradle build. The crate's README, "Android" section, describes the Gradle lines.
2. **One init call before the first connection**, with an Android `Context` (the Application context is fine), from the app's startup code.

For item 2 the core will export one JNI function:

```kotlin
package uk.gcartee.encore.core

object EncoreTls {
    init { System.loadLibrary("encore_core") }
    @JvmStatic external fun init(context: android.content.Context)
}
```

This calls `rustls_platform_verifier::android::init_with_env` inside the core. Without it, every https call fails on Android with `Unreachable` ("a secure connection couldn't be set up"); plain http on the local network still works.

## Why

Plan section 12: "TLS for every connection outside the home network." Board 21, item 3: "A self-signed certificate asks once to trust its fingerprint." Using the platform verifier keeps Android's own trust store and user-installed CAs in charge. A bundled list of public roots would refuse a private CA.

## What I did meanwhile

The core builds for all four Android ABIs, and CI checks them (16 KB aligned, symbols exported). The JNI function is **not added yet**. It needs the `jni` crate (MIT or Apache-2.0) in `encore-core` for Android builds only, and that crate was not on the dependency list the owner approved for Phase 2. The core track asks the owner before adding it. Until then, the Android track can build everything else and sign in over plain http on the local network.

## Answer

<Filled in by the track it is addressed to: what was done, in which commit, or why not.>
