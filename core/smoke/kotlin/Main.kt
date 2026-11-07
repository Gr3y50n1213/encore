// Smoke test: Kotlin calls the Rust core through the UniFFI bindings (JNA on a desktop JVM).
// CI compiles this with bindings/kotlin and bindings/tokens/EncoreTokens.kt on Linux, with
// JNA and kotlinx-coroutines (the async calls are suspend functions).
// It proves the bindings work. The same file runs on Android phones through app_process
// (tools/android-smoke), where it also checks the https guard of request core-0001.
//
// The only network is 127.0.0.1: a replay of the real Jellyfin recording (see the end).

import kotlinx.coroutines.runBlocking
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.CoreConfig
import uk.gcartee.encore.core.DropTarget
import uk.gcartee.encore.core.DropTargetKind
import uk.gcartee.encore.core.LyricsSync
import uk.gcartee.encore.core.SongSort
import uk.gcartee.encore.core.SyncState
import uk.gcartee.encore.core.ChosenBy
import uk.gcartee.encore.core.NotInterestedReason
import uk.gcartee.encore.core.OutputState
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

// FNV-1a (32 bit) of the flow's clock-free summary: every platform must print the same
// "parity" value (the same account gives the same answers everywhere; Phase 6).
fun parity(lines: List<String>): String {
    var h = 0x811c9dc5u
    for (b in lines.joinToString("\n").toByteArray(Charsets.UTF_8)) {
        h = (h xor (b.toUInt() and 0xffu)) * 0x01000193u
    }
    return h.toString(16).padStart(8, '0')
}

val onAndroid = System.getProperty("java.vm.name") == "Dalvik"

class Listener : CoreListener {
    override fun onEvent(event: CoreEvent) {}
}

// A failure prints its reason and exits 1 (on Android an uncaught exception is a
// silent SIGKILL, which tells the phone runner nothing).
fun main() {
    try {
        smoke()
    } catch (e: Throwable) {
        println("FAILED: $e")
        kotlin.system.exitProcess(1)
    }
}

fun smoke() {
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
    var parityLine = ""
    // On Android without EncoreTls.initialize, an https server must fail with the
    // documented reason (an Unreachable error), never crash (request core-0001).
    var tls = ""
    val tlsUrl = System.getenv("ENCORE_TLS_URL")
    if (onAndroid && tlsUrl != null) {
        runBlocking {
            try {
                core.checkAddress(tlsUrl)
                error("an https server must be refused before EncoreTls.initialize")
            } catch (e: EncoreException) {
                check(e.toString().contains("EncoreTls.initialize")) { "unexpected: $e" }
                tls = "; https guard OK (${e.javaClass.simpleName})"
            }
        }
    }
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

            // Phase 3: lyrics, an album's colours (the same bytes on every platform),
            // a playlist edit and its undo.
            val all = core.songs(SongSort.TITLE, 0u, 500u)
            val lyrics = all.filter { it.hasLyrics }.firstNotNullOfOrNull { core.lyrics(it.id) }
            checkNotNull(lyrics) { "no lyrics" }
            check(lyrics.syncLevel == LyricsSync.LINE && lyrics.lines.isNotEmpty())
    // Search finds the song by a line of its lyrics (or by its title).
    val words = lyrics.lines.first { it.text.split(" ").size >= 2 }.text.split(" ")
    val byLyrics = core.search("${words[0]} ${words[1]}")
    check(byLyrics.lyrics.any { it.song.id == lyrics.songId } || byLyrics.songs.any { it.id == lyrics.songId }) { "lyrics search" }
    // Export: one open JSON file.
    val exportFile = java.io.File(dir, "export.json")
    val exported = core.exportProfile(signedIn.profileId, exportFile.path)
    check(exported.bytes > 0uL && exportFile.length() == exported.bytes.toLong()) { "export" }
    // Phase 4: the switcher's fields and the sync status cross the boundary.
    val me = core.profiles().first { it.profileId == signedIn.profileId }
    check(me.active && !me.locked && me.initials.isNotEmpty()) { "profiles" }
    check(core.syncStatus().state == SyncState.NOT_CONNECTED) { "sync status" }
    // Phase 5: smart picks cross the boundary.
    val auto = core.refreshAutoplay()
    check(auto.autoplay.size == 10 && auto.autoplay.all { it.chosenBy == ChosenBy.APP }) { "autoplay" }
    val home = core.home()
    check(home.madeForYou.playsNeeded == 50u && home.recentlyAdded.isNotEmpty()) { "home" }
    val skip = core.notInterestedArtist(auto.autoplay[0].song.artist, NotInterestedReason.NOT_MY_TASTE)
    core.removeNotInterested(skip)
    // Phase 7: without a companion the output picker hides handoff, and it's refused.
    check(core.outputDevices().state == OutputState.NO_COMPANION) { "output devices" }
    try {
        core.handOffTo("nobody")
        error("handoff needs a companion")
    } catch (e: EncoreException) {
    }
            val artAlbum = core.albums(AlbumSort.TITLE, 0u, 100u).first { it.hasArtwork }
            val palette = checkNotNull(core.albumPalette(artAlbum.id)) { "no palette" }
            check(palette.base == "3D0000" && palette.stops.size == 4) { palette.base }
            val list = core.playlists(0u, 1u)[0]
            val entriesBefore = core.playlist(list.id).entries
            val inList = entriesBefore.mapNotNull { it.song?.id }.toSet()
            val fresh = all.first { it.id !in inList }
            val edit = core.addTo(DropTarget(DropTargetKind.PLAYLIST, list.id), listOf(fresh.id), null, false)
            check(edit.added == 1u && edit.editId != null)
            core.undo(edit.editId!!)
            check(core.playlist(list.id).entries.size == entriesBefore.size)
            parityLine = "; parity " + parity(listOf(
                "songs=${synced[0].songs}",
                "albums=" + core.albums(AlbumSort.TITLE, 0u, 100u).joinToString("|") { it.title },
                "album=" + songs.joinToString("|") { it.title },
                "search=" + core.search(songs[0].title).songs.joinToString("|") { it.title },
                "lyrics=${lyrics.lines.size}:${lyrics.lines[0].text}",
                "palette=${palette.base}," + palette.stops.joinToString(","),
                "playlist=${list.name}:${entriesBefore.size}",
            ))
            flow = "library flow OK (signed in, synced ${synced[0].songs} songs, played an album, searched, lyrics, lyrics search, export, profiles, sync status, smart picks, handoff, palette ${palette.base}, playlist edit and undo)"
        }
    }

    core.close()
    val where = if (onAndroid) "Kotlin on Android" else "Kotlin"
    println("$where -> Rust: $greeting (core ${coreVersion()}); EncoreCore suspend calls and errors OK; $flow$tls$parityLine")
}
