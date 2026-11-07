# Android review: the full UI (Phases 3, 4, 5 and 7 screens)

7 November 2026, on the owner's request: "do the complete UI and UX build and test it when you're done", after "the playlist screen is completely broken".

**Evidence:** CI run 42420855371 (`android-ui`, merged to `main` as 6281b09), every job green: lint, unit tests, debug and R8 release builds for four ABIs, and on the API 28 and API 36 (16 KB pages) emulators the fresh-install tests plus the end-to-end test, which signs in to the owner's Jellyfin recording through the real Connect screen, syncs, opens an album, plays it, then opens a playlist and checks its songs are listed. The release APK is signed with the owner's test key (SHA-256 14:16:5A:C2...). Installed on the Tab S11+ and the A13; nothing has been judged on a phone yet.

## What was built, by board

| Board | What the app does now |
| --- | --- |
| 05 Home | The core's Home: Made for you with every shelf state (fresh, out of date with Refresh, still learning with the count, learning off, private), Recently played, Rediscover, Deep cuts, Recently added, Most played; tiles open and never play; the profile button with an unsynced dot; pull to refresh |
| 06 Library | Chips, sort, grid or list remembered per profile; songs get the menu and swipes; Settings in the header (board 17) |
| 08 Album | Palette tint, Play and Shuffle as a connected group (outer round, 10 dp facing), disc headers, playing row bars, partial-album line, More by this artist, Replaced queue with Undo |
| 09 Now Playing | The album's moving field (AGSL shader from Android 13, radial brushes before, still with Reduce Motion); collapse, "Playing from", More (with Private listening); artwork that swipes to skip, double-taps to like with a heart, holds for the menu, shrinks to 88 % paused; marquee title; like in a tonal circle; the wavy scrub bar with a pill handle, slowdown bands and start/end haptics; the state-shaped transport with hold-to-scan; Lyrics / Output / Queue as one connected group; swipe down to collapse; landscape split |
| 10 Lyrics | Word-by-word fill, line highlighting, 40 % past lines with a 1.2 dp blur, active line 30 % from the top, opposite-side singers, background vocals, breathing dots in gaps, tap to seek with "Back to 2:31" for 4 s, auto-scroll paused 3 s after a manual scroll, Not synced, No lyrics |
| 11 Queue | Shuffle, Repeat (off, all, one) and Autoplay; History collapsed; now playing row without swipes; Up Next reordered by the handle only, swipe right Play Next, left Remove with Undo; Autoplay rows with "Not this" and the spark mark; Clear with a confirm |
| 12 Output and handoff | This phone, the system output switcher, your other devices with every state, hand off with success or error haptic, Control only, Rename; the Continue banner (8 s, swipe up); handoff and remote commands handled by the player |
| 13 Song menu | On every row: Play Next, Add to Queue, Add to Playlist (several at once, or new), Like, Start radio, Go to Album/Artist (hidden where you are), Why this song (app picks), page extras, Not interested in red last |
| 14 Profiles | Switch with Keep playing / Stop and switch, PIN, Guest, Add account, Remove from this device; Home resets after a switch; appearance per profile |
| 15 Mix page | 2x2 mosaic cover, tint, description and stamp, Play/Shuffle, Pin (fills), a reason per song, Not in this mix, Save as playlist, Hide |
| 17-19 Settings | Search, account card, separated rows (24/6 dp); Playback quality per network; Mixes and suggestions (3 steps, Not interested list, Reset); Appearance (automatic/light/dark, eight token accents or the wallpaper's); Servers and the companion server; Privacy and data (Learn from my listening, what it connects to, export through the share sheet, clear history, delete everywhere with the typed name); About and licences |
| 20 States | Empty texts, the offline banner after 2 s, error toasts with the core's reasons |
| 23 Mixes | Today's mixes in a grid, each in its own Material shape from its id, a corner play button, pinned first, where and when they were made |
| 29 Tablet | 80 dp rail from 600 dp, the expanded 240 dp rail from 840 dp (playlists, the profile at the foot), Library list and detail as two 28 dp panes 12 dp apart, the player bar with the whole transport |
| 42 Swipes | Mini player and artwork: 12 dp direction lock, 60 dp or a 500 dp/s flick, one haptic, the neighbour only drawn, rubber band and "End of queue", the 24 dp edge left to Back, swipe up to expand; songs passed over within 600 ms never load. Rows: 25 % (60 % destructive), one haptic, TalkBack actions |

**The playlist screen** was rebuilt like the album page (tint, cover, Play and Shuffle, menu and swipes, Remove with Undo, removed songs kept and dimmed), and CI now opens one.

## Where the boards disagree (for the owner)

| Item | Board 29 (Android tablet) | Section 22, boards 35 and 42 | What the app does |
| --- | --- | --- | --- |
| Play button shape | Rounded square paused, circle playing | Round paused, rounded square playing | Section 22, everywhere |
| Track swipe end to start | Likes, past 40 % | Add to Queue, past 25 % | Board 42 |
| Facing corners in a button group | 8 dp | 10 dp (tokens) | The tokens |

## Not built yet, and why

- Downloads, the Download buttons and offline-only browsing (Phase 8); Android Auto (8); the sleep timer, widgets, Quick Settings tile and Live Updates (9); the year recap (11).
- Search's scope chips and Browse by mood: moods come from audio analysis (Phase 10).
- The demo library (bundled tracks), multi-select, drag and drop onto playlists on tablets (board 43), the Large supporting pane, fold postures (tabletop and book), and the "Playing on ..." remote-control capsule.
- Settings with no core behind them yet: crossfade, normalisation, equaliser, lyrics options, Hide explicit.
- Screenshot tests for every control state, and comparing each screen with its board at the same size: next, on the emulators.

## What the owner should try on a phone

Now Playing (swipes, double-tap like, hold Previous/Next, scrubbing slower lower down, Lyrics/Queue), the queue (reorder by the handle, swipe to remove and Undo), a long press on any song, a playlist, Home and Mixes once there are enough plays, the profile switcher, Settings > Appearance, and on the tablet the rail and the side-by-side Library.
