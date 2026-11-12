# Encore for Android: track plan

You are the **Android track**. You build the Android app natively in Kotlin and Jetpack Compose with Material 3 Expressive, on top of the shared Rust core. Read `CLAUDE.md`, then `android/CLAUDE.md` once it exists, then `docs/ROADMAP.md`, then this plan.

**You own:** `android/` (the Android Studio project, widgets, Quick Settings tile, Android Auto), `.github/workflows/android.yml`, `docs/android/`.

**You never edit:** `core/`, `server/`, `tools/`, `design/`, `apple/`, `flutter/`, the shared documents. If you need a core call, a token or a decision, write a request in `docs/requests/`.

Section numbers in this plan are the plan's original numbers (`docs/SECTION-MAP.md`). The shared plans are `docs/shared/PLAN-PRODUCT.md` (product, core, server, recommender, recap, privacy) and `docs/shared/PLAN-UX.md` (design system, interaction, screens, buttons, cut-outs, swipes, checklist). Headings that start with "Section N" below hold the part of section N that belongs only to this track. Read the shared section as well.

## 1. What you build

- **Target.** The Android app, native, in `android/` (section 3, repository layout). Flutter is not used on Android: Flutter has said it is not building Material 3 Expressive, so Compose is how Android gets the expressive components (section 15, decided).
- **Minimum system.** Android 9 (section 15 default, `docs/DECISIONS.md`). New releases target Android 16 (API 36) from the first release (section 19).
- **Look.** Material 3 Expressive: the expressive motion scheme, connected button groups, a shape set for generated covers (cookie, sunny, clover, arch, squircle), wavy progress, shape morphing, edge-to-edge, predictive back and haptics. Same structure and flows as iOS but not a copy of its look. On tablets and foldables: a navigation rail and expanded rail, list and detail panes with a supporting pane, fold postures and free resizing. On low-end phones swap live blur for tinted translucent fills.
- **Surfaces beyond the app window.** The MediaSession notification and lock screen player, Android Auto, Glance home screen and lock screen widgets, a Quick Settings tile, Live Updates for the sleep timer and downloads, Bluetooth controls (sections 8 and 20).
- **What you do not build.** Sources, the queue, sync, smart picks, the recap engine and the server are in the core. You call the generated Kotlin bindings and draw what they return.

**Stack for this track (section 3)**

| Layer | Technology | Why |
| --- | --- | --- |
| Other UI | Windows and Linux: Flutter (stable), Riverpod, go\_router, with a custom Material 3 Expressive theme. Android: Kotlin, Jetpack Compose, Material 3 Expressive | One Flutter codebase for Windows and Linux; Android is native so it gets the expressive components. Flutter has none, so the Encore theme draws the shapes, connected button groups and wavy progress itself |
| Android audio | Media3 ExoPlayer + MediaSessionService | Background play, Android Auto |
| On-device ML | Core ML on Apple, ONNX Runtime elsewhere | Uses the Neural Engine or NPU when present |
| CI | GitHub Actions: macOS runner for Apple, Linux and Windows runners | Every PR builds every target |

The "Other UI" row also describes the Windows and Linux Flutter app, which is another track. Your part of it is: Kotlin, Jetpack Compose, Material 3 Expressive.

**Fonts:** SF Pro may only be used on Apple platforms. Use the system font on Apple devices and bundle Inter (SIL Open Font License) on Windows, Linux and Android to get the same clean look.

**Compiling the core.** You build the Rust core for the Android targets inside `android/` (the plan names UniFFI for the Kotlin bindings). The Rust source stays in `core/`. Phase 1 proves this path; if it fails, stop and say which step and why (`docs/DECISIONS.md` section 6, spike 1). The native libraries must also be built for 16 KB page-size alignment: that is listed as not checked in section 19 and must be checked before release.

## 2. What you need

