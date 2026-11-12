# Widget redesign: first test on a real phone (12 November 2026)

Build: branch `widgets-redesign`, commit 45c174a (CI run 43231709762, green), release APK installed over the
existing app on the S20 FE (One UI 5.1, Android 13), the A13 and the Tab A7 Lite. Only the S20 FE was looked at
on screen (the others were asleep or in use). Volume was 0 (speaker). Findings are recorded here for a later fix;
nothing below has been changed in code.

## Bugs

1. **Mixes and Recently played draw Now Playing's content after the app has run (high).**
   Right after the install the three widgets were correct (Now Playing, Mixes hero + tiles, Recently played
   3x2 with artists). After Encore was opened once, the Mixes instance (appWidgetId 16) and the Recently
   played instance (17) both showed Now Playing: `dumpsys appwidget` still lists them under
   `MixesWidgetReceiver` and `RecentWidgetReceiver`. Suspect: `WidgetSync.sync` calls
   `NowPlayingWidget().updateAll`, `MixesWidget().updateAll`, `RecentWidget().updateAll`; if
   `getGlanceIds` hands one class another's ids, a session for the wrong widget class is started for
   them (a session that is already running is not replaced by `update`). Fix idea: update per
   instance, pairing each receiver's `AppWidgetManager.getAppWidgetIds(ComponentName)` with its own widget
   (`widget.update(context, GlanceAppWidgetManager.getGlanceIdBy(id))`), and add a device test that
   runs `WidgetSync` with all three placed. CI did not catch it: the gallery hosts each widget through its
   own receiver and never runs `WidgetSync`.
2. **Play on the Now Playing widget does nothing when nothing is queued (medium; likely older than this
   redesign).** The tap starts `PlaybackService` (MEDIA_BUTTON), which logs
   `MediaSessionImpl: Failure calling MediaSession.Callback.onPlaybackResumption(): Nothing queued`
   and stops 3 s later. The widget shows the last played song, so Play should start that song (or the
   queue the core restores) instead.
3. **Palettes appear only after the app has been opened (medium).** Right after the update the widgets drew
   in the theme's colors (no palette in the snapshot yet); the album colors arrived about 30 s after Encore
   was opened. Fix idea: have the widget fall back to a palette computed from its own cover bitmap (the
   widget already holds it) or run the writer once from the package-replaced receiver.

## Layout shortcomings

4. **Sizes between my buckets leave dead space.** One UI's 4-wide cell is 326 dp on the S20 FE, between my 250
   and 340 dp buckets, so the 326x354 Recently played widget used the 250x250 layout (3x2 with artists) and left
   about 100 dp empty under it; the 326x226 Mixes widget used the 250x200 hero layout (3 tiles) and left
   about 40 dp. Fix: add buckets at 320 dp wide and for 3 rows, or let the covers grow with the spare height.
5. **A tall Now Playing instance leaves a gap.** A 326x354 instance (placed before the 260 dp limit) uses the
   FULL_TALL layout, with a large empty area between the cover/title and the controls. The new limit stops
   new ones; an existing one keeps its size. The tall layout could grow the cover to use the room.
6. **Picker previews show square covers** (the artwork in the preview layouts has no rounded corners), unlike the
   real widgets.
7. **A dark album gives a dull field.** Serotonin Dreams (a near-black cover) draws as grey-black with a faint
   brown blob; it is faithful to the app's field, but the widget looks flat. Consider a minimum chroma or
   lightness for the widget field only.

8. **The album gradient behind the widget looks stretched (owner's note, seen on the phone).** The field is one
   160x80 bitmap drawn with `ContentScale.FillBounds`, so it is pulled to whatever shape the widget has:
   a wide, short 4x1 (326x108 dp) or a tall one stretches the blobs into streaks, unlike the app's field.
   Suspect: the fixed 2:1 bitmap against widget shapes far from 2:1. Fix idea: render the bitmap at the
   widget's own aspect (one per size bucket, or per reported size), or scale the blob radii by aspect as
   `AlbumField` now does, and add a gallery check that compares the field's look at 4x1, 2x2 and 4x2.
   Not yet looked at closely on the other two phones.

## Checked and fine

- Real Samsung cell sizes (S20 FE: 4x1 = 326x108 dp, 2x2 = 158x226; A13: 4x1 = 418x121, 2x1 = 204x121,
  3x2 = 311x252) all fall inside the buckets and nothing was cropped.
- Now Playing 4x1 and 4x2 (album colors, rounded-corner clipping, tonal previous/next, round play pill when
  paused) look right on One UI.
- The widget picker shows the new previews for all three widgets (album field, rounded-square pause, level bars).
- No crash or widget error in the logs on any of the three phones after the install.

## Not tested

Landscape, resizing on the launcher, the lock-screen Now Playing widget, a playing state (bug 2 stopped it),
Mixes tile taps (bug 1 hid the widget), font scale above 1.0, the A13 and the tablet on screen.
