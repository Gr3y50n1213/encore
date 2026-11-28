# Encore for Windows and Linux: track plan

You are the **Windows and Linux track**. You build the desktop apps in Flutter with the Encore Material 3 Expressive theme, on top of the shared Rust core. Read `CLAUDE.md`, then `flutter/CLAUDE.md` once it exists, then `docs/ROADMAP.md`, then this plan.

**You own:** `flutter/` (the Flutter project with `windows/` and `linux/`, the Windows widget provider), `core/encore-audio/` (the Rust desktop audio engine: Symphonia and cpal, SMTC and MPRIS glue; the core track only creates the empty crate in Phase 1), `.github/workflows/flutter.yml`, `docs/windows-linux/`.

**You never edit:** the rest of `core/`, `server/`, `tools/`, `design/`, `apple/`, `android/`, the shared documents. If you need a core call, a token or a decision, write a request in `docs/requests/`.

Section numbers in this plan are the plan's original numbers (`docs/SECTION-MAP.md`). The shared plans are `docs/shared/PLAN-PRODUCT.md` (product, core, server, recommender, recap, privacy) and `docs/shared/PLAN-UX.md` (design system, interaction, screens, buttons, cut-outs, swipes, checklist). Headings that start with "Section N" below hold the part of section N that belongs only to this track. Read the shared section as well.

## 1. What you build

- **Targets.** The Windows app and the Linux app, from one Flutter codebase in `flutter/` (section 3, repository layout). Android is native and is another track.
- **Minimum systems.** Windows 10, and the current Ubuntu and Fedora LTS (section 15 default, `docs/DECISIONS.md`).
- **Look.** The same Material 3 Expressive look as Android, drawn by hand in Flutter at desktop density (Flutter has no expressive components, so the Encore theme draws the shapes, connected button groups and wavy progress itself). No glass on Windows and Linux. Structure, flows and behaviour are the same as everywhere. Layout follows the window width in four size classes, and every window is resizable.
- **Windows.** An optional Mica backdrop on Windows 11, a 48 px title bar with the system caption buttons, Snap Layouts (to verify), a compact-overlay mini player, System Media Transport Controls (SMTC) and taskbar thumbnail buttons, taskbar progress and a Jump List, and Widgets board widgets in the Store (MSIX) build.
- **Linux.** A header bar that follows the desktop's button side; works on Wayland and X11; MPRIS media controls and an optional tray icon (GNOME needs an extension for tray icons); no custom widgets in version 1.
- **What you do not build.** Sources, the queue, sync, smart picks, the recap engine and the server are in the core. You call the generated Dart bindings and draw what they return.

**Stack for this track (section 3)**

| Layer | Technology | Why |
| --- | --- | --- |
| Other UI | Windows and Linux: Flutter (stable), Riverpod, go\_router, with a custom Material 3 Expressive theme. Android: Kotlin, Jetpack Compose, Material 3 Expressive | One Flutter codebase for Windows and Linux; Android is native so it gets the expressive components. Flutter has none, so the Encore theme draws the shapes, connected button groups and wavy progress itself |
| Desktop audio | Symphonia + cpal (Rust) | No GStreamer or libmpv dependency |
| On-device ML | Core ML on Apple, ONNX Runtime elsewhere | Uses the Neural Engine or NPU when present |
| CI | GitHub Actions: macOS runner for Apple, Linux and Windows runners | Every PR builds every target |

The "Other UI" row also describes Android, which is another track. Your part of it is: Flutter (stable), Riverpod, go\_router, with a custom Material 3 Expressive theme.

**Fonts:** SF Pro may only be used on Apple platforms. Use the system font on Apple devices and bundle Inter (SIL Open Font License) on Windows, Linux and Android to get the same clean look.

**Compiling the core.** You build the Rust core for Windows and Linux inside `flutter/` (the plan names flutter\_rust\_bridge for the Dart bindings). The Rust source stays in `core/`. Phase 1 proves this path; if it fails, stop and say which step and why (`docs/DECISIONS.md` section 6, spike 1).

## 2. What you need

- Flutter (stable) and the Rust toolchain. Flutter desktop apps are built on the system they target, so you need a Windows computer for the Windows app and a Linux computer or VM for the Linux app; one session usually sees only one of them. CI builds both, but the owner should see both (`docs/START-HERE.md`). This is stated from general knowledge of Flutter and is listed as unverified in `docs/DECISIONS.md`.
- Tell the owner in your first reply which system you are on, and which desktop environments you can see for Linux (GNOME, KDE, Wayland, X11).
- For Windows Store builds: a packaged (MSIX) build, because the Widgets board takes widgets from packaged apps only (section 20).
- A Jellyfin server the core track has already tested against.

