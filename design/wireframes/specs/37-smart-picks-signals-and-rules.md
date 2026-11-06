# Board 37: Smart picks: signals and rules

PNG: `design/wireframes/png/37-smart-picks-signals-and-rules.png`  
HTML: `design/wireframes/html/Smart-Logic.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Smart picks: signals and rules

Everything that chooses a song for the user (autoplay, mixes, smart shuffle, radio, suggestions, smart downloads) goes through one engine with one set of rules. It extends section 7 of the plan: the weights, the 30 second early-skip boundary, the 10% default exploration and the five mixes are unchanged, and the additions are marked New. Skips are read carefully so the app learns from real dislike and not from a dropped call.

The pipeline

1 · Capture
Log what happened
One event per play: song, who chose it, where it started, seconds played, how it ended, time bucket, output. Likes, adds and downloads are events too.

›

2 · Interpret
Read the event
Weights from the table below, minus events that do not count (call, route change, error, Private listening).

›

3 · Learn
Update the model
Per-profile ranking model plus the co-listen graph. Trained nightly on the server, or on the device while charging and idle.

›

4 · Candidates
About 500 songs
Co-listened, similar sounding, related artists, rediscovery and new additions, then hard filters.

›

5 · Rank
Score each one
Chance the song is played past 30 s and finished, from the model, or hand-tuned until one exists.

›

6 · Shape
Diversify and order
Exploration slots, diversity re-rank, artist and album caps, smooth tempo and energy.

›

7 · Deliver
Show it, explain it
Shelves, queue, widgets. Every pick keeps its top two reasons for "Why this song".

Live, on the device, before the next song starts: steps 1, 2, 4 to 7 and the in-session adjustment.

Nightly: step 3 and the mixes.

At request: queue refills and shelves read the precomputed lists, so the next song never waits for a model.

How each event is read

| Event | Reads as | Weight | Rule

| Liked | Permanent positive | +3.0 | Never decays. Unliking removes it.

| Replayed, queued by hand, searched then played | Strong intent | +1.5 | A replay is a restart within 10 minutes, or Repeat One.

| Added to a playlist or downloaded New | Strong intent | +1.5 | Removing it later does not undo the signal.

| Finished | Positive | +1.0 | Strongest when autoplay or a mix chose the song.

| Played past 80%, then skipped | Mostly enjoyed | +0.5 | Treated as finished for the context it was played in.

| Skipped between 30 s and 80% | Mild negative | −0.5 | Fades with a 30 day half-life New.

| Skipped in the first 30 s, app-chosen | The key signal | −1.5 | Also steers the rest of this session (below). Fades with a 30 day half-life.

| Skipped, song the user picked | Browsing | −0.2 | Ignored until the same song is skipped 3 times in 14 days New.

| Paused and never resumed | Ambiguous | 0 | Ignored.

| Not interested | Exclude | Exclude | The song, or the artist if chosen. Never decays. Cleared by Undo, by removing it in Settings, or by Clear list.

What does not count as a signal

Interruptions. A skip within 3 s of a headphone unplug, a Bluetooth or AirPlay switch, a call or an assistant request.

Errors. Unavailable songs, failed loads and automatic skips by the app.

Seeking. Dragging the progress bar is never a skip.

Accidents. Anything stopped under 2 s after starting.

Too soon. A negative for a song heard in the last 2 hours is recorded as "too soon", not dislike.

Ends by system. A sleep timer, a closed app or a lost connection.

Private listening and any time Learn from my listening is off: nothing is stored.

In-session adjustment Starting values

Three early skips in a row of app-chosen songs: exploration drops to zero for the next 10 picks, the session's sound moves 30% away from the skipped songs, and the unplayed autoplay songs are swapped in the background. Songs the user queued are never touched.

A like or a replay moves the session 30% toward that song and the next two picks come from its nearest neighbours.

Both run on the device before the next song starts. Tune the numbers with replay tests on real histories.

Where the engine is used

| Where | What it asks the engine for | Size | Refreshes | When there is no data, offline or learning is off

| Home: Made for you | The five mixes (plan section 7.1) | 5 tiles | Nightly, and after 25 or more new plays. Never while a mix is playing. | Yesterday's mixes with an age label. Under 50 plays: three mixes from library audio. Learning off: Recently added and Most played.

| Home: Rediscover | Loved songs not played for 60+ days | 12 | Daily | Random liked songs.

| Home: Recently played | Not smart. Plain history order. | 12 | Live | Always available.

| Mixes tab | The same mixes, each with a reason line, plus Deep Cuts | 5 + 1 | With the mixes | Same fallbacks as Home.

| Queue: Autoplay | Next songs seeded by the current song and the last 5 finished | 10 | Refills when fewer than 5 are upcoming. Touches only the Autoplay section. | Related artists, genre and era from metadata. Offline: downloaded songs only.

| Smart shuffle New | One suggestion per 4 songs from the list being shuffled | 1 in 4 | Live | Plain shuffle. Suggestions are marked with a spark and swipe away.

| Radio from a song, album, artist or playlist | Autoplay with that seed held for the whole session | endless | Live | Seed's related artists.

| Search | Order of equally good text matches only | – | Live | Text relevance. Personal boost is capped at 15% and never hides an exact match.

| Widgets, CarPlay, Android Auto | The same mix snapshot, written per profile | 5 | With the mixes | Last snapshot with an age label.

| Smart downloads New | Top songs of pinned mixes inside a size budget | 2 GB default | Nightly, on Wi-Fi and charging | Off by default. Without data it downloads nothing.

| Year-end recap | Reads the event log only, never the model | – | Year end | Needs 100 plays; otherwise a short card.

Ranking and shaping rules

Hard filters first. Not interested, hidden explicit songs when set, unavailable or not downloaded when offline, duplicates of the same recording, anything played in the last 24 hours unless loved.

Score is the model's chance that the song is played past 30 s and finished. Until a model exists: 0.35 affinity, 0.20 session fit, 0.15 time-of-day fit, 0.10 freshness, minus 0.20 for the recent skip rate in similar contexts Starting values.

Exploration. Familiar 5%, Balanced 10% (default, as in section 7), Adventurous 20% of slots, picked by Thompson sampling. Never the first slot, never two in a row, always after a song the user is likely to finish.

Diversity. Re-rank with maximal marginal relevance (weight 0.7 on score) over sound and artist. No artist twice within 3 songs, at most 2 per artist and 4 per album in a mix.

Flow. Tempo and energy move one step at a time, except at a planned mood shift.

Stability. A shelf changes by at most 30% on refresh unless the user pinned it or taps Regenerate.

Explainable. Each pick keeps its top two reasons.

Budgets Starting values

Next-song decision under 50 ms, read from the precomputed list. A full mix build under 20 s for 20,000 songs on a mid-range phone, under 150 MB of memory, and never during the first seconds of playback. Background jobs stop on a thermal warning.

Cold start

0. plays

Home shows Recently added, Rediscover and a Starter mix from the server's favourites and most played. Autoplay follows artist, genre and era. An optional import of Last.fm or ListenBrainz history skips this step.

1–49
plays

Up to three mixes from library audio (calm, upbeat, focus), a one-time "Encore is still learning" note, exploration fixed at Balanced.

50+
plays

Five mixes, session adaptation and specific "Why this song" reasons.

30 days
of history

Time-of-day mix, a mix for where taste has drifted, and the stretch mix with about 30% new-to-you songs.

When something is missing

| Situation | What happens

| No companion server | The device trains while charging and idle with the same Rust code. Nothing else changes.

| Server unreachable | Last model keeps working. Events queue on the device and sync later.

| Model file missing, corrupt or too new | Rebuilt from the event log. Hand-tuned scoring runs in the meantime.

| Audio analysis not finished | Metadata and co-listen only. Analysis continues in the background and upgrades the picks.

| Songs removed from the library | Dropped silently and refilled. A tile never points at a missing song.

| Offline | Candidates are limited to downloaded songs. Mixes say "Only downloaded songs".

| Battery saver or high temperature | Background builds wait. Live ranking still runs, it takes milliseconds.

| Library over 100,000 songs | Candidates come from an approximate nearest-neighbour index, capped at 200 per pool.

| Profile switch or Guest | Model, events and mixes belong to the profile and swap in under a second. Guest learns nothing and uses the plain fallbacks.

Rules that apply everywhere

1. The event log is the truth. Models, mixes and caches can always be rebuilt from it. Reset suggestions and Clear listening history are different: reset ignores old events for learning, clear deletes them.

2. Who chose the song matters. A skip of a song the user picked is browsing. A skip of a song the app picked is the strongest negative there is.

3. Sync. Events are append-only and merged by id. Models carry a version and a training epoch, and the newest epoch wins. Not-interested is a set of adds and removes, last write wins per item. Clear and Reset sync as tombstones, so another device cannot bring the data back.

4. Privacy. Listening data stays on the user's devices and their own server. Looking up related artists on MusicBrainz, importing from or scrobbling to ListenBrainz or Last.fm are off by default, ask per profile, and send song identifiers only.

5. Plain reasons. "Why this song" uses facts the user can check: what they played, when, and what sounds similar. It never guesses mood, health or location.

6. Measuring it. A private page, How suggestions are doing, shows skip rate of app-chosen songs, completion, distinct artists per 20 songs and repeats, per shelf. Offline replay on test histories runs in CI. No telemetry leaves the device. The v1 target is to beat shuffle by a wide margin (section 7).

7. Licences. The audio embedding model and the nearest-neighbour library each need a licence check before release. Some audio analysis tools use copyleft licences that fit the server (AGPL) but not the apps (MPL).

8. Guest and Private listening. Neither stores events. Private listening is a per-session switch in the Now Playing menu that ends after 2 hours of silence and shows a small badge while on.
