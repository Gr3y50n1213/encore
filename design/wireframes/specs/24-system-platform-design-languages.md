# Board 24: System: platform design languages

PNG: `design/wireframes/png/24-system-platform-design-languages.png`  
HTML: `design/wireframes/html/System-Platforms.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Platform design languages

Every platform has the same structure, copy, gestures and behavior. What differs is the material. iPhone, iPad and Mac get Liquid Glass: light, translucent surfaces that float over the album. Android gets Material 3 Expressive: tonal color, big shapes, and springy motion that you can read from the shape of a control. Windows and Linux get the same Expressive look, built as a small Flutter theme and drawn at desktop density.

iPhone, iPad, Mac · Liquid Glass

Play

Shuffle

Home
Mixes
Library

Golden Hour Static

Ember Cove

Material Glass is for things that float over content: the tab bar, mini player, buttons over artwork, menus and sheets. Lists and page backgrounds stay plain, and glass is never stacked on glass.

Shape Capsules for buttons and bars. Where one shape sits inside another, the radii are concentric (inner = outer minus padding) so corners run parallel. Album art takes one radius, 14 pt, on all four corners, even inside a capsule.

Color The content supplies the color. The album gradient sits behind the glass and shows through it. Selection uses the system tint. Text over light art flips to dark.

Motion Springs only. Glass lifts and shimmers under the finger, menus morph out of the pressed item, and the tab bar shrinks while scrolling down.

Type Large bold titles that collapse into the bar, tight tracking, one clear step between title and subtitle.

Built with SwiftUI glassEffect, GlassEffectContainer, tabBarMinimizeBehavior and MeshGradient. Earlier iOS versions fall back to the system blur materials. iPad and Mac use the same components; the Mac adds a menu bar, hover and a floating sidebar.

Android phone and tablet · Material 3 Expressive

Play

Shuffle

Grid

List

Cookie

Sunny

Clover

Arch

Squircle

Pill

Home
Mixes
Library
Search

Color Tonal surfaces from dynamic color: primary, secondary and tertiary containers, with a scheme built from the playing album. Color carries hierarchy, so shadows stay rare.

Shape Large radii (28 dp on art and cards) and a shape set for generated content: cookie, sunny, clover, arch, squircle, pill. Real album art never takes these; it keeps one plain radius on all four corners.

Button groups Outer corners fully round, facing corners 10 dp. The selected item becomes fully round, so the choice reads from shape alone, and pressing morphs a button while its neighbors squeeze.

Motion Expressive springs. Wavy progress with a pill handle, a loading indicator that turns and morphs between shapes, and a navigation pill that slides under the chosen tab.

Type Emphasized styles: medium-weight titles (28 sp, weight 500) and compact body text. The full scale is `typography.scale` in `design/tokens.json`. (Owner, 6 November 2026: page titles are 28 sp as on the Home board; boards 17, 18 and 23 still draw 34.)

Built with Jetpack Compose Material 3 expressive APIs: MotionScheme.expressive(), ButtonGroup, MaterialShapes, LoadingIndicator and the wavy progress indicators.

Play

Shuffle

Focus ring

Windows and Linux · Material 3 Expressive, desktop density

Look The Android language at desktop density: tonal surfaces, 28 px pane corners, connected button groups, wavy progress, spring motion. The same Play, Shuffle and Add group, hover and focus states, a table for songs, and the system caption buttons.

Why not Liquid Glass Glass is Apple's material. Windows and Linux windows sit over arbitrary desktops, so a tonal, opaque surface reads reliably everywhere and stays close to Android, as requested.

Built with Flutter with a small Encore theme: color roles from the album, shape tokens, connected button groups and a wavy progress painter, because Flutter does not ship the Expressive components. Optional Mica backdrop on Windows 11.

Shared on all platforms. Screen structure, wording, gestures and what each haptic means, accessibility behavior, and the album-driven color. A change to behavior is made once and lands everywhere.
