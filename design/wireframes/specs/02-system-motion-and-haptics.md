# Board 02: System: motion and haptics

PNG: `design/wireframes/png/02-system-motion-and-haptics.png`  
HTML: `design/wireframes/html/System-Motion-Haptics.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Motion and haptics

Both platforms use springs, never fixed-duration easing, and every spring can be interrupted mid-flight. Haptics are sparse: they confirm a decision or mark a threshold, and nothing else. Reduce Motion swaps flights and sweeps for cross-fades; system haptic settings are always respected.

Motion roles

| Moment | iOS 26 (SwiftUI) | Android 16 (Compose M3E)

| Press feedback | .snappy, glass lifts and shimmers (.interactive()) | Shape morph on press, fastSpatial

| Toggle, chip, like | .snappy, heart pop 1 → 1.25 → 1 | Shape morph square ↔ round, fastSpatial

| Navigate push | System push, interactive edge swipe back | Predictive back with shared container

| Mini player → Now Playing | matchedGeometryEffect artwork flight, hero spring | Container transform, slowSpatial

| Sheet present | System sheet with glass detents | Modal bottom sheet, defaultSpatial

| Menu open | Glass menu morphs out of the pressed item (glassEffectID) | Popup scales from anchor, fastSpatial

| Tab bar on scroll | tabBarMinimizeBehavior(.onScrollDown) | Bar hides on scroll, returns on scroll up

| Color changes | Cross-fade, smooth | defaultEffects (no bounce)

| Progress | Linear bar with live scrub thumb | Wavy progress while playing, flat when paused

| Loading | Skeleton rows, 1.2 s shimmer | Expressive loading indicator, morphing shapes

Android values come from MotionScheme.expressive() in the library, not from numbers copied into the app.

Haptic map

| Event | iOS (UIKit generators) | Android (HapticFeedbackConstants)

| Play / pause, skip | impact(.soft) | VIRTUAL_KEY

| Like on | impact(.medium) | TOGGLE_ON, CONFIRM below API 34

| Like off | impact(.light) | TOGGLE_OFF

| Double-tap artwork to like | impact(.medium) | CONFIRM

| Long press opens menu | System context menu haptic | LONG_PRESS

| Scrub start / end | selection / impact(.light) | GESTURE_START / GESTURE_END

| Volume and slider detents | selection | SEGMENT_FREQUENT_TICK

| Row picked up to reorder | impact(.medium) | DRAG_START

| Row dropped | impact(.light) | GESTURE_END

| Swipe action armed | impact(.rigid) | GESTURE_THRESHOLD_ACTIVATE

| Swipe action disarmed | selection | GESTURE_THRESHOLD_DEACTIVATE

| Profile switched, handoff done | notification(.success) | CONFIRM

| Blocked or failed action | notification(.warning) / (.error) | REJECT

| Lyric word sweep, gradient drift | No haptics. Ambient motion never vibrates.

Constants marked API 34 fall back through HapticFeedbackConstantsCompat, which maps them to the nearest older effect.
