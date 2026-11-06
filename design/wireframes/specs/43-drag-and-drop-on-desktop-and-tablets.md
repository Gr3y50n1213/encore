# Board 43: Drag and drop on desktop and tablets

PNG: `design/wireframes/png/43-drag-and-drop-on-desktop-and-tablets.png`  
HTML: `design/wireframes/html/Gestures-DragDrop.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Drag and drop on Mac, Windows, Linux, iPad and tablets

Any list is a source and any playlist is a target, so building a playlist is a few drags with no dialogs. The rules are the same on every platform; only the modifier keys and the look change. Left: Mac with Liquid Glass, three songs dragged to a playlist. Right: Windows and Linux with the Expressive look, one song dragged into Up Next.

Mac · Liquid Glass

Home

Mixes

Library

Playlists

Road Trip

Late Night

Focus

+ New Playlist

Songs

3 selected · Shift or ⌘ to extend

1. Slow Burn Season Marigold Ave3:27

2. Glasshouse Marigold Ave3:34

3. Paper Lanterns Lowlands3:41

4. Low Tide Radio Sable Coast3:48

5. Night Shift Neon Orchard3:55

6. Harbor Lights Lowlands3:22

7. Afterglow The Quiet Hours3:29

Golden Hour Static

3. +

Add 3 songs to Late Night

Windows and Linux · Material 3 Expressive

Home

Mixes

Library

Playlists

Road Trip

Late Night

Focus

Songs

1 selected · Ctrl or Shift to extend

1. Slow Burn Season Marigold Ave3:27

2. Glasshouse Marigold Ave3:34

3. Paper Lanterns Lowlands3:41

4. Low Tide Radio Sable Coast3:48

5. Night Shift Neon Orchard3:55

6. Harbor Lights Lowlands3:22

7. Afterglow The Quiet Hours3:29

Up next

Glasshouse
Marigold Ave

Paper Lanterns
Lowlands

Harbor Lights
Lowlands

Golden Hour Static

+

Insert after Glasshouse
Alt: add to end instead

What each drop does

Road Trip

Late Night

Focus

Playlist in the sidebar

Adds to the end. The row outlines and the tip names it. Already-there songs are skipped.

Focus

+ New Playlist

New Playlist row

Creates a playlist named after the album or the first artist, with the name field already open to rename.

Glasshouse

Paper Lanterns

Playlist page, between rows

An insertion line shows where it lands. From another list it adds; inside the same playlist it reorders.

Playing: Golden Hour Static

Glasshouse

Up Next

Between rows it inserts there. Dropped on the top strip it plays next. Hold Alt or Option to add to the end instead.

Golden Hour Static    Play next

Player bar

Plays next. It never replaces what is playing, so a stray drop cannot stop the music.

Liked Songs

Like 3 songs

Liked Songs

Likes every dropped song and syncs, like tapping the heart on each.

Albums, mixes, artists

Albums come from your files and mixes are made by the app. Drop on a playlist.

Targets that refuse

Dim, a not-allowed cursor and a one-line reason after 600 ms. The drag image slides back if released there.

Toolbar

Slow Burn Season

Anything else

Dropping on the toolbar, empty space or another app does nothing. The drag image slides back. Nothing is ever created by accident.

After the drop and in awkward cases

Added 3 songs to Late Night

A toast for 6 s with Undo. Ctrl or Cmd+Z undoes the last drop for 10 s in that window.

Added 2 · 1 already there

Duplicates are found by the song itself, not the file, so a song on two albums counts as one. The toast offers Add anyway.

Added 214 songs?

Dropping an artist or a very large selection (over 100 songs) asks first, because it is hard to review afterwards.

Dragging near an edge

Lists and the sidebar scroll when the pointer is within 40 px of the top or bottom, faster the closer it gets.

Narrow window

Under 600 the sidebar is hidden. Holding a drag at the left edge for 600 ms opens it as an overlay, and it closes after the drop.

Escape or a bad drop

Esc cancels. Dropping outside any target cancels. The drag image slides back to where it came from.

1. What can be dragged. Songs from any list: Library, Album, Artist, Search, a playlist, Up Next, History, a mix and Recently played. Whole albums, artists and playlists, which become their songs in order. The cover and title in the player bar, which drags the current song. Unavailable songs are not draggable. Guest cannot save, so playlist targets are disabled with the reason "Guest cannot save playlists".

2. Selecting. Click selects. Shift extends a range. Ctrl (Windows, Linux) or Command (Mac) toggles one row. Ctrl or Command+A selects all in view. Dragging an empty gutter draws a selection box. Selection is by song, so it survives sorting and scrolling, up to 500 songs.

3. Starting a drag. Press on a row and move 5 px. A press without movement never drags, and double click still plays. Pressing an unselected row drags just that row. Pressing a selected row drags the whole selection. On touch (iPad, Android tablets, Windows touch) a long press of 300 ms lifts the row with a haptic and the finger can drag; on iPad, tapping more rows while dragging adds them.

4. The drag image. The top three covers stacked, a count badge when more than one, and a green plus when the target will accept. The target outlines in the accent colour and a tip names the result. Songs go in the order of the list as shown, not the order they were selected.

5. Copy, not move. Dropping on another playlist always copies, so a song is never lost from where it came from. Hold Command (Mac) or Shift (Windows, Linux) to move instead: it adds to the target and removes from the source. The badge changes from plus to an arrow. Reordering inside one playlist or the queue is always a move.

6. Queue and player bar. Drops in Up Next insert at the line. The top strip and the player bar mean Play Next. Alt or Option means add to the end. Nothing dropped ever starts playing by itself.

7. Albums. Albums come from the tags and folders on the server, and the app never rewrites your files or your server. So songs cannot be added to an album. Album tiles and rows refuse the drop and say why. A playlist is the place for a collection you build yourself.

8. One write. A drop is one call in the core with the whole list and the position, one sync event and one undo entry, even for 500 songs. It works offline and syncs later. Where the playlist lives is decided when it is created and the drop uses the same call.

9. No drag required. Every drop has a menu twin: right click, Add to Playlist and a New Playlist entry, Play Next and Add to Queue, plus a menu bar item with its shortcut. Screen readers get Add to playlist as a row action. Keyboard: select rows, open the menu with the context key, choose.

10. Nothing leaves the app. No audio file is ever created by a drag. Dragged into a text field or another app it gives only "Title · Artist" as text.

11. Feedback. Valid target: outline, plus badge, tip. Invalid: dim, not-allowed cursor, reason after 600 ms. After the drop the new rows flash the accent for 300 ms and the sidebar count updates. No sound. No haptic on desktop, except the Mac trackpad's own alignment feel. Touch devices use the pick-up and drop haptics from board 35.

12. Platform parts. Apple: SwiftUI draggable and dropDestination with Transferable. Android tablets: Compose dragAndDropSource and dragAndDropTarget. Windows and Linux: Flutter Draggable and DragTarget. Names are to confirm against the pinned versions. All of them read the same payload type from the core.
