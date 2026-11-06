# Board 19: Privacy and data

PNG: `design/wireframes/png/19-privacy-and-data.png`  
HTML: `design/wireframes/html/Privacy.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Privacy and data

1. Opening line. A plain promise at the top that matches what the app does. No account with the app maker exists; the only network traffic goes to servers the user added.

2. Switches. Sync listening to my server (off keeps history on this device only and turns off handoff of taste, not of the queue). Learn from my listening (off stops all recommendation learning and hides Mixes). Neural engine (Core ML, NNAPI or CPU fallback). [TAP] flips with a toggle haptic. Turning a learning switch off asks nothing and is reversible.

3. What this app connects to. Lists every address contacted in the last 7 days, with counts, so the claim can be checked. Read-only.

4. Crash reports. Off by default and opt-in. If on, a report is shown before it is sent and contains no song titles or addresses.

5. Export. Creates one file with the play log, likes, playlists and settings in an open format, shared through the system sheet. Runs in the background with a progress row.

6. Clear and delete. Destructive rows sit apart and are red. [TAP] opens a confirm sheet that says exactly what will go: this profile's history, likes, playlists, recaps, learned taste, tokens and downloads, on this device and on the companion server, and on every device signed in to the profile. It also says what stays: the Jellyfin account and the music on the user's media server are never touched. The sheet asks to type the profile's name, and offers Export first. [GUARD] the destructive button is never under the thumb's resting position, and there is no undo. [HAPTIC] warning on opening and success once done. This is the in-app account deletion the stores require, because profiles are created in the app.

7. Privacy policy. [TAP] opens the policy in an in-app browser sheet. It says what is collected (nothing by the project; listening and profile data go only to servers the user added), how it is used, how long it is kept, and how to delete it. The same page lists the deletion steps, so the Play Console "delete account" web link and the App Store privacy link both have a home outside the app. Deleting by web is done on the user's own companion server, so the page explains the in-app path and the server admin page.

Multi-profile. Every switch and delete applies to the active profile only, and the page title shows its name. Offline. Clear and delete queue locally and finish on the server when it returns, with a visible "Waiting to finish on server" row. Guest. The page shows only "Nothing is saved in Guest mode".
