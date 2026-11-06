// Smoke test: Kotlin calls the Rust core through the UniFFI bindings (JNA on a desktop JVM).
// CI compiles this with bindings/kotlin and bindings/tokens/EncoreTokens.kt on Linux, with
// JNA and kotlinx-coroutines (the async calls are suspend functions).
// It proves the bindings work; the Android track proves them on Android.
//
// The only network is 127.0.0.1: a replay of the real Jellyfin recording (see the end).

import kotlinx.coroutines.runBlocking
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.CoreConfig
import uk.gcartee.encore.core.CoreEvent
import uk.gcartee.encore.core.CoreListener
import uk.gcartee.encore.core.EncoreCore
import uk.gcartee.encore.core.EncoreException
import uk.gcartee.encore.core.EndReason
import uk.gcartee.encore.core.StartedFrom
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
        check(e.reason.isNotEmpty())
    }

    runBlocking {
        try {
            core.checkAddress("http://music.example.org")
            error("plain http to a public address must be refused")
        } catch (e: EncoreException.InsecureAddress) {
            check(e.reason.contains("https")) { "unexpected: ${e.reason}" }
        }
    }

    // The library flow over real HTTP: sign in, sync, browse, play and search against
    // the real Jellyfin recording, which tools/ci/replay_jellyfin.py serves on this machine.
    var flow = "library flow skipped (set ENCORE_REPLAY_URL, see tools/ci/replay.sh)"
    val replay = System.getenv("ENCORE_REPLAY_URL")
    if (replay != null) {
        runBlocking {
            val server = core.checkAddress(replay)
            check(server.version == "12.0.0") { server.version }
            check(server.plainHttpOnLocalNetwork)
            check(core.quickConnectStart(server.baseUrl).code.length == 6)
            val signedIn = core.quickConnectWait()
            check(signedIn.profileId.startsWith("jf-"))
            check(core.currentProfile() == signedIn.profileId)
            val synced = core.syncLibrary()
            check(synced.size == 1 && synced[0].songs == 150u) { synced.toString() }
            val album = core.albums(AlbumSort.TITLE, 0u, 100u).first { it.songCount >= 2u }
            val songs = core.album(album.id).songs
            val state = core.play(songs.map { it.id }, 0u, StartedFrom.ALBUM, "smoke")
            val current = checkNotNull(state.current)
            check(current.song.id == songs[0].id && state.upNext.size == songs.size - 1)
            core.playbackStarted(current.uid, 0)
            val next = core.playbackEnded(EndReason.FINISHED, current.song.durationMs)
            check(next?.song?.id == songs[1].id)
            check(core.search(songs[0].title).songs.any { it.title == songs[0].title })
            flow = "library flow OK (signed in, synced ${synced[0].songs} songs, played an album, searched)"
        }
    }

    core.close()
    println("Kotlin -> Rust: $greeting (core ${coreVersion()}); EncoreCore suspend calls and errors OK; $flow")
}
