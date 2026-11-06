# Board 09: Now Playing

PNG: `design/wireframes/png/09-now-playing.png`  
HTML: `design/wireframes/html/NowPlaying.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Now Playing

1. Handle and back. [SWIPE] down follows the finger and collapses into the mini player; release past 30% height or fast velocity commits. [TAP] the arrow (Android) collapses. [GUARD] ignored while the scrub bar is held.

2. Artwork and background. The background is built from this album's cover, as on Apple Music: colors are sampled from the artwork, so every album looks different, and it cross-fades over 600 ms when the track changes to a new album. [2×TAP] likes with a heart pop; [HAPTIC] medium. [HOLD] menu: Share, Go to Album, Go to Artist, Add to Playlist, Download. [SWIPE] left or right skips when the drag passes 60 pt or fast velocity. Paused: art shrinks to 88%; playing: full size.

3. Title and artist. [TAP] title opens the album, artist opens the artist page, both collapse the player first. Long names scroll as a marquee after 1.5 s, stopped by Reduce Motion.

4. Like and More. [TAP] like toggles with a haptic and syncs; it is also the "love" signal for recommendations. More opens the same menu as a long press on the artwork, plus Sleep Timer, Why this song and Private listening.

5. Progress. [TAP] and drag scrubs; sliding the finger down halves, then quarters the scrub speed. [TAP] the right time swaps remaining and total. Android's bar is wavy while playing and flat when paused, with a tall pill handle and a gap around it, as in the Material 3 Expressive slider. [HAPTIC] start and end only.

6. Transport. [TAP] previous restarts after 3 s, otherwise goes back. [HOLD] previous or next scans at 4× with the haptic ticking each second. [GUARD] 200 ms debounce. On Android the three buttons are separate rounded squares 8 dp apart: Previous and Next are tonal and shaped by their arrow: the end the arrow points to is fully round (36 dp) and the end beside play is 10 dp, so Previous is round on the left and Next on the right. Play is wider and changes shape with its state. Playing it is a rounded square (the flatter shape, 24 dp), paused it morphs to a full round shape, and pressed it tightens to 14 dp (see the Buttons board). Lyrics, Output and Queue below are one connected group.

7. Volume (iOS). [SWIPE] drags the slider with selection ticks at 10% steps. Android relies on the hardware keys, and the system volume panel shows its own output control.

8. Lyrics, Output, Queue. [TAP] toggles each view in place. The active one fills with the glass or tonal highlight. Output opens the device and handoff sheet.

States. Loading: spinner replaces play, controls stay tappable. Buffering: bar shows a moving stripe. Track unavailable: toast, skips automatically after 2 s, the failed song is flagged in the queue. Offline and not downloaded: warning haptic and a reason. Landscape phone: artwork left, lyrics right.

Same screen, three other albums

Tidewater

Brass Arches

Paper Moon (light art, dark text)
