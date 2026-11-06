// Smoke test: Kotlin calls the Rust core through the UniFFI bindings (JNA on a desktop JVM).
// CI compiles this with bindings/kotlin and bindings/tokens/EncoreTokens.kt on Linux.
// It proves the bindings work; the Android track proves them on Android.

import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.core.hello
import uk.gcartee.encore.tokens.EncoreTokens

fun main() {
    val greeting = hello("CI")
    check(greeting == "Hello, CI, from the Encore core.") { "unexpected greeting: $greeting" }
    check(hello("") == "Hello from the Encore core.")
    check(appName() == "Encore")
    check(coreVersion().isNotEmpty())
    check(EncoreTokens.Breakpoints.railWidth == 80)
    println("Kotlin -> Rust: $greeting (core ${coreVersion()})")
}
