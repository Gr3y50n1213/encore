# ADR core-0005: HTTP, certificates, tokens and recorded fixtures

Date: 6 November 2026
Status: accepted

## Context
The core talks to the media server for every platform (plan section 2). Section 12: TLS outside the home network, tokens only in the OS keychain, no third-party calls. Board 21: plain http to a local address warns, to anything else is refused; a self-signed certificate asks once to trust its fingerprint. Section 4: "Each adapter ships with recorded HTTP fixtures, so tests run without a live server." Jellyfin 12.0 turned off the legacy ways of sending a token (release notes v12.0, PRs 13306 and 15559).

## Decision
- **HTTP.** reqwest 0.13 without its default TLS backend, on rustls 0.23 with the ring crypto provider. reqwest's default, aws-lc-rs, needs CMake to cross-compile for iOS and Android; ring builds with a C compiler on every target CI builds.
- **Certificates.** rustls-platform-verifier: each operating system's own verifier and trust store decide, so a private CA the user installed works. A wrapper pins one certificate by SHA-256 when the user trusts it. The handshake signature is still verified, and a rejected certificate's fingerprint is reported so the app can show it. Android needs an init call with a `Context`: request `core-0001-android-tls-init`.
- **Jellyfin auth.** Only `Authorization: MediaBrowser Client, Device, DeviceId, Version, Token`. Values are percent-encoded, because the server URL-decodes them (`AuthorizationContext.cs`). Players that cannot send headers get the `ApiKey` query parameter, the only other form 12.0 accepts.
- **Tokens.** Never stored by the core. Sign-in hands the token to the app for its keychain; `open_profile` takes it back. `Debug` output of every type that holds a token or a token-bearing URL leaves it out.
- **Sync cursor.** The server's own clock (HTTP `Date`) when a sync starts, minus a minute; later syncs ask for `minDateLastSaved` and list IDs to find removals. Jellyfin 12.0 does not return `DateLastSaved` on items; the first recording showed this.
- **Fixtures.** `tools/fixture-recorder` records through the adapter's own code, signing in with Quick Connect so no password is typed. Before writing, it replaces every string except kinds, formats, dates and versions with a same-shape stand-in from a random key that is not kept. It removes paths, BlurHash previews, media stream details and tokens. A leak check refuses to write if any original value survives. Recordings live in `tools/fixtures/jellyfin/` with notes on how they were made; error paths a recording cannot hold use hand-written stand-ins from the server's OpenAPI document.

## Alternatives considered
- native-tls: the system TLS stack everywhere, but OpenSSL on Android and Linux, which is harder to cross-compile and does not help pinning.
- Bundled Mozilla roots only (webpki-roots): simpler, but a private CA would need a trust prompt on every device.
- A keychain callback interface from the core to each platform: async callbacks across three bindings, for what the app can do with one call after sign-in.
- Recording the owner's library unchanged: quicker, but the fixtures will be published with the code.

## Consequences
- Every new adapter gets a `Transport`-based client and its own recorded fixtures.
- The Android track has one extra setup step (the verifier's Kotlin component and an init call).
- Moving off reqwest or rustls touches only `encore-sources/src/http.rs`.