## 3. Your phases

The phase numbers and the master “done when” lines are the plan's. The master roadmap built Windows and Linux inside Phase 6; here you follow Phases 2 to 5 in the same order as the other tracks, as soon as the core part of each exists, and the Phase 6 list (the Rust audio engine, SMTC and MPRIS, drag and drop, resizable layouts) is part of those phases. The core track builds the core part of each phase one step ahead of you: `git pull` first and read `docs/core/CORE-API.md`. Where a master line says “every platform” or names iPhone and Mac, read it as your platform. The owner checks the done-when line on a real machine.

| Phase | Your part | Master "done when" |
| --- | --- | --- |
| 1 Foundations | Flutter project for Windows and Linux; flutter\_rust\_bridge hello-world; CI on Windows and Linux runners | every platform shows a screen whose text comes from the Rust core, and CI is green. |
| 2 Jellyfin and playback | The same with the Rust audio engine (Symphonia and cpal), SMTC and MPRIS (master Phase 6) | you can sign in, browse your whole library offline-fast, and listen to an album gaplessly. |
| 3 Apple Music-grade UI and lyrics | The same screens in Flutter with the Encore theme, resizable windows, drag and drop (master Phase 6) | side by side with Apple Music, the screens feel equally smooth at 120 Hz, and lyrics sync word by word. |
| 4 Server, profiles and sync | The same | two people share an iPad with fully separate history, and a like on the phone shows on the Mac within a second. |
| 5 Recommendations v1 | The same | autoplay's early-skip rate is clearly lower than shuffle on your own history, and the smart-picks items in section 24 pass. |
| 6 Android, Windows, Linux | Phases 2 to 5 for Windows and Linux, with the Rust audio engine, SMTC, MPRIS | the same account works on all six platforms and the UI passes the same visual checklist. |
| 7 Handoff and remote control | Output picker, pull banner | phone-to-desktop transfer starts in under 1 second for cached tracks. |
| 8 Offline and car | Downloads screens, taskbar and notification progress | a downloaded playlist plays in airplane mode and in a car. |
| 9 Widgets, controls and live surfaces | Windows Widgets board provider (Store build), Jump List, taskbar progress; Linux progress through notifications and launchers | a widget set to one profile never shows another profile's history, the locked screen shows nothing from the listening history, the sleep timer ends with the fade-out, and no widget or Live Activity makes a network call. |
| 10 Recommendations v2 | ONNX Runtime with DirectML on Windows | v2 beats v1's early-skip rate in an offline replay test. |
| 11 Year-end recap | The same in Flutter | two test profiles with different histories get visibly different recaps, a re-run for next year picks a different pack, the same log gives byte-identical recap data on the server and on every platform, and late plays synced before January 8 appear in the final recap. |
| 13 Release | Windows installer (winget), Flathub | The section 24 checklist is ticked for every platform. |

**Shared sections to read.** In PLAN-PRODUCT: 1 to 8, 10 to 12, 15, 19, 21 and 25 (sections 4, 5, 7 and 25 describe what the core hands you; you rarely build them). In PLAN-UX: 9, 16, 17, 20, 22, 23, 24 and 26. Read sections 1, 2 and 3 first, and the others when the phase you are on touches them.

## 4. Your boards

