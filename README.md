# Encore (working name)

A free, open-source music player for self-hosted libraries, with Apple Music-grade design, word-by-word synced lyrics, a private on-device recommender, per-person profiles, handoff between devices and a year-end recap. Platforms: macOS, iOS, iPadOS, Windows, Android and Linux. Jellyfin first, behind a generic source layer. "Encore" is a placeholder name.

This folder is a **seed for Claude Code**: the plans, the design and the rules, with no code yet. The plan is split into four tracks that live in one repository, so that a session on a Mac and a session on a PC can work at the same time.

| Track | Kit (zip) | Plan | Folder it builds |
| --- | --- | --- | --- |
| Core and server | `encore-0-core-and-server.zip` | `docs/core/PLAN-CORE-SERVER.md` | `core/`, `server/`, `tools/` |
| Apple (iPhone, iPad, Mac) | `encore-1-apple.zip` | `docs/apple/PLAN-APPLE.md` | `apple/` |
| Android | `encore-2-android.zip` | `docs/android/PLAN-ANDROID.md` | `android/` |
| Windows and Linux | `encore-3-windows-linux.zip` | `docs/windows-linux/PLAN-WINDOWS-LINUX.md` | `flutter/` |

| Path | What it holds |
| --- | --- |
| `CLAUDE.md` | The rules every Claude Code session must never break, the track rules and the reading order. Loaded automatically |
| `docs/START-HERE.md` | For the human owner: how the four kits fit together, what to settle first and how to run sessions |
| `docs/ROADMAP.md` | The thirteen phases, which track builds which part, and each "done when" line |
| `docs/DECISIONS.md` | Decided, chosen for the owner, assumed, open, unverified, spikes, lead times |
| `docs/SECTION-MAP.md` | Where each of the plan's 26 numbered sections lives now |
| `docs/shared/` | The shared plans: product and engine (`PLAN-PRODUCT.md`), design and interaction (`PLAN-UX.md`) |
| `docs/<track>/` | One plan and one prompts file per track |
| `docs/requests/` | How one track asks another for something, with a template |
| `docs/adr/` | Decision records (template included) |
| `docs/reference/` | Where reference screenshots go |
| `design/tokens.json` | Design values with their sources |
| `design/wireframes/` | 43 wireframe boards: PNG, HTML and a text spec each, with an `INDEX.md` that also says which track builds from which board |
| `tools/fixtures/` | Empty; test media and recorded HTTP responses go here |
| `Makefile` | Hands `make check-core`, `check-apple`, `check-android` and `check-flutter` to each track's own Makefile |

## Begin

Owner: read `docs/START-HERE.md`. Claude Code: read `CLAUDE.md`.

## Licence

Not added yet. The plan suggests MPL-2.0 for the apps and AGPL-3.0 for the server; confirm in `docs/DECISIONS.md` before adding a licence file.
