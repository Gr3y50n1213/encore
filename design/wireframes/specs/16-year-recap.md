# Board 16: Year recap

PNG: `design/wireframes/png/16-year-recap.png`  
HTML: `design/wireframes/html/Recap.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Year recap

1. Story bar. One segment per card, 10 to 12 cards. Each card auto-advances after 6 to 8 s, and the bar fills as it goes. [TAP] the right 65% of the screen goes forward, the left 35% goes back. [HOLD] anywhere pauses and hides the controls until release. [SWIPE] down closes. [GUARD] the first tap is ignored for 300 ms after a card appears, so a double tap cannot skip two cards.

2. Close. Asks nothing. The recap reopens where it left off from Home for 30 days.

3. Card content. One idea per card, with a big number and one sentence. Real numbers only, computed by the shared core from the profile's play log, on the server or on this device. Numbers count up over 600 ms and the bars rise in order; reduced motion shows the final state at once.

4. Chart marks. Bars, dots and shapes use the pack's own forms, so no two years share the same look. Every chart has its values as text for screen readers.

5. Share this card. [TAP] renders the card to an image on the device and opens the system share sheet. Nothing is uploaded, and saving to Photos goes through the share sheet, so the app never asks for photo library access. Settings can hide names of songs or artists before sharing.

Beats and packs. The card order is a story chosen from a pool of beats, such as top song, top artist, night versus day, new discoveries, a streak, genre or mood drift, a single obsession week, and a closing summary. A beat only appears if the data is interesting, for example the streak card needs 7 days or more. The design pack (Editorial, Tide, Bloom or Glass), covering palette, shapes, type and motion, is picked from a seed made from the profile id and the year and is never the same as last year's. The story opens on December 1 labeled "so far" and is final on January 8. Boards 39 to 41 give the full card list, the packs and the device rules, and replace this paragraph.

Too little data. Under 20 hours or 100 plays, the card on Home offers a short 3-card recap, and under 10 plays there is no card yet. Guest. No recap. Profile switch. Each profile has its own recap and its own progress. Offline. Works fully, since the data is local, but album art shown on cards may be blank if not cached; those cards use the pack's shapes instead.
