# Board 22: Welcome

PNG: `design/wireframes/png/22-welcome.png`  
HTML: `design/wireframes/html/Welcome.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Welcome

1. Background. The same slow-drifting gradient system as Now Playing, with the app's own palette, on a 40 s loop. Reduced motion shows one still frame. The name and line fade up over 500 ms, staggered by 120 ms. On Android a Material cookie shape sits above the name, turning slowly and morphing to the sunny shape and back every 8 s.

2. Promise line. One sentence, with the privacy stance, and no feature list. Nothing here asks for permissions.

3. Get started. [TAP] moves to Connect your server with a push transition (shared glass morph on iOS, shared-axis on Android). [GUARD] disabled for 300 ms after the screen appears so a stray tap cannot skip it. [HAPTIC] soft.

4. Existing profile. [TAP] asks for the companion server address and signs in, then pulls this profile's settings, history and queue down. Used when the user already set the app up elsewhere.

5. Policy links. Privacy policy and Open-source licenses open in an in-app browser sheet (SFSafariViewController on iOS, Custom Tabs on Android), so the policy is reachable before anything is collected or any account exists. The same two links live in Settings, About and licenses, and the privacy policy also appears in Privacy and data. The store listing links the same page.

First launch only. Later launches go straight to Home, or to the profile chooser if more than one profile is signed in. Tablet and desktop. The content sits in a centered column, 420 pt wide, over the same background. Accessibility. The gradient is decorative and hidden from screen readers; the buttons are the only focus stops. Dark and light. This screen is dark in both themes by design.
