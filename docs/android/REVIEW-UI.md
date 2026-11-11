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
| Search "Browse by mood" and Settings "Lyrics", "Downloads", "Lock screen and widgets", "Crossfade" (boards 07, 17) | Drawn | No core behind them yet (Phases 8 to 10) | Left out rather than shown doing nothing. Since then, Downloads (Phase 8) and Lock screen and widgets (Phase 9) are built |

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

## The owner's third round on the phone (7 November 2026)

Decisions the owner made after using the app with their own library, which override the boards where they differ:

| Owner's request | What the app does | Board it changes |
| --- | --- | --- |
| The long-press and three-dot menus "do not fit the theme" | One action sheet for songs, albums and mixes: the item and Like at the top, connected quick tiles (Play next, Add to queue, Add to playlist), then rounded tonal groups, Not interested apart in red | Board 13's popup that scales from the row |
| More should turn into a squircle | Round at rest, squeezes when pressed, a squircle while its sheet is open | Board 35's toggle rule applied to More |
| A tap on the cover plays or pauses | Tap toggles playback; 2xTAP likes, HOLD opens the sheet | Board 09, item 2 had no single tap |
| The wave should come from the playing point and move smoothly | The app draws the wave: pinned to the centre line at the handle, travelling away from it, slower; the position runs on frame time | Board 09, item 5 (the library's wavy indicator) |
| Rows should move out of the way while dragging | Live reorder with a tick per swap; saved on release | Board 11, item 4 |
| Home "doesn't call people to use it" | A greeting, a Jump back in card, quick tiles, then Made for you and larger shelves | Board 05 |
| Mixes while learning should feel like it is working | A morphing Material shape with sparkles, the plays on a progress line, Play something | Boards 15 and 23 |
| The bottom bar is sharp; the gesture area is grey | 28 dp top corners, floating with the mini player; the system scrim is off | Board 05's flat bar |
| Settings is hard to read | 16 sp rows, icons in tonal circles, left-aligned section labels in the accent, long choices as radio rows with a line each | Boards 17 to 19, 38 (centred grey uppercase labels, segmented choices) |
| "I already have a profile" is greyed out | It signs in, which is how an existing profile comes back | Board 22 |
| Start radio "doesn't do anything" (8 November) | Its toast was drawn under Now Playing, Autoplay off left the radio nowhere to go, and the song playing started again. Radio now turns Autoplay on, carries on the song playing, and its toast shows over Now Playing | Section 21 |
| Toasts don't match the app | Rounded like the mini player, in the app's tonal colours, and above Now Playing when it is open (under its top bar, never over controls) | Board 20 (the default dark snackbar) |
| The licences page printed the raw lists | About card (version, source, licence), what the app is built on counted by licence with filter chips and search, families that open, each licence's text a tap away | Board 17 |
## Phase 8 (offline and car), 8 November 2026

| What | Decision | Against |
| --- | --- | --- |
| The offline sign | A slim amber pill that floats under the status bar ("Offline · Showing downloaded music"); pages move their content by its height and keep their colours behind it | Board 20's 16 dp bar across the top: the owner asked for "not too intrusive but notable" |
| An album tile with nothing downloaded, offline | It dims but still opens, so the page can say which songs are saved; its songs refuse with a shake and "Not downloaded" | Board 06, item 5 (a shake on the tile itself) |
| A download that waits | A still ring and the reason ("Waiting for Wi-Fi"); the wave is kept for songs arriving | Boards 06 and 08 draw only the downloading ring |
| A connected group that scrolls | The page margin sits inside the scrolling row, so buttons slide out under the screen edge, and the chosen one is scrolled into view | Boards 06 and 24 draw four shelves that fit; Downloaded is a fifth |
| The storage bar | Every part is named: Downloads, Cache, Other (the system and other apps) and Free | Board 17, item 3 names used and free only |

## The owner's polish round (9 November 2026)

| Owner's request | What the app does | Board it changes |
| --- | --- | --- |
| Skip forward and back show no animation; check the other buttons | Previous and Next squeeze and kick in their direction on a press, like Play. Every remaining plain icon button (Home and Mixes Play, Search and Licences Clear, show password, Now Playing's collapse) has a pressed shape | Board 35 |
| The Output pop-up is "very basic" | "Play on" is rebuilt from the action sheet's parts: grouped tonal rows with icon discs, this device marked with level bars, Speakers and headphones, then "Your other devices"; each state (no server, Guest, no other device, unreachable) is one row with its reason and action | The plain list Phase 7 shipped |
| The three-dot menu in Now Playing should match better | Sheets opened from Now Playing (More, Add to playlist, Why this song, Play on) take the album's colours. Add to playlist and Why this song are drawn with the same grouped rows, and every row squeezes when pressed | Board 13 |
| The squiggle's tip passes the handle | The wave stops one gap before the handle and nothing of the bar is drawn inside the handle's clear space | Board 09, item 5 |
| Made for you cuts tiles off while swiping | A row of equal square tiles that snaps to a tile's edge, with the next one showing at the side | Board 05's multi-browse carousel, which resized tiles as they moved |
| Dragging a song in the queue can't go further than the screen | The list scrolls on its own when the held row nears the top or bottom, faster the nearer it is, and a second finger can scroll the list while the first holds the row | Board 11, item 4 |
| A real app icon | The repeat sign with round dots and pill bars, in Iris (ADR android-0004) | The placeholder note |

## Phase 9 (widgets, controls and live surfaces), 11 November 2026

| What | Decision | Against |
| --- | --- | --- |
| The sleep timer sheet, not running | Presets in rows (5, 10, 15, 30, 45, 60 minutes, then End of this song and End of this album), one line saying what will happen ("Music fades out over the last 10 seconds" or "Music stops after this song"), a Start button, and "Music pauses at ..." beneath it | Board 31's Android panel |
| The sleep timer sheet, running | The time left, "Music fades out at ...", a wavy bar from when it was set to when it ends, Add 15 min and Cancel timer, and Change time with 15, 30, 45 and 60 minutes, End of song and End of album | Board 31's Android panel |
| Cancel | No confirmation. The sheet closes, then "Timer cancelled" with Undo for 5 seconds | PLAN-UX section 20 and board 31, both satisfied |
| The sheet opened from Now Playing | Takes the album's colors, like the other sheets opened there | The polish round's rule (9 November) |
| Where the timer is set | Now Playing's More, Settings > Playback and quality (a row showing Off or "Ends at ..."), and a tap on the card | Board 31 |
| Per device or per profile | Per device; it carries on through a profile switch | Board 31 item 5 says per profile; item 7 says switching needs no change |
| Mixes widget and tile order | Next up first, then the pinned mixes, four at most, no mix twice. The tile starts the first pinned mix, else Next up | Board 34, which doesn't say what comes first |
| "Hide listening history while locked" | On by default. On Android it decides whether Now Playing on the lock screen shows the last played song (on: only Play and the mix name) and whether the download notification is private on the lock screen. Mixes and Recently played never go on the lock screen either way | Board 31's row, which names only widgets and notifications |
| The root Settings row | "Lock screen and widgets" is the fourth row of the first group, after Mixes and suggestions | Board 17's list |
| The page's Live Update toggles | Turning one off stops promotion only. The timer or download goes on with a normal notification | Board 31 |
| A widget set to another profile | Tapping opens the profile switcher instead of playing | Board 34 |
| Quick Settings tile "Add" | A button on the page from Android 13; below that, a line saying where to add it | Board 31 |

## Not built yet, and why

- Android Auto on a real head unit (built, checked as a media browser); the year recap (11).
- The first-sync Live Update (not on PLAN-ANDROID's list; sync doesn't run as a job) and the Recap widget (Phase 11).
- Lock-screen widgets on a real Android 16 QPR1 phone: built, but nothing available to check them on.
- Browse by mood: moods come from audio analysis (Phase 10).
- The demo library (bundled tracks), multi-select, drag and drop onto playlists on tablets (board 43), the Large supporting pane, fold postures (tabletop and book), and the "Playing on ..." remote-control capsule.
- Settings with no core behind them yet: crossfade, normalisation, equaliser, lyrics options, Hide explicit.
- From the second review: the list-mode fast scroll handle (board 06, item 6), the row lift and dim behind the song menu (board 13, item 1), the Not interested sheet with scope and reason (board 38), "Lyrics and artwork" sources on the Servers page (board 18, item 4, no core yet), the tablet album detail as a grouped track list (board 29, item 3).
- Screenshot tests for every control state, and comparing each screen with its board at the same size: next, on the emulators.

## What the owner should try on a phone

Now Playing (swipes, double-tap like, hold Previous/Next, scrubbing slower lower down, Lyrics/Queue), the queue (reorder by the handle, swipe to remove and Undo), a long press on any song, a playlist, Home and Mixes once there are enough plays, the profile switcher, Settings > Appearance, and on the tablet the rail and the side-by-side Library.
