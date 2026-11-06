# Board 12: Output and handoff

PNG: `design/wireframes/png/12-output-and-handoff.png`  
HTML: `design/wireframes/html/Handoff.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Output and handoff

1. Continue banner. Appears when this app opens while another of your devices is playing. [TAP] Continue pulls playback here. [SWIPE] up dismisses it, and it also fades after 8 s. [GUARD] it never transfers by itself, never shows while this device is playing, and shows once per session for each device.

2. Sheet. Opened from Output on Now Playing, the mini player, or the banner. [SWIPE] down closes. Detents: medium and full.

3. This device. Selected row with level bars while sound plays here. [TAP] pulls playback back from a remote device.

4. System outputs. iOS embeds the system route picker for AirPlay and Bluetooth. Android opens the system media output switcher. The app cannot see which speaker was chosen, only that the route changed.

5. Your devices. Only devices signed in to the same profile. [TAP] moves playback there: snapshot of queue, position, shuffle, repeat and autoplay context goes over the sync connection, the target preloads and confirms, then this device fades out. [HAPTIC] success when confirmed, error if it fails, in which case this device keeps playing and a toast says why. [HOLD] menu: Control only (keep sound here), Rename, Sign out this device. Shared household devices need a confirm first.

6. Offline devices. Dimmed. [TAP] shakes with the warning haptic and the reason. They disappear after 30 days.

Remote control. While another device plays, Now Playing shows a capsule, "Playing on Living Room Mac". Transport, scrub, queue edits and volume act on that device. Tap the capsule to move playback here.
States. No other devices: "Sign in on another device to hand off music." Companion server unreachable: the devices section is disabled with the reason and a retry. Different profile on the other device: not shown.
