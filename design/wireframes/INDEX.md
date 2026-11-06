# Wireframe index

43 boards. Each has a PNG (`png/`), a standalone HTML page you can open in a browser (`html/`, needs `wf.css` beside it) and a text spec (`specs/`) with the same numbering as the circled marks on the PNG.

How to use a board:

1. Open the PNG with the image reader to see layout, spacing, proportion and every state.
2. Read the matching spec for the numbered behaviour rules: tap, hold, swipe, double tap, guard against accidental presses, haptic, motion and states. Keyword tags are in square brackets.
3. Use `../tokens.json` for numbers. If a board and the plan disagree, stop and ask the owner.

The boards were drawn in IBM Plex only for their own labels. The apps use SF Pro on Apple devices and Inter on Android, Windows and Linux (plan section 3). Phone mock-ups in the PNGs show iOS on the left and Android on the right wherever both exist.

The right-hand column below names the plan section that holds the written rules for the same area.

## Start and system rules

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 01 | Start here | 17 | [png](png/01-start-here.png) | [spec](specs/01-start-here.md) |
| 02 | System: motion and haptics | 16 | [png](png/02-system-motion-and-haptics.png) | [spec](specs/02-system-motion-and-haptics.md) |
| 03 | System: gestures and accidental presses | 16 | [png](png/03-system-gestures-and-accidental-presses.png) | [spec](specs/03-system-gestures-and-accidental-presses.md) |
| 04 | System: album gradients | 9 | [png](png/04-system-album-gradients.png) | [spec](specs/04-system-album-gradients.md) |

## Core screens

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 05 | Home | 17 | [png](png/05-home.png) | [spec](specs/05-home.md) |
| 06 | Library | 17 | [png](png/06-library.png) | [spec](specs/06-library.md) |
| 07 | Search | 17 | [png](png/07-search.png) | [spec](specs/07-search.md) |
| 08 | Album | 17 | [png](png/08-album.png) | [spec](specs/08-album.md) |
| 09 | Now Playing | 9, 16, 22 | [png](png/09-now-playing.png) | [spec](specs/09-now-playing.md) |
| 10 | Lyrics | 9 | [png](png/10-lyrics.png) | [spec](specs/10-lyrics.md) |
| 11 | Queue | 17 | [png](png/11-queue.png) | [spec](specs/11-queue.md) |
| 12 | Output and handoff | 11 | [png](png/12-output-and-handoff.png) | [spec](specs/12-output-and-handoff.md) |
| 13 | Song menu | 17 | [png](png/13-song-menu.png) | [spec](specs/13-song-menu.md) |
| 14 | Profile switcher | 6 | [png](png/14-profile-switcher.png) | [spec](specs/14-profile-switcher.md) |
| 15 | Made for you playlist | 7.1 | [png](png/15-made-for-you-playlist.png) | [spec](specs/15-made-for-you-playlist.md) |
| 16 | Year recap (entry card, story player, share sheet; card count, timing and where numbers are computed are superseded by 39 to 41) | 10, 25 | [png](png/16-year-recap.png) | [spec](specs/16-year-recap.md) |
| 17 | Settings | 17 | [png](png/17-settings.png) | [spec](specs/17-settings.md) |
| 18 | Servers and sources | 4 | [png](png/18-servers-and-sources.png) | [spec](specs/18-servers-and-sources.md) |
| 19 | Privacy and data | 12, 19 | [png](png/19-privacy-and-data.png) | [spec](specs/19-privacy-and-data.md) |
| 20 | Empty, error and offline | 17 | [png](png/20-empty-error-and-offline.png) | [spec](specs/20-empty-error-and-offline.md) |
| 21 | Connect a server | 4, 17 | [png](png/21-connect-a-server.png) | [spec](specs/21-connect-a-server.md) |
| 22 | Welcome | 17, 19 | [png](png/22-welcome.png) | [spec](specs/22-welcome.md) |
| 23 | Mixes tab | 7.1, 21 | [png](png/23-mixes-tab.png) | [spec](specs/23-mixes-tab.md) |

