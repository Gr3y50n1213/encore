# Board 31: Live surfaces and rules

PNG: `design/wireframes/png/31-live-surfaces-and-rules.png`  
HTML: `design/wireframes/html/Live-Rules.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Live surfaces and rules

The lock screen, Live Activities and widgets in one place: what each platform gets, the four rules behind it, the settings page and the sleep timer, which is the one Live Activity Encore starts itself.

What each platform gets

1. | Surface | iPhone | iPad | Mac | Android | Windows | Linux

| Player on the lock screen, drawn by the system | Lock Screen, Dynamic Island, StandBy, Control Center | Lock Screen, Control Center | Control Center, menu bar | Lock screen, quick settings, shade | Lock screen, volume flyout | Lock screen and shell media controls

| Timed tasks: sleep timer, user-started downloads, first sync | Live Activity | Live Activity on the Lock Screen | App window, Dock progress, menu bar item | Live Update (Android 16 QPR1 and later), else an ongoing notification | Taskbar progress, progress notification | Notification and launcher progress where the desktop shows them

| Home widgets | Home Screen, Today View, StandBy | Home Screen, Today View | Desktop, Notification Center | Home screen (Glance) | Widgets board, Store build | Not in version 1

| Lock screen widgets | Circular, rectangular, inline | Circular, rectangular, inline, small | Not available | Phones from Android 16 QPR1, Pixel Tablet | Not available to apps | Not available

| Quick controls | Control Center, Lock Screen, Action button | Control Center | Control Center (macOS 26) | Quick Settings tile | Jump List, tray menu | Tray menu, desktop actions

| Remote updates (push) | None | None | None | None | None | None

Comes with no extra work. An iPhone Live Activity also appears in the Mac menu bar, on the CarPlay Dashboard and in the Apple Watch Smart Stack, and a small widget works in StandBy and CarPlay.

Four rules that hold everywhere

2. 1. The system draws playback Encore feeds the Now Playing data. It never draws a second player on the lock screen.

2. A Live Activity is for a timed task The sleep timer, a download or the first sync, and only when the person started it.

3. A widget shows what the app saved A small snapshot per profile, on the device. No database, no network.

4. Nothing needs a push service No push token leaves the device. The running app updates everything.

iPhone · Settings

Encore100%

Lock screen and widgets

Timers and downloads

Sleep timer on the Lock Screen

Time left and a +15 min button

Download progress on the Lock Screen

Only downloads you start

Live Activities also need to be allowed for Encore in the Settings app.

Privacy

Hide listening history while locked

Recently played and Recap stay hidden until you unlock

Widgets

Widget profile

Active profile

Each widget can use its own profile. Touch and hold a widget, then choose Edit Widget.

3. 6

7. Android · Settings

9:41100%

Lock screen and widgets

Live updates are off for Encore

Timers and downloads use a normal notification instead.

Open settings

8. Sleep timer Live Update

Time left and Add 15 min

Download progress

Only downloads you start

Hide listening history while locked

Recently played and Recap stay hidden until you unlock

Widget profile

Active profile

Quick Settings tile

Start a mix

Add

3. 6

7. iPhone · Sleep timer sheet

Encore100%

Sleep timer

Music fades out over the last 10 seconds.

5 min

10 min

15 min

30 min
9. 45 min

60 min

End of this song

End of this album

Start for 30 minutes

Music pauses at 10:11 PM

4. Android · Sleep timer, running

9:41100%

Sleep timer

Music fades out at 10:05 PM

24:10

Add 15 min

Cancel timer

Change time

15. 30

45. 60

End of song

End of album

4. Sleep timer states

5. Idle No card on the lock screen.

↓ pick a time

Running Live Activity or Live Update shows the time left. +15 min adds time and the bar shows it at once.

↓ last 10 seconds

Fading Volume drops to silence over 10 seconds, or less if the song ends sooner.

↓ pause

Ended Music paused. The card is removed at once. Nothing is posted afterward.

10. Test on real hardware Always-On, Low Power Mode, Focus, StandBy at night, CarPlay. Android 15 and 16, Live Updates off, a tablet. Windows packaged and unpackaged. GNOME and Plasma on Wayland.

1. What each platform gets. Every platform gets the surfaces it really has and nothing is faked where the system has none. Mac has no lock screen widgets and Linux has no widgets in version 1. Windows has no lock screen widgets for any app. Where a platform has a timed-task surface, the sleep timer, a download the person started and the first sync use it; everything else is a normal notification at most, and often not even that.

2. The four rules. Playback is drawn by the system from the Now Playing data, so there is never a second player. A Live Activity or Live Update starts only from something the person did. A widget reads only the small snapshot the app saved for that profile on the device. Nothing uses a push service, because a self-hosted server has no push key and listening state would otherwise pass through a third party.

3. Settings page. Settings › Lock screen and widgets has four rows: Sleep timer on the Lock Screen (a Live Update on Android), Download progress on the Lock Screen, Hide listening history while locked (on) and Widget profile (Active profile). Turning a card off never stops the timer or the download, it only removes the card. If the system has Live Activities or Live updates off for Encore, a note says so and, on Android, a button opens the system page. The page plays no custom haptics.

4. Sleep timer sheet. Opened from More on Now Playing, from Settings › Playback and quality, from the Sleep timer Control and from a tap on the Live Activity. Presets are 5, 10, 15, 30, 45 and 60 minutes, End of this song and End of this album; the end time is shown under the Start button before anything starts. While it runs the sheet shows the time left, Add 15 min, Cancel timer and Change time. An accidental tap on the scrim or a swipe down closes the sheet and leaves the timer running. Cancel timer shows Timer cancelled with Undo for five seconds. End of song and End of album let the song play to its end and then pause, with no fade.

5. States and the clock. The end time is stored with the profile's player state, so the timer survives the app being restarted. At the end the music pauses, the Live Activity ends at once (the Dynamic Island removes it immediately; the Lock Screen may keep it up to four hours, so Encore ends it itself) and a finished download stays 15 minutes. Without push, the Live Activity gets a stale date equal to the end time. If the app was killed, the system greys the card and the next launch removes it; on Android the notification has a timeout at the end time.

6. Privacy. A passer-by sees only what the system player already shows on a locked phone, plus that a timer or download is running. Personal text in widgets is marked privacy sensitive on Apple devices, and widgets that show history are kept off the Android lock screen. Hide listening history while locked is on by default. A profile with a PIN, and the guest profile, show the neutral Open Encore card. No Live Activity shows an advertisement or a promotion.

7. Profiles. Each widget has a Profile setting, Active profile by default. A widget set to one profile keeps showing that profile after the switcher changes; a deleted profile's widgets say Profile removed. The sleep timer card names no profile and shows no history, so switching profile needs no change to it.

8. When posting is off. If Live Activities are off in iOS Settings, Encore shows the timer only in the app and in the system player; it never asks twice in a session. On Android, when the system Live updates setting is off or the phone is older than Android 16 QPR1, the same words appear in a normal ongoing notification in a low-importance channel, and the banner on this page links to the system setting. Unwanted Live Updates make people revoke the permission, so none starts without something the person did.

9. Haptics, motion and accessibility. Widgets and Live Activities run outside the app, so the system's feedback applies, and the app plays its usual haptic only when it handles the action in the foreground: a selection tick on a preset, a light impact on Start, nothing on Cancel. VoiceOver, TalkBack and Narrator read Sleep timer, 24 minutes left, and the button Add 15 minutes. Text follows Dynamic Type and font scale until the layout would clip, then drops a line. Animations last at most two seconds and stop under Reduce Motion and on Always-On.

10. To test on real hardware. The simulator and emulator do not show Always-On, Low Power Mode, Focus filtering, StandBy at night or CarPlay. Check: the activity ends on time, a +15 min tap adds time at once, a long download that the system pauses, an Android 15 phone, an Android 16 phone with Live Updates off, a tablet, Windows packaged and unpackaged builds, light and dark everywhere, and GNOME and Plasma on Wayland. Not checked in this pass: whether Google Play treats a download-progress Live Update as eligible.

How to read the boards. The next three boards draw each surface: Lock screen and Live Activities, Widgets and controls on Apple devices, and Widgets and media surfaces on Android, Windows and Linux. Dashed outlines mark what the system draws. Version 1 has no Apple Watch app, no Wear OS app and no Android Auto layout; the Watch and CarPlay views come from the iPhone Live Activity.
