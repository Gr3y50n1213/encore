# Board 21: Connect a server

PNG: `design/wireframes/png/21-connect-a-server.png`  
HTML: `design/wireframes/html/Connect.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Connect a server

1. Steps. The setup flow has four steps: Welcome, Connect your server, Choose what to include, Optional sync across devices. Back always works and keeps what was typed. Setup can be left and resumed.

2. Server type. A connected button group; the selected option is fully round and the others have tighter inner corners. Jellyfin is available now. Other types are shown dimmed as "soon" only if they are on the roadmap, and "Other" explains that new servers are added through adapters. [TAP] selects with a selection haptic.

3. Address. The URL field checks as the user stops typing (600 ms). It accepts a bare host or an address with a port, adds https if missing, and tries the common port. The result line says what was found: server type, version, and whether the connection is secure. Plain http to a local-network address (192.168.x.x, 10.x.x.x, .local) shows a quiet warning and is allowed. Plain http to any other address is refused with "This address is not secure. Use https", because iOS blocks it and the app enforces the same rule on every platform. A self-signed certificate asks once to trust its fingerprint.

4. Credentials. Username and password, with the password revealable by an eye button. Autofill from the system password manager works. Nothing is stored except a token in the system keychain or keystore, never the password.

5. Sign in. [TAP] shows a spinner inside the button and disables it, which prevents double submits. Wrong password: the fields shake once, the password field clears, the warning haptic fires, and the message is plain: "Wrong username or password". Server unreachable names the cause. [HAPTIC] success when signed in.

6. Quick Connect. Shows a 6-character code and a "Waiting for approval" row with a countdown. The code is approved from another signed-in Jellyfin device, then this one continues by itself. Cancel returns to the form. The code expires after 5 min.

7. Try a demo library. [TAP] opens a small library of openly licensed tracks, with synced lyrics and artwork, built from files bundled in the app. It needs no server, no sign-in and no network, and every feature that does not need a server works, so App Review and Play reviewers can try the app without credentials. A banner reads "Demo library" with a "Connect a server" button, and demo data is never mixed into a real profile. Tracks are credited in About and licenses.

Local network access. The system may ask to find devices on the local network the first time this screen contacts a home address. The app explains why first; see the Permissions board. Denying it only turns off address discovery and LAN addresses, and a public https address still works. Next steps. Choose what to include lists the server's music libraries with check rows, all ticked by default. Then the optional companion step offers a sync address, or "Skip, use this device only". Finish opens Home while indexing runs in the background. Keyboard. The form scrolls above the keyboard, the Return key moves to the next field and then submits. Add account / add server later reuses this exact screen.
