# Board 38: Smart picks: screens and states

PNG: `design/wireframes/png/38-smart-picks-screens-and-states.png`  
HTML: `design/wireframes/html/Smart-Screens.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Smart picks: screens and states

The controls that let people see why a song was chosen, correct it, tune how adventurous the app is, switch learning off and start over. They sit on the smart-picks rules board; every one is reversible and everything stays on the user's own devices.

Screens

Why this song

Golden Hour Static · Ember Cove

You finished Night Market 9 of the last 10 times, and this sounds like it.

You often play songs like this around 10 pm.

Sounds like

Night MarketSalt and Signal

More like thisFewer like this

Not interested…

Based only on what you played on this device and your own server.

Why this song
iOS. From More, a queue row or a mix tile.

Queue

Autoplay

Golden Hour Static

Ember Cove

Picked for you

Artist

Not interested

Encore will stop suggesting it. You can undo this any time in Settings.

This song

Golden Hour Static

This artist

Ember Cove · 14 songs in your library

Why (optional)

Not my tasteHeard it too muchWrong moment

CancelNot interested

Not interested
Android. Scope, an optional reason, then Undo for 5 s.

Mixes and suggestions

How adventurous

Familiar

Balanced

Adventurous

About 1 in 10 picks will be new to you.

Learn from my listening

Hide explicit songs

Smart downloads

Off›

Off: nothing is saved and suggestions use Recently added and Most played.

Not interested

12 songs, 3 artists›

How suggestions are doing

›

Reset suggestions…

Clear listening history…

Reset forgets what Encore learned. Likes, playlists and history stay.

Mixes and suggestions
iOS. Settings page that holds every control.

Mixes and
suggestions

How adventurous

Familiar

Balanced

Adventurous

About 1 in 10 picks will be new to you.

Learn from my listening

Songs you play shape your suggestions

Hide explicit songs

Smart downloads

Off

Not interested

12 songs, 3 artists

How suggestions are doing

Reset suggestions…

Forgets what Encore learned

Clear listening history…

Mixes and suggestions
Android. Same controls, Material 3 Expressive.

States of a mix and of the smart surfaces

Fresh

Quiet Hours

Slow, warm songs you play after 10 pm

Refreshed today, 6:00 AM

Normal state. Tap plays, long press opens Pin, Regenerate and Why these songs.

Building

Building your mixes…

First build or Regenerate, up to 20 s. The rest of Home stays usable and yesterday’s mix keeps playing.

Out of date

Quiet Hours

Refreshed 2 days ago

Refresh

Shown after 48 hours without a successful build. Never an empty tile.

Not enough plays yet

Encore is still learning

Play a few more songs and your mixes will start.

12 of 50 plays

Under 50 plays. Starter mixes from library audio sit beside it.

Learning is off

Suggestions are off

Turn on Learn from my listening to get mixes made for you.

Turn on

Home shows Recently added and Most played. Nothing is deleted by turning it off.

Private listening

Privateon Now Playing

This session is not saved and does not affect suggestions. Ends after 2 hours of silence. Guest sessions behave the same way.

Offline

Rainy Commute

Only downloaded songs

8 of 32 available

Candidates are limited to downloads. Autoplay keeps going from them.

Not interested: undo

Won’t suggest this againUndo

Shows for 5 s. Afterwards undo lives in Settings, Not interested. The song still plays if the user picks it.

Notes

1. Why this song. Opened from More on Now Playing, a queue row or a mix tile. It shows one or two reasons, never more, and up to two "sounds like" songs that open on tap. More like this counts as +1.5 for the song and its neighbours in this context but does not add a like. Fewer like this counts as −1.0. Both give a light haptic and a one-line confirmation.

2. Not interested. Scope is this song or this artist. Reasons change what is learned: Not my taste lowers the song and its neighbours, Heard it too much only rests the song for 30 days, Wrong moment lowers it in that time of day only. The song is removed from every smart surface at once, while the user can still play it by choice. Confirming gives a warning haptic. The same sheet opens from an artist page.

3. Adventurousness. Three steps: Familiar 5%, Balanced 10%, Adventurous 20% of slots. It applies from the next refill and never reshuffles a list the user is listening to.

4. Learn from my listening. Turning it off asks once, keeps saved history, and switches smart surfaces to the plain fallbacks. Turning it on again does not back-fill the time it was off.

5. Reset suggestions. A confirm sheet with a red Reset button, separated from Cancel. It forgets the learned model, co-listen graph and mixes and starts a new learning epoch. Likes, playlists, history and the Not interested list stay. Not interested has its own Clear list button.

6. Clear listening history. Deletes the events on every device and the server. No device can bring them back. The year-end recap then starts from the day it was cleared.

7. How suggestions are doing. A private, local page. It shows skip rate of app-chosen songs, completion, distinct artists per 20 songs and repeats for each shelf over the last 4 weeks, and nothing leaves the device.

8. Mix card states. Every state above has a text label, so colour is never the only signal. A mix never shows an empty tile, and a failed build keeps the last good mix with its age.
