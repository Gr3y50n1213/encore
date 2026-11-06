# Board 06: Library

PNG: `design/wireframes/png/06-library.png`  
HTML: `design/wireframes/html/Library.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Library

1. Filter chips. [TAP] switches the list; one chip is always active. [SWIPE] scrolls the row. The last choice is remembered per profile. [HAPTIC] selection tick on iOS, [SEGMENT_TICK] on Android.

2. Sort. [TAP] opens a menu: Recently added, Title, Artist, Most played, Fewest skips. The list reorders with a spring, rows keep their identity so nothing flickers. [GUARD] a menu tap needs a second touch; releasing the first touch on an item does nothing.

3. Grid or list. [TAP] toggles. iOS uses a two-segment glass control; Android uses a connected button group of two: the selected one is fully round and filled with the tonal color, the other has a tighter inner corner, and the shapes morph as the choice changes. [HAPTIC] toggle on. The choice is remembered per chip.

4. Album tile or row. [TAP] opens the album. [HOLD] menu: Play, Play Next, Add to Queue, Add to Playlist, Download, Go to Artist, Remove from Library. [SWIPE] (list mode) right: Play Next; left: Add to Queue. [2×TAP] nothing. [GUARD] Remove needs a confirm sheet; the queue actions show an Undo toast.

5. Download badge. States: not downloaded (none), downloading (ring), saved (badge), failed (warning dot). [TAP] on a ring pauses, on a warning retries. Offline, items without the badge dim and a tap shakes with the warning haptic and "Not downloaded".

6. Fast scroll. List mode only. iOS: A to Z index on the right edge with a tick per letter. Android: draggable scroll handle with a letter bubble. [GUARD] the strip is 24 pt wide and sits inside the edge-back dead zone, so back swipes never grab it.

States. No server yet: full-screen card, "Connect a server", button to onboarding. Syncing: top row with count and a thin progress line, content stays usable. Empty filter: "Nothing here yet" with the reason (for example no downloads) and one action. Pull down syncs with the server; the haptic fires when the pull arms.
