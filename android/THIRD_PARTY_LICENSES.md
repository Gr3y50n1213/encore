# Third-party licences: the Android app

What the Android app adds on top of the core, and under which licence. The Rust core's own list is `core/THIRD_PARTY_LICENSES.md`; everything there is linked into the app as `libencore_core.so`. Keep this file current with every dependency change (docs/adr/android-0001-project-and-core-build.md).

Rule: nothing linked into the app may be copyleft stronger than MPL-2.0.

## What was read

- **Licence texts read (7 November 2026):** Apache-2.0 (the standard text, which every library below uses or offers); JNA's `LICENSE` at tag 5.17.0 (dual: LGPL-2.1-or-later or Apache-2.0; **Apache-2.0 is used**); rustls-platform-verifier's README and licence files (MIT or Apache-2.0). Its Maven POM declares no licence, so the crate's licence is recorded.
- **Everything else:** the licence each library's POM declares (following its parent POM), read from the Gradle cache by `android/ci/pom_licences.py` over `./gradlew :app:dependencies --configuration releaseRuntimeClasspath`.
- **Assets:** Inter 4.1, `res/font/inter_variable.ttf` (SIL Open Font License 1.1, text read; the licence ships in `assets/licenses/Inter-OFL.txt`); Material Symbols Rounded icons, `res/drawable/sym_*.xml` and the placeholder launcher note (Apache-2.0), made by `android/ci/symbols.py` from google/material-design-icons.
- **Not shipped:** build tools (Android Gradle plugin, Kotlin compiler, Gradle) and test libraries (JUnit 4, EPL-1.0; AndroidX Test, Apache-2.0) never go into the APK.

## In the release APK

