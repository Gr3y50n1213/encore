# Board 13: Song menu

PNG: `design/wireframes/png/13-song-menu.png`  
HTML: `design/wireframes/html/ContextMenu.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Song menu

1. Opening. [HOLD] 400 ms on Android (it follows the system setting) or the system menu timing on iOS, on any song row or tile. The row lifts to 1.03 with a shadow, everything else dims and blurs. iOS menu morphs out of the row as one glass shape; Android's popup scales from the row. [HAPTIC] fires at 400 ms, [LONG_PRESS] on Android. [GUARD] moving more than 10 pt before 400 ms cancels it, so scrolling never opens a menu.

2. Queue group. Play Next, Add to Queue, Add to Playlist. Queue actions close the menu and show a toast with Undo. Add to Playlist opens a sheet where several playlists can be ticked at once, plus a New Playlist row.

3. Library group. Like toggles (the label becomes Unlike), Download becomes Remove Download when saved, Share sends a link to the song's metadata, never the file.

4. Navigate group. Go to Album and Go to Artist are hidden on pages that already are that album or artist. Why this song appears only for songs the app picked.

5. Destructive group. Separated by a gap and red text, always last. Not interested hides the song from suggestions and trains the recommender, with Undo for 5 s. Remove from Library needs a confirm sheet whose destructive button sits apart from Cancel.

Choosing. [TAP] an item. [GUARD] lifting the finger without sliding picks nothing, and the first item is never auto-selected. [SWIPE] or tap outside closes with no change. Items past the screen edge scroll inside the menu.

States. Offline: Download and Share dim with a reason. Song unavailable: only Remove and Go to Album remain. Already in the queue: Add to Queue shows a small check and adds a second copy only on a deliberate second tap. Multi-select (Library, Playlist): the same menu applies to every selected song, and counts appear in the labels.
