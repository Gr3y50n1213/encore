# Release run plan (owner, 27 November 2026)

The owner asked for everything to be tested, secure and ready to publish, then for the rest of the roadmap. Their answers:

- Publish on GitHub Releases only: a signed APK, the server files and the server image.
- Scope: everything on the roadmap. First test, fix, harden and release what exists, then Phase 11 (recap), Play on a TV, smartwatches and the desktop app, each with mock-ups for approval.
- iOS: keep the core iOS-ready, with no SwiftUI app yet.
- Tested work may be merged into main (with a restore tag each time). Small look-and-feel fixes need no mock-up; new screens and layouts do.
- Heavy builds run in CI, not on the owner's PC. The phones and the tablet can still be used for tests, when they are free.

Work goes on the branch `release-run`. Tick each step as it lands, and note the commit or CI run.

## R1. Security

- [ ] Dependabot alerts: none open (checked 27 November).
- [ ] An independent review of the companion server: sign-in, tokens, rate limits, admin and status pages, the live link, file and SQL handling.
- [ ] Review of the core: TLS rules, the plain-http rule, token handling, logs, the local-file hand-off.
- [ ] Android: exported components, the content provider's grants, backup rules, logs in the release build, the network security config, permissions.
- [ ] Fix every real finding, with a test for each.
- [ ] A secret and personal-data scan of the whole history before the release (`git grep` over `git rev-list --all`).

## R2. Tests

- [x] Core CI: every source kind against a real server (Jellyfin replay, Navidrome, Plex, Emby), Windows, Apple, Android ABIs, the server image end to end (PR 1, all green before the merge).
- [x] Android CI: lint, unit tests, emulators (API 28, 36 phone, 36 tablet) green; API 37's emulator image still breaks by itself.
- [ ] The companion server: CI end to end, then the live Windows service updated to the release build (back it up first) and checked locally and over the tailnet.
- [ ] A full pass on the tablet (and on the S20 FE when free), against the section 24 list in `docs/android/PLAN-ANDROID.md`:
  - every screen;
  - playback interruptions;
  - offline mode;
  - downloads;
  - music on this device;
  - profiles, PIN and Guest;
  - handoff;
  - widgets and the Quick Settings tile;
  - the sleep timer;
  - Android Auto;
  - TalkBack and 200% font;
  - rotation.
- [ ] Every bug found is fixed and checked again.

## R3. iOS readiness (core only)

- [x] The Apple CI job stays green (iOS, Simulator, macOS, XCFramework).
- [ ] CORE-API says how Apple hands over phone music (a security-scoped folder URL; files passed as open descriptors, the same `read_fd` path).
- [ ] Nothing in the core is Android-only without an Apple twin or a note.

## R4. Release 1.0.0 on GitHub

- [ ] Version 1.0.0 in the app, the core and the server.
- [ ] A release workflow, run on a `v*` tag:
  - the signed phone and TV APKs;
  - the server files for the four platforms;
  - the server image on GHCR;
  - SHA-256 sums;
  - the release notes.
- [ ] Docs:
  - README install steps;
  - the server install guide brought up to date (all four media servers);
  - a privacy page;
  - a changelog;
  - SECURITY.md's supported versions.
- [ ] THIRD_PARTY_LICENSES matches the About screen.
- [x] Tag, check the release page, and install the release APK over the test build on the tablet: v1.0.0 published 27 November 2026 (release run 46081323974); the APK's SHA-256 matched and it installed over the owner-key build; the image pulls without sign-in.

## R5. Phase 11: year-end recap

- [x] `encore-recap` engine, golden files, parity on every platform (plan section 25): the golden files are byte for byte the same on Linux x64 and arm64, Windows and macOS (recap.yml); 101,266 plays in 460 ms, 34 KB.
- [ ] Mock-ups from boards 38 to 41 to the owner. Build only after approval.

## R6 to R8. After the recap

Each starts with a plan and mock-ups for the owner.

- R6. Play on a TV (Cast needs the owner's OK for Google's library and a US$5 registration).
- R7. Smartwatches: Wear OS and Pebble.
- R8. Windows and Linux app (Flutter).

## Only the owner can do

- The Sony ULT pause check (their headphones, the S20 FE on USB).
- Approving mock-ups.
- Trying the release on their own phone.