- Android Studio with Kotlin and Compose, and the Rust toolchain. Any of the owner's computers can run it.
- A real Android phone (haptics and motion cannot be judged in an emulator), ideally a second one with a different cut-out, and a foldable if the owner has one. The emulator's foldable and cut-out profiles stand in where a device is missing; mark those test rows as simulated.
- Google Play Console (one-time fee). A new personal account must run a closed test with at least 12 testers for 14 consecutive days before production, so recruit testers early (section 19, `docs/DECISIONS.md` section 7).
- A Jellyfin server the core track has already tested against.

## 3. Your phases

The phase numbers and the master “done when” lines are the plan's. The master roadmap built Android inside Phase 6; here you follow Phases 2 to 5 in the same order as the other tracks, as soon as the core part of each exists, and the Phase 6 list (Media3, cut-outs and fold postures, swipes, tablet drag and drop) is part of those phases. The core track builds the core part of each phase one step ahead of you: `git pull` first and read `docs/core/CORE-API.md`. Where a master line says “every platform” or names iPhone and Mac, read it as your platform. The owner checks the done-when line on a real device.

| Phase | Your part | Master "done when" |
| --- | --- | --- |
| 1 Foundations | Android Studio project; builds the core for Android; hello-world screen from the core; CI | every platform shows a screen whose text comes from the Rust core, and CI is green. |
| 2 Jellyfin and playback | The same on Android with Media3 and a MediaSession (master Phase 6) | you can sign in, browse your whole library offline-fast, and listen to an album gaplessly. |
| 3 Apple Music-grade UI and lyrics | The same screens in Compose with Material 3 Expressive, cut-outs and fold postures, swipes, tablet drag and drop (master Phase 6) | side by side with Apple Music, the screens feel equally smooth at 120 Hz, and lyrics sync word by word. |
| 4 Server, profiles and sync | The same | two people share an iPad with fully separate history, and a like on the phone shows on the Mac within a second. |
| 5 Recommendations v1 | The same | autoplay's early-skip rate is clearly lower than shuffle on your own history, and the smart-picks items in section 24 pass. |
| 6 Android, Windows, Linux | Phases 2 to 5 for Android, with Media3, cut-outs and folds | the same account works on all six platforms and the UI passes the same visual checklist. |
| 7 Handoff and remote control | Output picker, pull banner | phone-to-desktop transfer starts in under 1 second for cached tracks. |
| 8 Offline and car | Downloads screens, Android Auto, progress for user-started downloads | a downloaded playlist plays in airplane mode and in a car. |
| 9 Widgets, controls and live surfaces | Glance widgets, Quick Settings tile, Live Updates | a widget set to one profile never shows another profile's history, the locked screen shows nothing from the listening history, the sleep timer ends with the fade-out, and no widget or Live Activity makes a network call. |
| 10 Recommendations v2 | ONNX Runtime with NNAPI or QNN | v2 beats v1's early-skip rate in an offline replay test. |
| 11 Year-end recap | The same in Compose | two test profiles with different histories get visibly different recaps, a re-run for next year picks a different pack, the same log gives byte-identical recap data on the server and on every platform, and late plays synced before January 8 appear in the final recap. |
| 13 Release | Play Store and F-Droid | The section 24 checklist is ticked for every platform. |

**Shared sections to read.** In PLAN-PRODUCT: 1 to 8, 10 to 12, 15, 19, 21 and 25 (sections 4, 5, 7 and 25 describe what the core hands you; you rarely build them). In PLAN-UX: 9, 16, 17, 20, 22, 23, 24 and 26. Read sections 1, 2 and 3 first, and the others when the phase you are on touches them.

## 4. Your boards

