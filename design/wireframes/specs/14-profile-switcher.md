# Board 14: Profile switcher

PNG: `design/wireframes/png/14-profile-switcher.png`  
HTML: `design/wireframes/html/Profiles.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Profile switcher

1. Opening. [TAP] the avatar on Home or Library. [HOLD] the avatar for a short list that switches in one tap. The sheet lists every profile signed in on this device, active first. [SWIPE] down closes.

2. Switching. [TAP] a profile. If music is playing, an inline choice appears: "Keep playing" or "Stop and switch". Locked profiles ask for biometrics, then a 4 to 6 digit PIN. Then the content cross-fades, the tab resets to Home, and the new profile's last queue loads paused. [HAPTIC] success. [GUARD] switching never deletes or signs out anything.

3. Row actions. [HOLD] a row: Lock with PIN, Sign out, Remove from this device. Sign out and remove need a confirm sheet; removal keeps the profile's data on the server and deletes only the local copy. Removing is not deleting: deleting a profile and its data lives in Privacy and data, and is the in-app account deletion path.

4. Add account. [TAP] starts sign-in for the same server or a new one, using the onboarding flow. The new profile becomes active when sign-in succeeds.

5. Guest. [TAP] starts a temporary profile that records no history, makes no recommendations and syncs nothing. A banner reads "Guest, nothing is saved". It ends with a tap, or after 4 hours idle, and returns to the previous profile.

6. Manage profiles. Rename the display name, change the avatar, set or remove a PIN, reorder, sign out. Avatars come from the media server's user image, with initials as the fallback. Biometric locks use Face ID or Touch ID through the system prompt, with the PIN as the fallback.

Isolation. Each profile has its own database, caches, downloads list, recommender state and settings. Downloaded audio files may be shared on disk by checksum to save space, but never the lists that point to them.
States. One profile only: the sheet shows just that profile, Add account and Guest. Session ended: the row shows "Sign in again" and keeps local data until the user removes it. PIN: five wrong tries lock for 30 s, then longer. Biometrics changed on the device: PIN is required once.
