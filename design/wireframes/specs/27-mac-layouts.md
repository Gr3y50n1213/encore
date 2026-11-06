# Board 27: Mac layouts

PNG: `design/wireframes/png/27-mac-layouts.png`  
HTML: `design/wireframes/html/Desktop-Mac.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Mac · macOS 26 · Liquid Glass

The Mac app is the iPhone and iPad app on a resizable window, with the same glass and the same gradients. It adds what a Mac window needs: a sidebar that floats, a toolbar, a menu bar, shortcuts, hover, right-click, a mini player and a menu bar item. Every window can be any size above 360 by 480 pt, and the layout follows the width, not the device.

Main window · wide

Encore FileEditViewControlsWindowHelp

Thu 5 Nov  9:41

Albums

1,248 albums

Golden Hour Static Ember Cove

Night Market Pale Harbor

Salt and Signal Moth Choir

Terminal Bloom Juno Weft

Tidewater Pale Harbor

Brass Arches Moth Choir

Paper Moon Juno Weft

Slow Static Ember Cove

Low Orbit Ember Cove

Harbor Lights Pale Harbor

Field Notes Moth Choir

Glass Hours Juno Weft

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

Library

Search

Slow Tide

Ember Cove

1:24

3:41

1. 2

3. 4

Same window, dragged narrow

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

6. Now Playing window

LyricsUp NextInfo

Golden Hour Static

Ember Cove

1:24-2:18

Slow tide pulling at the lights

paper lanterns drift away

count the hours as they glow

golden hour, static in the air

hold the sky a little longer

5. Mini player window

Slow Tide

Ember Cove

7. Menu bar item and popover

Thu 5 Nov  9:41

Slow Tide

Ember Cove · Golden Hour Static

1:24

3:41

Open Encore

Mini player

Quit Encore

8. 1

Window and sidebar. The window and its 26 pt corners belong to the system. The sidebar is a floating Liquid Glass panel inset 8 pt, with concentric 20 pt corners, and the traffic lights sit inside it. Home, Mixes, Library, then Pinned mixes and Playlists, with the profile and settings at the foot. Selection is a tinted capsule. Drag the edge to resize from 180 to 300 pt, or hide it with the toolbar button or Cmd+Opt+S.

2. Toolbar. Glass is used only for the controls: back and forward (Cmd+[ and Cmd+] ), the grid and list switch, sort, and the search field. The title stays plain. Search grows to 320 pt on focus (Cmd+F) and drops a results panel under it. Content scrolls under the toolbar with a soft edge, never a hard line.

3. Library grid. Tiles are at least 150 pt wide, so the grid goes from 2 to 8 columns as the window changes, with one 10 pt corner on every cover. Hover lifts a tile and shows a glass play button. Click selects, double-click plays, right-click opens the song menu, and arrow keys move a visible focus ring. Dragging songs or an album onto a playlist in the sidebar adds them; the full rules are on board 43.

4. Player bar. A floating glass capsule over the content, not over the sidebar. Left: cover, title, like. Centre: transport and a draggable scrubber with a time tooltip. Right: lyrics, queue, volume (scroll over it to change). Clicking the cover opens the Now Playing window; the lyrics and queue buttons open a right-hand panel, which squeezes the grid rather than covering it.

5. Now Playing window. Its own window, built from the album the way the phone is: a mesh gradient with glass controls. Lyrics, Up Next and Info are one segmented control. Lyrics sweep word by word, and clicking a line seeks. The green button enters full screen for a large display; Esc leaves. Text flips to dark on light artwork, as on the phone.

6. Narrow window. Below 600 pt the sidebar hides behind a toolbar button and opens as an overlay, the grid drops to two columns, and the player becomes the mini capsule. The smallest window is 360 by 480 pt. Size and position are remembered per window, and nothing is lost when the user drags the corner mid-song.

7. Mini player. Opt+Cmd+M. A small floating glass window that stays above other windows, with the cover, title, scrubber and transport. Hovering shows close and expand. It never takes keyboard focus from the app underneath unless clicked.

8. Menu bar and Dock. An optional menu bar item with a popover for now playing, transport and quick actions; it can be hidden in Settings. The Dock menu has the same controls. Control Center, the lock screen and media keys come from the system Now Playing integration.

Menus. Encore (About, Settings, Servers, Profiles, Quit) · File (New Playlist, Add a server, Export my data) · Edit · View (Show Sidebar, Show Queue, Show Lyrics, as Grid or List, Enter Full Screen) · Controls (Play or Pause Space, Next Cmd+Right, Previous Cmd+Left, Volume Cmd+Up and Down, Like Opt+Cmd+L, Shuffle, Repeat) · Window · Help. Every action in the UI also has a menu item and, where sensible, a shortcut. Material. The Mac, iPad and iPhone share one Liquid Glass look: glass for floating controls, plain surfaces for content. With Reduce Transparency or Increase Contrast the glass becomes solid and gets a clear outline. Accessibility. Full keyboard access, VoiceOver labels on every control, Reduce Motion stops the gradient drift and the lyric sweep becomes a plain highlight.
