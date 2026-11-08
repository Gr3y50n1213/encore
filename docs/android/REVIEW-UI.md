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
| Queue toggles (board 11 render) | Shuffle on drawn as a circle, off as rounded squares | On is the rounded square, off round | Section 22 |
| Search "Browse by mood" and Settings "Lyrics", "Downloads", "Lock screen and widgets", "Crossfade" (boards 07, 17) | Drawn | No core behind them yet (Phases 8 to 10) | Left out rather than shown doing nothing |

## Second review against every board (7 November 2026)

The owner asked for "a deep look one more time at all of the wireframes and renders and all the documentation on the Android side". Every Android board was compared with the app (01 to 26, 29, 35, 38, 42 and the specs), and the owner named three things:

| Owner's point | What the app does now | Source |
| --- | --- | --- |
| "The play button ... is an odd shape instead of the usual icons" | Play and pause always show their glyph; loading shows Pause (a tap cancels the load), and a stall over 500 ms runs the indeterminate wavy bar instead | Owner, in place of spec 09 "spinner replaces play" and board 20 "the play button becomes a ring" |
| The album page should take the album's colour | Album and playlist pages fade from the palette's leading stop down to its dark base, the page pinned under the scrolling list; the status bar icons follow the top colour; no artwork falls back to the profile accent | Board 04, "Album and playlist pages"; spec 08, item 2 |
| A download button beside Play and Shuffle | Play, Shuffle and Download are one connected group (Play widest). Downloads are Phase 8 and the core has none yet, so the button says so | Boards 08 and 24 |

Also brought in line: the mini player (board 42: a filled accent play button and a tonal Next, the container lightly tinted by the album, board 04); the mix page (board 15: the moving field, the mosaic in the mix's own shape, Play, Shuffle and Pin as one group, Back and More as tonal rounded squares, Save and Hide moved into More); filter chips (board 35: a 12 dp corner and a check on the selected one); the song menu (board 13: text only, sentence case, groups split by bands); Settings pages (boards 18 and 19: rounded-square Back, the page action as a tonal pill, small centered section labels, health dots before server names, switches with a check); How adventurous (board 38: a segmented choice with a check and "About 1 in 10 picks will be new to you"); Like (board 35: round off, squarer and filled on); empty states as tonal cards (board 20).

| Item | Boards | What the app does |
| --- | --- | --- |
| How adventurous | Board 17 says "a 3-step slider"; board 38 draws a segmented choice | Board 38, the page's own board |
| Filter chips | Board 35: 12 dp corner, a check when selected, unselected outlined; renders 06 and 07: tonal fill, no check | 12 dp, tonal fill (renders) and the check (board 35) |
| The queue on a phone | Board 09, item 8: Lyrics, Output and Queue "toggle each view in place"; board 29 draws Up next on the album field; board 11's render draws a light sheet | On the field, like Lyrics (the owner disliked the light card that mixed the two): the field's text colour, tonal toggles that fill and square up when on, a tonal highlight on the playing row, edges that fade into the field; Autoplay uses the Material Symbols "autoplay" icon, not a radio |
| Where Settings opens on a phone | Spec 17: "Settings sits in the Library tab on phones"; render 06 shows no entry; board 14's sheet has none | The avatar (Home, Library, Search) opens the profile sheet, which has a Settings row |

## Not built yet, and why

- Downloads, the Download buttons and offline-only browsing (Phase 8); Android Auto (8); the sleep timer, widgets, Quick Settings tile and Live Updates (9); the year recap (11).
- Search's Downloaded scope (shown, disabled until Phase 8) and Browse by mood: moods come from audio analysis (Phase 10).
- The demo library (bundled tracks), multi-select, drag and drop onto playlists on tablets (board 43), the Large supporting pane, fold postures (tabletop and book), and the "Playing on ..." remote-control capsule.
- Settings with no core behind them yet: crossfade, normalisation, equaliser, lyrics options, Hide explicit.
- From the second review: the album tile's long-press menu and the list-mode fast scroll handle (board 06, items 4 and 6), the row lift and dim behind the song menu (board 13, item 1), the Not interested sheet with scope and reason (board 38), "Lyrics and artwork" sources on the Servers page (board 18, item 4, no core yet), the tablet album detail as a grouped track list (board 29, item 3).
- Screenshot tests for every control state, and comparing each screen with its board at the same size: next, on the emulators.

## What the owner should try on a phone

Now Playing (swipes, double-tap like, hold Previous/Next, scrubbing slower lower down, Lyrics/Queue), the queue (reorder by the handle, swipe to remove and Undo), a long press on any song, a playlist, Home and Mixes once there are enough plays, the profile switcher, Settings > Appearance, and on the tablet the rail and the side-by-side Library.
