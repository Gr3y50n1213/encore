# Board 15: Made for you playlist

PNG: `design/wireframes/png/15-made-for-you-playlist.png`  
HTML: `design/wireframes/html/MadeForYou.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Made for you playlist

1. Cover and title. The cover is a 2×2 mosaic of the artwork from four of its songs, so it always looks like this playlist, and the page tint comes from the same palette as on the gradients board. The title is two to four plain words, such as "Quiet hours" or "Gym push". [HOLD] cover opens Share and Save Image.

2. Description and stamp. One short line saying what the playlist is based on, in plain terms, with no hype. The stamp shows when it was last made. Both are generated on the device from the mood clusters.

3. Play and Shuffle. [TAP] replaces the queue, with the same 5 s Undo toast as an album. Play keeps the order the playlist was ranked in.

4. Pin. [TAP] turns it into a saved playlist, so tomorrow's refresh will not replace it. [HAPTIC] success, and the button fills. Unpin asks nothing, since the playlist returns to the daily set. [GUARD] a pinned playlist's songs still update only when the user taps Refresh.

5. Song rows with a reason. Each row has one short reason under the title, such as "Played 14 times at night". [TAP] plays from here. [HOLD] the song menu, with Not in this playlist added. [SWIPE] left: Not in this playlist, which removes the song and teaches the mood model that it does not fit. [SWIPE] right: Play Next. Undo for 5 s.

6. More menu. Why these songs, Refresh now, Save as playlist, Hide this mix, Share. Why these songs opens a sheet with the signals in words: time of day, finished versus skipped counts, and the songs that anchor the mood. Nothing in it leaves the device.

Refresh now. Asks for confirmation if the playlist is pinned or is being played. The new songs fade in over 300 ms and the stamp changes. The previous version stays reachable through Undo for 5 s.

Not enough history. Under about 40 plays on the account, the Mixes tab says "Listen for a few days and mixes will appear here", with a Play something button. Offline. Songs not downloaded are dimmed, and the Play button starts at the first downloaded song. Songs gone from the server. Dropped from the list at the next refresh, with a quiet line, "2 songs are no longer on your server". New set. Five playlists are made at 6:00 AM or on the first launch after it; a changed library only adds to the pool.
