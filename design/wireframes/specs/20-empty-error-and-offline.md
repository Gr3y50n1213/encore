# Board 20: Empty, error and offline

PNG: `design/wireframes/png/20-empty-error-and-offline.png`  
HTML: `design/wireframes/html/States.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Empty, error and offline states

1. Offline banner. Slides in under the status area when the network is lost for more than 2 s, and leaves 2 s after it returns. It never covers controls. Browsing is limited to downloaded and already cached items; everything else dims with "Needs connection". Playback of the current song continues from its buffer.

2. Empty. One sentence saying what will appear and one action. No cartoons. Home, Library sections, Likes, Downloads, Queue ("Nothing is playing next") and Search recents each have their own text.

3. Error. A short cause and one action, Try again. [TAP] retries and shows the spinner for at least 400 ms so the tap feels received. After the third failure it also shows "Check servers". [HAPTIC] error only on a user-started retry, never on a background failure.

4. Loading. Skeleton shapes in the exact layout of the content, with a slow shimmer. For waits with no known layout, Android shows the Material expressive loading indicator, a turning shape that morphs between cookie and sunny forms, and iOS shows the system spinner. They appear only if loading takes more than 250 ms, so quick loads do not flash. Reduced motion stops the shimmer.

5. Unavailable item. Dimmed. [TAP] shakes 6 pt twice with the warning haptic and shows the reason as a toast. The item stays in lists so nothing silently vanishes, and the song menu offers Remove.

6. No results. Echoes the query, suggests a fix, and offers "Search all sources" when only the downloaded ones were searched.

Playback errors. Buffering: the play button becomes a ring after 500 ms of stalling. Stream fails twice: skip to the next song and show "Couldn't play Song". Three in a row: pause and say why. Toasts and banners are announced to screen readers and never trap focus. Server error text shows the real reason from the server in a "Details" disclosure.
