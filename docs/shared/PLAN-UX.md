# Encore: design and interaction plan

Exported 5 November 2026 from the planning document and split by track the same day. This file is shared by the Apple, Android and Windows and Linux tracks, and by the core track where the core has to feed a screen. It holds the design system, the interaction rules, the screen inventory, the shared button, cut-out and live-surface rules, the production-readiness checklist and the swipe and drag and drop spec.

Section numbers are the plan's original numbers. `docs/SECTION-MAP.md` says which file holds each section. The parts that belong to one platform (the Android shape rule, Apple's button behaviour, the devices in the cut-out and foldable tables, the live-surface limits and store checks) are in that platform's plan under the same section number.

The wireframes in `design/wireframes/` are the source of truth for structure, spacing, behaviour, states and haptics. Wherever this file says "the canvas", read them.

## 9. Design system and UI

The target is the current Apple Music (iOS 26 Liquid Glass, plus the iOS 27 changes): content first, artwork-driven color, glass controls that float above content, and spring motion everywhere. On iPhone, iPad and Mac use the real system components with Liquid Glass; elsewhere, rebuild the same structure natively: Jetpack Compose with Material 3 Expressive on Android, and on Windows and Linux Flutter with an Encore theme in the same Material 3 Expressive look, drawn at desktop density.

**Wireframes.** Every phone screen is drawn for iOS (Liquid Glass) and Android (Material 3 Expressive) on the Encore Wireframes design canvas, with the behavior of each control written beside it. Because every window is resizable, tablets and desktop windows have their own boards: Size classes and platform rules, then Mac and iPad (Liquid Glass), Android tablet, and Windows and Linux (Material 3 Expressive). Sections 16 to 18 below summarize its rules, list its screens and name the free libraries to use, section 19 lists what the App Store and Google Play check before they accept the app, and section 20 covers widgets, Live Activities and the lock screen, section 21 the logic behind every smart pick, section 22 buttons, states and haptics, section 23 notches, Dynamic Islands and foldables, section 24 what must be true before release, section 25 the year recap engine and look, and section 26 swipe gestures and drag and drop.

**Principles every screen must follow**

1. Artwork is the hero. Big, crisp, rounded, and it sets the color of the screen around it.
2. Controls float above content, never in heavy bars: glass on iPhone, iPad and Mac, tonal Expressive surfaces on Android, Windows and Linux.
3. Every transition is a spring and is interruptible; nothing just appears.
4. Typography does the hierarchy: bold large titles, few weights, generous spacing on an 8-point grid.
5. No dead ends: every empty state offers an action ("Connect a server", "Play something to train your mixes").
6. Under 100 ms to visible feedback for every tap; the next screen draws from the local library mirror, not the network.

**Navigation**

- iPhone: a floating glass tab bar with Home, Mixes, Library and Search, and the mini player floating just above it. Android phones: a Material 3 Expressive navigation bar with the same four destinations and the mini player above it.
- iPad, Mac, Android tablets, Windows and Linux: from 600 wide the tab bar becomes a floating glass sidebar on Apple devices and a navigation rail on the others (80 wide, growing to a 240 wide expanded rail from 840), with Home, Mixes, Library, Search, Pinned mixes and playlists, and pages gain panes. The layout follows the window width in four size classes (Compact under 600, Medium 600 to 839, Expanded 840 to 1199, Large 1200 and up), never the device type; the Size classes board has the details.

**Key screens**

| Screen | What it contains |
| --- | --- |
| Home | Greeting, up to six personal mixes as large artwork tiles, Recently Played, Rediscover, Deep Cuts, New in your library, time capsules |
| Mixes | Radio from anything, mood and activity mixes, Familiar to Adventurous control |
| Library | Playlists, Artists, Albums, Songs, Genres, Downloaded; sort, filter, and grid or list views |
| Album | Large artwork, artwork-tinted background, prominent Play and Shuffle, track list, "More by" row |
| Artist | Full-bleed artist image that fades into the page (iOS 27 style), prominent Shuffle, top songs, albums, appears on, similar artists in your library |
| Search | Instant local results as you type, grouped by type, recent searches per profile |
| Now Playing | See below |
| Queue | History, Up Next and Autoplay sections, drag to reorder, swipe to remove, autoplay toggle |
| Settings | Profiles, servers, quality, downloads, privacy controls, recommendation controls, lyrics providers |

**Now Playing**

- Background: a slowly moving mesh gradient built from the artwork's palette, behind a soft blur. It shifts gently with the music's energy if the user allows.
- Artwork shrinks slightly with a spring when paused and grows back on play.
- Opens from the mini player with the artwork flying into place (matched geometry); drag down to dismiss with a live, interruptible spring.
- Controls: scrubber with elapsed and remaining time, play/pause, previous, next, volume, and buttons for lyrics, output device (AirPlay, Bluetooth) and queue. Like and "Why this song?" live in the overflow menu.
- Landscape on phones and wide windows: artwork on the left, lyrics on the right, as in iOS 27.

**Lyrics view (the signature feature)**