The phone boards draw iOS (Liquid Glass) on the left and Android on the right. Build the right side. The Android tablet layouts are board 29. Boards 42 and 43 draw swipes and drag and drop for several platforms; take the Android phone and tablet parts.

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
| 29 | Android tablet layouts | 9 | [png](../../design/wireframes/png/29-android-tablet-layouts.png) | [spec](../../design/wireframes/specs/29-android-tablet-layouts.md) |
| 31 | Live surfaces and rules | 20 | [png](../../design/wireframes/png/31-live-surfaces-and-rules.png) | [spec](../../design/wireframes/specs/31-live-surfaces-and-rules.md) |
| 32 | Lock screen and Live Activities | 20 | [png](../../design/wireframes/png/32-lock-screen-and-live-activities.png) | [spec](../../design/wireframes/specs/32-lock-screen-and-live-activities.md) |
| 34 | Widgets and media surfaces on Android, Windows and Linux | 20 | [png](../../design/wireframes/png/34-widgets-and-media-surfaces-on-android-windows-and-linux.png) | [spec](../../design/wireframes/specs/34-widgets-and-media-surfaces-on-android-windows-and-linux.md) |
| 35 | Buttons, states and haptics | 22 | [png](../../design/wireframes/png/35-buttons-states-and-haptics.png) | [spec](../../design/wireframes/specs/35-buttons-states-and-haptics.md) |
| 36 | Notch, Dynamic Island, cutouts and foldables | 23 | [png](../../design/wireframes/png/36-notch-dynamic-island-cutouts-and-foldables.png) | [spec](../../design/wireframes/specs/36-notch-dynamic-island-cutouts-and-foldables.md) |
| 37 | Smart picks: signals and rules | 21 | [png](../../design/wireframes/png/37-smart-picks-signals-and-rules.png) | [spec](../../design/wireframes/specs/37-smart-picks-signals-and-rules.md) |
| 38 | Smart picks: screens and states | 21 | [png](../../design/wireframes/png/38-smart-picks-screens-and-states.png) | [spec](../../design/wireframes/specs/38-smart-picks-screens-and-states.md) |
| 39 | Year recap: the story | 10, 25 | [png](../../design/wireframes/png/39-year-recap-the-story.png) | [spec](../../design/wireframes/specs/39-year-recap-the-story.md) |
| 40 | Year recap: packs and years | 10, 25 | [png](../../design/wireframes/png/40-year-recap-packs-and-years.png) | [spec](../../design/wireframes/specs/40-year-recap-packs-and-years.md) |
| 41 | Year recap: devices and states | 10, 25 | [png](../../design/wireframes/png/41-year-recap-devices-and-states.png) | [spec](../../design/wireframes/specs/41-year-recap-devices-and-states.md) |
| 42 | Swipe gestures: mini player, Now Playing and rows | 16, 26 | [png](../../design/wireframes/png/42-swipe-gestures-mini-player-now-playing-and-rows.png) | [spec](../../design/wireframes/specs/42-swipe-gestures-mini-player-now-playing-and-rows.md) |
| 43 | Drag and drop on desktop and tablets | 16, 26 | [png](../../design/wireframes/png/43-drag-and-drop-on-desktop-and-tablets.png) | [spec](../../design/wireframes/specs/43-drag-and-drop-on-desktop-and-tablets.md) |

## Section 8: Playback engine: operating-system integration (Android)

| Platform | Must support |
| --- | --- |
| Android | MediaSession notification and lock screen player, Android Auto, Glance home screen and lock screen widgets, a Quick Settings tile, Live Updates for the sleep timer and downloads (section 20), Bluetooth controls |

Section 2 adds the audio path: Media3 ExoPlayer with a `MediaSessionService`, called straight from the Kotlin Compose app. The core owns the queue and tells the native player what to load next.

## Section 9: Design language (Android row)

| Platform | Approach |
| --- | --- |
| Android | Jetpack Compose with Material 3 Expressive (expressive motion scheme, connected button groups, a shape set for generated covers (cookie, sunny, clover, arch, squircle), wavy progress, shape morphing), edge-to-edge, predictive back, haptics; same structure and flows as iOS but not a copy of its look; on tablets and foldables a navigation rail and expanded rail, list and detail panes with a supporting pane, fold postures and free resizing; on low-end phones swap live blur for tinted translucent fills |

