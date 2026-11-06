# Board 10: Lyrics

PNG: `design/wireframes/png/10-lyrics.png`  
HTML: `design/wireframes/html/Lyrics.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Lyrics

1. Header. [TAP] returns to the artwork view. Art and title stay pinned so the song is always identifiable.

2. Past lines. 40% opacity and a 1.2 px blur, so the eye stays on the present. They keep their position until the list scrolls.

3. Current line. Full brightness. Each word fills left to right across its own sung time with a soft light edge; lines with only line timing light up whole. The active line sits about 30% from the top. [MOTION] every line moves on a spring, delayed 18 ms per line of distance up to six lines, so the column ripples. Reduce Motion: cross-fade only.

4. Second singer. Duet lines align right, same size, so who is singing reads at a glance.

5. Background vocals. Smaller and dimmer, shown under the main line while sung.

6. Instrumental gap. For gaps over 5 s, three dots breathe in time and fill one by one before the next line. They collapse the moment singing starts.

7. Upcoming lines. [TAP] any line seeks to its start and shows a 4 s pill, "Back to 2:31", that undoes the jump. [GUARD] a touch that moved more than 10 pt is a scroll, never a seek. [HOLD] selects lines for sharing, capped at five lines per image card.

8. Controls. Same as Now Playing, compact. After 4 s with no touch they fade and lyrics fill the screen; a tap on empty space brings them back. [SWIPE] by hand pauses auto-scroll until 3 s after release.

States. Synced word by word, synced by line, and unsynced (plain scroll, no highlight, a small "Not synced" tag). Loading: five shimmer lines. No lyrics: "No lyrics for this song", with a search button only if an online provider is on. Instrumental flag: dots and the title only. Translation and pronunciation lines, when added, sit under each line at 60% size.