- Large bold type, left-aligned. The current line is full brightness; upcoming lines are dimmer; past lines fade and blur slightly.
- Word-by-word sync: each word fills with a soft light sweep as it is sung, with a tiny lift. Lines with only line timing highlight as a whole.
- Duets: the second singer's lines align right. Background vocals appear smaller under the main line.
- Instrumental gaps show three dots that breathe in time and fill as the next line approaches.
- Lines move with a staggered spring, so the column flows like a wave rather than jumping.
- Tap any line to jump there. Scrolling by hand pauses auto-scroll until the user stops for 3 seconds.
- Full-screen lyrics mode hides the controls after a few seconds of no touch.
- Share a lyric: select lines and export an image card tinted with the artwork colors.
- Later: translation and pronunciation lines under the original, using on-device translation (Apple's Translation framework on Apple devices).

**Motion tokens**

- Default spring: response 0.4 s, damping 0.85. Snappy spring (taps, toggles): response 0.25 s, damping 0.9. Hero spring (Now Playing open and close): response 0.5 s, damping 0.82.
- Run at 120 Hz where the screen supports it. Respect Reduce Motion (cross-fades instead of flights) and Reduce Transparency (solid surfaces instead of glass).

**Per-platform adaptation**

| Platform | Approach |
| --- | --- |
| iOS and iPadOS 26+ | Native SwiftUI with Liquid Glass (`glassEffect`, glass tab bar, tab bar accessory for the mini player); on iPad a floating glass sidebar, toolbar, inspector pane and floating mini player that follow the window width, with free windows, Split View, Stage Manager, pointer and keyboard |
| iOS 17 to 25 | Same layouts using system materials (`.ultraThinMaterial`) in place of glass |
| macOS | Liquid Glass on macOS 26: floating sidebar, glass toolbar and player bar, a separate Now Playing window, a mini player window, a menu bar item, the full menu bar and keyboard shortcuts |
| Android | Jetpack Compose with Material 3 Expressive (expressive motion scheme, connected button groups, a shape set for generated covers (cookie, sunny, clover, arch, squircle), wavy progress, shape morphing), edge-to-edge, predictive back, haptics; same structure and flows as iOS but not a copy of its look; on tablets and foldables a navigation rail and expanded rail, list and detail panes with a supporting pane, fold postures and free resizing; on low-end phones swap live blur for tinted translucent fills |
| Windows | Flutter with the Encore Material 3 Expressive theme, an optional Mica backdrop on Windows 11, a 48 px title bar with the system caption buttons, Snap Layouts (to verify), a compact-overlay mini player, System Media Transport Controls and taskbar thumbnail buttons |
| Linux | Flutter with the same Encore theme and a header bar that follows the desktop's button side; works on Wayland and X11; MPRIS media controls and an optional tray icon (GNOME needs an extension for tray icons) |

**Design tokens** live in `design/tokens.json` (colors, type scale, spacing, radii, blur strengths, motion curves) and generate Swift and Dart constants, so both UIs stay in step.

**Accessibility**: Dynamic Type and font scaling, VoiceOver and TalkBack labels on every control, full keyboard navigation on desktop, and an automatic contrast check that darkens or lightens artwork-derived colors until text meets WCAG AA.

Reference on the iOS 27 changes: [9to5Mac](https://9to5mac.com/2026/09/18/apple-music-in-ios-27-five-new-features-iphone/), [iDrop News](https://www.idropnews.com/?p=267056).

## 16. Interaction, motion and haptics spec

The Encore Wireframes canvas is the source of truth for each screen. This section keeps the rules that apply everywhere, so Claude Code can follow them without opening the canvas.

**Gesture vocabulary**

| Gesture | Rule | Means |
| --- | --- | --- |
| Tap | Release within 300 ms, under 10 pt (iOS) or 8 dp (Android) of drift | Primary action: play, open, toggle |
| Double tap | Two taps within 250 ms, same spot | Only on Now Playing artwork: like. Never on rows or buttons |
| Long press | Hold about 0.4 to 0.5 s (Android 400 ms, iOS system menu timing; see section 22) | Context menu for that item, with the system menu haptic when it opens |
| Row swipe | A non-destructive action arms at 25%, a destructive one at 60%; release performs it | Leading: Play Next. Trailing: Add to Queue, or Remove where the list is yours (section 26) |
| Player swipe | Direction locks after 12 pt; horizontal arms at 60 pt or a flick faster than 500 pt/s; vertical follows the finger | Left: next song. Right: previous song. Down collapses; up on the mini player expands (section 26) |
| Drag handle | Touch: starts on the handle only | Reorder queue and playlists |
| Drag and drop | Pointer: press a row and move 5 px; touch: long press 300 ms | Add songs to a playlist, Up Next or Liked Songs on Mac, Windows, Linux, iPad and tablets (section 26) |
| Scrub | Drag the bar; vertical drift slows it to 50% or 25% | Fine seek |
| Edge swipe | Starts within 24 pt of the left edge | Back (predictive on Android) |
| Pull down | Armed at 72 pt | Refresh the library |

**Accidental-press rules**

- Every target is at least 44 pt on iOS and 48 dp on Android, with 8 pt between close icon buttons.
- Actions run on release inside the control. Sliding off cancels with no haptic.
- Play, pause, next and previous ignore a second tap for 200 ms.
- A skip is reversible: Previous within 3 s returns to the song just skipped.
- Remove from library, delete playlist, sign out and clear downloads need a confirm sheet with the destructive button apart from Cancel. Removing from a queue or playlist shows an Undo toast for 5 s.
- A touch in the first 150 ms after the screen wakes is ignored.
- While the finger is on the progress bar, edge-back, collapse and tab taps are ignored.
- A long-press menu never triggers an item on release; the first item needs a second tap.
- A blocked action shakes the control 4 pt, fires the warning haptic and shows a one-line reason.
- Likes and playlist edits apply on the device at once and sync later; a failed sync shows a small badge, never a blocking error.

**Motion**

- Both platforms use springs, never fixed-duration easing, and every spring can be interrupted.
- iOS uses system components first: `glassEffect`, `GlassEffectContainer`, `glassEffectID` for menu morphs, `tabBarMinimizeBehavior(.onScrollDown)` and `matchedGeometryEffect` for the mini player to Now Playing flight.
- Android takes its values from `MotionScheme.expressive()` (fast, default and slow spatial and effects springs), with shape morphing on press, wavy progress while playing and container transforms for the player.
- Reduce Motion replaces flights and sweeps with cross-fades. System haptic settings are always respected. Ambient motion such as lyric sweeps and gradient drift never vibrates.

**Haptics**

| Event | iOS | Android |
| --- | --- | --- |
| Play, pause, skip | impact soft | VIRTUAL\_KEY |
| Like on / off | impact medium / light | TOGGLE\_ON / TOGGLE\_OFF |
| Long press opens menu | system menu haptic | LONG\_PRESS |
| Scrub start / end | selection / impact light | GESTURE\_START / GESTURE\_END |
| Slider detents | selection | SEGMENT\_FREQUENT\_TICK |
| Reorder pick up / drop | impact medium / light | DRAG\_START / GESTURE\_END |
| Swipe action armed / disarmed | impact rigid / selection | GESTURE\_THRESHOLD\_ACTIVATE / DEACTIVATE |
| Mini player or artwork swipe armed | impact soft (one tick, none on release) | GESTURE\_THRESHOLD\_ACTIVATE |
| Profile switched, handoff done | notification success | CONFIRM |
| Blocked or failed action | notification warning or error | REJECT |

Use `HapticFeedbackConstantsCompat` on Android so constants newer than the device's API fall back to the nearest older effect.

Section 22 holds the button shapes, press timing and the haptics for each control. Where it differs from this table, section 22 wins.

**Album gradients.** Sample the artwork at 64 by 64 pixels, cluster into five colors in OKLab, keep four stops, and clamp each so text keeps a 4.5:1 contrast ratio (light artwork flips to a dark scheme). Draw 4 to 6 blurred blobs that drift on a 40 to 70 second loop, and cross-fade over 600 ms when the track changes. Album and playlist pages use the full-bleed artwork with one tint, which is how Apple Music does it. On iOS use `MeshGradient`; on Android use an AGSL `RuntimeShader` (API 33 and up) with a tinted fill below that. Fall back to a still gradient for Reduce Motion, Reduce Transparency and battery saver.

## 17. Screen inventory

| Screen | What it covers |
| --- | --- |
| Home | Shelves, recap entry, account avatar |
| Mixes tab | Five daily mixes, pinned mixes, shelves |
| Made for you playlist | Mosaic cover, reason per song, pin, refresh, why these songs |
| Library | Sections, filters, downloads, settings entry |
| Search | Recents, live results, list freeze on touch |
| Album | Tinted page, play and shuffle, track rows |
| Now Playing | Artwork, gradient, transport, scrub, output |
| Lyrics | Word-by-word sweep, tap a line to seek |
| Queue | History, now, next, reorder, undo |
| Output and handoff | Devices, continue banner, remote control |
| Song menu | Long-press menu and its groups |
| Profile switcher | Switch, lock, guest, add account |
| Year recap | Entry card on Home, story player and share sheet. Card count, timing and where numbers are computed are set by the three boards below |
| Settings | Root, playback, lyrics, downloads, suggestions, appearance, lock screen and widgets |
| Servers and sources | Health, add, lyrics and artwork order, companion server |
| Privacy and data | Switches, connections list, privacy policy, export, delete profile and its data |
| Empty, error and offline | Banners, skeletons, unavailable items |
| Welcome and Connect a server | First run, policy links, Jellyfin sign-in, Quick Connect, demo library |
| Permissions and system prompts | Primer screens, system prompts, purpose strings, what happens when denied |
| Size classes and platform rules | Four size classes, what changes at each, platforms and layout rules |
| Mac layouts | Liquid Glass window at every width: floating sidebar, toolbar, player bar, Now Playing window, mini player window, menu bar item, menus and shortcuts |
| iPad layouts | Expanded window with inspector, Medium window with overlay sidebar, Stage Manager compact window, Now Playing, size strip |
| Android tablet layouts | Large, Medium and Compact windows, rail and expanded rail, Now Playing, tabletop and book fold postures |
| Windows and Linux layouts | Title bar and caption buttons, rail and panes, context menu, compact mini player, tray and media controls (SMTC, MPRIS) |
| Live surfaces and rules | What each platform gets for the lock screen, timed tasks, widgets and quick controls; privacy and no-push rules; the Lock screen and widgets settings page |
| Lock screen and Live Activities | System player, lock screen widgets, sleep timer and download progress in the Dynamic Island, Lock Screen, StandBy, Mac menu bar, CarPlay and Watch; Android lock screen player, Live Update and status bar chip |
| Widgets and controls on Apple devices | Now Playing, Mixes, Recently played and Recap widgets in each size, Lock Screen accessory widgets, StandBy, Mac desktop, rendering modes, controls, states |
| Widgets and media surfaces on Android, Windows and Linux | Glance widgets and lock screen widget, Quick Settings tile, Windows Widgets board and taskbar progress, Linux MPRIS and progress |
| Buttons, states and haptics | Android play, pause, like, shuffle and repeat in every state, button sizes, Apple, Mac and desktop button states, press timing, what every control does, haptics, accessibility |
| Notch, Dynamic Island, cutouts and foldables | iPhone notch and island, Android cutouts, Mac notch, foldable postures, what each screen does at a cut-out or a fold, rules and test matrix |
| Smart picks: signals and rules | Pipeline, how each event is read, what does not count, where the engine is used, ranking rules, cold start, fallbacks, sync and privacy |
| Smart picks: screens and states | Why this song, Not interested, the Mixes and suggestions settings page on iOS and Android, and every state of a mix |
| Year recap: the story | Twelve cards in the Editorial pack and the rules for length, arc, rotating beats, sound, taps, copy, sharing, the detail page, honest counting and motion |
| Year recap: packs and years | Four launch packs on the same data, what a pack decides, how the palette is made, one profile over five years, the pack contract |
| Year recap: devices and states | How the recap is made, where the engine runs, the 1 December to 8 January timeline, each window size, sync rules and twelve states |
| Swipe gestures | Mini player and Now Playing artwork swipes in every state on iPhone and Android, row swipes, and the rules for direction, ends of the queue, signals and accessibility |
| Drag and drop | Mac and Windows or Linux windows mid-drag, eight drop targets, six awkward cases and the rules for selecting, copy and move, duplicates, undo and menu twins |

**Not drawn yet.** Artist page, playlist page, downloads manager, equalizer, and CarPlay and Android Auto screens. The sleep timer, widgets, controls and Live Activities are drawn on the four live-surface boards. Build each from the closest board above and follow the system rules in sections 16, 22, 23 and 26.

## 20. Widgets, Live Activities and the lock screen

Every platform gets the surfaces it really has, and nothing is faked where the system has none. Four rules hold everywhere. Playback is drawn by the system from the Now Playing data. A Live Activity is only for a timed task. A widget only shows what the app saved on the device. Nothing here needs a push service. The wireframe canvas has four boards for this: Live surfaces and rules, Lock screen and Live Activities, Widgets and controls on Apple devices, and Widgets and media surfaces on Android, Windows and Linux.

### What each platform gets

| Surface | iPhone | iPad | Mac | Android | Windows | Linux |
| --- | --- | --- | --- | --- | --- | --- |
| Player on the lock screen, drawn by the system | Lock Screen, Dynamic Island, StandBy, Control Center | Lock Screen, Control Center | Control Center, menu bar | Lock screen, quick settings, shade | Lock screen, volume flyout | Lock screen and shell media controls |
| Timed tasks: sleep timer, user-started downloads, first sync | Live Activity | Live Activity on the Lock Screen | App window, Dock progress, menu bar item | Live Update (Android 16 QPR1 and later), else an ongoing notification | Taskbar progress, progress notification | Notification and launcher progress where the desktop shows them |
| Home widgets | Home Screen, Today View, StandBy | Home Screen, Today View | Desktop, Notification Center | Home screen (Glance) | Widgets board, Store build | Not in version 1 |
| Lock screen widgets | Circular, rectangular, inline | Circular, rectangular, inline, small | Not available | Phones from Android 16 QPR1, Pixel Tablet | Not available to apps | Not available |
| Quick controls | Control Center, Lock Screen, Action button | Control Center | Control Center (macOS 26) | Quick Settings tile | Jump List, tray menu | Tray menu, desktop actions |

Comes with no extra work: an iPhone Live Activity also appears in the Mac menu bar, on the CarPlay Dashboard and in the Apple Watch Smart Stack, and a small widget works in StandBy and CarPlay.

### Decisions

- **Playback is not a Live Activity.** iOS and Android already draw a player from the Now Playing data: the Lock Screen, the Dynamic Island, Control Center, StandBy, CarPlay, the Android shade and quick settings, the Windows flyout and the Linux shell. A second one would show two players for one song. Apple asks Live Activities to track a task or event with a defined beginning and end, and Android's Live Updates cannot use the media style. So the app's job is to give the system complete and correct data: title, artist, album, artwork, duration, position, rate and the enabled commands, including like.
- **Live Activities are for timed tasks.** Three, all started by the person: the sleep timer, a download they started, and the first library sync. Encore starts at most one Live Activity of its own (the sleep timer); iOS 26 draws download and sync progress itself. Automatic downloads and background syncs show nothing. One control at most, because Apple prefers a single control and the system turns controls off in CarPlay.
- **Never push.** No remote Live Activity updates, no push-to-start, no WidgetKit push, and no APNs or FCM token goes to any server. A self-hosted server has no project key for Apple's push service, and sending listening state through a push service would break the privacy rule. Everything is updated by the app on the device, which is awake because audio is playing or because the system runs the task: a continued processing task on iOS 26, a user-initiated data transfer job on Android 14 and later. So a Live Activity is not updated by another device, and controlling a device from a locked phone that is not playing is not promised in version 1.
- **Widgets read a local snapshot.** The core writes a small snapshot per profile after each change: now playing or last played, pinned mixes with cover thumbnails, recent albums and the recap headline. It goes in the Apple App Group container, in the Android widget state and in the Windows app data folder. A widget never opens the database and never calls the network.
- **Private by default.** A widget can be seen by anyone who picks up the phone. Listening history never shows on a locked screen: personal text is marked privacy sensitive so Apple's system redacts it while locked, and Android widgets that show history opt out of the lock screen. Lock screen widgets default to controls and the pinned mix name. A profile with a PIN, and the guest profile, show a neutral Open Encore card instead of any history.
- **Widgets are per profile.** Each widget has a Profile setting, Active profile by default. A widget set to one profile keeps showing that profile after the switcher changes. A deleted profile's widgets say Profile removed.

### Sleep timer

There was no sleep timer in the plan, and it is the first Live Activity. Presets are 5, 10, 15, 30, 45 and 60 minutes, End of this song and End of this album. It fades out over the last 10 seconds (less if the song ends sooner), pauses, and ends the Live Activity at once. The Live Activity has one button, +15 min, which adds 15 minutes; a tap anywhere else on it opens the sleep timer sheet. The end time is saved, so the timer survives the app being closed. The sleep timer sheet opens from More on Now Playing, from Settings, Playback and quality, from the Sleep timer control and from a tap on the Live Activity. Apple shows it as a Live Activity with a countdown that the system keeps ticking without updates. Android shows a Live Update with a chip counting down, or an ongoing notification before Android 16 QPR1. Mac shows a menu bar item, Windows a taskbar overlay icon and tooltip, Linux a tray tooltip.

### Widgets

| Widget | Apple sizes | Android and Windows | What it does |
| --- | --- | --- | --- |
| Now Playing | Small, medium, Lock Screen circular, rectangular and inline | 2 by 1 up to 6 by 2 (Android), small (Windows) | Cover, title, artist, progress, Play or pause and Next. Tapping the cover opens Now Playing. Shows the last played song with a Play button when nothing is playing. |
| Mixes | Small (1 mix), medium (3), large (5), iPad extra large (5 and recents) | 4 by 2 (Android), medium (Windows) | Each tile starts its mix with one tap. Configurable: pick a mix or follow Next up. |
| Recently played | Medium (4 albums), large (8) | 4 by 3 (Android), large (Windows) | Tapping an album opens it. History, so redacted while locked and off the lock screen. |
| Recap | Small, medium | Android only, 2 by 2 and 4 by 2 | Top artist and minutes so far this year in the year's design pack. Says Your recap builds as you listen until there is data. |

Controls on Apple devices (Control Center, Lock Screen, Action button): Play or pause (toggle), Like current song (toggle), Start a mix (button, choose the mix) and Sleep timer (button, choose the preset, starts the Live Activity). On Android the Quick Settings tile is Start a mix. On Windows the Jump List has Play or pause, Next and Start a mix, with the pinned mixes above them, and the system Recent list is turned off for Encore so it never records what was played. The taskbar thumbnail has Previous, Play or pause and Next.

### Platform notes, limits and store checks

The Android, Windows and Linux notes, the limits to build against, the store and policy checks and the sources are in each platform plan under section 20. One limit applies everywhere:

- Live Activities and widgets must support light and dark, Always-On, Night Mode in StandBy, and on iOS 26 the full color, accented and clear rendering modes, with album art as the only full-color image. Hit targets are 44 pt and 48 dp. VoiceOver, TalkBack and Narrator read the same labels as the app.

### Settings page: Lock screen and widgets

Sleep timer on the Lock Screen (a Live Update on Android), Download progress on the Lock Screen, Hide listening history while locked (on), and Widget profile (Active profile). On Android a row links to the system Live updates setting when posting is off. The page is a row in the Settings root, under Mixes and suggestions. No custom haptics: widgets and Live Activities run outside the app, so the system's feedback applies, and the app plays its usual haptic only when it handles the action in the foreground.

## 22. Buttons, states and haptics

Every control has one defined shape, one set of states, one haptic and one guard against accidents. The Buttons, states and haptics board draws each state; this section is the rule behind it, and it replaces any earlier wording that described the Android transport as one connected pill.

### The Android shape rule

The state-shape rule for Android buttons (and the Windows and Linux theme that follows it), the sizes, the transport row and the matching Not verified note are in `docs/android/PLAN-ANDROID.md` and `docs/windows-linux/PLAN-WINDOWS-LINUX.md` under section 22. Apple devices do not change shape: see `docs/apple/PLAN-APPLE.md` under section 22. The board that draws every state is board 35.

### Press and hold timing

- **Feedback** shows within 100 ms of touch down (state layer first, shape morph follows). The action runs on release inside the control.
- **Slop**: moving more than 10 pt on iOS or 8 dp on Android before release means a scroll or a drag, never a tap.
- **Long press**: Android 400 ms by default, following the system touch and hold delay. iOS uses the system menu timing, about half a second, and follows Touch Accommodations. Mac: right-click, Control-click, or a press and hold on a touch screen. Windows and Linux: right-click, the Menu key or Shift+F10, and 500 ms on touch.
- **Transport debounce**: a second tap on Play, Previous or Next inside 200 ms is dropped.
- **Order of events**: touch down (state layer and morph), release inside, the action, then the haptic, then the glyph change. Sliding off before release cancels everything with no haptic.
- **Optimistic state**: the button changes on release and corrects itself if the player reports an error. If playback fails the button flips back and says why.
- **One haptic per action.** A press never produces two. Scans tick once a second. Ambient motion such as lyrics and gradients never vibrates. System haptic settings win.
- **Mac and desktop**: Mac fires trackpad haptics only for scrub snaps and step changes. Windows and Linux have no haptics and rely on motion and sound.

### What every control does

| Control | Tap | Hold | Guards and edge cases |
| --- | --- | --- | --- |
| Play and pause | Flips at once; audio follows within 150 ms | Nothing | Disabled with an empty queue; while loading it stays tappable and cancels the load |
| Previous | After 3 s into the song restarts it, before that goes to the previous song; on the first song it always restarts | Scans back at 4x, returns to normal on release | A second tap within 1 s of a restart goes back one song |
| Next | Skips and logs a skip with its position (section 21); at the end of the queue autoplay adds songs, or playback stops if autoplay is off | Scans forward at 4x | Previous within 3 s returns to the song just skipped, never two back; skips caused by a route change or an error are not logged |
| Like | Toggles at once, saved on the device, synced later; a strong "more like this" | Nothing | Double tap on artwork only adds a like and never removes one; works offline |
| Shuffle | Toggles; reorders only what is up next, never the song playing or the history | Opens Shuffle or Smart shuffle | Turning it off restores the original order after the current song |
| Repeat | Cycles Off, All, One | Nothing | Next always moves on, so Repeat One never traps the user |
| Progress bar | Tap or drag to scrub; sliding down halves then quarters the speed; the right-hand time swaps between remaining and total | Touching the handle for 150 ms arms the scrub | Edge back, collapse swipe and tab taps are ignored while a finger is on the bar; seeks throttled to one per 50 ms; seeking is never a skip |
| Song row | Plays it; in the queue it jumps to the song and what was skipped stays in History | Opens the context menu with the row lifted; release without moving does nothing | Swipe right is Play next, swipe left is Add to queue or Remove; arms past 25%, commits past 60%; Remove shows Undo for 5 s |
| Sleep timer preset or +15 | Starts the timer or adds 15 minutes; a toast confirms the end time | Nothing | Cancel needs no confirmation; the 10 s fade-out cannot be skipped by a tap |
| Destructive action | Opens a confirm sheet with Cancel as the default and the red button separated | Nothing | A tap outside cancels; applies to remove from library, delete playlist, sign out and clear history |
| Blocked action | Shakes 4 pt, warning haptic and a one-line reason (offline, not downloaded, no permission) | Nothing | The reason is also read out by screen readers; never a blocking dialog |

### Haptics for the controls above

| Event | iOS | Android |
| --- | --- | --- |
| Play, pause, next, previous | impact soft | VIRTUAL\_KEY |
| Like on, like off, shuffle, repeat | impact medium on and light off; selection for repeat | TOGGLE\_ON, TOGGLE\_OFF |
| Previous or next scan tick (each second) | selection | SEGMENT\_FREQUENT\_TICK |
| Scrub start and end | impact light | GESTURE\_START, GESTURE\_END |
| Long press opens menu | system menu haptic | LONG\_PRESS |
| Swipe action armed | impact rigid | GESTURE\_THRESHOLD\_ACTIVATE |
| Sleep timer set | notification success | CONFIRM |
| Destructive confirm | notification warning | CONFIRM |
| Blocked action | notification warning | REJECT |

The general haptics table in section 16 still lists the full event set; where the two overlap they agree.

### Motion

- Press morph about 100 ms (`fastSpatial` on Android). iOS glyphs replace with `.snappy`; the heart pops 1, 1.25, 1; the Repeat One badge pops in at 120 ms; Previous and Next glyphs slide 3 pt toward their direction; the blocked-action shake is 4 pt for 3 cycles in 240 ms; the destructive sheet rises on `defaultSpatial`.
- **Reduce Motion**: morphs, lifts, springs and shakes become 120 ms cross-fades. State layers, focus rings and haptics stay.

### Accessibility for every button

- Every icon button has a label and a role. Play and pause announce "Playing" or "Paused", toggles announce "On" or "Off", and a disabled button says why when focused.
- **Right to left**: transport keeps its order because time still runs left to right; back arrows, lists and swipe actions mirror.
- **Large text**: buttons grow with the font scale up to 200%, labels wrap, targets stay at 44 pt or 48 dp, and the transport row stays on one line down to 320 dp.
- **Touch Macs**: hit areas grow to 36 pt, controls use gesture recognizers only, and anything that appears on hover also has a touch route (tooltips become labels or a long-press menu).

## 23. Notch, Dynamic Island, cutouts and foldables

Every screen has to work around a camera cut-out, a gesture area and, on foldables, a fold. The rule is the same everywhere: backgrounds run edge to edge, controls stay inside the safe area, and nothing tappable ever crosses a fold. The Notch, Dynamic Island, cutouts and foldables board draws it, and every phone frame on the other boards now shows a status bar, the Dynamic Island on iPhone and the punch-hole camera on Android, with the content placed clear of them.

### Rules

1. **Read the insets from the system. Never type numbers.** iOS: the safe area. Android: the window insets for system bars, display cutout and gestures. Mac: `NSScreen.safeAreaInsets`, with the left and right areas beside the notch. Flutter on Windows and Linux: the view padding. On iPhone the top inset is about 47 pt with a notch and 59 to 62 pt with a Dynamic Island, and these figures are only for drawing the wireframes.
2. **Draw behind, place inside.** Album gradients and artwork run edge to edge under the status bar, island, notch and gesture area. Text, buttons, sliders and drag handles sit inside the safe area.
3. **Status text follows the artwork.** The clock and battery turn white on dark gradients and dark on light covers, with a 200 ms cross-fade when the album changes.
4. **Gesture zones.** Keep sliders and swipe areas at least 24 dp from the screen edges. A touch that starts in the edge zone belongs to the system back gesture, not to a scrub.
5. **Mac menu bar item.** One optional item. When the bar is crowded macOS can hide items behind the notch, so everything it does is also in the Dock menu, the mini player and the keyboard shortcuts.
6. **Live Activities on Mac.** An iPhone Live Activity already mirrors to the Mac menu bar. Whether a Mac app can start its own is not documented yet (Reported), so Encore plans for the mirror only. The system draws the island; Encore supplies content and never positions anything there.
7. **Touch on Mac.** On Macs with a touch screen, controls use gesture recognizers only, targets grow to 36 pt and every hover-only detail has a touch route. Test on a touch Mac when one is available; until then, review every hover-only detail by hand.
8. **Folding keeps the session.** The Rust core owns playback, queue and position, and screen state is saved outside the view, so a fold, unfold, rotate or window resize never restarts the song or loses the scroll position. Orientation is never locked. Android 16 ignores orientation and resize opt-outs on large screens for apps that target it.
9. **Keep off the fold.** Nothing tappable, no slider thumb and no line of text crosses the fold or hinge gap. Android reads it from the folding feature (state, orientation, separating, bounds, occlusion). On iPhone Duo the equivalent is a reserved region at the fold plus an arrangement for split or overlay layouts (Reported: the API names come from a developer write-up of Apple's Tech Talks); confirm them in the Xcode documentation.
10. **Postures.** Half open with a horizontal hinge: content on top, controls below. Half open with a vertical hinge, or flat and wide: two panes. Closed: compact. Show a short transition when entering or leaving tabletop, as Samsung asks for Flex mode.
11. **Cover screens.** A Fold cover screen runs the compact layout. A Flip cover screen is served by the system player and widgets, and running Encore there is optional (a product decision: Samsung's page gives no Flip-specific rules and only asks that apps display correctly on both screens, fill the screen without letterboxing, keep scroll position and stay resizable).

### Cut-outs by device and foldables

The device tables and the sources are in the plan of the track that owns the device: iPhone, Mac and iPhone Duo in `docs/apple/PLAN-APPLE.md`, Android cut-outs and foldables in `docs/android/PLAN-ANDROID.md`, under section 23. The plan has no cut-out or fold device rows for Windows and Linux; they read the view padding (rule 1) and keep state across window resizes (rule 8), in `docs/windows-linux/PLAN-WINDOWS-LINUX.md` under section 23.

### What each screen does

| Screen | iPhone, iPad and Mac | Android and Windows | Foldables, split screen and resizable windows |
| --- | --- | --- | --- |
| Mini player | Sits above the tab bar and home indicator | Sits above the navigation or gesture bar; 48 dp targets; the bottom inset is read from the system, so both navigation styles work | With two panes it becomes the footer of the list pane; in tabletop it is replaced by the full transport in the bottom half |
| Now Playing | Background runs under the island, notch and home indicator; handle, title and buttons start below the island | Edge to edge; the progress bar keeps 24 dp from both edges so the back gesture cannot start on it | Flat: artwork and transport left, lyrics right. Tabletop: artwork and lyrics above, transport below. Book: controls and lyrics on one side, hinge empty |
| Lyrics | Lines start below the island; the active line stays in the middle third, clear of the home indicator | Same; lines wrap inside the side insets on curved screens | Lines wrap inside one pane and are never split across the hinge; a pane under 320 dp turns the lyrics into a sheet |
| Queue, lists, search | Large titles start below the island; lists scroll under the glass bars | Lists scroll under the system bars with the content inset, so the last row is reachable | Two-pane list and detail from 600 dp; selection and scroll position survive resizing |
| Sheets and menus | Detents stop below the island; context menus flip to stay on screen | Bottom sheets stop 8 dp under the status bar inset; popups avoid the cutout | A sheet opens in the pane it came from, or centred on one side of the fold, never across it |
| Recap stories and full-screen cards | Progress segments and the left and right tap zones start below the island | Progress segments sit below the status bar inset; Share clears the gesture bar | A story card fits inside one side of the fold; on a wide inner screen it centres at phone width |

### Test matrix before release

iPhone with a Dynamic Island, iPhone with a notch, iPhone with a home button, Mac with a notch, Mac with a Dynamic Island and touch (when available), a Pixel, a Samsung with a centre hole and one with a corner hole, a Galaxy Z Fold, a Galaxy Z Flip, a Pixel Fold, iPhone Duo (Apple says Xcode's Device Hub will add support later, so use a real device until then), an Android tablet, landscape, split screen, 200% font and right to left. Where a device is not at hand, use the Android emulator's foldable and cutout profiles and Xcode's simulators, and mark the row as simulated.

## 24. Production-readiness checklist

The wireframes are production-ready when every item below can be ticked with evidence (a test, a screenshot or a device run), and the unverified items at the end have been checked. Work through it per platform before each public release; sections 19 and 20 hold the store and live-surface checklists, and this one covers the rest.

### Playback and state

- [ ] Gapless playback, crossfade, Repeat One and Repeat All behave as section 8 says on every platform, including across a profile switch.
- [ ] Interruptions pause and resume correctly: phone call, assistant, headphone unplug, Bluetooth and AirPlay switch, audio focus loss on Android, and a lost network.
- [ ] Play and pause stay in step with the player under a failing server: the button flips back and says why, and never shows the wrong state after an error.
- [ ] Fold, unfold, rotate, window resize and split screen never restart the song or lose scroll position (section 23, rule 8).
- [ ] Sleep timer fades out for 10 s, cannot be skipped by a tap, and ends playback without logging a skip.

### Buttons, haptics and motion

- [ ] Every control in section 22 has screenshot tests for rest, pressed, selected, loading, disabled and focused on each platform.
- [ ] No Android screen shows a pill-shaped pause button; play, like, shuffle and repeat follow the state shape rule on phone, tablet, desktop, lock screen and widgets.
- [ ] Shapes and radii checked against the pinned Compose Material 3 Expressive version, and the Encore tokens adjusted if it differs.
- [ ] Haptics tried on a real iPhone and a real Android phone: one haptic per action, none on slide-off, none from ambient motion.
- [ ] Reduce Motion, Reduce Transparency, Increase Contrast, 200% font and right to left pass on every main screen.
- [ ] Every icon button has a screen reader label and role; VoiceOver, TalkBack, Narrator and Orca each complete play, like, queue and profile switch.

### Cut-outs and foldables

- [ ] Layouts read insets from the system and no source file contains typed inset numbers (a lint rule enforces it).
- [ ] The test matrix in section 23 is run, with simulated rows marked as simulated.
- [ ] Nothing tappable crosses a fold on a Fold, Flip or Pixel Fold, in flat, tabletop and book postures.
- [ ] Status text colour changes with the artwork and never becomes unreadable during the cross-fade.

### Smart picks

- [ ] Offline replay on at least three test histories (light, heavy, one with many skips) beats plain shuffle on skip rate of app-chosen songs, with the numbers saved in CI.
- [ ] Unit tests prove that interruptions, errors, seeks and plays under 2 s write no negative signal.
- [ ] Private listening, Guest and Learn from my listening off write zero events, checked at the database level.
- [ ] Reset suggestions and Clear listening history behave differently as designed, and both sync as tombstones that another device cannot undo.
- [ ] A cold profile, a profile with 49 plays and one with 50 plays each show the right mixes, labels and fallbacks.
- [ ] Every smart surface has a working fallback with no model, with the server off and with no network.
- [ ] Budgets hold on a mid-range phone: next-song decision under 50 ms, mix build under 20 s and under 150 MB.
- [ ] Licences of the audio embedding model and the nearest-neighbour library are checked and recorded in `THIRD_PARTY`.

### Year recap

- [ ] The same log, year and seed produce byte-identical recap data from the server build and from each platform's core, checked against golden files in CI.
- [ ] Snapshot tests cover every pack and every beat on the extreme histories in section 25 (one artist, 5,000 artists, very light and very dark art, very long names, non-Latin and right-to-left scripts, 100 plays and 100,000 plays).
- [ ] Private listening, Guest, interruptions and plays under 2 s are never counted; the same song on two albums counts once; plays land on the day of their own time zone across a daylight saving change.
- [ ] Plays synced between 31 December and 8 January appear in the final recap, and after 8 January the recap never changes. Clear listening history restarts the recap as designed.
- [ ] A recap build over 100,000 plays runs in the background on a mid-range phone with the server off without blocking playback, and its time is recorded.
- [ ] A new year picks a different pack and a lead hue at least 60° from last year's; an older app draws the fallback look and says so; a profile with under 10 plays shows no recap and one with 10 plays shows the short version.
- [ ] Every card has a text version for screen readers, Reduce Motion shows the final state, and nothing flashes more than three times a second.

### Swipes and drag and drop

- [ ] Mini player and artwork swipes: a completed swipe writes exactly the signal the Next button writes; two swipes within 600 ms write a signal only for the song landed on; a swipe while paused stays paused; the end of the queue rubber-bands; a drag that starts within 24 pt of the edge is never a skip.
- [ ] Next song and Previous song exist as VoiceOver and TalkBack custom actions, and headset, car and lock screen buttons run the same code.
- [ ] Row swipes arm at 25% (non-destructive) and 60% (destructive), release early does nothing, and Undo lasts 5 s.
- [ ] Drag and drop on Mac, Windows, Linux, iPad and Android tablets: songs go in list order, copy and move modifiers work, duplicates are found by song key with Add anyway, over 100 songs asks first, and 500 songs make one core call, one sync event and one undo entry.
- [ ] Album tiles refuse a drop with the reason, Guest sees the playlist targets disabled with the reason, Escape cancels, and every drop has a menu, keyboard and screen reader equivalent.

### Data, sync and security

- [ ] Local and server databases have versioned migrations tested forward and, where promised, backward, and a backup and restore test passes.
- [ ] Sync conflict tests cover two devices editing a like, a playlist and Not interested at the same time while offline.
- [ ] No telemetry or crash report leaves the device without the user pressing Send; the network log of a full session shows only the media server, the companion server and the providers the user enabled.
- [ ] Tokens live in the OS keychain or keystore; nothing sensitive appears in logs, backups or screenshots.
- [ ] Dependency and licence scans run in CI and fail on a new unreviewed licence.

### Release

- [ ] Builds are signed and reproducible from a tag; release notes are generated from the changelog.
- [ ] Public beta through TestFlight and Google Play internal and closed testing before production, then a staged rollout with a way to halt it.
- [ ] The store checklist in section 19 and the live-surface checklist in section 20 are ticked.

### Unverified and reported items to check before release

| Item | Why it is open | Check against |
| --- | --- | --- |
| Every smart-picks weight, half-life, threshold and budget | Starting values, not measurements | Offline replay on real histories |
| Audio embedding model and nearest-neighbour library licences | Not yet chosen or checked | The licence texts, before release |
| Recap numbers: card timing, thresholds, 60° hue gap, 90-day device window | Starting values, not measurements | Testing with real histories and real people |
| Recap build time for 100,000 plays | Not measured | A benchmark on a mid-range phone and on a Raspberry Pi 5 |
| Swipe numbers: 12 pt lock, 60 pt arm, 500 pt/s flick, 600 ms coalesce, 40 pt rubber band | Starting values, not measurements | Trying them on real phones, and against Apple Music |
| SwiftUI, Compose and Flutter drag and drop API names and touch behaviour | Written from memory of the frameworks, not checked against the pinned versions | Each framework's official documentation and a real device |

Rows that belong to one platform (Material 3 Expressive radii, the new MacBook Pro, Mac Live Activities, the iPhone Duo fold region, Samsung Flex mode) are in that platform's plan under section 24.

## 26. Swipe gestures and drag and drop

This section is the build spec for the gestures a good music app has under your thumb and the drag and drop that desktop and tablet users expect. Boards 42 and 43 draw it. It replaces the one-line mini player note on board 05 and the player swipe row in section 16. All numbers are starting values; keep them in `design/tokens.json` (`gestures`).

### Swipes on the mini player and Now Playing (board 42)

1. **Direction lock.** The first 12 pt of movement decide it: horizontal if sideways is more than 1.2 times vertical, otherwise vertical. It stays locked until the finger lifts. Vertical up on the mini player expands it, following the finger. Vertical down does nothing on the mini player, so playback is never stopped by a slip.
2. **Which way.** In left-to-right languages, swipe left for the next song and right for the previous; mirrored in right-to-left languages. The song you are heading to always arrives from the side you drag toward.
3. **Arming.** The swipe is armed past 60 pt, or when the flick is faster than 500 pt/s. Released armed, the new song settles. Released early, it springs back with no change and no haptic.
4. **Back means back.** A swipe right goes to the previous song every time and never restarts the current one. The Previous button keeps its rule of restarting after 3 s. Swiping back right after a skip returns to the song you skipped.
5. **Play state is kept.** Skipping while paused stays paused: the new song shows at 0:00 and loads without sound. Repeat One moves to the next song and applies to it. Shuffle follows the shuffled order.
6. **The finger decides, not the audio.** During a drag the neighbour is only a drawing and nothing loads. A second swipe within 600 ms replaces the first, so songs passed over never load, never make a sound and never write a signal. Only the song you land on counts.
7. **Signals.** A completed swipe is a skip exactly like the Next button, so section 21 applies: an early skip of an app-chosen song counts against it, a skip of a song you picked barely counts, and seeks and errors never count.
8. **Ends of the queue.** Last song with Autoplay on: the next song is the autoplay pick. With Autoplay and repeat off: the content follows at a third of the speed, stops after 40 pt and springs back, with "End of queue" under the title for 1.5 s. At the first song with repeat off the same rubber band applies; with Repeat All it wraps.
9. **Buttons never move.** Play and Next stay where they are while the content slides. A drag that starts on a button and passes 10 pt cancels the button and becomes a swipe. Buttons still fire on release inside, and a second tap within 200 ms is ignored.
10. **One haptic.** One soft tick when it arms (iOS soft impact, Android `GESTURE_THRESHOLD_ACTIVATE`), nothing on release and nothing when you move back under the line. Reduce Motion replaces the slide with a 120 ms cross-fade; the tick stays.
11. **Catch it mid-flight.** The settle uses the Now Playing hero spring and can be caught by a new touch. The finger takes over from the current position and velocity.
12. **Edges and system gestures.** A horizontal drag that starts within 24 pt of the screen edge belongs to the system back gesture and is never read as a skip. The mini player sits above the bottom inset on both navigation styles.
13. **Now Playing artwork.** The same swipe works on the large artwork, and double tap still likes. On the lyrics and queue panels a horizontal swipe never skips.
14. **Not by touch only.** VoiceOver and TalkBack get Next song and Previous song as custom actions on the mini player. Switch users use the buttons. Mac, Windows and Linux have no swipe: they use the buttons, media keys and Space.
15. **Headsets, cars and the lock screen.** Next and Previous from headphones, Bluetooth, a car and the lock screen run the same code as the buttons and the swipe, including the 3 s restart rule on Previous and the signals above.

### Swipes on rows (board 42)

- **Arm points.** A non-destructive action arms at 25% of the row width. A destructive one arms at 60%. Under the arm point the row springs back with no action and no haptic. The haptic fires at the moment it arms.
- **Leading swipe (right):** Play Next, on every song list. It performs on release, with a toast and Undo.
- **Trailing swipe (left):** Add to Queue on song lists in Library, Album, Search and Mixes; Remove on the queue and on your own playlists (destructive, so it arms at 60%, with Undo for 5 s); Not this on Autoplay (removes the song and counts as a strong dislike, arms at 60%).
- **Edge zone.** Horizontal swipes that begin within 24 pt of the edge are left to Back.
- A long press on a row opens the menu (board 13).

### Drag and drop on Mac, Windows, Linux, iPad and tablets (board 43)

Any list is a source and any playlist is a target, so building a playlist is a few drags with no dialogs. The rules are the same on every platform; only the modifier keys and the look change.

1. **What can be dragged.** Songs from any list: Library, Album, Artist, Search, a playlist, Up Next, History, a mix and Recently played. Whole albums, artists and playlists, which become their songs in order. The cover and title in the player bar, which drags the current song. Unavailable songs are not draggable. Guest cannot save, so playlist targets are disabled with the reason "Guest cannot save playlists".
2. **Selecting.** Click selects. Shift extends a range. Ctrl (Windows, Linux) or Command (Mac) toggles one row. Ctrl or Command+A selects all in view. Dragging an empty gutter draws a selection box. Selection is by song, so it survives sorting and scrolling, up to 500 songs.
3. **Starting a drag.** Press on a row and move 5 px. A press without movement never drags, and double click still plays. Pressing an unselected row drags just that row. Pressing a selected row drags the whole selection. On touch, a long press of 300 ms lifts the row with a haptic and the finger can drag; on iPad, tapping more rows while dragging adds them.
4. **The drag image.** The top three covers stacked, a count badge when more than one, and a green plus when the target will accept. The target outlines in the accent colour and a tip names the result. Songs go in the order of the list as shown, not the order they were selected.
5. **Copy, not move.** Dropping on another playlist always copies, so a song is never lost from where it came from. Hold Command (Mac) or Shift (Windows, Linux) to move instead: it adds to the target and removes from the source. Reordering inside one playlist or the queue is always a move.
6. **Queue and player bar.** Drops in Up Next insert at the line. The top strip and the player bar mean Play Next. Alt or Option means add to the end. Nothing dropped ever starts playing by itself.
7. **Targets.** A playlist in the sidebar adds to the end. The New Playlist row creates a playlist named after the album or the first artist, with the name field open to rename. A playlist page inserts at the line, and from another list it adds, while inside the same playlist it reorders. Liked Songs likes every dropped song and syncs.
8. **Albums.** Albums come from the tags and folders on the server, and the app never rewrites your files or your server. So songs cannot be added to an album. Album tiles and rows refuse the drop and say why. A playlist is the place for a collection you build yourself.
9. **Duplicates.** They are found by the song itself, not the file, so a song on two albums counts as one. The toast reads "Added 2 · 1 already there" and offers Add anyway.
10. **Big drops.** Dropping an artist or a selection of more than 100 songs asks first, because it is hard to review afterwards.
11. **One write.** A drop is one call in the core with the whole list and the position, one sync event and one undo entry, even for 500 songs. It works offline and syncs later. A toast with Undo shows for 6 s, and Ctrl or Command+Z undoes the last drop for 10 s in that window.
12. **Awkward cases.** Near the top or bottom edge (within 40 px) lists scroll, faster the closer the pointer gets. In a narrow window (under 600) holding a drag at the left edge for 600 ms opens the sidebar as an overlay, and it closes after the drop. Esc cancels; so does dropping outside any target, and the drag image slides back.
13. **No drag required.** Every drop has a menu twin: right click, Add to Playlist with a New Playlist entry, Play Next and Add to Queue, plus a menu bar item with its shortcut. Screen readers get Add to playlist as a row action. With the keyboard: select rows, open the menu with the context key, choose.
14. **Nothing leaves the app.** No audio file is ever created by a drag. Dragged into a text field or another app it gives only "Title · Artist" as text.
15. **Feedback.** Valid target: outline, plus badge, tip. Invalid: dim, not-allowed cursor, reason after 600 ms. After the drop the new rows flash the accent for 300 ms and the sidebar count updates. No sound. No haptic on desktop, except the Mac trackpad's own alignment feel. Touch devices use the pick-up and drop haptics from section 22.
16. **Platform parts.** Apple: SwiftUI `draggable` and `dropDestination` with `Transferable`. Android tablets: Compose `dragAndDropSource` and `dragAndDropTarget`. Windows and Linux: Flutter `Draggable` and `DragTarget`. These names are to be confirmed against the pinned versions. All of them read the same payload type from the core.

### Tests

- Swipe: coalescing within 600 ms writes a signal only for the song landed on; a swipe while paused stays paused; the rubber band at the end of the queue; a drag from the edge zone is never a skip; a completed swipe writes the same signal as the Next button; custom accessibility actions exist.
- Row swipes: arm points, Undo for 5 s, destructive actions need 60%.
- Drag and drop: order as shown, copy and move modifiers, duplicates by song key, confirm over 100 songs, one core call, one sync event and one undo entry for 500 songs, album refusal with its reason, Guest refusal, Escape, and a menu twin for every drop.

### Unverified

The drag and drop API names above and their touch behaviour on each pinned version, and every number in this section, are starting values or written from memory. Check them against the official documentation and a real device before relying on them.
