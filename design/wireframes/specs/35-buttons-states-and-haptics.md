# Board 35: Buttons, states and haptics

PNG: `design/wireframes/png/35-buttons-states-and-haptics.png`  
HTML: `design/wireframes/html/System-Buttons.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Buttons, states and haptics

Every control is drawn in every state it can be in, and one rule decides its shape: on Android a selected or playing button is squarer and an unselected or paused one is round. The tables below give the tap, hold, drag, haptic, motion and guard for each control so Claude Code has nothing to guess.

Android 16 · Material 3 Expressive · play and pause

1 · Paused

Round, 36 dp on 72 dp. Shows play. Nothing moves until touched.

2 · Pressed, from paused

14 dp corners, 14% state layer. Shape morphs in about 100 ms, fastSpatial. Action runs on release.

3 · Playing

Rounded square, 24 dp, the flatter shape. Shows pause. Same size and colour as paused.

4 · Pressed, while playing

14 dp again, then it springs to round when playback stops. Haptic VIRTUAL_KEY on release.

5 · Loading or buffering

Round, spinner replaces the glyph. Stays tappable: a tap cancels the load.

6 · Disabled

Empty queue. 12% container, 38% glyph. No morph, no haptic, no ripple.

7 · Focused

Keyboard, D-pad, Switch Access, TV remote: 3 dp ring, 3 dp gap. Never shown for touch.

8 · Hovered

Mouse or stylus hover (tablets, ChromeOS, desktop): 8% state layer, shape stays.

Sizes and neighbours

Large · phone Now Playing

112 × 72, 24 dp. Neighbours 88 × 72, tonal, round outside and 10 dp toward play, 8 dp apart. Drawn at 60%.

Medium · Lyrics

96 × 60, 20 dp. Neighbours 76 × 60, same corners. Drawn at 60%.

Bar · tablet and desktop

52 × 52, 17 dp, filled. Neighbours 52 × 52 tonal. Drawn at 70%.

Compact · mini player

44 × 44, 15 dp. A 48 dp touch area stays.

Toggles: selected is squarer

Like

Off round, on squarer and filled. Heart pops 1 → 1.25 → 1.

Shuffle

Same rule: selected is squarer. Hold opens Shuffle or Smart shuffle.

Repeat

1. Off, All, One. The badge pops in at 120 ms.

Filter chip

Albums

Songs

Chips keep a 12 dp corner; selected adds a check and a tonal fill.

iOS, iPadOS · Liquid Glass · transport

Rest

Plain glyph on the album gradient. No container, as on Apple Music.

Pressed

A glass lens lifts under the finger and shimmers. Runs on release.

Selected

Shuffle and repeat: tinted glass in the accent colour while on.

Loading

Spinner replaces the glyph. Still tappable.

Focused

iPad keyboard and Full Keyboard Access: 3 pt ring.

Disabled

35% opacity, not in the focus order, announced as dimmed.

Reduce Transparency swaps the lens for an opaque tinted disc. Increase Contrast adds a 1 pt outline. Reduce Motion removes the lift and shimmer and cross-fades the glyph instead. The press feedback still shows within 100 ms.

One rule for every Android toggle

Unselected
or paused
round

Selected
or playing
squarer, 33%

Pressed
19%

Previous
round on the left

Next
round on the right

Pressed
inner end tightens

Previous and Next are not toggles, so they keep one shape: the end the arrow points to is fully round and the end beside Play is 10 dp. It flips with the arrow in right-to-left languages. Pressed, the inner corners tighten to 4 dp.

Play and pause is a toggle whose checked state is is playing. The same shapes apply to like, shuffle and repeat, so every button on a screen tells the same story.

In code use the library's toggleable icon button shapes and set the three shapes once (shape, pressedShape, checkedShape). The library morphs between them when all are rounded-corner shapes, so Encore never animates a radius by hand.

Connected button groups (segmented controls, Play | Shuffle | Add) keep the library rule instead: the selected item is fully round and the others square off toward their neighbours.

To check: 24 dp (33%), 50% and 14 dp are Encore's own tokens. The library documents shape slots, not numbers, so confirm the look against the pinned Compose version before release.

Press and hold timing

Feedback shows within 100 ms of touch down (state layer first, shape morph follows). The action runs on release inside the control.

Slop. Moving more than 10 pt on iOS or 8 dp on Android before release means a scroll or drag, never a tap.

Long press. Android uses 400 ms by default and follows the system touch and hold delay. iOS uses the system menu timing, about half a second, and follows Touch Accommodations. Mac: right-click, Control-click or a press and hold on touch screens. Windows and Linux: right-click, the Menu key or Shift+F10, 500 ms on touch.

Haptics. Phones and tablets use the tables below. Mac fires trackpad haptics for scrub snaps and step changes only. Windows and Linux have none, so they rely on motion and sound.

Accessibility for every button

Labels. Every icon button has a label and a role. Play and pause announce "Playing" or "Paused", toggles announce "On" or "Off", and a disabled button says why when focused.

Right to left. Transport keeps its order because time still runs left to right. Back arrows, lists and swipe actions mirror.

Large text. Buttons grow with font scale up to 200%, labels wrap, targets stay at 44 pt or 48 dp, and the transport row stays on one line down to 320 dp.

macOS · Liquid Glass toolbar button

Rest

Glass toolbar button.

Hover

Pause

Soft lift. Tooltip after about 1 s.

Pressed

Darkens, scales to 96%.

Keyboard focus

Accent ring, Tab order.

Selected

Tinted fill and accent glyph.

Touch Mac

36 pt hit area. Hover tips repeat as labels.

Windows and Linux · Encore Material 3 Expressive

Rest

Tonal, round.

Hover

8% state layer.

Pressed

12% layer, corners tighten.

Focus-visible

Ring only for keyboard.

Playing

Flat shape, as on Android.

Disabled

12% and 38%.

What every control does

| Control | Tap | Hold | Second tap, drag or slide off | Haptic | Motion | Guards

| Play and pause | Flips at once: the glyph and shape change on release and audio follows within 150 ms. If playback fails it flips back and says why. | Nothing. A stray hold does nothing. | A second tap inside 200 ms is dropped. Sliding off before release cancels with no haptic. | iOS impact(.soft)
Android VIRTUAL_KEY | iOS glyph replace, .snappy
Android shape morph, fastSpatial | Disabled with an empty queue. While loading it stays tappable and cancels the load.

| Previous | After 3 s into the song it restarts it, before that it goes to the previous song. On the first song it always restarts. | Scans back at 4× while held and returns to normal speed on release. | A second tap within 1 s of a restart goes back one song. | Tap as above. Scan tick each second: selection · SEGMENT_FREQUENT_TICK | iOS glyph slides 3 pt left
Android press morph | 200 ms debounce shared with play and next.

| Next | Skips. Logged as a skip with its position (see Smart picks). At the end of the queue autoplay adds songs, or playback stops if autoplay is off. | Scans forward at 4×. | Previous within 3 s returns to the song just skipped, never two back. | Same as Previous | iOS glyph slides 3 pt right
Android press morph | Skips caused by a route change or error are not logged as skips.

| Like | Toggles at once, saved on the device, synced later. A like is a strong "more like this". | Nothing. More has the rest. | Double tap on artwork only adds a like, never removes one. Remove with the heart. | iOS impact(.medium) on, .light off
Android TOGGLE_ON / TOGGLE_OFF | iOS heart pop 1 → 1.25 → 1
Android round → squarer, fill | Works offline. A failed sync shows only a badge in Settings.

| Shuffle | Toggles. Reorders only what is up next, never the song playing or the history. | Opens Shuffle options: Shuffle, or Smart shuffle which adds suggestions. | Turning it off restores the original order after the current song. | Toggle haptics as Like | iOS tint fill
Android squarer ↔ round | Never interrupts the current song.

| Repeat | Cycles Off, All, One. A screen reader announces each state. | Nothing. | Repeat One ends after the song is skipped by hand. | iOS selection
Android TOGGLE_ON for All and One, TOGGLE_OFF for Off | The "1" badge pops in at 120 ms | Repeat One never traps a user: Next always moves on.

| Progress bar | Tap and drag scrub. Sliding down halves the speed, then quarters it. The time on the right swaps between remaining and total. | Touching the handle for 150 ms arms the scrub. | While a finger is on the bar, edge back, collapse swipe and tab taps are ignored. | Start and end only: impact(.light) / GESTURE_START, GESTURE_END | iOS line thickens
Android wave flattens under the finger | Seeks are throttled to one per 50 ms. Seeking is never logged as a skip.

| Song row | Plays it. In the queue it jumps to that song, and what was skipped stays in History. | Hold 400 ms opens the context menu with the row lifted. Release without moving does nothing; the first item needs a second tap. | Swipe right: Play next. Swipe left: Add to queue or Remove. Past 25% arms, past 60% commits. | Menu: system menu haptic · LONG_PRESS
Armed: impact(.rigid) · GESTURE_THRESHOLD_ACTIVATE | Row lifts 4 pt, 150 ms | Remove shows Undo for 5 s.

| Sleep timer preset or +15 | Starts the timer, or adds 15 minutes. A toast confirms the end time. | Nothing. | Cancel needs no confirmation; it can simply be started again. | iOS notification(.success)
Android CONFIRM | Chip fills, toast slides up 200 ms | The 10 s fade-out cannot be skipped by a tap.

| Destructive action | Opens a confirm sheet. The red button is separated from Cancel, and Cancel is the default. | Nothing. | Confirming needs a deliberate second press. A tap outside the sheet cancels. | On confirm: notification(.warning) · CONFIRM | Sheet rises on defaultSpatial | Applies to remove from library, delete playlist, sign out, clear history.

| Blocked action | Shakes 4 pt, warning haptic and a one-line reason (offline, not downloaded, no permission). | Nothing. | The reason is also read out by screen readers. | notification(.warning) · REJECT | 4 pt shake, 3 cycles, 240 ms | Never a blocking dialog.

Notes

1. State decides shape. The Android play button is a toggle: paused is round, playing is a rounded square (24 dp on a 72 dp button), pressed tightens to 14 dp. The same rule applies on the phone, tablet, desktop, lock screen and widget boards, so no screen shows a pill-shaped pause button. Previous and Next do not toggle: the end the arrow points to is fully round and the end beside Play is 10 dp.

2. Order of events. Touch down: state layer and shape morph start (under 100 ms). Release inside: the action, then the haptic, then the glyph change. Slide off: everything reverses with no haptic.

3. Optimistic state. The button changes as soon as it is released and corrects itself if the player reports an error. This keeps taps feeling instant on slow servers.

4. One haptic per action. A press never produces two haptics. Scans tick once a second. Ambient motion such as lyrics and gradients never vibrates. System haptic settings win.

5. Reduce Motion. Shape morphs, lifts, springs and shakes become 120 ms cross-fades. State layers, focus rings and haptics stay.

6. Touch Mac. On Macs with touch, controls use gesture recognizers only, hit areas grow to 36 pt, and anything that appears on hover is also reachable by touch (tooltips become labels or a long-press menu).