The phone boards draw iOS on the left and Android (Material 3 Expressive) on the right. Take the right side's look and structure, at desktop density. The Windows and Linux layouts are board 30. Boards 43 draws drag and drop for several platforms; take the Windows and Linux part. Swipe gestures (board 42) do not apply: desktop has no swipe, and uses the buttons, media keys and Space (section 26, rule 14).

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 01 | Start here | 17 | [png](../../design/wireframes/png/01-start-here.png) | [spec](../../design/wireframes/specs/01-start-here.md) |
| 02 | System: motion and haptics | 16 | [png](../../design/wireframes/png/02-system-motion-and-haptics.png) | [spec](../../design/wireframes/specs/02-system-motion-and-haptics.md) |
| 03 | System: gestures and accidental presses | 16 | [png](../../design/wireframes/png/03-system-gestures-and-accidental-presses.png) | [spec](../../design/wireframes/specs/03-system-gestures-and-accidental-presses.md) |
| 04 | System: album gradients | 9 | [png](../../design/wireframes/png/04-system-album-gradients.png) | [spec](../../design/wireframes/specs/04-system-album-gradients.md) |
| 05 | Home | 17 | [png](../../design/wireframes/png/05-home.png) | [spec](../../design/wireframes/specs/05-home.md) |
| 06 | Library | 17 | [png](../../design/wireframes/png/06-library.png) | [spec](../../design/wireframes/specs/06-library.md) |
| 07 | Search | 17 | [png](../../design/wireframes/png/07-search.png) | [spec](../../design/wireframes/specs/07-search.md) |
| 08 | Album | 17 | [png](../../design/wireframes/png/08-album.png) | [spec](../../design/wireframes/specs/08-album.md) |
| 09 | Now Playing | 9, 16, 22 | [png](../../design/wireframes/png/09-now-playing.png) | [spec](../../design/wireframes/specs/09-now-playing.md) |
| 10 | Lyrics | 9 | [png](../../design/wireframes/png/10-lyrics.png) | [spec](../../design/wireframes/specs/10-lyrics.md) |
| 11 | Queue | 17 | [png](../../design/wireframes/png/11-queue.png) | [spec](../../design/wireframes/specs/11-queue.md) |
| 12 | Output and handoff | 11 | [png](../../design/wireframes/png/12-output-and-handoff.png) | [spec](../../design/wireframes/specs/12-output-and-handoff.md) |
| 13 | Song menu | 17 | [png](../../design/wireframes/png/13-song-menu.png) | [spec](../../design/wireframes/specs/13-song-menu.md) |
| 14 | Profile switcher | 6 | [png](../../design/wireframes/png/14-profile-switcher.png) | [spec](../../design/wireframes/specs/14-profile-switcher.md) |
| 15 | Made for you playlist | 7.1 | [png](../../design/wireframes/png/15-made-for-you-playlist.png) | [spec](../../design/wireframes/specs/15-made-for-you-playlist.md) |
| 16 | Year recap (entry card, story player, share sheet; card count, timing and where numbers are computed are superseded by 39 to 41) | 10, 25 | [png](../../design/wireframes/png/16-year-recap.png) | [spec](../../design/wireframes/specs/16-year-recap.md) |
| 17 | Settings | 17 | [png](../../design/wireframes/png/17-settings.png) | [spec](../../design/wireframes/specs/17-settings.md) |
| 18 | Servers and sources | 4 | [png](../../design/wireframes/png/18-servers-and-sources.png) | [spec](../../design/wireframes/specs/18-servers-and-sources.md) |
| 19 | Privacy and data | 12, 19 | [png](../../design/wireframes/png/19-privacy-and-data.png) | [spec](../../design/wireframes/specs/19-privacy-and-data.md) |
| 20 | Empty, error and offline | 17 | [png](../../design/wireframes/png/20-empty-error-and-offline.png) | [spec](../../design/wireframes/specs/20-empty-error-and-offline.md) |
| 21 | Connect a server | 4, 17 | [png](../../design/wireframes/png/21-connect-a-server.png) | [spec](../../design/wireframes/specs/21-connect-a-server.md) |
| 22 | Welcome | 17, 19 | [png](../../design/wireframes/png/22-welcome.png) | [spec](../../design/wireframes/specs/22-welcome.md) |
| 23 | Mixes tab | 7.1, 21 | [png](../../design/wireframes/png/23-mixes-tab.png) | [spec](../../design/wireframes/specs/23-mixes-tab.md) |
| 24 | System: platform design languages | 9 | [png](../../design/wireframes/png/24-system-platform-design-languages.png) | [spec](../../design/wireframes/specs/24-system-platform-design-languages.md) |
| 25 | Permissions and system prompts | 19 | [png](../../design/wireframes/png/25-permissions-and-system-prompts.png) | [spec](../../design/wireframes/specs/25-permissions-and-system-prompts.md) |
| 26 | Size classes and platform rules | 9, 19 | [png](../../design/wireframes/png/26-size-classes-and-platform-rules.png) | [spec](../../design/wireframes/specs/26-size-classes-and-platform-rules.md) |
| 30 | Windows and Linux layouts | 9 | [png](../../design/wireframes/png/30-windows-and-linux-layouts.png) | [spec](../../design/wireframes/specs/30-windows-and-linux-layouts.md) |
| 31 | Live surfaces and rules | 20 | [png](../../design/wireframes/png/31-live-surfaces-and-rules.png) | [spec](../../design/wireframes/specs/31-live-surfaces-and-rules.md) |
| 34 | Widgets and media surfaces on Android, Windows and Linux | 20 | [png](../../design/wireframes/png/34-widgets-and-media-surfaces-on-android-windows-and-linux.png) | [spec](../../design/wireframes/specs/34-widgets-and-media-surfaces-on-android-windows-and-linux.md) |
| 35 | Buttons, states and haptics | 22 | [png](../../design/wireframes/png/35-buttons-states-and-haptics.png) | [spec](../../design/wireframes/specs/35-buttons-states-and-haptics.md) |
| 37 | Smart picks: signals and rules | 21 | [png](../../design/wireframes/png/37-smart-picks-signals-and-rules.png) | [spec](../../design/wireframes/specs/37-smart-picks-signals-and-rules.md) |
| 38 | Smart picks: screens and states | 21 | [png](../../design/wireframes/png/38-smart-picks-screens-and-states.png) | [spec](../../design/wireframes/specs/38-smart-picks-screens-and-states.md) |
| 39 | Year recap: the story | 10, 25 | [png](../../design/wireframes/png/39-year-recap-the-story.png) | [spec](../../design/wireframes/specs/39-year-recap-the-story.md) |
| 40 | Year recap: packs and years | 10, 25 | [png](../../design/wireframes/png/40-year-recap-packs-and-years.png) | [spec](../../design/wireframes/specs/40-year-recap-packs-and-years.md) |
| 41 | Year recap: devices and states | 10, 25 | [png](../../design/wireframes/png/41-year-recap-devices-and-states.png) | [spec](../../design/wireframes/specs/41-year-recap-devices-and-states.md) |
| 43 | Drag and drop on desktop and tablets | 16, 26 | [png](../../design/wireframes/png/43-drag-and-drop-on-desktop-and-tablets.png) | [spec](../../design/wireframes/specs/43-drag-and-drop-on-desktop-and-tablets.md) |

