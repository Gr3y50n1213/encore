# Board 30: Windows and Linux layouts

PNG: `design/wireframes/png/30-windows-and-linux-layouts.png`  
HTML: `design/wireframes/html/Desktop-Windows-Linux.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Windows and Linux · Material 3 Expressive

Windows and Linux get the Android look at desktop density: tonal surfaces, rounded panes, connected button groups and wavy progress, with a title bar, a table, a context menu, hover and focus states, keyboard shortcuts and the system media controls that desktop apps need. Every window can be resized, so the layout follows the width through the same four classes as the tablets.

Windows · main window · Large

Encore

Search songs, albums, artistsCtrl+F

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

Album · 2024 · 11 songs · 41 min

Golden Hour Static

Ember Cove

Play

Shuffle

#Title ▲ArtistAlbumPlaysTime

Slow TideEmber CoveGolden Hour Static12,4083:41

2Paper LanternsEmber CoveGolden Hour Static9,3103:12

3Count the HoursEmber CoveGolden Hour Static7,9044:05

4Golden Hour, StaticEmber CoveGolden Hour Static6,1183:28

5Hold the SkyEmber CoveGolden Hour Static5,2772:57

6Low OrbitEmber CoveGolden Hour Static4,4023:50

7Harbor RadioEmber CoveGolden Hour Static3,8604:19

LyricsUp nextInfo

Playing from Golden Hour Static

Night Market

Pale Harbor

3:55

Salt and Signal

Moth Choir

4:21

Terminal Bloom

Juno Weft

3:10

Tidewater

Pale Harbor

4:02

Brass Arches

Moth Choir

3:36

Slow Tide

Ember Cove

1:243:41

PlayEnter

Play next

Add to queue

Add to playlist▸

LikeCtrl+L

Go to artist

Song info

1. 2

3. 4

5. Linux · Medium window · header bar

Library

Home

Mixes

Library

Search

You

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

6. Now Playing window

LyricsUp nextInfo

Golden Hour Static

Ember Cove

1:24-2:18

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

7. Windows · Compact window

Encore

Library

Albums

Artists

Songs

Golden Hour Static

Ember Cove

Night Market

Pale Harbor

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

8. Mini player · always on top

Slow Tide

Ember Cove

9. Windows · media flyout

Encore⋯

Slow Tide

Ember Cove

1:24

3:41

10. Windows · taskbar thumbnail

Encore - Slow Tide✕

count the hours
as they glow

Linux · media controls (MPRIS)

Encore

Slow Tide

Ember Cove

1:24

3:41

10. Windows · notification area menu

Slow Tide · Ember Cove

Pause

Next

Previous

Open Encore

Settings

Quit Encore

Linux · tray menu (optional)

Slow Tide · Ember Cove

Pause

Next

Previous

Open Encore

Settings

Quit Encore

1. Title bar and window. The app draws a 48 px title bar in the window: icon and name on the left, a centred search field (Ctrl+F), and the three Windows caption buttons, 46 px wide, on the right. The close button turns red on hover and the others get a neutral fill. Dragging the bar moves the window and double-click maximises it. The frame, shadow, resize edges and rounded corners stay the system's. Windows 10 and 11 are supported, with an optional Mica backdrop behind the rail on Windows 11 and a solid tonal fill elsewhere. [CHECK] Snap Layouts on the maximise button must be verified in the first Windows build.

2. Navigation and panes. The same structure as the Android tablet, drawn at desktop density. The expanded rail (240 px) holds the destinations, Pinned mixes, Playlists and the profile. Below 840 px it becomes the 80 px rail, and below 600 px the bottom bar. Panes are rounded containers, with the supporting pane docked at 1200 px and wider. Keyboard: Tab moves through panes, arrow keys inside lists, and every focus shows a 3 px ring in the primary colour.

3. Album table. A desktop table: number, title, artist, album, plays and time. Clicking a header sorts by it and shows an arrow; dragging a header border resizes the column. [CLICK] selects a row, [DOUBLE-CLICK] or Enter plays it, Ctrl and Shift extend the selection, and rows can be dragged to the queue, the Up next pane or a playlist in the rail (rules on board 43). Hover adds a faint tint; the playing row is tonal. Scrollbars are thin and widen when the pointer is near them.

4. Context menu. [RIGHT-CLICK] on a row, or the Menu key or Shift+F10, opens a menu at the pointer, flipped to stay on screen. It holds Play, Play next, Add to queue, Add to playlist (opens a submenu after 200 ms hover), Like, Go to artist and Song info, with shortcuts on the right. Arrow keys move, Enter chooses and Esc closes. It is the same menu that long press opens on touch screens.

5. Player bar and Up next. A tonal bar floats over the panes with cover, title, transport, shuffle and repeat, a wavy progress line with times, and lyrics, queue and volume. [CLICK] the line to seek or drag it for a time tooltip; scrolling over the volume changes it by 5 percent. Space plays or pauses when no text field has focus, and media keys always work. In the Up next pane rows show a drag handle for discoverability, though the whole row drags; dragging shows an insertion line, Delete removes the selected row, and dropping outside the pane returns the row with a spring.

6. Linux window. The Linux window uses a header bar with a sidebar button, the page title, search and the window controls, whose position and look follow the desktop. The app keeps its own Material look on every desktop and does not borrow the GTK theme, but it follows the system light or dark setting. Hover shows a play button on the cover, keyboard focus a 3 px ring. The window works under Wayland and X11 and tiles cleanly with no minimum-size trap beyond 360 by 480 px.

7. Now Playing window. The same Now Playing as the tablet, in its own full window: album gradient, wavy progress, the same three-button transport (Previous round on the left, Next round on the right, play flat while playing and round when paused) and word-by-word lyrics. A chevron (or Esc) returns to the library, F11 enters full screen, and the compact button opens the mini player. The caption buttons stay visible in white. Clicking a lyric line seeks, and scrolling by hand pauses auto-follow for 4 seconds.

8. Compact window. Any window narrower than 600 px shows the phone layout: large title, a navigation bar and the mini player. The smallest window is 360 by 480 px. Resizing re-flows live with no reload, keeps the scroll position and selection, and animates pane changes with a short spring, or a cross-fade with reduced motion.

9. Mini player. A small always-on-top window (compact overlay on Windows; keep-above on Linux where the compositor allows it, otherwise a normal window). Cover, title, wavy progress and transport. The pin keeps it on top, the restore button returns to the main window, and it remembers its place. It never takes focus from another app unless clicked.

10. System surfaces. Not drawn by the app. Windows reads title, artist, cover and position through the System Media Transport Controls for the volume flyout, lock screen and media keys, and shows previous, play and next on the taskbar thumbnail. Linux publishes the same through MPRIS for GNOME, KDE, playerctl and media keys. An optional tray icon with the menu shown here is off by default, and no feature depends on it because GNOME has no tray without an extension.

Design language. Windows and Linux use Material 3 Expressive, built as a small Encore theme in Flutter because Flutter does not ship the Expressive components: color roles from each album, tonal surfaces, connected button groups, 28 px pane corners, a wavy progress painter and spring motion. It is the same look as Android, at desktop density and with desktop input. Size classes. Compact under 600 px, Medium 600 to 839, Expanded 840 to 1199, Large from 1200, exactly as on the tablets, and the layout never checks the operating system. Theme. Light and dark follow the system; dark uses the Material dark tonal roles with the same shapes. Accessibility. Narrator and Orca get labels and roles for every control, text scales with the system setting, contrast stays at least 4.5 to 1, and Reduce motion stops the wave and the gradient drift.
