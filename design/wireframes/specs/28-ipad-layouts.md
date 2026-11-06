# Board 28: iPad layouts

PNG: `design/wireframes/png/28-ipad-layouts.png`  
HTML: `design/wireframes/html/Tablet-iPad.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iPad · iPadOS 26 · Liquid Glass

The iPad app is the iPhone design given more room: a floating glass sidebar, a toolbar, an inspector and a floating mini player. In iPadOS 26 every app is a window the user can resize, so nothing here depends on the device or its orientation. The layout follows the window width through four size classes, and rotating, Split View, Slide Over, Stage Manager and an external display all use the same rules.

Expanded window · 840 to 1199 pt

9:41  Thu 5 Nov100%

Album

Golden Hour Static

Ember Cove

2024 · 11 songs · 41 min

Play

Shuffle

Slow Tide3:41

2Paper Lanterns3:12

3Count the Hours4:05

4Golden Hour, Static3:28

5Hold the Sky2:57

6Low Orbit3:50

7Harbor Radio4:19

LyricsUp NextInfo

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

Search

Home

Mixes

Library

Pinned

Evening Drive

Rainy Focus

Drive Home

Playlists

Road trip

Study

You
You

Slow Tide

Ember Cove

1. 2

3. 4

5. Compact window · Stage Manager

9:41  Thu 5 Nov100%

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

8. Medium window · 600 to 839 pt · sidebar as overlay

9:41  Thu 5 Nov100%

Library

Albums

Golden Hour Static Ember Cove

Night Market Pale Harbor

Salt and Signal Moth Choir

Terminal Bloom Juno Weft

Tidewater Pale Harbor

Brass Arches Moth Choir

Slow Tide

Ember Cove

Search

Home

Mixes

Library

Pinned

Evening Drive

Rainy Focus

Drive Home

Playlists

Road trip

Study

You
You

6. 7

Now Playing · full screen

LyricsUp NextInfo

Golden Hour Static

Ember Cove

1:24-2:18

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

7. One app, four widths

Full screen · Large about 1376 pt

Two thirds · Expanded about 900 pt

Half · Medium about 690 pt

Third · Compact about 450 pt

9. 1

Sidebar. A floating Liquid Glass panel inset 10 pt, with a search field and a toggle at the top, then Home, Mixes and Library, Pinned mixes, Playlists, and the profile and settings at the foot. Rows are 40 pt high. Selection is a tinted capsule. [TAP] opens the section. [LONG PRESS] opens a menu (Pin, Rename, Remove) with a soft haptic. Drag an album onto a pinned mix or playlist to add it. Swipe it away or tap the toggle to hide it; the state is remembered for each window.

2. Toolbar. Glass is used only for the controls: back, share and like, and the inspector toggle. The title is plain and appears in the bar only after the album title scrolls out of view. Content scrolls under the controls with a soft edge. [TAP] back returns with a gentle slide that follows the finger, as on the phone.

3. Album page. The cover has 18 pt corners and the page is the album's own colours. Play is the one filled button; Shuffle is glass. [TAP] a song starts it and keeps the album as the queue. [SWIPE] right on a row plays it next, left adds it to the queue, and a short swipe does nothing, so a brush of the screen never acts. A full swipe fires a haptic and shows an Undo toast. [LONG PRESS] opens the song menu.

4. Inspector. A right-hand pane, 280 to 340 pt wide, that holds Lyrics, Up Next and Info in one segmented control. Lyrics sweep word by word, and tapping a line seeks to it. It is toggled by the toolbar button or Cmd+Opt+I, and it squeezes the album page rather than covering it. Below the expanded width it opens as a sheet instead.

5. Mini player. A floating glass capsule across the content and inspector, never across the sidebar. Left: cover, title, artist. Centre: transport. Right: like, AirPlay, queue. [TAP] the cover or title expands to Now Playing. [SWIPE UP] does the same, following the finger. [LONG PRESS] opens quick actions. The buttons are at least 44 pt, with 12 pt between them, so a stray touch on the capsule only expands it.

6. Medium window. From 600 to 839 pt the sidebar is hidden and the toolbar button opens it as an overlay. [TAP] the button, or swipe from the left edge, to open. Tap the dimmed content, swipe left, or choose a section to close. The overlay turns solid with Reduce Transparency. The grid shows three columns and the inspector becomes a sheet.

7. Now Playing. A full-screen view built from the album, with a grabber at the top. Artwork and controls sit on the left and lyrics fill the right, which gives each line more room than on the phone. [SWIPE DOWN] from the grabber dismisses it with the matching spring back into the mini player. In a window below 600 pt it becomes the phone layout. Text turns dark on light artwork.

8. Compact window. Any window narrower than 600 pt uses the iPhone layout: a floating glass tab bar with the mini player above it. This is what Slide Over, a third of the screen and a small Stage Manager window show. The corner handle resizes freely; the layout changes at the class boundaries with a short spring, and the scroll position and selection stay. The window controls are the system's.

9. Width, not device. The classes are Compact under 600 pt, Medium 600 to 839, Expanded 840 to 1199 and Large from 1200. Example on a 13-inch iPad Pro (the system decides the real widths): full screen is Large, two thirds is Expanded, half is Medium and a third is Compact. Rotating works the same way. Nothing in the app checks for "iPad".

Pointer, keyboard, multitasking. Hover lifts a control and shows a glass highlight; right-click opens the same menu as long press. Space plays or pauses, Cmd+Left and Cmd+Right change song, Cmd+F searches, Cmd+Opt+S toggles the sidebar. The menu bar lists every action. Music keeps playing in Split View and Stage Manager; an external display shows Now Playing in full screen.

Rotation and resizing. Rotation and window changes re-flow the layout without reloading it: the playing song, the open page, the scroll position, the selection and the sidebar state are kept. With Reduce Motion the spring becomes a cross-fade. Material. The iPhone, iPad and Mac share one Liquid Glass look: glass for floating controls only, plain surfaces for content, concentric corners (an inner radius is the outer radius minus its inset), and a solid surface with a clear outline for Reduce Transparency and Increase Contrast. Accessibility. Every control has a VoiceOver label, and Dynamic Type grows rows rather than clipping text. Back and forward. Edge swipe and the toolbar arrow do the same thing.