## Section 8: Playback engine: operating-system integration (Windows, Linux)

| Platform | Must support |
| --- | --- |
| Windows | System Media Transport Controls, taskbar thumbnail buttons, media keys, Widgets board widgets in the Store build, taskbar progress and a Jump List (section 20) |
| Linux | MPRIS (desktop media controls), media keys, tray icon, progress through notifications and launcher entries where the desktop shows them; no custom widgets in version 1 (section 20) |

Section 2 adds the audio path: a Rust audio engine (Symphonia to decode, cpal to output) with System Media Transport Controls on Windows and MPRIS on Linux. The core owns the queue and tells the audio engine what to load next.

## Section 9: Design language (Windows and Linux rows)

| Platform | Approach |
| --- | --- |
| Windows | Flutter with the Encore Material 3 Expressive theme, an optional Mica backdrop on Windows 11, a 48 px title bar with the system caption buttons, Snap Layouts (to verify), a compact-overlay mini player, System Media Transport Controls and taskbar thumbnail buttons |
| Linux | Flutter with the same Encore theme and a header bar that follows the desktop's button side; works on Wayland and X11; MPRIS media controls and an optional tray icon (GNOME needs an extension for tray icons) |

## Section 16: Motion and haptics on desktop

Windows and Linux have no haptics and rely on motion and sound (section 22, Press and hold timing). Mac-style trackpad haptics do not apply. Springs, interruptible motion and Reduce Motion (120 ms cross-fades) apply as in section 16 of PLAN-UX. Android's motion scheme is not available in Flutter, so the Encore theme carries the equivalent spring values from `design/tokens.json`.

## Section 18: Libraries (Windows and Linux)

| Library | Used for | License |
| --- | --- | --- |
| Symphonia | Decoding on Windows and Linux | MPL-2.0 |
| cpal | Audio output on Windows and Linux | Apache-2.0 |
| Riverpod | Flutter state | MIT |
| go\_router | Flutter navigation | BSD-3-Clause |
| flutter\_liquid\_glasses | Glass look on Flutter | Apache-2.0 |
| Inter | Type on Android, Windows and Linux | SIL OFL 1.1 |
| Material Symbols | Icons on Android, Windows and Linux | Apache-2.0 |

