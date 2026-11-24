# Encore TV (projector and Android TV)

The owner asked on 23 November 2026 for Encore on their projector (Android TV OS 13, 1 GB of
memory, 8 GB of storage), installed from an APK, driven by a TV remote, with a Sleep mode.
Mock-up: `design/wireframes/html/45-projector-mockup.html` (approved the same day).

## How it is built

- One code base, two product flavors (dimension `device`): `phone` (the default, unchanged)
  and `tv` (application ID suffix `.tv`, named "Encore TV"), so both can sit side by side.
- `src/tv/` adds only a manifest (the `LEANBACK_LAUNCHER` entry, the banner, leanback and
  touchscreen not required) and the 320 by 180 dp banner. Everything else is `src/main`.
- Code that differs on a TV asks `ui/tv/Tv.kt`: `isTv(context)` (the TV flavor, or any device
  Android calls a television) and `LocalTv` in Compose.
- Builds: `make check` builds `assembleTvRelease` beside the phone app; CI uploads
  `tv/release/app-tv-release.apk` in `encore-android-apks`, signed with the test key.

## What changes on a TV

- **Layout:** the wide layout with the rail, inside a 40 by 20 dp margin (projector overscan).
  Now Playing puts the cover and controls left and the lyrics (or the queue) right.
- **Remote:** Material 3's inset focus ring on everything that ripples (`TvFocus`), and on the
  two tiles that otherwise draw no ripple (album tiles, Made for you). Holding OK opens a song's
  menu (combinedClickable's key long press); the Menu key does too.
- **1 GB of memory:** the calm path everywhere (`reduceMotion()` is true: a still album field,
  cross-fades instead of the player morph); cover art in memory is an eighth of the app's heap
  (also on phones, at most 48 MB); the rolling playback cache at most 300 MB.
- **Sound analysis:** never on a TV (jobs cancelled, card, settings row and the mood prompt
  hidden). Mood tiles show only if the profile already has them.
- **Crashes:** a projector can't be reached for logs, so a crash is written to
  `files/last-crash.txt` and shown once at the next launch.

## Sleep mode (phones and TV)

Now Playing > More > Sleep mode, and a button under the controls on TV. Full screen and black,
the sung line large and centred in dim warm grey with the lines before and after faint, using
the lyrics panel's own timing (`lyricIndex`). A song without synced lyrics shows its title and
artist. A key press or a tap shows the controls for 5 s (TV adds the sleep timer and Exit);
Back or a swipe down leaves. The window asks for 8 % brightness and keeps the screen on only
while music plays.

## Not done yet

- No Android TV emulator in CI yet; the TV build is compiled and packaged, not run, in CI.
- Moods on a TV need the analysis a phone does to reach the TV's profile.
- The TV APK carries all four ABIs (about 56 MB); arm only would roughly halve it.
