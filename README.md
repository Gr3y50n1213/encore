# Encore (working name)

A free, open-source music player for your own music server, with:
- Apple Music-grade design;
- word-by-word synced lyrics;
- private smart picks that learn on your own devices;
- per-person profiles;
- handoff between devices;
- a year-end recap.

It plays from Jellyfin, Emby, Navidrome and other OpenSubsonic servers, Plex, and folders on the phone. Planned platforms: iPhone, iPad, Mac, Android, Windows and Linux. "Encore" is a placeholder name.

**Status:** version 1.0.0 has the Android app, Encore TV (Android TV and projectors) and the companion server. The shared Rust core is also built and tested for Apple, Windows and Linux, whose apps come next (docs/ROADMAP.md).

## Install

- **Android:** download `encore-<version>.apk` from the [Releases page](https://github.com/Gr3y50n1213/encore/releases) on your phone and open it. It needs Android 9 or later. Encore TV is `encore-tv-<version>.apk`.
- **The companion server (optional):** docs/server/INSTALL.md. It runs on Windows, macOS, Linux or Docker (`ghcr.io/gr3y50n1213/encore-server`).
- Each file's SHA-256 is in the release's `SHA256SUMS`.

Privacy: PRIVACY.md. In short, nothing goes anywhere but your own servers.

## How it fits together

- **The core** (`core/`, Rust) holds everything the apps share:
  - the media-server clients and an offline library mirror;
  - search, the queue and the play log;
  - lyrics, album colours and playlist edits;
  - profiles, sync, smart picks and handoff.

  Swift and Kotlin call it through UniFFI, and Dart through flutter_rust_bridge (docs/core/CORE-API.md).
- **The apps**: native SwiftUI on Apple devices (`apple/`), Jetpack Compose with Media3 on Android (`android/`), and Flutter on Windows and Linux (`flutter/`).
- **The companion server** (`server/`, optional) runs next to your media server, as one small program or Docker image (amd64 or arm64). It handles:
  - profiles and sync across your devices;
  - the daily mixes;
  - handoff.

  It never stores music. See `server/compose.yaml`.

Nothing is sent anywhere except to your own servers: no telemetry, no analytics, no account with the project.

## The repository

| Path | What it holds |
| --- | --- |
| `docs/ROADMAP.md` | The thirteen phases and each one's "done when" line |
| `docs/shared/` | The product and design plans (`PLAN-PRODUCT.md`, `PLAN-UX.md`) |
| `docs/core/` | The core's plan, its API contract (`CORE-API.md`) and the review of each phase |
| `docs/adr/`, `docs/DECISIONS.md` | Decision records and the status of every decision |
| `docs/api/openapi.yaml` | The companion server's API |
| `docs/requests/` | How one part of the project asks another for something |
| `design/` | Design tokens (`tokens.json`) and 43 wireframe boards (PNG, HTML and a text spec each) |
| `core/`, `server/`, `tools/` | The Rust core, the companion server, codegen, fixtures and test tools |
| `.github/workflows/core.yml` | CI: every target, every test, licence and privacy checks |

## Building

The core and the server: `make check-core` (Rust stable; fmt, clippy and tests). The server image: `docker build -f server/Dockerfile .`. Each app builds the core for its own targets (docs/core/CORE-API.md, "How every platform gets the core").

## Security

See SECURITY.md. Report vulnerabilities privately through GitHub's "Report a vulnerability". Contributors: docs/PUBLIC-REPO-CHECKLIST.md lists what must never be committed.

## Licence

- The core, the apps, the tools and the docs: **MPL-2.0** (LICENSE).
- The companion server (`server/`): **AGPL-3.0** (server/LICENSE).

Third-party licences: core/THIRD_PARTY_LICENSES.md.