## Platform design languages and permissions

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 24 | System: platform design languages | 9 | [png](png/24-system-platform-design-languages.png) | [spec](specs/24-system-platform-design-languages.md) |
| 25 | Permissions and system prompts | 19 | [png](png/25-permissions-and-system-prompts.png) | [spec](specs/25-permissions-and-system-prompts.md) |

## Large screens and every window size

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 26 | Size classes and platform rules | 9, 19 | [png](png/26-size-classes-and-platform-rules.png) | [spec](specs/26-size-classes-and-platform-rules.md) |
| 27 | Mac layouts | 9 | [png](png/27-mac-layouts.png) | [spec](specs/27-mac-layouts.md) |
| 28 | iPad layouts | 9 | [png](png/28-ipad-layouts.png) | [spec](specs/28-ipad-layouts.md) |
| 29 | Android tablet layouts | 9 | [png](png/29-android-tablet-layouts.png) | [spec](specs/29-android-tablet-layouts.md) |
| 30 | Windows and Linux layouts | 9 | [png](png/30-windows-and-linux-layouts.png) | [spec](specs/30-windows-and-linux-layouts.md) |

## Widgets, lock screen and live surfaces

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 31 | Live surfaces and rules | 20 | [png](png/31-live-surfaces-and-rules.png) | [spec](specs/31-live-surfaces-and-rules.md) |
| 32 | Lock screen and Live Activities | 20 | [png](png/32-lock-screen-and-live-activities.png) | [spec](specs/32-lock-screen-and-live-activities.md) |
| 33 | Widgets and controls on Apple devices | 20 | [png](png/33-widgets-and-controls-on-apple-devices.png) | [spec](specs/33-widgets-and-controls-on-apple-devices.md) |
| 34 | Widgets and media surfaces on Android, Windows and Linux | 20 | [png](png/34-widgets-and-media-surfaces-on-android-windows-and-linux.png) | [spec](specs/34-widgets-and-media-surfaces-on-android-windows-and-linux.md) |

## Buttons, cut-outs and foldables

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 35 | Buttons, states and haptics | 22 | [png](png/35-buttons-states-and-haptics.png) | [spec](specs/35-buttons-states-and-haptics.md) |
| 36 | Notch, Dynamic Island, cutouts and foldables | 23 | [png](png/36-notch-dynamic-island-cutouts-and-foldables.png) | [spec](specs/36-notch-dynamic-island-cutouts-and-foldables.md) |

## Smart picks

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 37 | Smart picks: signals and rules | 21 | [png](png/37-smart-picks-signals-and-rules.png) | [spec](specs/37-smart-picks-signals-and-rules.md) |
| 38 | Smart picks: screens and states | 21 | [png](png/38-smart-picks-screens-and-states.png) | [spec](specs/38-smart-picks-screens-and-states.md) |

## Year recap, gestures and drag and drop

| Board | Screen | Plan section | PNG | Spec |
| --- | --- | --- | --- | --- |
| 39 | Year recap: the story | 10, 25 | [png](png/39-year-recap-the-story.png) | [spec](specs/39-year-recap-the-story.md) |
| 40 | Year recap: packs and years | 10, 25 | [png](png/40-year-recap-packs-and-years.png) | [spec](specs/40-year-recap-packs-and-years.md) |
| 41 | Year recap: devices and states | 10, 25 | [png](png/41-year-recap-devices-and-states.png) | [spec](specs/41-year-recap-devices-and-states.md) |
| 42 | Swipe gestures: mini player, Now Playing and rows | 16, 26 | [png](png/42-swipe-gestures-mini-player-now-playing-and-rows.png) | [spec](specs/42-swipe-gestures-mini-player-now-playing-and-rows.md) |
| 43 | Drag and drop on desktop and tablets | 16, 26 | [png](png/43-drag-and-drop-on-desktop-and-tablets.png) | [spec](specs/43-drag-and-drop-on-desktop-and-tablets.md) |

## Which boards each roadmap phase needs

Phase numbers are from plan section 13. Boards 02 to 04, 24, 26, 35, 36 and 42 are system rules that apply from phase 3 onward.

