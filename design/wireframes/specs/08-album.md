# Board 08: Album

PNG: `design/wireframes/png/08-album.png`  
HTML: `design/wireframes/html/Album.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Album

1. Back and More. [TAP] back pops the page; the left-edge swipe does the same, interactively. More: Add to Library, Download, Share, Go to Artist. Over the tinted page, iOS shows glass buttons and Android shows tonal ones.

2. Artwork and tint. The page takes one tint from the artwork's palette, as on the gradients board, and the field stays pinned while the list scrolls. On scroll the art shrinks and fades under the bar. [HOLD] art opens Share and Save Image. [2×TAP] nothing.

3. Artist link. [TAP] opens the artist page. The hit area is the full text width plus 8 pt of padding.

4. Play and Shuffle. [TAP] replaces the queue with this album. [GUARD] if music is playing, a toast shows "Replaced queue" with Undo for 5 s that restores the old queue and position. [HAPTIC] soft. On Android the three buttons are one connected group: the outer corners are fully round, the corners that face each other are 10 dp, and the pressed button widens while its neighbors squeeze.

5. Track rows. [TAP] plays the song and queues the rest of the album. [HOLD] menu: Play Next, Add to Queue, Add to Playlist, Like, Download, Go to Artist, Why is this here. [SWIPE] right: Play Next; left: Add to Queue, each armed with a haptic. The playing row shows level bars. Multi-disc albums add disc headers.

Footer. Release info, then "More by this artist" and "In your library you might also like", both horizontal rows.

States. Loading: art and rows as skeletons. Partial album (server has fewer songs than the release): a quiet line, "9 of 12 songs in your library". Partly downloaded: "7 of 12 saved". Song unavailable: dimmed, tap shakes. Light or gray artwork: the tint flips to a dark scheme so text keeps contrast. No artwork: the profile accent.
