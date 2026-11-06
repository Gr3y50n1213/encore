# Board 32: Lock screen and Live Activities

PNG: `design/wireframes/png/32-lock-screen-and-live-activities.png`  
HTML: `design/wireframes/html/Live-Lock.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Lock screen and Live Activities

Music already has a player on every lock screen, drawn by the system from the Now Playing data, so Encore feeds it and does not draw a second one. Live Activities and Live Updates are for timed tasks the person starts: the sleep timer, a download and the first sync. Everything is updated by the app on the device. Dashed outlines mark what the system draws.

iPhone · Lock Screen with music

Encore100%

Thu 5 NovEvening Drive

9:41

Slow Tide Ember Cove

Drawn by iOS from Now Playing

Slow Tide

Ember Cove

1:24-2:18

1. 2

iPhone · Lock Screen with timed tasks

Encore100%

Thu 5 Nov

9:41

Sleep timer Ends 10:05 PM

24:10

+15 min

Drawn by iOS (continued task)

Downloading Road trip

15 of 24 songs

Slow Tide

Ember Cove

3. 5

Dynamic Island · sleep timer and downloads

Compact

24:10

Moon, then time. Tap opens the sleep timer sheet.

Minimal · two activities

Moon and the download ring. Touch and hold expands.

Narrow island (landscape)

One 28 pt circle, so only the moon.

Expanded · sleep timer

Sleep timer Ends 10:05 PM

24:10

+15 min

4. Title and end time beside the camera, countdown and one button below.

Expanded · download (drawn by iOS)

Downloading Road trip
15 of 24 songs

StandBy · Lock Screen view scaled 2×, and Night Mode

Sleep timer Ends 10:05 PM

24:10

Sleep timer

24:10

Minimal view first, a tap scales the Lock Screen view. Night Mode tints it red, so the text and the bar carry the meaning, not the indigo.

The same Live Activity on other Apple devices

iPad · Lock Screen (425 pt wide)

Sleep timer Ends 10:05 PM

24:10

+15 min

6. Same Lock Screen view. Live Activities run on iPad too.

Mac · menu bar (from the iPhone)

24:10
Thu 5 Nov  9:41

Sleep timer Ends 10:05 PM

24:10

Compact, minimal and expanded. A click opens iPhone Mirroring.

CarPlay Dashboard

Sleep timer

24:10

240 by 78 pt, controls switched off.

Apple Watch · Smart Stack

Sleep timer

24:10

Compact view, merged. No watch app needed.

Android · Lock screen

9:4124m100%

9:41

Thursday, 5 Nov

Slow Tide

Ember Cove

Drawn by Android from the media session

Slow Tide

Ember Cove · Encore

Encore · Sleep timer24:10

Music fades out at 10:05 PM

7. 8

9. Android · Notification shade

9:4162%100%

Start a mix

Cast

Slow Tide

Ember Cove

Encore · Downloads62%

Downloading Road trip

15 of 24 songs

Cancel

Encore · Sleep timer24:10

Music fades out at 10:05 PM

Add 15 min

9. Status bar chip

9:4124mLTE ▲ 100%

9:4162%LTE ▲ 100%

9:41LTE ▲ 100%

96 dp at most. Under 7 characters the whole text shows. If less than half fits, only the icon shows. A time 2 minutes or more ahead counts down on its own.

Live Update, expanded

Encore · Sleep timer24:10

Music fades out at 10:05 PM

Add 15 min

Progress style with a tracker icon. Pinned above other notifications, on the lock screen and in a status bar chip. The person can demote it.

Before Android 16 QPR1, or Live Updates off

Encore · Sleep timer24:10

Music fades out at 10:05 PM

Add 15 min

Same text and action, as a normal ongoing notification in the low-importance channel.

Live Update, download

Encore · Downloads62%

Downloading Road trip

15 of 24 songs

Cancel

Only for a download the person started, as a user-initiated transfer. Automatic downloads use a quiet progress notification.

What is never promoted

Playback (the media style cannot be promoted), Recap ready, new mixes, sync finished, anything the person did not start. Each of those is a normal notification, if allowed at all.

1. Player on the lock screen. Encore does not draw it. iOS builds the Lock Screen, Dynamic Island, StandBy, Control Center and CarPlay player from the Now Playing data: title, artist, album, artwork, duration, position, rate and the enabled commands (play, pause, next, previous, seek, like). The core updates it when the track or the state changes, not every second; the system keeps the clock moving, so nothing runs while the screen is locked.

2. Lock Screen widgets and controls. Circular, rectangular and inline widgets, drawn on the next board, and the two buttons at the bottom, which are Controls (Sleep timer and Like). They read the local snapshot and update when the song changes. The defaults are controls and the pinned mix name; nothing from the listening history shows while the phone is locked. A play or pause tap works without unlocking; opening the app asks for Face ID as usual.

3. Sleep timer Live Activity. Starts only when the person picks a time. Shows the time left, which counts down by itself with no updates, and one button, +15 min. Ends when the fade-out finishes and the music pauses, and is removed at once. A tap anywhere else opens the sleep timer sheet. Swiping it away ends the Live Activity only; the timer keeps running and can be changed or cancelled in the app. An accidental tap adds 15 minutes, which the countdown shows straight away.

4. Dynamic Island. Compact: moon on the left, time on the right, snug to the camera and balanced. Minimal: the moon alone when another activity runs; the system shows up to three. Expanded (touch and hold): title and end time on the top row beside the camera, then the countdown and the button. Tap opens the sleep timer sheet. On a narrow island both views shrink to one 28 pt circle. The key line takes the indigo tint. Animations last under two seconds and stop on Always-On and under Reduce Motion.

5. Downloads and first sync. On iOS 26 and iPadOS 26 a download or first sync that the person starts runs as a continued processing task, and the system draws its own Live Activity from a title, a subtitle and progress, with Cancel. Encore supplies Downloading Road trip and 15 of 24 songs, and reports real progress, because the system stops tasks that show little. It ends with Road trip is ready and stays 15 minutes. Automatic downloads and background syncs show nothing. Earlier iOS versions use a background session and one local notification at the end, if allowed.

6. Same activity elsewhere. Nothing extra to build. The system reuses the compact view on the Mac menu bar (a click opens iPhone Mirroring), on the CarPlay Dashboard (240 by 78 pt, with controls switched off) and in the Apple Watch Smart Stack. iPad shows the Lock Screen view at 425 pt wide. In StandBy the minimal view shows and a tap scales the Lock Screen view 2×. Night Mode tints everything red, so a symbol and text carry the meaning, never the color alone.

7. Android lock screen widget. Phones from Android 16 QPR1 and the Pixel Tablet can show widgets on the lock screen. Only the Now Playing widget is allowed there, with play, pause and next, and no history. Widgets that show history declare not_keyguard. A widget that would open the app asks the person to unlock first, because the main activity is not marked showWhenLocked.

8. Android player. Media3 builds the media session, and Android draws the lock screen player, the quick settings player and the notification from it, with the same controls everywhere. Metadata and artwork are set on track change only. The player is not a Live Update: the media style cannot be promoted, and a second card for the same song would just crowd the shade.

9. Live Update. The sleep timer and downloads the person started post one: an ongoing notification with a title, no custom view, not colorized, progress style, a short chip text such as 24m and the permission POST_PROMOTED_NOTIFICATIONS. If the person turned Live Updates off, or the phone is older than Android 16 QPR1, it is a normal ongoing notification with the same words. A dismissed one is not posted again, and it ends when the task ends.

10. Nothing leaves the device. A Live Activity or Live Update is started and updated only by the app, on the device, while it is running for another reason: playing audio or a task the system granted. No push token is created or sent, so there is no push-to-start and no update from another device. A stale date lets the system grey out an activity that stopped updating. What a passer-by sees on a locked phone: that a timer or download runs, and the song the system player shows anyway. Nothing from the listening history.

Drawn here. iPhone, iPad, Mac, CarPlay and Apple Watch for iOS, and Android phones. Windows and Linux have no equivalent, so their timed-task and lock screen surfaces are on the last board. Not drawn. A watchOS app, Wear OS and Android Auto layouts; the Apple Watch and CarPlay views above come from the iPhone Live Activity.
