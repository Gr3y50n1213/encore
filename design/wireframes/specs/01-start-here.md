# Board 01: Start here

PNG: `design/wireframes/png/01-start-here.png`  
HTML: `design/wireframes/html/Main.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Encore wireframes

Every phone screen is drawn twice with the same structure: iOS 26 with Liquid Glass on the left, Android 16 with Material 3 Expressive on the right. Tablets and desktop windows have their own boards, because every window is resizable. A spec column beside each pair lists what every control does on tap, double tap, long press and swipe, how it guards against accidental presses, which haptic fires, and how it moves.

How to read a screen board

Left phone iOS: SwiftUI, glass tab bar and mini player, spring motion, UIKit haptics

Right phone Android: Jetpack Compose, M3 Expressive shapes, springs and haptic constants

Numbered marks Match the numbered rows in the spec column

Spec rows TAP 2×TAP HOLD SWIPE GUARD HAPTIC MOTION

States Loading, empty, offline, error and signed-out states are listed under each screen

Decision made

Liquid Glass on iPhone, iPad and Mac. Android uses native Jetpack Compose, which ships the Material 3 Expressive components. Flutter has said it is not building them, so Windows and Linux use Flutter with a small Encore theme in the same Expressive look.

Boards on this canvas

System Motion and haptics, gestures and accidental-press rules, album gradients, platform design languages

Browse Home, Library, Search, Album

Playback Now Playing, Lyrics, Queue, Output and handoff

Menus and special Song long-press menu, profile switcher, Made-for-you playlist, year recap

Settings and setup Settings, servers and sources, privacy and data, empty, error and offline states, connect a server, welcome, Mixes tab, permissions and system prompts

Tablet and desktop Size classes and platform rules, then Mac, iPad, Android tablet, and Windows and Linux: sidebar or rail, panes, player bar, system media controls

Lock screen and widgets Live surfaces and rules, the sleep timer, lock screen and Live Activities, widgets and controls on Apple devices, and widgets and media surfaces on Android, Windows and Linux

Polish and smart picks Buttons, states and haptics; notch, Dynamic Island, cutouts and foldables; how smart tracks and playlists are chosen; and the screens that explain and control them
