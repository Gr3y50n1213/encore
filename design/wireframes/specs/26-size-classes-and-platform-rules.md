# Board 26: Size classes and platform rules

PNG: `design/wireframes/png/26-size-classes-and-platform-rules.png`  
HTML: `design/wireframes/html/Large-Screens.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Size classes and platform rules

Every window on every platform can be resized, so the app has one layout engine with four size classes and two design languages. Apple devices share Liquid Glass. Android has Material 3 Expressive, built natively, and Windows and Linux get the same Expressive look from a small Flutter theme. This board is the rulebook; the boards beside it show each family at every width.

Apple · iPhone, iPad, Mac · Liquid Glass

Large sidebar, content, inspector

Expanded sidebar, content, inspector on demand

Medium content, sidebar as overlay

Compact tab bar

Android, Windows, Linux · Material 3 Expressive

Large rail, list, detail, supporting

Expanded rail, list, detail

Medium rail, one pane, side sheet

Compact navigation bar

Platforms

| Platform | Look | Built with | Window

| iPhone | Liquid Glass | SwiftUI | One window, both orientations

| iPad | Liquid Glass | SwiftUI | Full screen, Split View, Slide Over, Stage Manager, external display

| Mac | Liquid Glass | SwiftUI, macOS 26 | Free windows, mini player, menu bar item

| Android phone | Material 3 Expressive | Jetpack Compose, Media3 | Full screen, split screen

| Android tablet, foldable | Material 3 Expressive | Jetpack Compose, adaptive layouts | Split screen, freeform windows, fold postures

| Windows | Material 3 Expressive, desktop density | Flutter, Encore theme | Free windows, Snap Layouts, compact overlay

| Linux | Material 3 Expressive, desktop density | Flutter, Encore theme | Free windows on Wayland and X11, header bar

What changes with the window width

| | Compact · under 600 | Medium · 600 to 839 | Expanded · 840 to 1199 | Large · 1200 and up

| Navigation on Apple | Floating glass tab bar | Sidebar as an overlay | Floating glass sidebar | Floating glass sidebar

| Navigation on Android, Windows, Linux | Navigation bar | Rail, 80 dp | Expanded rail, 240 dp | Expanded rail, 240 dp

| Panes on Apple | One | One | Content, inspector on demand | Content and docked inspector

| Panes on Android, Windows, Linux | One | One, detail replaces list | List and detail | List, detail and supporting

| Lyrics and Up next | Inside Now Playing | Sheet or side sheet | Inspector or side sheet | Docked

| Player | Mini player above the bar | Floating over the content | Floating over the content | Floating over content and side pane

| Album grid | 2 columns | 3 columns | 4 to 5 columns | 6 to 8 columns

1. Width, not device. The class comes from the width of the window and nothing else: never the device type, the operating system or the orientation. Rotating a tablet, folding a phone, dragging a Mac window and snapping a Windows window all go through the same four classes. Every board is tested by dragging an edge slowly through all of them.

2. State survives a resize. The playing song, the open page, the scroll position, the selection and the sidebar state are kept when the class changes. Playback lives in the media session (the player service on Android, the audio session on Apple, the player engine on desktop), never in the window, so resizing never interrupts the music.

3. Windows are resizable. iPad apps are free windows in iPadOS 26, so there is no full-screen lock and every orientation works. Android 16 ignores orientation and resizability locks for apps that target API 36 on screens of 600 dp and wider. Mac, Windows and Linux windows go down to 360 by 480 points. Nothing is drawn that needs a fixed size.

4. Input follows the window, not the device. Touch targets are at least 44 pt on Apple devices and 48 dp on Android. Any window can also be used with a pointer (hover, right-click) and a keyboard (every control reachable, a visible focus ring, the same shortcuts on iPad, Mac, Windows and Linux, with Cmd or Ctrl). Long press and right-click open the same menu.

5. The system draws its own surfaces. Lock screen, Control Center, the Android shade, the Windows volume flyout and taskbar thumbnail, GNOME and KDE media controls, and media keys are all drawn by the system from the metadata the app supplies: title, artist, cover, position and commands. The app never imitates them and never depends on them.

6. One behaviour, two looks. Structure, wording, gestures, haptic meanings, handoff and accessibility are the same everywhere; only the material changes. Liquid Glass on iPhone, iPad and Mac. Material 3 Expressive on Android and, rebuilt as a small Flutter theme, on Windows and Linux. A behaviour change is made once and applied to every platform.

7. Motion. Springs only. A change of class animates panes with a short spring of about 350 ms, and with Reduce Motion or Reduce animations it becomes a cross-fade. The lyric sweep, the gradient drift and the wavy progress stop under the same setting.

8. Accessibility floor. Labels and roles on every control, Dynamic Type and font scale up to 200 percent without clipping, contrast of at least 4.5 to 1, and a solid fallback for every glass or translucent surface with Reduce Transparency or Increase Contrast. VoiceOver, TalkBack, Narrator and Orca all read the same label text.

Where each is drawn. The Mac, iPad, Android tablet and Windows and Linux boards show each family at every width with the system pieces around it. The phone boards are the Compact class. Not drawn. Apple TV, watchOS, Wear OS, car displays and web are outside this version.
