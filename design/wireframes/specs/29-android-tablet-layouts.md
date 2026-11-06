# Board 29: Android tablet layouts

PNG: `design/wireframes/png/29-android-tablet-layouts.png`  
HTML: `design/wireframes/html/Tablet-Android.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Android tablet · Material 3 Expressive

The Android tablet app is the phone app with an adaptive layout, built the Material way: panes are rounded containers, the rail grows into an expanded rail, and shapes, tonal colour and spring motion carry the Expressive look. Android 16 lets users resize every app on a large screen, so the layout follows the window width in four size classes, whether the window is a tablet, a split screen, a freeform window or a folded phone.

Large window · 1200 dp and up · rail, list, detail and supporting pane

9:41100%

Home

Mixes

Library

Search

Pinned

Evening Drive

Rainy Focus

Drive Home

Playlists

Road trip

Study

You
You

Library

Albums

Artists

Songs

Golden Hour Static

Ember Cove · 2024

Night Market

Pale Harbor · 2023

Salt and Signal

Moth Choir · 2024

Terminal Bloom

Juno Weft · 2022

Tidewater

Pale Harbor · 2021

Brass Arches

Moth Choir · 2020

Paper Moon

Juno Weft · 2019

Album · 2024

Golden Hour Static

Ember Cove

11 songs · 41 min

Play

Shuffle

Slow Tide3:41

2Paper Lanterns3:12

3Count the Hours4:05

4Golden Hour, Static3:28

5Hold the Sky2:57

6Low Orbit3:50

LyricsUp nextInfo

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

Slow Tide

Ember Cove

1:243:41

1. 2

3. 4

5. Medium window · 600 to 839 dp · supporting pane as a side sheet

9:41100%

Home

Mixes

Library

Search

You

Library

Albums

Artists

Songs

Golden Hour Static Ember Cove

Night Market Pale Harbor

Salt and Signal Moth Choir

Terminal Bloom Juno Weft

Tidewater Pale Harbor

Brass Arches Moth Choir

Slow Tide

Ember Cove

LyricsUp nextInfo

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

6. Now Playing · full screen

LyricsUp nextInfo

Golden Hour Static

Ember Cove

1:24-2:18

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

7. One app, four widths

Large rail, list, detail, supporting

Expanded rail, list, detail

Medium rail, one pane

Compact bar, one pane

9. Compact window · freeform

Encore

Home

Made for you

Evening Drive

Mix

Rainy Focus

Mix

Recently played

Salt and Signal

Moth Choir

Terminal Bloom

Juno Weft

Slow Tide

Ember Cove

Home

Mixes

Library

Search

8. Foldable · tabletop posture

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

Golden Hour Static

Ember Cove

10. Foldable · book posture

Library

Golden Hour Static

Ember Cove · 2024

Night Market

Pale Harbor · 2023

Salt and Signal

Moth Choir · 2024

Terminal Bloom

Juno Weft · 2022

Tidewater

Pale Harbor · 2021

Golden Hour Static

Ember Cove

Play

Shuffle

Slow Tide

2Paper Lanterns

3Count the Hours

10. | | Compact · under 600 | Medium · 600 to 839 | Expanded · 840 to 1199 | Large · 1200 and up

| Navigation | Navigation bar | Rail, 80 dp | Expanded rail, 240 dp | Expanded rail, 240 dp

| Panes | One | One, detail replaces list | List and detail | List, detail and supporting

| Supporting pane | Bottom sheet | Side sheet | Side sheet | Docked

| Player | Mini player above the bar | Bar over the content | Bar over the panes | Bar over all panes

| Albums | Two columns | Three columns | List pane | List pane

1. Navigation. One set of destinations in three shapes. Compact (under 600 dp) uses a navigation bar. Medium (600 to 839) uses an 80 dp rail with labels under the icons. From 840 dp the expanded rail, 240 dp, puts labels beside the icons, adds Pinned mixes and Playlists under the main destinations, and keeps the profile at the foot. The menu button collapses it to the rail. The selected item is a full-round tonal pill. [TAP] opens it; tapping the selected one scrolls to the top. [BACK] predictive back previews the previous screen while the edge is dragged.

2. List and detail. Library is a 340 dp list with the chosen album open beside it. The panes are separate containers with 28 dp corners and 12 dp between them, not one slab. A tonal container marks the selected row, and the list keeps its scroll position. Chips switch between Albums, Artists and Songs. On Medium and Compact the detail replaces the list and Back returns to it with a shared-axis motion.

3. Album detail. Cover with 28 dp corners, the title, and a connected button group: Play (filled), Shuffle (tonal) and Add (tonal icon). Outer corners are fully round and inner ones 8 dp; a pressed button goes fully round and its neighbours narrow, with a spring of about 200 ms. Tracks are a grouped list with 24 dp outer and 6 dp inner corners and 2 dp gaps. [TAP] plays and keeps the album as the queue. [SWIPE] start to end plays next, end to start likes, and only past 40 percent of the row, with an Undo snackbar. [LONG PRESS] starts selection with a haptic and a contextual bar.

4. Supporting pane. Lyrics, Up next and Info in a 340 dp pane on the right, switched by a connected toggle group. It takes the album's gradient, and text turns dark on light artwork. Lyrics sweep word by word and tapping a line seeks to it. Scrolling by hand pauses auto-follow for 4 seconds and shows a "Back to now" chip. Large shows it docked; Expanded and Medium open it as a modal side sheet from the lyrics button; Compact uses a bottom sheet.

5. Now playing bar. A tonal container floating 12 dp inside the panes, with cover, title and artist, transport, a wavy progress line with times, and like, queue and volume. The play button is a rounded square when paused and morphs to a circle while playing, with a spring of about 250 ms. [TAP] the bar or [SWIPE UP] expands it with a container transform; swiping down collapses it. [LONG PRESS] opens quick actions. Buttons are 48 dp with at least 8 dp between them, so a stray touch only expands the bar.

6. Medium window. From 600 to 839 dp the expanded rail becomes the 80 dp rail, the library is a three-column grid, and the detail replaces the list. The lyrics button opens the supporting pane as a modal side sheet over a scrim; [TAP] the scrim, swipe it right, or press Back to close it. The now playing bar spans the content area.

7. Now Playing. Full screen on a gradient built from the album. Transport is three separate rounded-square buttons: previous and next are tonal and round on the side their arrow points to (10 dp on the side beside play), and play changes shape with its state (a rounded square while playing, fully round when paused, tighter while pressed). Progress is a wavy line that waves while playing and goes flat when paused, and static with Reduce animations. Lyrics fill the right side. Below 600 dp it uses the phone layout. [SWIPE DOWN] collapses it back into the bar.

8. Compact window. On screens of 600 dp or more, Android 16 ignores orientation and resizability locks for apps that target API 36, so the app resizes freely in split screen, freeform windows and on a foldable. Under 600 dp the phone layout appears: a large app bar, the mini player and a navigation bar. The caption bar and resize handles belong to the system. State survives every resize, and playback never restarts because it lives in the media session service.

9. Width, not device. Compact under 600 dp, Medium 600 to 839, Expanded 840 to 1199, Large from 1200, the same four classes as on Apple devices. Large shows rail, list, detail and supporting pane; Expanded drops the supporting pane to a sheet; Medium keeps one pane; Compact swaps the rail for the bar. The app reads the window class and never asks for a device type. Panes animate with a spring, or cross-fade with Reduce animations.

10. Foldables. Jetpack WindowManager reports the fold. In tabletop posture (half open, horizontal hinge) Now Playing puts artwork and lyrics on the top half and the controls on the bottom half, within reach. In book posture (vertical hinge) the list sits on one side and the detail on the other. Nothing important is ever placed under the hinge, and a flat screen uses the normal layout.

Input. Space plays or pauses, Ctrl+F searches, Ctrl+Left and Ctrl+Right change song, and media keys work with the screen off. With a mouse or trackpad, hover shows a state layer on rows and buttons, right-click opens the long-press menu, and scrolling over the volume changes it. Accessibility. TalkBack gets a label and role on every control, and the wavy progress is announced as a slider. Font scale up to 200 percent grows the rows rather than clipping. Targets are at least 48 dp. Material. The Android app is native Jetpack Compose with Material 3 Expressive: tonal surfaces, expressive shapes, connected button groups and spring motion. Windows and Linux carry the same language in Flutter, and the next board shows them.
