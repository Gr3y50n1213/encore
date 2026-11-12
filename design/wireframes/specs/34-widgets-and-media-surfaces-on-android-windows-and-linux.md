# Board 34: Widgets and media surfaces on Android, Windows and Linux

PNG: `design/wireframes/png/34-widgets-and-media-surfaces-on-android-windows-and-linux.png`  
HTML: `design/wireframes/html/Widgets-Other.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Widgets and media surfaces on Android, Windows and Linux

Android gets the same four widgets as Apple devices, in Material 3 Expressive. Windows 11 gets Adaptive Card widgets, a taskbar that shows progress, and system media controls. Linux gets MPRIS, notifications and dock actions, and no desktop widgets in version 1. Everything reads a local snapshot or the media session, and nothing is pushed. Dashed outlines mark what the system draws.

Android · Home screen widgets · Jetpack Glance, Material 3 Expressive

1. 9:41100%

Slow Tide

Ember Cove

Your mixes

Evening Drive

Rainy Focus

Drive Home

Slow Morning

Slow Tide

Ember Cove

Your 2026 so far

Top artist

Moth Choir

4,210 minutes

Search

2. 3

3. Slow Tide

Ember Cove

2. Now Playing 4×2, dark. ~~Colors follow the wallpaper (dynamic color), not the album; the cover keeps its own.~~ Changed 12 November 2026: the widget wears the playing album's colors (ADR android-0006); the drawing above is the old look.

Recently played

Golden Hour

Night Market

Salt and Signal

Terminal Bloom

Tidewater

Brass Arches

Glass Bell

Low Tide

3. Recently played 4×3. Tap a cover to open the album. History, so never on the lock screen.

Slow Tide

Ember Cove

Resized to 4×1. Cover, title and one button.

2×1 keeps the cover and the button. Other wallpapers, other colors.

Windows 11 · Widgets board · Adaptive Cards drawn by Windows

5. Encore

Slow Tide

Ember Cove

Pause

Now Playing

Nothing yet

Mixes build as you listen

Play a mix

Your mixes

Evening Drive

Rainy Focus

Drive Home

Recently played

Golden Hour

Night Market

Salt and Signal

Terminal Bloom

Tidewater

Brass Arches

Continue

Slow Tide

Customize widget

Mixes

Profile

Active profile

Alex

Guest

Mix

Next up⌄

Save

Cancel

10. Encore

Slow Tide

Ember Cove

Taskbar button. Green progress for a download the person started; a moon badge while the sleep timer runs. Hover shows the thumbnail with three buttons.

6. Android · Quick Settings, lock screen and states

Quick Settings tile

Start a mix
Evening Drive

Start a mix
Playing

Start a mix
Unlock Encore

Inactive, active, unavailable. Tap starts the pinned mix; touch and hold opens the mix in the app.

4. Lock screen widget

9:41

Thursday, 5 Nov

Slow Tide

Ember Cove

Now Playing only, on phones from Android 16 QPR1 and the Pixel Tablet. Play, pause and next work locked; the cover opens the app after unlock.

4. Widget states

Nothing yet

Mixes build as you listen

Play

Saved mixes

Server not reachable

Open Encore

Profile is locked

10. Profile removed

Choose a profile

Tablets and foldables · wider cells show more

1. Slow Tide

Ember Cove · Golden Hour Static

1:243:42

Your 2026 so far

Top artist

Moth Choir

4,210 minutes · 612 songs

1 Salt and Signal

2 Slow Tide

Windows · system surfaces

Jump List

Mixes

Evening Drive

Rainy Focus

Drive Home

Tasks

Play or pause

Next song

Start a mix

6. Mixes and tasks only. The system Recent list is off for Encore, so it holds no listening history.

Media flyout

Slow Tide

Ember Cove

Encore

1:243:42

7. Drawn by Windows from the media session.

Progress toast

EncoreNow

Downloading Road trip

15 of 24 songs

Cancel

6. Downloads the person started. Updated in place, no sound.

Windows lock screen

9:41

Thursday, 5 November

Slow Tide

Ember Cove

7. System media controls only. No third-party widgets there.

Linux · GNOME, KDE Plasma and the freedesktop standards

GNOME · Quick Settings and lock screen

Thu 5 Nov 9:41

Slow Tide

Ember Cove

Wi-Fi

Bluetooth

8. Drawn by GNOME from the MPRIS data: title, artist, art and the buttons that are enabled.

KDE Plasma · media controller

Slow Tide

Ember Cove

Golden Hour Static

1:243:42

8. Drawn by Plasma, in the tray and on the lock screen, from the same data. Plasma adds its own accent.

Notification with progress

Encorenow

Downloading Road trip

15 of 24 songs

Cancel

Progress shows where the desktop supports it (KDE Plasma). GNOME shows the words only, so the text carries the meaning.

9. Downloads the person started. One notification, updated in place.

Dock, panel and tray menu

1. Play or pause

Next song

Start a mix

Quit

Dock or panel. Progress bar and count where supported, and Desktop Actions in the right-click menu.

9. Desktop Actions work in every desktop; a tray icon is optional.

1. Android widget set. The same four widgets as on Apple devices, built with Jetpack Glance and Material 3 Expressive: Now Playing, Mixes, Recently played and Recap. Each is resizable between its minimum and maximum cell sizes, from 2×1 up to the sizes shown. The widget background uses the system corner radius, and inner shapes use the system inner radius, so it matches the launcher. Colors follow the album (changed 12 November 2026 from the wallpaper's dynamic color; ADR android-0006): the playing album's field behind, text and buttons in its own text color, with the theme's colors only until an album's palette exists. Every size the launcher can give a widget is drawn whole and sizes that would leave one cover in empty space cannot be placed. Previews in the picker are generated from the demo library, never the person's own history.

2. Now Playing (Android). Cover, title, artist, previous, play or pause, and next. The buttons go to the playback service as media session actions and never open the app. There is no progress bar (changed 12 November 2026): a widget cannot animate and the system limits how often it may update, so a bar could only be wrong. The playing state is shown by the play button's shape (round paused, rounded square playing) and three level bars shown only while playing. Tapping the cover or text opens Now Playing.

3. Mixes, Recently played and Recap. A tap on a mix tile starts it in the background through the playback service; a tap on a recent cover opens the album page. The widget is configured when it is placed: Profile, and for Mixes a pinned mix or Next up. Recap shows the top artist and minutes so far and says Your recap builds as you listen until there is enough data. Touch targets are at least 48 dp; the play button sits inside the tile so a stray tap near the edge opens the page instead of playing.

4. Quick Settings tile and lock screen. The Start a mix tile has three states: inactive (the mix name), active (Playing) and unavailable (Unlock Encore, when the profile has a PIN and the phone is locked). Tap starts the pinned mix; touch and hold opens the mix in the app. On phones from Android 16 QPR1 and on the Pixel Tablet, Now Playing is the only Encore widget offered on the lock screen; the widgets that show history declare not_keyguard so they cannot be placed there. States are always words, with no error icon.

5. Windows Widgets board. Windows 11 only. A packaged build (MSIX, or with package identity) registers a small widget provider, a Rust component beside the Flutter app, that runs out of process and is started by Windows when a widget is added; an unpackaged portable build has no widgets. Widgets are Adaptive Cards in small, medium and large, drawn in the Windows style with Windows buttons and colors, so Encore supplies the data and the layout and not the pixels. Customize widget (from the ⋯ menu) holds Profile and Mix, the same two settings as everywhere else. A new widget with no history shows Nothing yet and one button. There is no Recap widget on Windows. Windows 10 has no widgets board.

6. Windows taskbar, Jump List and toast. The taskbar button shows green progress for a download the person started and a moon badge while the sleep timer runs; both are removed when the task ends. Hovering shows the thumbnail with previous, play or pause and next. The Jump List has only Mixes and Tasks, and the system Recent list is turned off for Encore so it never records what was played. A progress toast covers downloads and the first sync; it updates in place, makes no sound and has Cancel. A small native Windows plugin connects these to the Flutter app.

7. Windows media controls. The system media flyout, the quick settings media card, the lock screen and headset keys all come from the System Media Transport Controls. The app sets the title, artist, album, cover, position and the enabled buttons when the track or state changes and the system does the rest, so nothing runs while the screen is locked. Windows shows no third-party widgets on the lock screen, so this is the whole lock screen story.

8. Linux media controls. Encore publishes MPRIS 2 on the session bus, from the Rust core, with title, artist, album, artwork, length, position and the enabled commands. GNOME draws it in Quick Settings and on the lock screen, Plasma in the tray media controller and on the lock screen, and headset and keyboard keys use it as well. Artwork is written to a private cache file, replaced on every track and removed on quit, because the shell reads it by path.

9. Linux notifications, dock and tray. A download the person started gets one freedesktop notification with a progress value, updated in place; desktops that ignore the value show the words only. The launcher entry reports progress and a count, shown by Plasma, Ubuntu's dock and Dash to Dock where they support it. The .desktop file has Desktop Actions (Play or pause, Next song, Start a mix) so every desktop gets them in the right-click menu. A tray icon through StatusNotifier is optional, because GNOME needs an extension for it, and nothing depends on it. When it is there, the sleep timer shows as its tooltip. There are no desktop widgets in version 1: Plasma plasmoids and GNOME extensions are not stable enough to promise.

10. Data, privacy and refresh. On Windows the core writes the same per-profile snapshot to the app's local data folder, and the widget provider reads that file and nothing else: never the database, never the network. On Android it is a Glance state the core fills. Each widget has a Profile setting, Active profile by default; a profile with a PIN and the guest profile show the neutral Open Encore card. Updates are made by the running app, never by push: no FCM token, no WNS channel, no server-side widget refresh. A widget that has not updated for 24 hours dims and says Open Encore to refresh.

Not drawn. Windows 10 and Windows live tiles (removed by Microsoft), KDE Plasma plasmoids, GNOME extensions, ChromeOS and Wear OS. To verify while building. The Windows widget provider from a Flutter app (it needs package identity), the Android 16 lock screen widget category on a real phone, and whether the Linux dock shows the launcher entry progress on the target desktops.
