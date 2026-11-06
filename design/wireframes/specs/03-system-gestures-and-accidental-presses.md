# Board 03: System: gestures and accidental presses

PNG: `design/wireframes/png/03-system-gestures-and-accidental-presses.png`  
HTML: `design/wireframes/html/System-Gestures.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Gestures and accidental presses

One gesture vocabulary on both platforms so a screen never surprises anyone. Anything that changes playback, deletes, or leaves the screen is either undoable for 5 seconds or needs a deliberate second action.

Gesture vocabulary

| Gesture | Timing and distance | Means

| Tap | Release within 300 ms, under 10 pt of drift | Primary action: play, open, toggle

| Double tap | Two taps within 250 ms, same spot | Only on artwork: like. Never on rows or buttons.

| Long press | Android 400 ms (follows the system setting). iOS the system menu timing, about 0.5 s | Context menu for that item. Haptic when it opens, menu opens with the item lifted.

| Horizontal swipe on a row | Past 25% of row width arms, past 60% commits | Leading: Play Next. Trailing: Add to Queue, Remove.

| Vertical swipe on a player | Velocity and distance, follows the finger | Down: collapse. Up on mini player: expand. Sideways on the mini player or artwork: skip, board 42.

| Drag handle | Starts on touch of the handle only | Reorder queue and playlists. With a mouse or trackpad the whole row drags, board 43.

| Scrub | Drag the progress bar, vertical drift slows it | Seek. Drift down to 50% or 25% speed for fine scrub.

| Edge swipe from left | Starts within 24 pt of the edge | Back. Predictive on Android.

| Pull down | Armed at 72 pt | Refresh library from the server

Accidental-press protection

Targets Every control is at least 44 pt on iOS and 48 dp on Android. Icon buttons that sit close together keep 8 pt between hit areas.

Cancel by sliding off Touch down highlights, the action runs on release inside the control. Sliding out cancels with no haptic.

Transport debounce Play, pause, next and previous ignore a second tap for 200 ms after a first, so a bouncing thumb does not skip two songs.

Skip is reversible A skip pushes the song onto History. Previous within 3 s of a new track returns to the one just skipped, never two back.

Destructive actions Remove from library, delete playlist, sign out and clear downloads need a confirm sheet with the destructive button separated from Cancel. Remove from queue and remove from a playlist show an Undo toast for 5 s instead.

Pocket and bag The lock screen controls and headphone buttons keep working. In-app, a touch that lands during the first 150 ms after the screen wakes is ignored.

Scrub lock While the finger is on the progress bar, edge-back, collapse-swipe and tab taps are ignored so a slip never closes the player.

Menus A long-press menu opens under the finger. Releasing without moving never triggers an item; the first item needs a second tap.

Blocked actions A tap that cannot work (offline, not downloaded) shakes the control 4 pt, fires the warning haptic, and shows a one-line reason.

Unsent changes Likes and playlist edits apply instantly on the device and sync later. A failed sync shows a small badge, never a blocking error.
