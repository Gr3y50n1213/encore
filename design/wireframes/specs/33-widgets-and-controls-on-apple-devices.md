# Board 33: Widgets and controls on Apple devices

PNG: `design/wireframes/png/33-widgets-and-controls-on-apple-devices.png`  
HTML: `design/wireframes/html/Widgets-Apple.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Widgets and controls on Apple devices

Four widgets and four controls, the same on iPhone, iPad and Mac. They read a small local snapshot that the app writes, never the network, so they are private, fast and work offline. Anything that shows listening history is redacted while the device is locked. Sizes are in points and drawn at 1×.

System families · iPhone, iPad, Mac

Sizes in pt, shown at 1×

1. Slow Tide

Ember Cove

2. Mix

Drive Home

Upbeat, familiar

3. Your 2026 so far

Top artist

Moth Choir

4,210 minutes

4. Slow Tide

Ember Cove · Golden Hour Static

1:24-2:18

10. 2

Your mixes

Evening Drive

Rainy Focus

Drive Home

3. Recently played

Golden Hour

Night Market

Salt and Signal

Terminal Bloom

4. Your 2026 so far

Top artist

Moth Choir

4,210 minutes · 612 songs

1 Salt and Signal

2 Slow Tide

4. Your mixes

Evening Drive

Warm and unhurried · 28 songs

Rainy Focus

Soft, steady · 34 songs

Drive Home

Upbeat, familiar · 25 songs

Slow Morning

Gentle start · 22 songs

Late Night

Quiet and low · 30 songs

3. Recently played

Golden Hour

Night Market

Salt and Signal

Terminal Bloom

Tidewater

Brass Arches

Glass Bell

Low Tide

Continue

Slow Tide

4. Your mixes

Evening Drive

Warm and unhurried

Rainy Focus

Soft, steady

Drive Home

Upbeat, familiar

Slow Morning

Gentle start

Late Night

Quiet and low

Recently played

Golden Hour

Night Market

Salt and Signal

Terminal Bloom

Tidewater

Brass Arches

5. Lock Screen · accessory widgets · iPhone and iPad

1. Evening Drive · Slow Tide

Inline · 257×26

Circular · 76×76

Slow Tide

Ember Cove

Rectangular · 172×76 · now playing

Evening Drive

Tap to play

Rectangular · pinned mix

7. Rendering modes · the same Now Playing widget

Slow Tide

Ember Cove

Full color
Album color behind white text

Slow Tide

Ember Cove

Accented
Tint chosen by the person; art desaturated

Slow Tide

Ember Cove

Clear
Liquid Glass; only the art keeps color

Slow Tide

Ember Cove

Vibrant
Lock Screen, in grays

Controls · Control Center, Lock Screen, Action button

8. Play or pause
Playing

Like current song
Liked

Start a mix
Evening Drive

Sleep timer
30 minutes

Sleep timer Hold to start

Action button hint, always a verb

States

9. 9

Nothing yet

Mixes build as you listen

Play a mix

Empty
First launch

9. Saved mixes

Server not reachable. Showing saved mixes.

No server
Saved copy, said plainly

6. Open Encore

Profile is locked

Locked profile
Neutral card, no history

6. Recently played

Redacted
Shown while the phone is locked

StandBy and CarPlay · two small widgets, background removed, scaled up

7. Slow Tide

Ember Cove

Rainy Focus

Tap to play

1. Families. Small works on the iPhone Home Screen, Today View, StandBy and CarPlay, the iPad Home Screen, Today View and Lock Screen, and the Mac desktop and Notification Center. Medium and large work on iPhone, iPad and Mac; extra large on iPad and Mac. Accessory circular, rectangular and inline work on the iPhone and iPad Lock Screen. Not every widget comes in every size: a size is offered only when it shows more, never the same content larger.

2. Now Playing. Cover, title, artist, a progress bar and the buttons. The bar is a timer date the system draws, so it moves with no reloads. Play or pause, previous and next are buttons that run an app intent; the rest of the widget opens Now Playing. With nothing playing it shows the last song and a Play button. Reload when the song changes: reloads do not count against the budget while Encore is the Now Playing app. Starting playback from a button without opening the app depends on an audio playback intent (to verify on iOS 18 and later).

3. Mixes. One tap on a tile plays that mix, in the background where the system allows it, otherwise the app opens. The widget has two settings, set when it is added: Profile, and Mix (a chosen mix or Next up, which follows the time of day). Touch and hold is the system Edit Widget menu. Tiles are at least 44 pt, with the play button inside each tile, so a stray tap near the edge opens the mix page instead of starting music.

4. Recently played and Recap. Recently played shows albums from the listening history, so it is private: its text and covers are redacted while the phone is locked, and it never appears as a lock screen widget. A tap opens the album. Recap shows the top artist and minutes so far, in the style of the year's design pack, and says Your recap builds as you listen until there is enough data. It reads only the local snapshot.

5. Data and refresh. The core writes a small per-profile snapshot to the App Group container: now playing, pinned mixes with thumbnails, recent albums, the recap headline. The widget reads that file, never the database, and never makes a network request. It gets about 40 to 70 reloads a day when viewed often, with timeline entries at least 5 minutes apart; it asks for relevance so Evening Drive rises in the Smart Stack in the evening. A snapshot older than 24 hours dims the widget and says Open Encore to refresh.

6. Profiles and privacy. Each widget has a Profile setting, Active profile by default. A profile with a PIN, and the guest profile, show the neutral Open Encore card with no history. Personal text is marked privacy sensitive, so the system redacts it on the Lock Screen and in StandBy while the device is locked. Settings has Hide listening history while locked, on by default. Anyone can see a widget, so the defaults assume they will.

7. Appearance. Full color, accented and clear (Liquid Glass) on the Home Screen, vibrant on the Lock Screen. In the accented and clear looks the system desaturates images; only the album cover keeps its color. Content is grouped into accented and primary parts, corners are concentric with the widget, contrast is at least 4.5 to 1, and the Always-On display uses grays that stay legible. In StandBy and CarPlay the background is removed, text is larger, and Night Mode turns it red.

8. Controls. Play or pause and Like current song are toggles that show their state; Start a mix and Sleep timer are buttons that ask which mix or how long when they are added. Sleep timer starts the Live Activity. In Control Center the control shows its symbol, title and value; on the Lock Screen only the symbol; on the Action button a hint in the Dynamic Island that starts with a verb, such as Hold to start. A locked phone may play, pause and like; Start a mix on a PIN profile asks the person to unlock.

9. States. Empty says what will fill it and offers one action. No server shows the saved copy and says so, with no error icon. A locked profile shows the neutral card. A removed profile says Profile removed. While loading, a widget shows gray shapes in the layout, never a spinner. Every state keeps a tap target that opens the app at the right place.

10. Accessibility, motion and haptics. Each widget has one spoken summary, such as Slow Tide by Ember Cove, playing, and its buttons keep their own labels. Text scales with Dynamic Type until the layout would clip, then drops a line. Animations last at most two seconds and stop under Reduce Motion. Widgets and controls play no custom haptics; the system gives its own feedback on a button, and the app plays its usual haptic only when the action is handled in the foreground.

Gallery. One entry per widget, sizes grouped under one description that starts with a verb: Now Playing, See what is playing and control it. Mixes, Play a mix in one tap. Recently played, Pick up an album you were listening to. Recap, See your year in music as it builds. Previews use realistic sample data from the demo library. Not drawn. Apple Watch complications and visionOS widgets; the app has no Watch or Vision version in this plan.
