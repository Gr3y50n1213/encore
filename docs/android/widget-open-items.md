# Widgets redesign: everything still to fix, check or write (12 November 2026)

Branch `widgets-redesign` (based on `phase9`; neither merged). Last green CI: run 43371385462 on commit 07bd3ac (API 28, API 36 phone and tablet; only the allowed API 37.0 job is red; `collections-play` failed twice on API 28 and passed on the third try, a timing flake in how long the core takes to build the first mixes). The history of the phone tests is in `widget-device-test-2026-11-12.md`; this page is the to-do list.

## 1. Bugs and shortcomings still open

1. **A 326x354 Recently played still has about 50 dp spare above and below** (covers can't outgrow their
   column and the core sends at most 8 albums, so a 3x3 grid doesn't fit). Options: a hero card for the last
   album, or a 3x3 grid with the core sending 9.
2. **The estimated palette is only a stand-in.** It is made from the cover's four quadrants, so its colors can
   shift a little when the core's palette arrives. Look at the change on a phone; if it jars, fade between the
   two or compute the same palette in the widget.
3. **Samsung's picker has not been checked for the rounded preview covers** (they are clipped vectors; right in
   the emulator).
4. **Widget Play with nothing queued has no emulator test.** The core has no call that empties the queue, so only
   the S20 FE showed the fix. Add one if such a call appears (`AutoLibrary.onPlaybackResumption` is the code).
5. **Release-only problems are invisible to CI.** The R8 class merge passed every emulator test because they run
   the debug build; today only `WidgetKeepRulesTest` reads the rule's text. Run the widget device tests against
   the minified release APK once in `device-check.sh`.
6. **API 37.0 emulator job fails** (allowed, cause not looked into since Phase 9).

Done and confirmed in CI (commits 1854160 and 6a4eb86, run 43322617742; the screenshots show the centered strips): a short strip of tiles is centered (the gallery has a one-mix and
two-album library); the gallery fails on a stretched field picture; `collections-play` waits up to a minute for the
mixes to build (it failed once on API 36 and passed on a rerun); the debug-only readout of the sizes Android
reports is on Settings > Lock screen and widgets (`WidgetSizeLog`).

## 2. Not tested yet (needs a phone, or the owner)

- Landscape: the Now Playing background now keeps round blobs of the same pixel size at the same screen-relative
  places (the first fix stretched portrait's picture and the owner did not like the oval look). `AlbumFieldAspectTest`
  checks roundness and the amount of colour; the owner should look at it on a phone.
- The press highlight on the widgets' round buttons, covers and tiles (it was a square; each control now has a
  ripple with a mask in its own shape, `widget_ripple_*`). A press cannot be shown by a screenshot test.
- Resizing on the launcher (dragging the handles through the buckets).
- The Now Playing widget on the lock screen.
- A tap on a Mixes tile and on the hero's play button.
- Font scale above 1.0 on a phone (Samsung's slider reaches 2.0).
- The A13 and the Tab A7 Lite on screen (A13 was asleep and locked, the tablet had Waypoint in front).
- The owner's own look: One UI rounded-corner clipping, AMOLED colors, Samsung's font at larger sizes.

## 3. Decisions waiting for the owner

- Frame behind Mixes and Recently played: option A (the lead album's softened field, in use) or the neutral
  frame B (one-line switch).
- Whether the widget Play with an empty queue should start the last song played (done) or the Next up mix.
- When to merge: `phase9`, then `widgets-redesign`, into main, only after the owner has tried the build.

## 4. Documents

Done: board 34 spec items 1 to 3 (amended), `PLAN-UX` section 20, `PLAN-ANDROID` section 20, `android-0006` and the superseded-in-part line in `android-0005`, `REVIEW-UI` and the mock-up beside the board files (`design/wireframes/html/34-widgets-redesign-mockup.html`). The board 34 PNG still shows the old look.

## 5. Phase 9 follow-ups (not from the redesign)

Fixed in the "landscape, highlights and follow-ups" round (12 November 2026, needs CI and the owner's phone):
the Live Updates note now says Android 16 QPR1 (version 16.1); the "Widget profile" row's subtitle is "For new
widgets" so it no longer wraps; the sleep timer sheet shows the end time in the sheet (and any error under its
title) instead of a toast hidden behind it; a long press on the Start a mix tile opens the pinned mix on top of the
Mixes tab (Next up: the tab alone); board 31 item 5 says one timer per device.

Still open:

- Live Updates are unverified on a phone (needs Android 16 QPR1).
- The tablet log "lost permission to set exact alarms" after an update (not investigated; may affect the sleep timer).

## 6. Later, in roadmap order

Phase 10 recommendations v2, Phase 11 recap, Phase 12 more servers (Plex and others), Phase 13 release.
