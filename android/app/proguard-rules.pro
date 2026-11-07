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
