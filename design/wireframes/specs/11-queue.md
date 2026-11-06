# Board 11: Queue

PNG: `design/wireframes/png/11-queue.png`  
HTML: `design/wireframes/html/Queue.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Queue

1. Shuffle, Repeat, Autoplay. [TAP] toggles. Repeat cycles off, all, one, with a "1" badge. Shuffle reorders Up Next only, never History. [HOLD] Autoplay opens the Familiar to Adventurous slider. [HAPTIC] toggle on and off; on Android the pressed button morphs between round and square.

2. History. Collapsed by default. [TAP] "Show 12" expands, newest at the bottom. Rows are dimmed; [TAP] a row plays it again and pushes the current song into History.

3. Now playing row. Highlighted, with live level bars. [TAP] returns to the player. [SWIPE] none, so the current song cannot be dismissed by a slip.

4. Up Next rows. [TAP] jumps to the song; skipped songs move to History and a toast offers Undo. [HOLD] the handle lifts the row to reorder, with a haptic on pick up and drop. [SWIPE] right: Play Next. Left, past 60%: Remove, with Undo for 5 s; the haptic fires when the action arms. [GUARD] the row body ignores drags, only the handle reorders, so scrolling never moves a song.

5. Autoplay rows. The same, plus swipe left on the red "Not this" action, which removes the song and counts as a strong dislike. [HOLD] menu includes "Why this song". Reordering an autoplay row moves it into Up Next.

6. Clear. [TAP] opens a confirm sheet with a separated destructive "Clear Up Next". Autoplay is not cleared; it refills from the current song.

States. Empty Up Next: "Nothing queued. Autoplay continues with songs you'll like." Autoplay off: the section is one row, "Autoplay is off", with a toggle. Refilling: three skeleton rows. Offline: songs that are not downloaded dim and are skipped automatically. Songs added from anywhere show a toast, "Added to Up Next", with Undo.
