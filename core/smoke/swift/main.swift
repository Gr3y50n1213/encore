// Smoke test: Swift calls the Rust core through the UniFFI bindings.
// CI compiles this with bindings/swift/EncoreCore.swift and bindings/tokens/EncoreTokens.swift
// on Linux, and on macOS for every Apple target in Swift 6 mode: it runs on macOS and in an
// iOS Simulator, and the iOS device build is linked but not run. The Apple track proves the
// bindings on real iPhones, iPads and Macs.
//
// The only network is 127.0.0.1: a replay of the real Jellyfin recording (see the end).

import Foundation

let greeting = hello(name: "CI")
precondition(greeting == "Hello, CI, from the Encore core.", "unexpected greeting: \(greeting)")
precondition(hello(name: "") == "Hello from the Encore core.")
precondition(appName() == "Encore")
precondition(!coreVersion().isEmpty)
precondition(EncoreTokens.Breakpoints.railWidth == 80)
precondition(EncoreTokens.Color.Accents.Iris.Light.primary == "#6146CB")
precondition(EncoreTokens.Typography.Scale.Compact.PageTitle.size == 28)
precondition(EncoreTokens.Recap.TypeGroup.tide == "Inter, heavy") // recap.type, renamed for Swift

final class Listener: CoreListener, @unchecked Sendable {
    func onEvent(event: CoreEvent) {}
}

let dir = FileManager.default.temporaryDirectory
    .appendingPathComponent("encore-smoke-\(ProcessInfo.processInfo.processIdentifier)")
let core = try EncoreCore(config: CoreConfig(
    dataDir: dir.path, deviceId: "smoke", deviceName: "CI", platform: "ci"))
core.setListener(listener: Listener())
let profiles = try core.profiles()
precondition(profiles.isEmpty)
precondition(core.currentProfile() == nil)

// A synchronous call with no profile open throws a typed error.
do {
    _ = try core.servers()
    preconditionFailure("servers() should need a profile")
} catch EncoreError.NoProfile(let reason) {
    precondition(!reason.isEmpty)
}

// An async call: plain http to a public address is refused before any network use.
do {
    _ = try await core.checkAddress(address: "http://music.example.org")
    preconditionFailure("plain http to a public address must be refused")
} catch EncoreError.InsecureAddress(let reason) {
    precondition(reason.contains("https"), reason)
}

