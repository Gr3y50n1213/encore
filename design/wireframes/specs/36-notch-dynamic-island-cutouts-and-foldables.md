# Board 36: Notch, Dynamic Island, cutouts and foldables

PNG: `design/wireframes/png/36-notch-dynamic-island-cutouts-and-foldables.png`  
HTML: `design/wireframes/html/System-Cutouts.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Notch, Dynamic Island, cutouts and foldables

Every screen has to work around a camera cut-out, a gesture area and, on foldables, a fold. The rule is the same everywhere: backgrounds run edge to edge, controls stay inside the safe area, and nothing tappable ever crosses a fold. iPhone Duo and the new MacBook Pro are announced Apple products. Items marked Reported come from press coverage or developer write-ups and should be checked against Apple's and Samsung's own documentation.

iPhone · notch and Dynamic Island

Background only
Home or gesture bar
Controls and text

Notch
iPhone X to 13 and the non-Pro 14. Top inset about 47 pt.

Dynamic Island
iPhone 14 Pro and later. Top inset 59 to 62 pt, read from the system.

Landscape
Both sides get an inset. The island sits on one side; the insets stay equal on both.

Android · cutouts and edge-to-edge

Background only
Home or gesture bar
Controls and text

Centre punch-hole
Pixel, most Samsung. Status bar inset about 24 to 40 dp.

Corner hole
Some Samsung and others. Top centre is free, the corner is not.

Waterfall edges
Curved sides: a side inset of 6 to 12 dp keeps controls off the curve.

Landscape
The cutout inset appears only on the side where it sits. Do not mirror it.

Mac · notch, Dynamic Island and touch

Encore File Edit Playback

♪ ▭ 9:41

Notch Macs
MacBook Pro 14 and 16, MacBook Air 2022 and later. The menu bar is taller than on other Macs; the stripes show the strips beside the notch that belong to the system.

Encore File Edit Playback

♪ ▭ 9:41

Dynamic Island and touch Reported
New 14 and 16 inch MacBook Pro (announced Oct 28, ships Nov 6): hole-punch camera, Dynamic Island and a touch screen. Details come from press reports; verify when the hardware and docs are out.

Foldable devices at a glance

| Device | Closed | Open | Half open

| Samsung Galaxy Z Fold | Cover screen, compact layout | Inner screen, medium or expanded width, two panes | Flex mode: content above, controls below the crease

| Samsung Galaxy Z Flip | Cover screen: system player and widgets | Tall phone, compact layout | Flex mode: content above, controls below

| Pixel Fold and similar | Outer screen, compact layout | Inner screen, expanded, two panes | Tabletop (horizontal hinge) or book (vertical hinge)

| iPhone Duo (on sale since Oct 23) | 5.4 inch outer screen, compact width | 7.6 inch inner screen, regular size classes, Split View. Dock and navigation move to the side and the island runs along the side, so insets differ per edge | Keep controls and text out of the fold region Reported

Foldables · cover, flat, half open

Fold cover screen
Compact layout. Fills the screen, no letterbox.

Flip cover screen
System controls and widgets. The app is optional here.

Open, flat (Expanded)
Library left, player right. The hinge strip stays empty.

Half open, horizontal hinge
Flex mode and tabletop. Content above, transport below.

Half open, vertical hinge
Book posture. List left, player right. Nothing crosses the hinge.

iPhone Duo, inner screen
Two columns, regular size classes. The fold region stays empty.

What each screen does

| Screen | iPhone, iPad and Mac | Android and Windows | Foldables, split screen and resizable windows

| Mini player | Sits above the tab bar and home indicator. On Mac it is the bottom bar of the window, never in the notch strip. | Sits above the navigation or gesture bar. 48 dp targets. Bottom inset is read from the system, so it works with both navigation styles. | With two panes it becomes the footer of the list pane. In tabletop it is replaced by the full transport at the bottom half.

| Now Playing | Background runs under the island, notch and home indicator. Handle, title and buttons start below the island. Status text turns white or dark from the artwork. | Edge to edge. Chevron and More sit below the status bar inset. The progress bar keeps 24 dp from both edges so the back gesture cannot start on it. | Flat: artwork and transport left, lyrics right. Tabletop: artwork and lyrics above, transport below. Book: controls and lyrics on one side, hinge empty. Playback never restarts on a fold or unfold.

| Lyrics | Lines start below the island. Active line stays in the middle third, clear of the home indicator. | Same. Lines wrap inside the side insets on curved screens. | Lines wrap inside one pane. A line is never split across the hinge; if a pane is under 320 dp the lyrics become a sheet.

| Queue, lists, search | Large titles start below the island. Lists scroll under the glass bars. | Lists scroll under the system bars with the content inset, so the last row is reachable. | Two-pane list and detail from 600 dp. The selected item and scroll position survive resizing.

| Sheets and menus | Detents stop below the island. Context menus flip to stay on screen. | Bottom sheets stop 8 dp under the status bar inset. Popups avoid the cutout. | A sheet opens in the pane it came from, or centred on one side of the fold, never across it.

| Live Activity, widgets, lock screen | The system draws the island and Live Activity. Encore supplies content only and never positions anything there (see the Live surfaces boards). | The system draws chips, widgets and the lock screen. Layouts use the widget size the launcher gives. | Cover screens and tablet lock screens show the system player. Widgets are resizable and use the size given.

| Recap stories and full screen cards | Progress segments sit below the island. Left and right tap zones start below it. | Progress segments sit below the status bar inset. Share button clears the gesture bar. | A story card always fits inside one side of the fold. On a wide inner screen it centres at phone width.

Rules

1. Read the insets from the system. iOS: the safe area. Android: the window insets for system bars, display cutout and gestures. Mac: NSScreen.safeAreaInsets with the left and right areas beside the notch. Flutter on Windows and Linux: the view padding. Never type numbers like 20, 44 or 59.

2. Draw behind, place inside. Album gradients and artwork run edge to edge under the status bar, island, notch and gesture area. Text, buttons, sliders and drag handles sit inside the safe area.

3. Status text follows the artwork. The clock and battery turn white on dark gradients and dark on light covers, with a 200 ms cross-fade when the album changes.

4. Gesture zones. Keep sliders and swipe areas at least 24 dp from the screen edges. A touch that starts in the edge zone belongs to the system back gesture, not to a scrub.

5. Mac menu bar item. One optional item. If the bar is crowded macOS can hide items behind the notch, so everything it does is also in the Dock menu, the mini player and the keyboard shortcuts.

6. Live Activities on Mac. An iPhone Live Activity already mirrors to the Mac menu bar. Whether a Mac app can start its own is not documented yet Reported, so Encore plans for the mirror only.

7. Touch on Mac. On Macs with a touch screen, controls use gesture recognizers only, targets grow to 36 pt, and every hover-only detail has a touch route. Test on a touch Mac when one is available; until then, review every hover-only detail by hand.

8. Folding keeps the session. The Rust core owns playback, queue and position, and screen state is saved outside the view, so a fold, unfold, rotate or window resize never restarts the song or loses the scroll position. Orientation is never locked. Android 16 ignores orientation and resize opt-outs on large screens for apps that target it.

9. Keep off the fold. Nothing tappable, no slider thumb and no line of text crosses the fold or hinge gap. Android reads it from the folding feature (state, orientation, separating, bounds, occlusion). On iPhone Duo the equivalent is a reserved region at the fold plus an arrangement for split or overlay layouts Reported; those API names come from a developer write-up of Apple’s Tech Talks, so confirm them in the Xcode documentation.

10. Postures. Half open with a horizontal hinge: content on top, controls below. Half open with a vertical hinge or flat and wide: two panes. Closed: compact. Show a short transition when entering or leaving tabletop, as Samsung asks for Flex mode.

11. Cover screens. A Fold cover screen runs the compact layout. A Flip cover screen shows the system player and widgets; running Encore there is optional and needs the full-screen, any-aspect-ratio and state-keeping rules Samsung lists.

12. Test matrix before release. iPhone with island and notch, iPhone with a home button, Mac with notch and with island and touch, Pixel, Samsung centre and corner hole, a Fold and a Flip, Pixel Fold, iPhone Duo, an Android tablet, landscape, split screen, 200% font and right to left.
