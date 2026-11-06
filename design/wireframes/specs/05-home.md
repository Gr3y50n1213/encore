# Board 05: Home

PNG: `design/wireframes/png/05-home.png`  
HTML: `design/wireframes/html/Home.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Home

1. Profile button. [TAP] opens the profile switcher sheet. [HOLD] quick list of signed-in profiles. [GUARD] locked profiles ask for PIN or biometrics; switching while music plays asks "Keep playing?". [HAPTIC] success on switch.

2. Made for you tiles. [TAP] opens the playlist. [HOLD] menu: Play, Shuffle, Pin, Why these songs, Regenerate. [SWIPE] scrolls and snaps one tile at a time. [2×TAP] nothing, so nobody likes a playlist by accident. [MOTION] tile presses to 0.97; Android's first tile is wide and shrinks as it scrolls off.

3. Recently played. [TAP] opens the album or playlist, never starts playback. [HOLD] item menu. [SWIPE] scrolls. Items appear after 30 s of play, so a mis-tap leaves no trace.

4. Rediscover. Albums you loved and have not played in 60+ days. [TAP] opens. [HOLD] menu with "Not interested", which removes it and trains the recommender.

5. Mini player. [TAP] body expands Now Playing. [TAP] play or next acts in place. [SWIPE] up expands. Left skips forward and right goes back, with the next song sliding in under the finger (board 42). [GUARD] transport buttons debounce 200 ms. [HAPTIC] soft on play and next.

6. Tab bar. [TAP] switches tab. Tapping the active tab scrolls to top, a second tap returns to the tab's root. iOS minimizes the bar while scrolling down and restores it on scroll up; Android hides it the same way. Search is its own glass button on iOS and the fourth destination on Android.

Whole screen. [SWIPE] pull down at 72 pt to refresh the library; haptic when armed. Content loads from the local mirror first, so the first frame is never empty.

States. New profile: one card, "Play something to train your mixes", plus library shortcuts. Fewer than 50 plays: up to three mixes. Building: skeleton tiles on iOS, expressive loading indicator on Android. Offline: only downloaded items stay active, the rest dim with a banner. Server unreachable: cached library stays usable with a dismissible banner. Unsynced changes: small dot on the avatar.
