# R8 cannot see calls made from native code or through JNA's reflection.

# JNA: its classes are looked up by name from libjnidispatch.so.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-dontwarn java.awt.**

# The UniFFI bindings: JNA maps their Structure fields, callbacks and library
# interfaces by reflection.
-keep class uk.gcartee.encore.core.** { *; }

# rustls-platform-verifier's Kotlin part is called from Rust through JNI
# (its README, "Proguard").
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }

# Appearance reads the accent schemes from the generated token constants by name.
-keep class uk.gcartee.encore.tokens.** { *; }

# Phase 9: Glance runs its widget sessions on WorkManager, which makes its Room database
# (WorkDatabase_Impl) and its workers by name. R8's full mode drops constructors that only
# reflection calls, and the release build crashed at start with "Failed to create an
# instance of androidx.work.impl.WorkDatabase" (CI run 43079181530).
-keep class * extends androidx.room.RoomDatabase { <init>(); }
-keep class * extends androidx.work.ListenableWorker {
    public <init>(android.content.Context, androidx.work.WorkerParameters);
}
-keep class * extends androidx.work.InputMerger { public <init>(); }
