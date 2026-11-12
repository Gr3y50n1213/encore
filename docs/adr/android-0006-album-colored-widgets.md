# ADR android-0006: Widgets wear the album's colors, every size is drawn whole, and nothing in them ticks

Date: 12 November 2026
Status: accepted
Supersedes in part: ADR android-0005 (the three widgets' look and sizes; everything else in it stands)

## Context
The owner tried the Phase 9 widgets on the S20 FE, A13 and Tab A7 Lite on 11 November 2026 and said they "kind of suck":
- Resizing Now Playing cut off the top and bottom of the play button.
- The look didn't match the app: a flat wallpaper-colored tile, bare glyph buttons, default type.
- Some sizes made no sense: Now Playing at 2x2 to 3x2 showed one small cover in empty space; Mixes always drew one row; Recently played at most two.
- The Now Playing progress bar can't tick (a widget can't animate), so it only lied.
- Turning the phone changed the Now Playing album gradient too much.

The cause of the cut-off button: the layout was chosen by width only, and fixed dp content (a 48 to 56 dp button, a 108 dp text stack) was drawn into buckets as short as 40 to 100 dp. The landscape smear was in `AlbumField`: every blob was measured in units of the screen width, so a turned phone (2.2 times wider, 2.2 times shorter) merged the four colors into one.

## Decision
**Album colors.** Board 34 said colors follow the wallpaper, not the album; that rule is dropped for all three widgets.
- The core puts each item's palette (`scheme`, `base`, four `stops`, `text`) in the snapshot (CORE-API v7, "Widgets", Colors), made from the same 64 px image the app uses, fetched once per new album by the writer's own round (at most about 20 a round, only while reachable). A widget never fetches anything.
- The widget draws the field itself from the palette (`FieldMath`, shared with the app, no core imports): a small bitmap made **at each size bucket's own shape**, so the blobs stay round and nothing is stretched. A very dark album's field is lifted out of flat grey (stops under value 0.30, base under 0.12). Stale files (over 24 h) fade toward the base.
- Until the core has made a palette (the first start after an update), the widget estimates one from its own cover (`CoverPalette`, four quadrant colors, white text, stops darkened until it reads). It is replaced when the core's arrives.
- Mixes and Recently played sit on the lead album's softened field (the mock-up's frame A); one line switches to a neutral frame B.
- Bold system font (Glance has no custom fonts), title in the palette's text color, secondary text lighter, buttons in the text color with the base color as the glyph. Contrast of at least 4.5 is tested on every pixel of the field, at every widget shape, for a dark and a light palette.

**No progress bar.** The playing state is the play button's shape (round paused, rounded square playing, PLAN-UX section 22) and three level bars shown only while playing.

**Every size is a bucket, sized from its content.** `WidgetSizing.kt` holds the model: text height `ceil(1.35 x sp x fontScale)`, a fixed set of buckets per widget (14 for Now Playing, 12 for Mixes, 11 for Recently played), one layout each. Lines drop in a fixed order when they don't fit (album, artist, header, tile names); a title never drops, covers stay at least 44 dp and buttons at least 48 dp.
- Now Playing: 2x1 up to 6x2, never taller than 260 dp, so the sizes that left one cover in empty space can't be placed.
- Mixes: a row of tiles, a hero card with a strip, or a grid of up to six. Recently played: rows, and at 4x3 six bigger covers in three columns with an artist line.
- Widths 250, 310 and 410 dp for the collection widgets, just under the cells One UI reports (311, 326, 418). Content is centered when a cell is taller than its bucket, and a lone row with fewer tiles than columns is centered.
- `WidgetLayoutTest` checks, for every bucket and font scale from 1.0 to 2.0, that what a layout needs fits the bucket, and every provider size has a bucket that fits.

**Landscape.** `AlbumField` measures the long side over the short (`max(size) / min(size)`), so portrait is unchanged and landscape keeps the colors at the same relative places. The brush fallback below Android 13 draws in a portrait-shaped virtual canvas. `AlbumFieldAspectTest` renders both shapes and compares five points.

**Redraw per instance, and keep the classes apart.** The app redraws each placed widget through its own receiver (`WidgetRefresh`), not one `updateAll` per class. On the S20 FE, R8 had merged the three widget classes in the release build (the log named all three the same), so Glance's per-class lookup handed Now Playing's redraw the other widgets' ids and drew its content in them. `proguard-rules.pro` now keeps every `GlanceAppWidget` and `GlanceAppWidgetReceiver` subclass, and `WidgetKeepRulesTest` fails if the rule goes.

**Play with nothing queued** starts the last song played (`AutoLibrary.onPlaybackResumption`) instead of starting the service and stopping it again.

**A debug-only readout** of the sizes Android reports for each placed widget, with the bucket each would draw, sits on Settings > Lock screen and widgets (`WidgetSizeLog`, shown only when the app is debuggable).

## Alternatives considered
- **Keep the wallpaper colors.** Simple, but a widget that doesn't look like what is playing is the thing the owner called basic.
- **Keep the progress bar and redraw often.** Launchers limit widget updates; a bar that jumps every few seconds looks broken.
- **One stretched bitmap for every size.** The first version; it smeared the blobs on wide and tall tiles.
- **A palette made in the widget alone.** It would differ from the app's. It stays only as the stand-in until the core's palette exists.
- **Choosing the layout by size alone, with scrolling or clipping content.** That was the cut-off button.
- **Merging widgets' classes as R8 likes.** It broke the release build only, which no emulator test sees; hence the keep rule and its test.

## Consequences
- **Easier:** a widget looks like the album and the app; every reachable size is drawn whole and the test says so; adding a size means adding a bucket the tests then check.
- **Harder:** more files (`WidgetSizing`, `WidgetField`, `CoverPalette`, `WidgetChrome`, `WidgetRefresh`); the snapshot is bigger by the palettes; the palette fetch is the writer's only new network use.
- **To undo:** drop the palettes from the snapshot, `FieldMath` use in `widgets/`, and go back to the Phase 9 layouts (git history before commit 2fd4211).

## Not verified
- Landscape on a phone, resizing on the launcher, the lock-screen widget, Mixes tile taps, and larger font scales on a phone.
- One UI's rounded-corner clipping of the field, AMOLED colors, Samsung's font at large sizes.
- The R8 keep rule on a phone (the S20 FE has a build without it that works through the per-instance redraw).
- The A13 and the Tab A7 Lite on screen.
