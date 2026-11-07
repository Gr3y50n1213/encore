# Where each plan section lives

The plan was one document with 26 numbered sections. It is now split so that each track reads what it needs. Section numbers did not change, so a reference such as "section 22" or "plan section 25" in the plan, the boards, the decisions log or the tokens still means the same thing.

PLAN-PRODUCT is `docs/shared/PLAN-PRODUCT.md`. PLAN-UX is `docs/shared/PLAN-UX.md`. A platform plan is `docs/apple/PLAN-APPLE.md`, `docs/android/PLAN-ANDROID.md` or `docs/windows-linux/PLAN-WINDOWS-LINUX.md`. The core and server plan is `docs/core/PLAN-CORE-SERVER.md`; it adds no new section numbers, it says which sections the core track builds and how.

| Section | What it covers | Where it is |
| --- | --- | --- |
| 1 to 7, 7.1 | Vision, architecture, repository layout and stack, media sources, companion server, profiles, recommendation engine, mood playlists | PLAN-PRODUCT |
| 8 | Playback engine. The OS integration table is split, one row per platform | PLAN-PRODUCT; OS rows in each platform plan |
| 9 | Design system and UI | PLAN-UX |
| 10 | Year-end recap | PLAN-PRODUCT |
| 11 | Handoff. The Apple `NSUserActivity` bullet is in the Apple plan | PLAN-PRODUCT; Apple plan |
| 12 | Privacy and security | PLAN-PRODUCT |
| 13 | Roadmap | `docs/ROADMAP.md` |
| 14 | How it is built | `docs/shared/PLAN-PRODUCT.md` section 14 (the working notes for the AI-assisted sessions are kept outside the repository) |
| 15 | Open questions and defaults | PLAN-PRODUCT |
| 16 | Interaction, motion and haptics | PLAN-UX |
| 17 | Screen inventory | PLAN-UX |
| 18 | Free and legal libraries. Shared libraries in PLAN-PRODUCT, the rest per platform | PLAN-PRODUCT; each platform plan |
| 19 | Store requirements. What applies to every store is in PLAN-PRODUCT; Apple, Google and the Windows and Linux stores are in their plans | PLAN-PRODUCT; each platform plan |
| 20 | Widgets, Live Activities and the lock screen. The rules, the per-platform table, decisions, sleep timer, widgets and the settings page are shared; limits, notes and store checks are per platform | PLAN-UX; each platform plan |
| 21 | Smart picks | PLAN-PRODUCT |
| 22 | Buttons, states and haptics. Press timing, the control table, haptics, motion and accessibility are shared; the Android shape rule and Apple's button behaviour are per platform | PLAN-UX; Apple, Android and Windows and Linux plans |
| 23 | Notch, Dynamic Island, cut-outs and foldables. The rules, the per-screen table and the test matrix are shared; the device tables and sources are per platform | PLAN-UX; Apple and Android plans |
| 24 | Production-readiness checklist. The whole checklist is shared; platform rows of the unverified table are per platform | PLAN-UX; each platform plan |
| 25 | Year recap: engine, devices and look | PLAN-PRODUCT |
| 26 | Swipe gestures and drag and drop | PLAN-UX |

Every platform plan names the sections it adds as `Section N` headings, so searching a plan for "Section 22" finds that platform's part.

If a platform plan and a shared file disagree, stop and ask the owner. Do not pick one silently.
