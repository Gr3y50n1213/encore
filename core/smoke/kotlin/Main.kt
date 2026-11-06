// Smoke test: Kotlin calls the Rust core through the UniFFI bindings (JNA on a desktop JVM).
// CI compiles this with bindings/kotlin and bindings/tokens/EncoreTokens.kt on Linux, with
// JNA and kotlinx-coroutines (the async calls are suspend functions).
// It proves the bindings work; the Android track proves them on Android.
//
// No network: signing in, syncing and playing are tested in Rust on a real recording.

import kotlinx.coroutines.runBlocking
import uk.gcartee.encore.core.CoreConfig
import uk.gcartee.encore.core.CoreEvent
import uk.gcartee.encore.core.CoreListener
import uk.gcartee.encore.core.EncoreCore
import uk.gcartee.encore.core.EncoreException
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.core.hello
import uk.gcartee.encore.tokens.EncoreTokens

class Listener : CoreListener {
    override fun onEvent(event: CoreEvent) {}
}

fun main() {
    val greeting = hello("CI")
    check(greeting == "Hello, CI, from the Encore core.") { "unexpected greeting: $greeting" }
    check(hello("") == "Hello from the Encore core.")
    check(appName() == "Encore")
    check(coreVersion().isNotEmpty())
    check(EncoreTokens.Breakpoints.railWidth == 80)
    check(EncoreTokens.Color.Accents.Iris.Light.primary == "#6146CB")
    check(EncoreTokens.Typography.Scale.Compact.PageTitle.size == 28.0)
    check(EncoreTokens.Recap.Type.tide == "Inter, heavy")

    val dir = kotlin.io.path.createTempDirectory("encore-smoke").toString()
    val core = EncoreCore(CoreConfig(dir, "smoke", "CI", "ci"))
    core.setListener(Listener())
    check(core.profiles().isEmpty())
    check(core.currentProfile() == null)

    try {
        core.servers()
        error("servers() should need a profile")
    } catch (e: EncoreException.NoProfile) {
        check(!e.message.isNullOrEmpty())
    }

    runBlocking {
        try {
            core.checkAddress("http://music.example.org")
            error("plain http to a public address must be refused")
        } catch (e: EncoreException.InsecureAddress) {
            check(e.message?.contains("https") == true) { "unexpected: ${e.message}" }
        }
    }

    core.close()
    println("Kotlin -> Rust: $greeting (core ${coreVersion()}); EncoreCore suspend calls and errors OK")
}
