# Board 42: Swipe gestures: mini player, Now Playing and rows

PNG: `design/wireframes/png/42-swipe-gestures-mini-player-now-playing-and-rows.png`  
HTML: `design/wireframes/html/Gestures-Swipe.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Swipe gestures: mini player, Now Playing and rows

What a good music app does under your thumb, written down so it is built the same way everywhere. A swipe on the mini player or the artwork skips songs the way people expect, and a swipe on a row queues or removes it. These rules replace the one-line mini player note on board 05 and the player swipe row in plan section 16.

Mini player · iPhone, Liquid Glass

1. Rest
Art and title on the left, play and next on the right. Nothing moves.

2. Dragging
Past 12 pt the direction locks. Art and title follow the finger and the next song slides in from the side. Nothing loads yet.

3. Armed
Past 60 pt the next song is fully in view and one soft tick fires. Moving back under 60 pt disarms with no tick.

4. Released armed
The new song settles, the play state is kept and it starts playing as soon as it is buffered.

5. Released early
Under 60 pt and slower than 500 pt/s it springs back with the same spring. No change, no haptic.

Mini player · Android, Material 3 Expressive

The same five states and numbers. The play button follows the Android shape rule on board 35, and Next is a tonal button that is round on the side its arrow points to.

Now Playing artwork · both platforms

Rest
The artwork is centred. Double tap likes (board 09).

Dragging
Cover and title follow the finger; the neighbour slides in and the one you are leaving dims and shrinks a little.

Armed
Past 60 pt or a quick flick. Release skips. On the lyrics and queue panels a horizontal swipe never skips.

Same rule, bigger target

Arming is 60 pt or a flick faster than 500 pt/s on the mini player and on the artwork, so the two never feel different. Swiping the artwork is the same skip as the Next button. Double tap likes and a long press opens the menu, as on board 09.

Row swipes

Right: Play Next
On every song list. Arms at 25%, release performs it, with a toast and Undo.

Left: Add to Queue
On song lists in Library, Album, Search and Mixes. Arms at 25%.

Left: Remove
Queue and your own playlists. Destructive, so it arms only at 60%. Undo for 5 s.

Left on Autoplay: Not this
Removes the song and counts as a strong dislike. Arms at 60%.

1. Direction lock. The first 12 pt of movement decide it: horizontal if sideways is more than 1.2 times vertical, otherwise vertical. It stays locked until the finger lifts. Vertical up on the mini player expands it following the finger. Vertical down does nothing, so playback is never stopped by a slip.

2. Which way. Left skips forward and right goes back in left-to-right languages, mirrored in right-to-left ones. The song you are heading to always arrives from the side you drag toward.

3. Back means back. A swipe right goes to the previous song every time and never restarts the current one. The Previous button keeps its rule of restarting after 3 s. Swiping back right after a skip returns to the song you skipped.

4. Play state is kept. Skipping while paused stays paused: the new song shows at 0:00 and loads without sound. Repeat One moves to the next song and applies to it. Shuffle follows the shuffled order.

5. The finger decides, not the audio. During a drag the neighbour is only a drawing and nothing loads. A second swipe within 600 ms replaces the first, so songs passed over never load, never make a sound and never write a signal. Only the song you land on counts.

6. Signals. A completed swipe is a skip exactly like the Next button, so section 21 applies: an early skip of an app-chosen song counts against it, a skip of a song you picked barely counts, and seeks and errors never count.

7. Ends of the queue. Last song with Autoplay on: the next song is the autoplay pick. With Autoplay and repeat off: the content follows at a third of the speed, stops after 40 pt and springs back, with "End of queue" under the title for 1.5 s. At the first song with repeat off the same rubber band applies; with Repeat All it wraps.

8. Buttons never move. Play and Next stay where they are while the content slides. A drag that starts on a button and passes 10 pt cancels the button and becomes a swipe. Buttons still fire on release inside, and a second tap within 200 ms is ignored.

9. One haptic. One soft tick when it arms (iOS soft impact, Android GESTURE_THRESHOLD_ACTIVATE), nothing on release, nothing when you move back under the line. Reduce Motion replaces the slide with a 120 ms cross-fade; the tick stays.

10. Catch it mid-flight. The settle uses the Now Playing hero spring and can be caught by a new touch. The finger takes over from the current position and velocity.

11. Edges and system gestures. A horizontal drag that starts within 24 pt of the screen edge belongs to the system back gesture and is never read as a skip. The mini player sits above the bottom inset on both navigation styles.

12. Not by touch only. VoiceOver and TalkBack get Next song and Previous song as custom actions on the mini player. Switch users use the buttons. Mac, Windows and Linux have no swipe: they use the buttons, media keys and Space.

13. Headsets, cars and the lock screen. Next and Previous from headphones, Bluetooth, a car and the lock screen run the same code as the buttons and the swipe, including the 3 s restart rule on Previous and the signals above. Double and triple presses are the headset's own mapping.

14. Rows. Swipes need to travel 25% to arm (non-destructive) or 60% (destructive). Under that, the row springs back with no action and no haptic. The haptic fires at the moment it arms. Horizontal swipes that begin in the 24 pt edge zone are left to Back. Rows lift on a long press, which opens the menu (board 13).