## Section 16: Haptics and motion on Android

- Android takes its values from `MotionScheme.expressive()` (fast, default and slow spatial and effects springs), with shape morphing on press, wavy progress while playing and container transforms for the player.

| Event | Android |
| --- | --- |
| Play, pause, skip | VIRTUAL\_KEY |
| Like on / off | TOGGLE\_ON / TOGGLE\_OFF |
| Long press opens menu | LONG\_PRESS |
| Scrub start / end | GESTURE\_START / GESTURE\_END |
| Slider detents | SEGMENT\_FREQUENT\_TICK |
| Reorder pick up / drop | DRAG\_START / GESTURE\_END |
| Swipe action armed / disarmed | GESTURE\_THRESHOLD\_ACTIVATE / DEACTIVATE |
| Mini player or artwork swipe armed | GESTURE\_THRESHOLD\_ACTIVATE |
| Profile switched, handoff done | CONFIRM |
| Blocked or failed action | REJECT |

Use `HapticFeedbackConstantsCompat` on Android so constants newer than the device's API fall back to the nearest older effect.

## Section 18: Libraries (Android)

| Library | Used for | License |
| --- | --- | --- |
| Media3 ExoPlayer | Android playback | Apache-2.0 |
| Jetpack Compose and Material 3 | Android UI, Material 3 Expressive | Apache-2.0 |
| Inter | Type on Android, Windows and Linux | SIL OFL 1.1 |
| Material Symbols | Icons on Android, Windows and Linux | Apache-2.0 |

Shared libraries (UniFFI, ONNX Runtime and the others) are in section 18 of PLAN-PRODUCT. Check each licence again when you add a library, and keep `THIRD_PARTY_LICENSES` current.

## Section 19: Store requirements (Google Play and Android)

**Build and platform deadlines**

| Store | Requirement | Date | What the plan does |
| --- | --- | --- | --- |
| Google | New apps and updates target Android 16 (API 36) | 31 Aug 2026; an extension to 1 Nov 2026 could be requested | Target API 36 from the first release |
| Google | New personal developer accounts (created after 13 Nov 2023) run a closed test with at least 12 testers opted in for 14 consecutive days before production | Now | Recruit testers before the first Play release and give them the demo library |
| Google | Declare the foreground service type for media playback in Play Console (Policy, App content) | Apps targeting Android 14 and up | Playback runs in a Media3 session service with type mediaPlayback and the FOREGROUND\_SERVICE\_MEDIA\_PLAYBACK permission |
| Google | Android developer verification: apps distributed outside Play must be registered; limited-distribution accounts (up to 20 devices, no ID or fee) exist | Enforced from 30 Sep 2026 in Brazil, Indonesia, Singapore and Thailand; global from 2027 | Register the package name; note it in the README for F-Droid and GitHub APK users |

**Android 16 behavior when targeting API 36**

- Edge-to-edge cannot be turned off, so every screen draws behind the system bars and applies insets.
- The predictive back gesture is on by default. Use Compose back handlers, never `onBackPressed`.
- On screens 600 dp wide or more, orientation, resizability and aspect-ratio locks are ignored. Every layout must work at any size and rotation, which is why the canvas has a Size classes board and one board each for Mac, iPad, Android tablet, and Windows and Linux. iPad needs the same: all orientations, resizable windows, Split View.

**Networking and encryption**

- **Android** blocks cleartext traffic by default for current targets. Allow it in the network security config and enforce the same rule in app code, so plain HTTP is limited to home-network addresses on every platform.
- **Android 17 local network permission.** ACCESS\_LOCAL\_NETWORK becomes a runtime permission, enforced for apps targeting API 37, and it covers raw sockets, NsdManager, OkHttp and Cronet. Apps targeting API 36 keep implicit access through INTERNET. Plan: request it behind the primer when the app moves to API 37, and use the system picker for service discovery where it fits.

