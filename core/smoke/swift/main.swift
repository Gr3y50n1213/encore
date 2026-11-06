// Smoke test: Swift calls the Rust core through the UniFFI bindings.
// CI compiles this with bindings/swift/EncoreCore.swift and bindings/tokens/EncoreTokens.swift
// on Linux, and on macOS for every Apple target in Swift 6 mode: it runs on macOS and in an
// iOS Simulator, and the iOS device build is linked but not run. The Apple track proves the
// bindings on real iPhones, iPads and Macs.
//
// No network: the API calls here fail or succeed without reaching a server. Signing in,
// syncing, browsing and playing are tested in Rust on a real Jellyfin recording.

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
precondition(try core.profiles().isEmpty)
precondition(core.currentProfile() == nil)

// A synchronous call with no profile open throws a typed error.
do {
    _ = try core.servers()
    preconditionFailure("servers() should need a profile")
} catch EncoreError.NoProfile(let message) {
    precondition(!message.isEmpty)
}

// An async call: plain http to a public address is refused before any network use.
do {
    _ = try await core.checkAddress(address: "http://music.example.org")
    preconditionFailure("plain http to a public address must be refused")
} catch EncoreError.InsecureAddress(let message) {
    precondition(message.contains("https"), message)
}

print("Swift -> Rust: \(greeting) (core \(coreVersion())); EncoreCore async and errors OK")
try? FileManager.default.removeItem(at: dir)
