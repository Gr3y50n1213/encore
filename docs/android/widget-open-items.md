# Widgets redesign: everything still to fix, check or write (12 November 2026)

Branch `widgets-redesign` (based on `phase9`; neither merged). Last CI: run 43305039297 on commit 32051b3,
green on API 28, API 36 phone and API 36 tablet, red only on the allowed API 37.0 job. The history of the
phone tests is in `widget-device-test-2026-11-12.md`; this page is the to-do list.

## 1. Bugs and shortcomings found, not yet fixed

1. **A short strip of tiles leaves an empty gap.** The Mixes widget with only two mixes (S20 FE, hero card
   plus one tile) draws the tile at the left and leaves the rest of the strip empty. Let the tiles share the
   width, or center them.
2. **A 326x354 Recently played still has about 50 dp spare above and below** (covers can't outgrow their
   column and the core sends at most 8 albums, so a 3x3 grid doesn't fit). Options: a hero card for the last
   album, or a 3x3 grid with the core sending 9.
3. **The estimated palette (bug 3's fix) is only a stand-in.** It is made from the cover's four quadrants, so
   its colors can shift a little when the core's palette arrives. Look at the change on a phone; if it
   jars, fade between the two or compute the same palette in the widget.
4. **Samsung's pickers and launcher have not been checked for the rounded preview covers** (they look right in
   the emulator).
5. **Widget Play has no emulator test.** The empty-queue fix was only seen on the S20 FE. Add a device test
   that clears the queue, taps Play and expects playback.
6. **Release-only problems are invisible to CI.** The R8 class merge (three widgets drawn as one class) passed
   every emulator test because they run the debug build. Today only `WidgetKeepRulesTest` reads the rule's
   text. Run the widget device tests against the minified release APK too, or in `device-check.sh`
   once, so a merge like that fails a test.
7. **The gallery does not check the background's shape.** Add a check that compares the field at 4x1, 2x2 and
   4x2 (round blobs, nothing stretched).
8. **`collections-play` is flaky on the API 36 phone emulator** ("the profile has a mix to start"; failed
   once and passed on rerun with nothing changed). The replay's mixes may not be built in time: wait for them
   or retry.
9. **API 37.0 emulator job fails** (allowed, cause not looked into since Phase 9).
10. **The debug-only readout of the widget sizes Android reports** is approved but not built (shown on the
    Lock screen and widgets page in debug builds only), needed to confirm One UI's cell sizes on the other phones.

## 2. Not tested yet (needs a phone, or the owner)

- Landscape (the Now Playing background must change much less; `AlbumFieldAspectTest` covers the math only).
- Resizing on the launcher (dragging the handles through the buckets).
- The Now Playing widget on the lock screen.
- A tap on a Mixes tile and on the hero's play button.
- Font scale above 1.0 on a phone (Samsung's slider reaches 2.0).
- The A13 and the Tab A7 Lite on screen (A13 was asleep and locked, the tablet had Waypoint in front).
- The R8 keep rule on a phone (it is in 32051b3; the phone has 9fea492, which works without it).
- The owner's own look: One UI rounded-corner clipping, AMOLED colors, Samsung's font at larger sizes.

## 3. Decisions waiting for the owner

- Frame behind Mixes and Recently played: option A (the lead album's softened field, in use) or the neutral
  frame B (one-line switch).
- Whether the widget Play with an empty queue should start the last song played (done) or the Next up mix.
- When to merge: `phase9`, then `widgets-redesign`, into main, only after the owner has tried the build.

## 4. Documents still to write

- Board 34 spec items 1 to 3 (colors follow the album now, not the wallpaper; no progress bar; every size drawn).
- `docs/shared/PLAN-UX.md` section 20 and `docs/android/PLAN-ANDROID.md` section 20.
- New `docs/adr/android-0006-album-colored-widgets.md`, and a "superseded in part" line in `android-0005`.
- `docs/android/REVIEW-UI.md`.
- Save the approved mock-up beside the board PNG.
- Note the R8 lesson (keep rules for Glance widgets) in the Android track notes.

## 5. Phase 9 follow-ups (not from the redesign)

- Live Updates note wording on Android 16 without QPR1; Live Updates are unverified on a phone.
- The "Widget profile" row's text wraps.
- The "Sleep timer set for..." toast shows behind the sheet.
- A tile's long press opens the Mixes tab, not the mix.
- Board 31 item 5: per-profile wording.
- The tablet log "lost permission to set exact alarms" after an update (not investigated; may affect the sleep timer).

## 6. Later, in roadmap order

Phase 10 recommendations v2, Phase 11 recap, Phase 12 more servers (Plex and others), Phase 13 release.
