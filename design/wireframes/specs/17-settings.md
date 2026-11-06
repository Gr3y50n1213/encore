# Board 17: Settings

PNG: `design/wireframes/png/17-settings.png`  
HTML: `design/wireframes/html/Settings.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Settings: root and what is inside

1. Account card. [TAP] opens the profile page: name, avatar, PIN, sign out, and the profile switcher. Opened from the avatar on Home or from here.

2. Playback and quality. Streaming quality on Wi-Fi and cellular (Original, High, Low), download quality, gapless, crossfade length (0 to 12 s), loudness normalization, equalizer with presets, sleep timer, "Pause when headphones unplug", and "Resume after interruptions".

Lock screen and widgets. Sleep timer on the Lock Screen (a Live Update on Android), download progress on the Lock Screen, Hide listening history while locked (on) and Widget profile. Drawn on the Live surfaces and rules board.

Lyrics. Show lyrics by default, word-by-word highlighting, translation and romanization where the source has them, text size, and sources in order: server, then an optional lyrics provider you add.

3. Downloads. Storage used, limit, download only on Wi-Fi, auto-remove after 30 days unplayed, and a per-item list with swipe to remove.

4. Mixes and suggestions. Turn Made for you mixes on or off, set the time they refresh, choose how adventurous suggestions are (a 3-step slider), and "Reset what the app learned", which asks for confirmation and keeps the play history. "Hide explicit songs" removes tracks the server marks explicit from Mixes, shuffle and suggestions, and rows show an E badge for them.

5. Toggles. [TAP] anywhere on the row flips the switch. [DRAG] the thumb also works. [HAPTIC] toggle on and toggle off, selection tick on iOS. The switch settles over 250 ms, with a spring on Android that stretches the thumb while pressed.

Appearance. Auto, light or dark; accent color follows the playing album by default or a fixed color; reduce motion override; larger text follows the system. [NOTE] Reduce Motion, Reduce Transparency and battery saver from the system are always respected first.

About and licenses. App name, version and build, Privacy policy, Terms (if any), Open-source licenses (the full THIRD_PARTY_LICENSES list, with the MPL-2.0 notice and links to the source for each library), Source code, Report a problem (opens a prefilled mail or issue with no personal data), and credits for the demo tracks.

Layout. iOS uses grouped inset lists under a large title that collapses on scroll; Android uses a large top app bar and separated rows with 24 dp outer and 6 dp inner corners. Settings sits in the Library tab on phones and has its own sidebar item on tablets and desktop. Search. A search field above the list filters every setting by name, and results show the page they live on. Changes apply at once; there is no Save button. A setting that needs a restart says so inline.