**Permissions (Android)**

- **Permissions.** Only the ones on the Permissions board, each asked in context after a primer, none at launch, none blocking a feature. iOS purpose strings: NSLocalNetworkUsageDescription (plus NSBonjourServices if the companion server advertises itself) and NSFaceIDUsageDescription. Android: FOREGROUND\_SERVICE\_MEDIA\_PLAYBACK, USE\_BIOMETRIC, optional POST\_NOTIFICATIONS (asked after the board 25 primer when the first download starts), RUN\_USER\_INITIATED\_JOBS (install-time, never a prompt: downloads the person starts, Android 14 and later; approved by the owner on 8 November 2026, Phase 8), POST\_PROMOTED\_NOTIFICATIONS (install-time, never a prompt: Live Updates for the sleep timer and downloads the person starts, Android 16 QPR1 and later; approved by the owner on 11 November 2026, Phase 9), and ACCESS\_LOCAL\_NETWORK once the app targets API 37. Nothing is tracked, so there is no App Tracking Transparency prompt, and sharing goes through the system share sheet, so there is no photo library permission.

**Before each submission (items that involve Google or every store)**

- [ ] Privacy policy page live and linked in App Store Connect, Play Console and the app
- [ ] Deletion web page live and its URL entered in the Play Data safety form
- [ ] Apple privacy label and Play Data safety answers match the "What this app connects to" list
- [ ] Android manifest reviewed: permissions, media playback service type, target API 36; Play foreground service declaration filled in
- [ ] Apple age rating and Google IARC questionnaires answered
- [ ] Review notes: demo server address and credentials, how to open the Demo library, why there is no account with the app maker; Play App access filled in
- [ ] Screenshots from real builds on each device class, with no Apple Music marks
- [ ] Name, subtitle and keywords checked for Jellyfin, Apple marks and trademark conflicts
- [ ] Play closed test finished: 12 testers, 14 consecutive days
- [ ] EU trader status declared; Android developer verification registered
- [ ] THIRD\_PARTY\_LICENSES matches the About screen, including demo track credits

**Not checked in this pass.** Apple's accessibility labels for the App Store page, Play's exact Data safety wording for self-hosted servers, how App Review treats a justification for `NSAllowsArbitraryLoads`, 16 KB page-size alignment for the Rust and ONNX native libraries on Android, and trademark clearance for "Encore". Check them before the first submission.

**Sources**