// The library flow over real HTTP: sign in, sync, browse, play and search against the
// real Jellyfin recording, which tools/ci/replay_jellyfin.py serves on this machine.
var flow = "library flow skipped (set ENCORE_REPLAY_URL, see tools/ci/replay.sh)"
if let replay = ProcessInfo.processInfo.environment["ENCORE_REPLAY_URL"] {
    let check = try await core.checkAddress(address: replay)
    precondition(check.version == "12.0.0", check.version)
    precondition(check.plainHttpOnLocalNetwork)
    let start = try await core.quickConnectStart(baseUrl: check.baseUrl)
    precondition(start.code.count == 6)
    let signedIn = try await core.quickConnectWait()
    precondition(signedIn.profileId.hasPrefix("jf-"))
    precondition(core.currentProfile() == signedIn.profileId)
    let synced = try await core.syncLibrary()
    precondition(synced.count == 1 && synced[0].songs == 150, "\(synced)")
    guard let album = try core.albums(sort: .title, offset: 0, limit: 100).first(where: { $0.songCount >= 2 })
    else { preconditionFailure("no album with two songs") }
    let songs = try core.album(albumId: album.id).songs
    let state = try core.play(
        songIds: songs.map { $0.id }, startIndex: 0, startedFrom: .album, contextId: "smoke")
    guard let current = state.current else { preconditionFailure("nothing playing") }
    precondition(current.song.id == songs[0].id && state.upNext.count == songs.count - 1)
    try core.playbackStarted(uid: current.uid, utcOffsetMinutes: 0)
    let next = try core.playbackEnded(reason: .finished, listenedMs: current.song.durationMs)
    precondition(next?.song.id == songs[1].id)
    let found = try core.search(query: songs[0].title)
    precondition(found.songs.contains { $0.title == songs[0].title })

    // Phase 3: lyrics, an album's colours (the same bytes on every platform: the
    // replay serves board 04's Golden Hour Static cover), a playlist edit and undo.
    let all = try core.songs(sort: .title, offset: 0, limit: 500)
    var lyrics: SongLyrics? = nil
    for song in all where song.hasLyrics {
        lyrics = try await core.lyrics(songId: song.id)
        if lyrics != nil { break }
    }
    guard let lyrics else { preconditionFailure("no lyrics") }
    precondition(lyrics.syncLevel == .line && !lyrics.lines.isEmpty)
    // Search finds the song by a line of its lyrics (or by its title).
    let words = lyrics.lines.first { $0.text.split(separator: " ").count >= 2 }!.text.split(separator: " ")
    let byLyrics = try core.search(query: "\(words[0]) \(words[1])")
    precondition(byLyrics.lyrics.contains { $0.song.id == lyrics.songId } || byLyrics.songs.contains { $0.id == lyrics.songId }, "lyrics search")
    // Export: one open JSON file.
    let exportFile = dir.appendingPathComponent("export.json").path
    let exported = try core.exportProfile(profileId: signedIn.profileId, destPath: exportFile)
    precondition(exported.bytes > 0 && FileManager.default.fileExists(atPath: exportFile), "export")
    // Phase 4: the switcher's fields and the sync status cross the boundary.
    let me = try core.profiles().first { $0.profileId == signedIn.profileId }!
    precondition(me.active && !me.locked && !me.initials.isEmpty, "profiles")
    let syncState = try core.syncStatus().state
    precondition(syncState == .notConnected, "sync status")
    // Phase 5: smart picks cross the boundary.
    let auto = try core.refreshAutoplay()
    precondition(auto.autoplay.count == 10 && auto.autoplay.allSatisfy { $0.chosenBy == .app }, "autoplay")
    let home = try await core.home()
    precondition(home.madeForYou.playsNeeded == 50 && !home.recentlyAdded.isEmpty, "home")
    let skip = try core.notInterestedArtist(artist: auto.autoplay[0].song.artist, reason: .notMyTaste)
    try core.removeNotInterested(item: skip)
    guard let artAlbum = try core.albums(sort: .title, offset: 0, limit: 100).first(where: { $0.hasArtwork }),
          let palette = try await core.albumPalette(albumId: artAlbum.id)
    else { preconditionFailure("no palette") }
    precondition(palette.base == "3D0000" && palette.stops.count == 4, palette.base)
    let list = try core.playlists(offset: 0, limit: 1)[0]
    let entriesBefore = try core.playlist(playlistId: list.id).entries
    let inList = Set(entriesBefore.compactMap { $0.song?.id })
    guard let fresh = all.first(where: { !inList.contains($0.id) }) else { preconditionFailure("no song") }
    let edit = try core.addTo(
        target: DropTarget(kind: .playlist, id: list.id), songIds: [fresh.id], position: nil,
        allowDuplicates: false)
    precondition(edit.added == 1 && edit.editId != nil)
    try core.undo(editId: edit.editId!)
    let afterUndo = try core.playlist(playlistId: list.id).entries.count
    precondition(afterUndo == entriesBefore.count, "\(afterUndo) vs \(entriesBefore.count)")
    flow = "library flow OK (signed in, synced \(synced[0].songs) songs, played an album, searched, lyrics, lyrics search, export, profiles, sync status, smart picks, palette \(palette.base), playlist edit and undo)"
}

print("Swift -> Rust: \(greeting) (core \(coreVersion())); EncoreCore async and errors OK; \(flow)")
try? FileManager.default.removeItem(at: dir)