| Library | Version | Licence |
| --- | --- | --- |
| androidx.activity:activity | 1.13.0 | Apache-2.0 |
| androidx.activity:activity-compose | 1.13.0 | Apache-2.0 |
| androidx.activity:activity-ktx | 1.13.0 | Apache-2.0 |
| androidx.annotation:annotation | 1.10.0 | Apache-2.0 |
| androidx.annotation:annotation-experimental | 1.4.1 | Apache-2.0 |
| androidx.annotation:annotation-jvm | 1.10.0 | Apache-2.0 |
| androidx.arch.core:core-common | 2.2.0 | Apache-2.0 |
| androidx.arch.core:core-runtime | 2.2.0 | Apache-2.0 |
| androidx.autofill:autofill | 1.0.0 | Apache-2.0 |
| androidx.collection:collection | 1.5.0 | Apache-2.0 |
| androidx.collection:collection-jvm | 1.5.0 | Apache-2.0 |
| androidx.collection:collection-ktx | 1.5.0 | Apache-2.0 |
| androidx.compose.animation:animation | 1.12.1 | Apache-2.0 |
| androidx.compose.animation:animation-android | 1.12.1 | Apache-2.0 |
| androidx.compose.animation:animation-core | 1.12.1 | Apache-2.0 |
| androidx.compose.animation:animation-core-android | 1.12.1 | Apache-2.0 |
| androidx.compose.foundation:foundation | 1.12.1 | Apache-2.0 |
| androidx.compose.foundation:foundation-android | 1.12.1 | Apache-2.0 |
| androidx.compose.foundation:foundation-layout | 1.12.1 | Apache-2.0 |
| androidx.compose.foundation:foundation-layout-android | 1.12.1 | Apache-2.0 |
| androidx.compose.material:material-ripple | 1.12.1 | Apache-2.0 |
| androidx.compose.material:material-ripple-android | 1.12.1 | Apache-2.0 |
| androidx.compose.material3:material3 | 1.5.0-rc01 | Apache-2.0 |
| androidx.compose.material3:material3-android | 1.5.0-rc01 | Apache-2.0 |
| androidx.compose.material3:material3-ripple | 1.5.0-rc01 | Apache-2.0 |
| androidx.compose.material3:material3-ripple-android | 1.5.0-rc01 | Apache-2.0 |
| androidx.compose.runtime:runtime | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-android | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-annotation | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-annotation-android | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-retain | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-retain-android | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-saveable | 1.12.1 | Apache-2.0 |
| androidx.compose.runtime:runtime-saveable-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-geometry | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-geometry-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-graphics | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-graphics-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-text | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-text-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-tooling-preview | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-tooling-preview-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-unit | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-unit-android | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-util | 1.12.1 | Apache-2.0 |
| androidx.compose.ui:ui-util-android | 1.12.1 | Apache-2.0 |
| androidx.concurrent:concurrent-futures | 1.2.0 | Apache-2.0 |
| androidx.core:core | 1.19.1 | Apache-2.0 |
| androidx.core:core-ktx | 1.19.1 | Apache-2.0 |
| androidx.core:core-viewtree | 1.0.0 | Apache-2.0 |
| androidx.customview:customview-poolingcontainer | 1.0.0 | Apache-2.0 |
| androidx.emoji2:emoji2 | 1.4.0 | Apache-2.0 |
| androidx.exifinterface:exifinterface | 1.3.6 | Apache-2.0 |
| androidx.graphics:graphics-path | 1.0.1 | Apache-2.0 |
| androidx.graphics:graphics-shapes | 1.0.1 | Apache-2.0 |
| androidx.graphics:graphics-shapes-android | 1.0.1 | Apache-2.0 |
| androidx.interpolator:interpolator | 1.0.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-common | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-common-java8 | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-common-jvm | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-livedata-core | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-process | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-runtime | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-runtime-android | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-runtime-compose | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-runtime-compose-android | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-runtime-ktx | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-runtime-ktx-android | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-service | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-viewmodel | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-viewmodel-android | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-viewmodel-ktx | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-viewmodel-savedstate | 2.10.0 | Apache-2.0 |
| androidx.lifecycle:lifecycle-viewmodel-savedstate-android | 2.10.0 | Apache-2.0 |
| androidx.media:media | 1.7.0 | Apache-2.0 |
| androidx.media3:media3-common | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-container | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-database | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-datasource | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-decoder | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-exoplayer | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-extractor | 1.11.1 | Apache-2.0 |
| androidx.media3:media3-session | 1.11.1 | Apache-2.0 |
| androidx.navigationevent:navigationevent | 1.0.0 | Apache-2.0 |
| androidx.navigationevent:navigationevent-android | 1.0.0 | Apache-2.0 |
| androidx.navigationevent:navigationevent-compose | 1.0.0 | Apache-2.0 |
| androidx.navigationevent:navigationevent-compose-android | 1.0.0 | Apache-2.0 |
| androidx.profileinstaller:profileinstaller | 1.4.0 | Apache-2.0 |
| androidx.savedstate:savedstate | 1.4.0 | Apache-2.0 |
| androidx.savedstate:savedstate-android | 1.4.0 | Apache-2.0 |
| androidx.savedstate:savedstate-compose | 1.4.0 | Apache-2.0 |
| androidx.savedstate:savedstate-compose-android | 1.4.0 | Apache-2.0 |
| androidx.savedstate:savedstate-ktx | 1.4.0 | Apache-2.0 |
| androidx.startup:startup-runtime | 1.1.1 | Apache-2.0 |
| androidx.tracing:tracing | 1.2.0 | Apache-2.0 |
| androidx.versionedparcelable:versionedparcelable | 1.1.1 | Apache-2.0 |
| androidx.window:window | 1.5.0 | Apache-2.0 |
| androidx.window:window-core | 1.5.0 | Apache-2.0 |
| androidx.window:window-core-android | 1.5.0 | Apache-2.0 |
| com.google.guava:failureaccess | 1.0.2 | Apache-2.0 |
| com.google.guava:guava | 33.3.1-android | Apache-2.0 |
| com.google.guava:listenablefuture | 9999.0-empty-to-avoid-conflict-with-guava | Apache-2.0 |
| net.java.dev.jna:jna | 5.17.0 | LGPL-2.1-or-later OR Apache-2.0 (Apache-2.0 used) |
| org.jetbrains:annotations | 23.0.0 | Apache-2.0 |
| org.jetbrains.kotlin:kotlin-stdlib | 2.4.21 | Apache-2.0 |
| org.jetbrains.kotlinx:kotlinx-coroutines-android | 1.11.0 | Apache-2.0 |
| org.jetbrains.kotlinx:kotlinx-coroutines-core | 1.11.0 | Apache-2.0 |
| org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm | 1.11.0 | Apache-2.0 |
| org.jetbrains.kotlinx:kotlinx-serialization-core | 1.7.3 | Apache-2.0 |
| org.jetbrains.kotlinx:kotlinx-serialization-core-jvm | 1.7.3 | Apache-2.0 |
| org.jspecify:jspecify | 1.0.0 | Apache-2.0 |
| org.rustls:rustls-platform-verifier | 0.2.0 | MIT OR Apache-2.0 (README; the POM declares none) |