- [Google Play, target API level requirements](https://support.google.com/googleplay/android-developer/answer/11926878)
- [Google Play, testing requirements for new personal accounts](https://support.google.com/googleplay/android-developer/answer/14151465)
- [Google Play, account deletion requirements](https://support.google.com/googleplay/android-developer/answer/13327111)
- [Android, foreground service types](https://developer.android.com/develop/background-work/services/fgs/service-types)
- [Android, local network permission](https://developer.android.com/privacy-and-security/local-network-permission)
- [Android 16 behavior changes](https://developer.android.com/about/versions/16/behavior-changes-16)
- [Android developer verification](https://developer.android.com/developer-verification)

## Section 20: Widgets, Live Updates and the lock screen (Android)

Read section 20 in PLAN-UX first: the four rules, the decisions, the sleep timer, the widgets and the settings page apply to you. These are your column and the parts that belong only to Android.

**What each platform gets (Android)**

| Surface | Android |
| --- | --- |
| Player on the lock screen, drawn by the system | Lock screen, quick settings, shade |
| Timed tasks: sleep timer, user-started downloads, first sync | Live Update (Android 16 QPR1 and later), else an ongoing notification |
| Home widgets | Home screen (Glance) |
| Lock screen widgets | Phones from Android 16 QPR1, Pixel Tablet |
| Quick controls | Quick Settings tile |

Comes with no extra work: an iPhone Live Activity also appears in the Mac menu bar, on the CarPlay Dashboard and in the Apple Watch Smart Stack, and a small widget works in StandBy and CarPlay.

**Widgets (Android sizes)**

| Widget | Android and Windows | What it does |
| --- | --- | --- |
| Now Playing | 2 by 1 up to 6 by 2 (Android), small (Windows) | Cover, title, artist, progress, Play or pause and Next. Tapping the cover opens Now Playing. Shows the last played song with a Play button when nothing is playing. |
| Mixes | 4 by 2 (Android), medium (Windows) | Each tile starts its mix with one tap. Configurable: pick a mix or follow Next up. |
| Recently played | 4 by 3 (Android), large (Windows) | Tapping an album opens it. History, so redacted while locked and off the lock screen. |
| Recap | Android only, 2 by 2 and 4 by 2 | Top artist and minutes so far this year in the year's design pack. Says Your recap builds as you listen until there is data. |

The "Android and Windows" column covers both; sizes written "2 by 1 up to 6 by 2" and the like are Android. On Android the Quick Settings tile is Start a mix.

**Notes**

- **Android.** The player comes from the Media3 session. Sleep timer and user-started downloads use a Live Update when the app can post one (permission POST\_PROMOTED\_NOTIFICATIONS, a title, ongoing, no custom views, not colorized, progress style), with a short chip text such as 24m, and fall back to a normal ongoing notification. Downloads run as user-initiated data transfer jobs. Widgets are built with Jetpack Glance and Material 3 Expressive shapes and colors, in resizable cells. Every widget is allowed on the lock screen unless it declares the not\_keyguard category, so the history widgets declare it and the Now Playing widget does not.

**Widget build rules (redesign, 12 November 2026, ADR android-0006)**

- Widgets wear the album's colors: the field is a small bitmap made at each size bucket's own shape from the palette in the snapshot (`FieldMath`, `WidgetField`), with `CoverPalette` as a stand-in until the core has made one. No progress bar: the playing state is the play button's shape and the level bars.
- Every size is a bucket in `WidgetSizing.kt` (Now Playing 14, Mixes 12, Recently played 11), sized from its content and the font scale; `WidgetLayoutTest` proves each fits at font scales 1.0 to 2.0 and `WidgetGalleryTest` hosts every bucket on the emulators and fails on cropping, buttons under 48 dp, unrounded corners or a stretched field. Now Playing is capped at 260 dp high.
- Redraw each placed widget through its own receiver (`WidgetRefresh`), never `updateAll` per class, and keep the Glance classes out of R8's class merging (`proguard-rules.pro`, `WidgetKeepRulesTest`): the release build merged the three widgets and one drew another's content. Emulator tests run the debug build and cannot see that.
- Debug builds show the sizes Android reports for each placed widget on Settings > Lock screen and widgets (`WidgetSizeLog`). One UI's cells are 311, 326 and 418 dp wide on the S20 FE and A13, not Google's table.

**Limits to build against (Android)**

- Android chips are at most 96 dp wide and show the whole text under 7 characters. Tapping a widget that opens an activity from the lock screen asks the person to unlock first unless the activity is marked showWhenLocked, which Encore's main activity is not.

**Store and policy checks (Google Play)**

- Google Play: POST\_PROMOTED\_NOTIFICATIONS only if Live Updates ship; foreground service types and data transfer jobs declared; unwanted Live Updates make people revoke the permission, so each one starts from something the person did.
- Not checked in this pass: AudioPlaybackIntent behavior for starting playback from a widget tap on iOS 18 and later, hosting the Windows widget provider from a Flutter and Rust app, the exact privacy manifest reason codes, whether Play treats a download-progress Live Update as eligible, and the Android widget picker cell sizes on each form factor.

**Sources**

- [Android, Live Update notifications](https://developer.android.com/develop/ui/views/notifications/live-update)
- [Android, Widgets on the lock screen FAQ](https://android-developers.googleblog.com/2025/03/widgets-on-lock-screen-faq.html)

## Section 22: Buttons, states and haptics (Android)

### The Android shape rule

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

The press timing, the control table, the motion rules and the accessibility rules are in section 22 of PLAN-UX. The haptics for the controls, Android column:

| Event | Android |
| --- | --- |
| Play, pause, next, previous | VIRTUAL\_KEY |
| Like on, like off, shuffle, repeat | TOGGLE\_ON, TOGGLE\_OFF |
| Previous or next scan tick (each second) | SEGMENT\_FREQUENT\_TICK |
| Scrub start and end | GESTURE\_START, GESTURE\_END |
| Long press opens menu | LONG\_PRESS |
| Swipe action armed | GESTURE\_THRESHOLD\_ACTIVATE |
| Sleep timer set | CONFIRM |
| Destructive confirm | CONFIRM |
| Blocked action | REJECT |

## Section 23: Cut-outs and foldables (Android)

**Cut-outs by device**

| Device | What to handle | Rule |
| --- | --- | --- |
| Android centre punch-hole | Status bar about 24 to 40 dp | Chevron and More sit below the status bar inset |
| Android corner hole | Top centre is free, the corner is not | Keep controls out of the corner |
| Android waterfall edges | Curved sides | A side inset of 6 to 12 dp keeps controls off the curve |
| Android landscape | The cutout inset appears only on the side where it sits | Do not mirror it |

**Foldables**

| Device | Closed | Open | Half open |
| --- | --- | --- | --- |
| Samsung Galaxy Z Fold | Cover screen, compact layout | Inner screen, medium or expanded width, two panes | Flex mode: content above, controls below the crease |
| Samsung Galaxy Z Flip | Cover screen: system player and widgets | Tall phone, compact layout | Flex mode: content above, controls below |
| Pixel Fold and similar | Outer screen, compact layout | Inner screen, expanded, two panes | Tabletop (horizontal hinge) or book (vertical hinge) |

**Your rows of the test matrix (section 23).** A Pixel, a Samsung with a centre hole and one with a corner hole, a Galaxy Z Fold, a Galaxy Z Flip, a Pixel Fold, an Android tablet, landscape, split screen, 200% font and right to left. Where a device is not at hand, use the Android emulator's foldable and cutout profiles and mark the row as simulated.

**Sources**

Checked on Nov 5, 2026.

- [Android Developers: Display cutouts](https://developer.android.com/develop/ui/views/layout/display-cutout): edge-to-edge first, `WindowInsetsCompat.getDisplayCutout()`, never hardcode the status bar height, keep fine-touch controls out of the cutout.
- [Android Developers: Make your app fold aware](https://developer.android.com/develop/ui/compose/layouts/adaptive/foldables/make-your-app-fold-aware): `FoldingFeature` state, orientation, occlusion type, `isSeparating` and bounds; tabletop and book postures; keep controls away from the fold.
- [Samsung: Designing for foldables](https://developer.samsung.com/one-ui/largescreen-and-foldable/designing_for_foldable.html) and [Samsung: Flex mode](https://developer.samsung.com/galaxy-z/flex-mode.html): content on the top half, controls on the bottom half, a visual transition into and out of Flex mode, nothing interactive on the crease, resume where the user left off, keep scroll position, no letterboxing, resizable apps.
- Android 16 large-screen behaviour is sourced in section 19 (behavior changes for Android 16).
- [Kotlin docs: IconButtonDefaults.toggleableShapes (Material 3 API reference)](https://kotlinlang.org/api/compose-multiplatform/material3/androidx.compose.material3/-icon-button-defaults/toggleable-shapes.html): `shape`, `pressedShape`, `checkedShape`, and morphing only between corner-based shapes.

## Section 24: Production readiness (Android)

Work through the whole checklist in section 24 of PLAN-UX. For Android it means this:

| Group | What it means on Android |
| --- | --- |
| Playback and state | Interruptions: phone call, assistant, headphone unplug, Bluetooth switch, audio focus loss, a lost network. Fold, unfold, rotate, split screen and window resize never restart a song |
| Buttons, haptics and motion | Screenshot tests for every control state; **no pill-shaped pause button** on phone, tablet, lock screen or widgets; shapes and radii checked against the pinned Compose Material 3 Expressive version; haptics tried on a real phone; Reduce Motion, 200% font and right to left; TalkBack completes play, like, queue and profile switch |
| Cut-outs and foldables | No typed inset numbers (a lint rule); nothing tappable crosses a fold in flat, tabletop and book postures; the Android rows of the test matrix |
| Smart picks and year recap | You do not build the engines. Your part: every screen state of boards 38 to 41 and the pack snapshot tests on the extreme histories, drawn in Compose |
| Swipes and drag and drop | Mini player, artwork and row swipes; drag and drop on tablets; TalkBack custom actions Next song and Previous song |
| Data, sync and security | Tokens in the Keystore, nothing sensitive in logs, the network log of a full session shows only the media server, the companion server and enabled providers |
| Release | Signed, reproducible builds, Play internal and closed testing, staged rollout; the Google items of section 19 above |

**Unverified items (Android)**

| Item | Why it is open | Check against |
| --- | --- | --- |
| Exact Material 3 Expressive corner radii | The library documents shape slots, not numbers; 24 dp, 14 dp and 50% are Encore tokens | The pinned Compose Material 3 version, on a device |
| Samsung Flex mode details | Flex mode is sourced from Samsung's pages; Samsung gives no Flip cover-screen app rules | A real Fold and a real Flip, and Samsung's developer pages |

## Section 26: Gestures and drag and drop (Android)

- **Rule 16: Platform parts.** Apple: SwiftUI `draggable` and `dropDestination` with `Transferable`. Android tablets: Compose `dragAndDropSource` and `dragAndDropTarget`. Windows and Linux: Flutter `Draggable` and `DragTarget`. These names are to be confirmed against the pinned versions. All of them read the same payload type from the core.
- **Rule 14: Not by touch only.** VoiceOver and TalkBack get Next song and Previous song as custom actions on the mini player. Switch users use the buttons. Mac, Windows and Linux have no swipe: they use the buttons, media keys and Space.

On Android phones you build the mini player and artwork swipes and the row swipes (board 42), with the edge zone left to predictive back. Drag and drop is for Android tablets (board 43). Check every API name above in the official documentation for the pinned Compose version before you use it, and say where you found it.

## Spikes to run early (Android)

- Spike 1: **Rust core over FFI on all six platforms**: UniFFI to Swift and Kotlin, flutter_rust_bridge to Dart. Phase 1 proves this; if one path fails, stop and report.
- Spike 4: **Material 3 Expressive in Compose**: check the toggleable shape and connected button group APIs in the pinned version, and update the Encore tokens if the numbers differ.
- Spike 8: **Recap parity and speed**: run a stub of the `encore-recap` engine over a synthetic log of 100,000 plays on the server build, an iPhone, an Android phone and a Raspberry Pi 5. The bytes must match and the time must be recorded (plan section 25).
- Spike 9: **Swipe and drag and drop APIs**: one throwaway screen each for the mini player swipe and for dragging songs onto a playlist, on SwiftUI (iPad and Mac), Compose (Android tablet) and Flutter (Windows or Linux). Confirm the API names and the touch and pointer behaviour in plan section 26.

## Hand-offs

- **From the core you consume:** the generated Kotlin bindings and `docs/core/CORE-API.md`: library and search queries, the queue and its events, the play-event writer, profiles and sync status, smart picks, lyrics, artwork palette, the add-to-playlist call, the per-profile widget snapshot and the recap data. If a call you need is not listed, write a request.
- **To the core you give:** requests, and any fact about Android's behaviour that changes a core rule (for example, how long a user-initiated data transfer job may run).
- **To the owner you give:** the list of things only a real device or an account can check, at the end of every phase.
