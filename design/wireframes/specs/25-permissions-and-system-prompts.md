# Board 25: Permissions and system prompts

PNG: `design/wireframes/png/25-permissions-and-system-prompts.png`  
HTML: `design/wireframes/html/System-Permissions.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Permissions and system prompts

1. Primer first. Before any system prompt, the app shows one short screen in its own words: what it will ask, why, and that denying costs only one feature. It appears in context, at the moment the feature is used, never at launch and never as a stack of prompts. [TAP] Continue shows the system prompt. [HAPTIC] soft.

2. System prompt. Drawn by the OS and not editable; only its purpose string comes from the app (iOS) or the permission name (Android). Dashed outline here marks what the OS owns. The text must say what the app really does, and each string is reviewed against the plan's network list.

3. Not now. Closes the primer with no system prompt, so the one-time OS prompt is saved for later. The feature shows a quiet "Turn on" row where it would have worked. [GUARD] no feature is blocked behind a permission, and the app never asks twice in one session.

| Permission | Why, and when asked | If denied

| Local network
iOS · Android 17+ | Reach a server or device on the home network. Asked the first time a LAN address or handoff search is used. iOS: NSLocalNetworkUsageDescription. Android: ACCESS_LOCAL_NETWORK only once the app targets API 37; until then INTERNET covers it. | LAN addresses and discovery are off; public https addresses still work. A banner links to Settings.

| Notifications
iOS · Android 13+ | Optional. Asked when the user turns on "Download finished" or "Recap ready", never at launch. | Playback is unaffected; lock-screen and shade controls come from the media session.

| Face ID, fingerprint | Unlock a locked profile. Asked when the user turns on a profile lock. iOS: NSFaceIDUsageDescription. Android: USE_BIOMETRIC, no prompt. | PIN only.

| Background audio
Media playback | Capabilities, not prompts: iOS background mode Audio; Android FOREGROUND_SERVICE_MEDIA_PLAYBACK declared in the Play Console. | Not applicable.

| Never requested | Photos (sharing uses the share sheet), camera, microphone, contacts, location, tracking (no ATT prompt, nothing is tracked), and storage (files are chosen with the system picker). | Not applicable.

Settings link. Each denied permission shows an inline "Turn on in Settings" row on the page that needs it, which opens the app's page in system Settings. Android 11+ auto-reset. Permissions unused for months are reset by the system; the app asks again with the same primer. Review. Every purpose string and every permission in the manifest is listed in the release checklist, and anything unused is removed before submission.
