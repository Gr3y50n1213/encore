# Board 18: Servers and sources

PNG: `design/wireframes/png/18-servers-and-sources.png`  
HTML: `design/wireframes/html/Servers.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Servers and sources

1. Add. [TAP] opens the add-server flow from the onboarding board: pick a type (Jellyfin now; the list grows as adapters are added), enter the address, sign in. Several servers can be active at once, and the library merges them.

2. Server row. A colored dot for health, the type, the song count and the last sync. [TAP] opens the server page: rename, address, sign-in status, library folders to include, sync now, sign out, remove. [HOLD] shows Sync now, Rename, Remove. [SWIPE] left reveals Remove, which asks for confirmation. Removing a server deletes only the local copy of its index and downloads, never anything on the server.

3. Unreachable. Red dot with the reason in words (wrong address, certificate not trusted, sign-in expired, server asleep). [TAP] retries with a spinner on the dot; [HAPTIC] error if it fails again. A self-signed certificate asks once to trust its fingerprint, and the fingerprint is shown. Songs from that server stay listed and dimmed, and downloaded ones still play.

4. Lyrics and artwork order. Each song takes lyrics from the first source that has them, in the order shown. Synced lines beat plain text. Drag handles reorder, and each source can be switched off. Providers are added by URL only, and the app never sends song titles to a provider the user did not add.

5. Companion server. Holds the event log, profile settings and handoff. Shows status, the address, the last sync, and a button for "Sync now". Sign out of the companion keeps everything on this device and stops syncing. Without a companion the app works fully on this device alone, with no handoff.

No servers. Empty state with one button, Add a server. Slow sync. A thin progress line under the title and "Indexing 3,200 of 12,480". Duplicates across servers. Matched by tags and duration; the best source is used and the others become fallbacks.