Note: `flutter_liquid_glasses` is listed in the original plan for a glass look on Flutter, but the owner decided that Windows and Linux get a look closer to Android and no glass (`docs/DECISIONS.md`). Do not add it without asking. Shared libraries (flutter\_rust\_bridge, ONNX Runtime and the others) are in section 18 of PLAN-PRODUCT. Check each licence again when you add a library, and keep `THIRD_PARTY_LICENSES` current.

## Section 19: Store requirements (Windows and Linux)

**Not researched.** Section 19 covers Apple and Google only. Nothing in the plan was checked for the Microsoft Store (MSIX), winget or Flathub, and Phase 13 names a Windows installer (winget) and Flathub. Before Phase 13, read each store's current documentation, write down the requirements the way section 19 does for Apple and Google (deadlines, privacy declarations, permissions, review access, packaging), and ask the owner to confirm. Until then treat every Windows and Linux store rule as unverified (`docs/DECISIONS.md` section 5).

What is already known and applies to you:

- The plain-HTTP rule is the same everywhere: plain HTTP only to home-network addresses, HTTPS everywhere else (section 12, section 19).
- The Widgets board takes widgets from packaged apps, so widgets ship in the Store (MSIX) build only; the widget provider is declared in the MSIX manifest, and direct builds have no Widgets board entry (section 20).
- Tokens live in Windows Credential Manager and the Linux Secret Service; BitLocker or Device Encryption and disk encryption are advised (section 12). SQLCipher may return later as an optional, off-by-default setting for Windows and Linux direct builds only.
- Every store item that is not tied to Apple or Google also applies to you: privacy policy linked in the app, account deletion in Privacy and data, no downloaded code, name and marks, media rights (section 19, shared part).

**Before each submission (items that apply to every store)**

- [ ] Privacy policy page live and linked in App Store Connect, Play Console and the app
- [ ] Screenshots from real builds on each device class, with no Apple Music marks
- [ ] Name, subtitle and keywords checked for Jellyfin, Apple marks and trademark conflicts
- [ ] THIRD\_PARTY\_LICENSES matches the About screen, including demo track credits

## Section 20: Widgets and media surfaces (Windows and Linux)

Read section 20 in PLAN-UX first: the four rules, the decisions, the sleep timer, the widgets and the settings page apply to you. These are your columns and the parts that belong only to Windows and Linux.

**What each platform gets (Windows, Linux)**

| Surface | Windows | Linux |
| --- | --- | --- |
| Player on the lock screen, drawn by the system | Lock screen, volume flyout | Lock screen and shell media controls |
| Timed tasks: sleep timer, user-started downloads, first sync | Taskbar progress, progress notification | Notification and launcher progress where the desktop shows them |
| Home widgets | Widgets board, Store build | Not in version 1 |
| Lock screen widgets | Not available to apps | Not available |
| Quick controls | Jump List, tray menu | Tray menu, desktop actions |

Comes with no extra work: an iPhone Live Activity also appears in the Mac menu bar, on the CarPlay Dashboard and in the Apple Watch Smart Stack, and a small widget works in StandBy and CarPlay.

**Widgets (Windows)**

| Widget | Android and Windows | What it does |
| --- | --- | --- |
| Now Playing | 2 by 1 up to 6 by 2 (Android), small (Windows) | Cover, title, artist, progress, Play or pause and Next. Tapping the cover opens Now Playing. Shows the last played song with a Play button when nothing is playing. |
| Mixes | 4 by 2 (Android), medium (Windows) | Each tile starts its mix with one tap. Configurable: pick a mix or follow Next up. |
| Recently played | 4 by 3 (Android), large (Windows) | Tapping an album opens it. History, so redacted while locked and off the lock screen. |
| Recap | Android only, 2 by 2 and 4 by 2 | Top artist and minutes so far this year in the year's design pack. Says Your recap builds as you listen until there is data. |

The "Android and Windows" column covers both; the sizes named small, medium and large are Windows.

On Windows the Jump List has Play or pause, Next and Start a mix, with the pinned mixes above them, and the system Recent list is turned off for Encore so it never records what was played. The taskbar thumbnail has Previous, Play or pause and Next.

**Notes**

