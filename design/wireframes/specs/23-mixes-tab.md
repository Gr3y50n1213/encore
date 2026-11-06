# Board 23: Mixes tab

PNG: `design/wireframes/png/23-mixes-tab.png`  
HTML: `design/wireframes/html/Mixes.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Mixes tab

1. Title and stamp. The tab shows the five mixes generated for the day, newest first by relevance, and a line saying when and where they were made. Pull to refresh asks for a new set only if it has been over 6 hours.

2. Cards. A 2-column grid on phones. The cover is a mosaic of four song covers. Title: two to four plain words. One short line says what the mix is. [TAP] opens the mix page. On Android each generated mix takes one shape from the Material shape set (cookie, squircle, arch, clover), picked from the mix id so it stays the same for that mix. Real album art never gets these shapes; it keeps one plain radius on all four corners. [HOLD] menu: Play, Shuffle, Pin, Why these songs, Hide this mix.

3. Play from the card. [2×TAP] does nothing. A small play button on the cover plays the mix directly, with the replace-queue Undo toast. [GUARD] the play button's hit area is 44 pt and sits at the cover's corner, away from the card's own tap area.

4. Pinned. A pinned mix keeps its songs until the user refreshes it, shows a "Pinned" label, and sorts first. Up to 10 are pinned; beyond that, Pin asks to unpin another.

Below the grid. A "Because you listened to" row, up to 3 shelves, each a horizontal strip of albums or artists, followed by "Your recap" when it is available. Nothing is infinite; the page ends.

Generation. At 6:00 AM or the first launch after it, on the device, from the play log and skips, never from genre alone. Five mixes need at least four distinct moods; fewer moods make fewer mixes, down to a minimum of two. Hidden mix. Removed from today's set and used as a signal; Undo for 5 s. Empty. See the states board. Tablet and desktop. 3 to 5 columns.