| Phase | Boards |
| --- | --- |
| 2 Jellyfin and playback | 18 Servers and sources, 21 Connect a server, 22 Welcome, 20 Empty, error and offline, 25 Permissions |
| 3 Apple Music-grade UI and lyrics | 05 to 11, 13, 17, 27, 28, 42, 43 (Mac and iPad parts), and the system boards above |
| 4 Server, profiles and sync | 14 Profile switcher, 19 Privacy and data |
| 5 Recommendations v1 | 15, 23, 37, 38 |
| 6 Android, Windows, Linux | 29, 30, 42, 43 (Android tablet, Windows and Linux parts), plus every screen board again for Android and Flutter |
| 7 Handoff | 12 Output and handoff |
| 9 Widgets and live surfaces | 31 to 34 |
| 11 Year recap | 16, 39, 40, 41 |

## Which track builds from which board

"Yes" means the track reads the board and its spec for this phase's work. Phone boards draw iOS (Liquid Glass) on the left and Android (Material 3 Expressive) on the right: the Apple track builds the left side, the Android track the right side, and the Windows and Linux track takes the right side's Material 3 Expressive look at desktop density. The core track reads the boards that define data, signals and rules, because the core must feed them; it draws nothing.

| Board | Screen | Core and server | Apple | Android | Windows and Linux |
| --- | --- | --- | --- | --- | --- |
| 01 | Start here | yes | yes | yes | yes |
| 02 | System: motion and haptics |  | yes | yes | yes |
| 03 | System: gestures and accidental presses |  | yes | yes | yes |
| 04 | System: album gradients | yes | yes | yes | yes |
| 05 | Home |  | yes | yes | yes |
| 06 | Library |  | yes | yes | yes |
| 07 | Search | yes | yes | yes | yes |
| 08 | Album |  | yes | yes | yes |
| 09 | Now Playing |  | yes | yes | yes |
| 10 | Lyrics | yes | yes | yes | yes |
| 11 | Queue |  | yes | yes | yes |
| 12 | Output and handoff | yes | yes | yes | yes |
| 13 | Song menu |  | yes | yes | yes |
| 14 | Profile switcher | yes | yes | yes | yes |
| 15 | Made for you playlist | yes | yes | yes | yes |
| 16 | Year recap (entry card, story player, share sheet; card count, timing and where numbers are computed are superseded by 39 to 41) | yes | yes | yes | yes |
| 17 | Settings |  | yes | yes | yes |
| 18 | Servers and sources | yes | yes | yes | yes |
| 19 | Privacy and data | yes | yes | yes | yes |
| 20 | Empty, error and offline |  | yes | yes | yes |
| 21 | Connect a server | yes | yes | yes | yes |
| 22 | Welcome |  | yes | yes | yes |
| 23 | Mixes tab | yes | yes | yes | yes |
| 24 | System: platform design languages |  | yes | yes | yes |
| 25 | Permissions and system prompts |  | yes | yes | yes |
| 26 | Size classes and platform rules |  | yes | yes | yes |
| 27 | Mac layouts |  | yes |  |  |
| 28 | iPad layouts |  | yes |  |  |
| 29 | Android tablet layouts |  |  | yes |  |
| 30 | Windows and Linux layouts |  |  |  | yes |
| 31 | Live surfaces and rules | yes | yes | yes | yes |
| 32 | Lock screen and Live Activities |  | yes | yes |  |
| 33 | Widgets and controls on Apple devices |  | yes |  |  |
| 34 | Widgets and media surfaces on Android, Windows and Linux |  |  | yes | yes |
| 35 | Buttons, states and haptics |  | yes | yes | yes |
| 36 | Notch, Dynamic Island, cutouts and foldables |  | yes | yes |  |
| 37 | Smart picks: signals and rules | yes | yes | yes | yes |
| 38 | Smart picks: screens and states | yes | yes | yes | yes |
| 39 | Year recap: the story | yes | yes | yes | yes |
| 40 | Year recap: packs and years | yes | yes | yes | yes |
| 41 | Year recap: devices and states | yes | yes | yes | yes |
| 42 | Swipe gestures: mini player, Now Playing and rows | yes | yes | yes |  |
| 43 | Drag and drop on desktop and tablets | yes | yes | yes | yes |