- **Windows.** The Widgets board takes Adaptive Card widgets from a packaged app, in small, medium and large, so widgets ship in the Store (MSIX) build only. The provider is a small Rust component beside the Flutter app. Third-party lock screen widgets do not exist, and web widgets are limited to the EEA, so neither is used. Progress shows on the taskbar button and in a progress toast.
- **Linux.** MPRIS with full metadata (title, artist, album, art URL, length, position, seek) feeds the GNOME and KDE lock screens and media controls. There are no custom widgets in version 1: GNOME has no widget surface, and KDE's own media widget already reads MPRIS. Progress uses notification hints and launcher entries where the desktop supports them. A Plasma widget can come later if you want one.

**Store and policy checks (Microsoft Store)**

- Microsoft Store: the widget provider is declared in the MSIX manifest; direct builds have no Widgets board entry.
- Not checked in this pass: AudioPlaybackIntent behavior for starting playback from a widget tap on iOS 18 and later, hosting the Windows widget provider from a Flutter and Rust app, the exact privacy manifest reason codes, whether Play treats a download-progress Live Update as eligible, and the Android widget picker cell sizes on each form factor.

**Sources**

- [Microsoft, Windows widgets](https://learn.microsoft.com/windows/apps/design/widgets)

## Section 22: Buttons and states (Windows and Linux)

- **Windows and Linux** use the same rule in the Encore Material 3 Expressive theme, drawn by hand in Flutter at desktop density: flat shape while playing, 8% hover layer, 12% pressed layer with tighter corners, focus ring only for the keyboard.

The rule it follows, from the Android shape rule in section 22:

On Android a button's shape tells the user its state. Play and pause is a toggle whose checked state is "is playing", and like, shuffle and repeat follow the same rule, so every button on a screen tells the same story.

| State | Shape | Play button (72 dp high) |
| --- | --- | --- |
| Unselected or paused | Round (50%) | 36 dp corners, shows play |
| Selected or playing | Rounded square (about 33% of the height) | 24 dp corners, shows pause; the same size and colour as paused |
| Pressed | Tighter again (about 19%) | 14 dp corners, 14% state layer |
| Loading | Round, spinner replaces the glyph | Stays tappable; a tap cancels the load |
| Disabled | No morph, no ripple, no haptic | 12% container, 38% glyph |
| Focused (keyboard, D-pad, Switch Access) | 3 dp ring with a 3 dp gap | Never shown for touch |
| Hovered (mouse, stylus) | 8% state layer, shape unchanged | Tablets, ChromeOS, desktop |

- **Sizes**: Large 112 x 72 (phone Now Playing), Medium 96 x 60 (Lyrics), Bar 52 x 52 (tablet and desktop), Compact 44 x 44 (mini player, with a 48 dp touch target kept). Corner radius for the playing shape is 24, 20, 17 and 15 dp respectively.
- **Transport row**: three separate buttons 8 dp apart (Previous and Next tonal, 88 x 72; Play 112 x 72). Not one joined pill. Previous and Next are shaped by their arrow: the end the arrow points to is fully round (36 dp on a 72 dp button) and the end beside Play is 10 dp, so Previous is round on the left and Next round on the right, flipping with the arrow in right-to-left languages. Pressed, the 10 dp corners tighten to 4 dp. The same rule scales to every size (Lyrics 76 x 60, tablet 76 x 56, desktop 68 x 52). A bare icon button with no container has no visible corners, but its ripple and focus ring use the same shape.
- **Connected button groups** (segmented controls, Play, Shuffle and Add) keep the library's own rule: the selected item is fully round and the others square off toward their neighbours.
- **In code**: use the library's toggleable icon button shapes and set the three shapes once (`shape`, `pressedShape`, `checkedShape`). The library morphs between them when all are rounded-corner shapes, so Encore never animates a radius by hand.

**Not verified.** 24 dp (33%), 50% and 14 dp are Encore tokens. The Compose documentation names shape slots but gives no numbers, so confirm the look against the pinned Compose version before release.

The press timing, the control table, the motion rules and the accessibility rules are in section 22 of PLAN-UX. On Windows and Linux a long press on touch is 500 ms; right-click, the Menu key or Shift+F10 open the context menu. Desktop sizes: Bar 52 x 52, and Previous and Next 68 x 52.

## Section 23: Resizing, view padding and state (Windows and Linux)

The plan has no cut-out or fold device rows for Windows and Linux. Two rules from section 23 apply: read the view padding from the system and never type inset numbers (rule 1), and keep the song, the queue, the scroll position and the selection across every window resize, because the core owns playback and screen state is saved outside the view (rule 8). Every window is resizable and the layout follows the window width. Android's fold postures do not apply.

## Section 24: Production readiness (Windows and Linux)

Work through the whole checklist in section 24 of PLAN-UX. For Windows and Linux it means this:

| Group | What it means on Windows and Linux |
| --- | --- |
| Playback and state | Interruptions: headphone unplug, a Bluetooth switch, a lost network, sleep and wake. SMTC and MPRIS show and control the right song. Window resize never restarts a song |
| Buttons, haptics and motion | Screenshot tests for every control state in the Encore theme; the state-shape rule drawn by hand (flat while playing, 8% hover layer, 12% pressed layer, focus ring for the keyboard only); Reduce Motion, 200% font scale and right to left; Narrator and Orca each complete play, like, queue and profile switch |
| Cut-outs and foldables | No typed inset numbers (a lint rule); state survives every resize |
| Smart picks and year recap | You do not build the engines. Your part: every screen state of boards 38 to 41 and the pack snapshot tests on the extreme histories, drawn in Flutter |
| Swipes and drag and drop | No swipes on desktop. Drag and drop with the mouse, the copy and move modifiers, and a menu, keyboard and screen reader twin for every drop |
| Data, sync and security | Tokens in Credential Manager and the Secret Service, nothing sensitive in logs, the network log of a full session shows only the media server, the companion server and enabled providers |
| Release | Signed, reproducible builds; the Windows installer (winget) and Flathub; the store checklist you write at the start of Phase 13 |

**Unverified items (Windows and Linux)**

| Item | Why it is open | Check against |
| --- | --- | --- |
| Exact Material 3 Expressive corner radii | The library documents shape slots, not numbers; 24 dp, 14 dp and 50% are Encore tokens | The pinned Compose Material 3 version, on a device |

## Section 26: Drag and drop (Windows and Linux)

- **Rule 16: Platform parts.** Apple: SwiftUI `draggable` and `dropDestination` with `Transferable`. Android tablets: Compose `dragAndDropSource` and `dragAndDropTarget`. Windows and Linux: Flutter `Draggable` and `DragTarget`. These names are to be confirmed against the pinned versions. All of them read the same payload type from the core.
- **Rule 14: Not by touch only.** VoiceOver and TalkBack get Next song and Previous song as custom actions on the mini player. Switch users use the buttons. Mac, Windows and Linux have no swipe: they use the buttons, media keys and Space.

You build drag and drop (board 43) with Flutter `Draggable` and `DragTarget`: Ctrl toggles a row, Shift extends a range and moves instead of copying when dropping on another playlist. Check every API name above in the official documentation for the pinned Flutter version before you use it, and say where you found it.

## Spikes to run early (Windows and Linux)

- Spike 1: **Rust core over FFI on all six platforms**: UniFFI to Swift and Kotlin, flutter_rust_bridge to Dart. Phase 1 proves this; if one path fails, stop and report.
- Spike 5: **Flutter Encore theme**: one screen (Now Playing) with the shape rule and wavy progress bar, before porting the rest.
- Spike 6: **Desktop audio**: gapless playback with Symphonia and cpal on Windows and Linux, plus SMTC and MPRIS.
- Spike 8: **Recap parity and speed**: run a stub of the `encore-recap` engine over a synthetic log of 100,000 plays on the server build, an iPhone, an Android phone and a Raspberry Pi 5. The bytes must match and the time must be recorded (plan section 25).
- Spike 9: **Swipe and drag and drop APIs**: one throwaway screen each for the mini player swipe and for dragging songs onto a playlist, on SwiftUI (iPad and Mac), Compose (Android tablet) and Flutter (Windows or Linux). Confirm the API names and the touch and pointer behaviour in plan section 26.

## Hand-offs

- **From the core you consume:** the generated Dart bindings and `docs/core/CORE-API.md`: library and search queries, the queue and its events, the play-event writer, profiles and sync status, smart picks, lyrics, artwork palette, the add-to-playlist call, the per-profile widget snapshot, the recap data and the `encore-audio` engine. If a call you need is not listed, write a request.
- **To the core you give:** requests, and any fact about Windows or Linux behaviour that changes a core rule. `core/encore-audio/` is yours; tell the core track in a request if the engine needs a new call from the queue.
- **To the owner you give:** the list of things only a real machine can check, at the end of every phase.
