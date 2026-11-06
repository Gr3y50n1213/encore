# Board 41: Year recap: devices and states

PNG: `design/wireframes/png/41-year-recap-devices-and-states.png`  
HTML: `design/wireframes/html/Recap-Devices.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Year recap: across devices, timing and states

One recap per profile and year, built the same way on every device the profile is signed in on and drawn natively on each. This board shows how it is made, when it is ready, how it fits each window size and what it does when something is missing.

How it is made

Every signed-in device
Writes play events as you listen
Even offline. Stable song key, listened time, local time and offset, lyric-line jumps.

→

Synced play log
Companion server and each device's own copy
Append only. The only source of truth.

→

Recap engine
encore-recap in the Rust core
Daily rollups, then about 40 candidate facts, then score and pick, then pack, palette and seed.

→

Recap data
Small JSON, under 100 KB
Beats, pack, seed, palette, counted-through date, a hash of the log it used.

→

Server archive
Keeps every year
Past years survive a reinstall.

→

Every device
Draws it natively
SwiftUI, Compose, Flutter. Same data, same look.

Where the engine runs
On the server on Dec 1, on Jan 1 and when new plays arrive. On any device when the server is off or has not caught up. The same log, year and seed give the same bytes anywhere, so the two never disagree.

Where the NPU or ONNX runtime helps
Only in building a sound fingerprint for each track, once, when it is added to the library (Core ML on Apple devices, ONNX elsewhere or on the server). The recap reads those fingerprints for the mood arc and the sound clusters. Counting plays needs no accelerator.

What stays out of it
No cloud service, no other people's data, no percentile claims. Optional household beats use only profiles on the same server that both opted in.

When it is ready

Counting

Jan 1 to Nov 30

Every play is counted as it happens, in the time zone it happened in. "So far this year" is available any time.

Story open, marked "so far"

Dec 1 to Dec 31

The full story opens and updates daily. Every card says what date it is counted through. The last plays are counted at 23:59:59 on Dec 31.

Final run

Jan 1 to Jan 8

Rebuilt quietly as devices that were offline sync late plays. A small note says how many were added since you last looked.

Locked

From Jan 8

Never changes again. It goes to the archive and stays there.

Not to scale.

At each window size

Compact, under 600

Your 2026

No. 03Top artist

Low
lands

1,904 plays · 61 hours

2Marigold Ave 1,511

3The Quiet Hours 1,203

4Sable Coast 1,066

5Neon Orchard 948

Full screen. The card fills the screen and the background runs under the system bars.

Medium, 600 to 839

Your 2026

No. 03Top artist

Low
lands

1,904 plays · 61 hours

2Marigold Ave 1,511

3The Quiet Hours 1,203

4Sable Coast 1,066

5Neon Orchard 948

The 9:16 card sits in the centre of a soft glow made from the same palette. No panel, so it reads like the phone.

Expanded and Large, 840 and up

Your 2026

Your 2026

No. 03Top artist

Low
lands

1,904 plays · 61 hours

2Marigold Ave 1,511

3The Quiet Hours 1,203

4Sable Coast 1,066

5Neon Orchard 948

Details
1 Lowlands 1,904
2 Marigold Ave 1,511
3 The Quiet Hours 1,203
4 Sable Coast 1,066
5 Neon Orchard 948

Show the math
Share this card

The card keeps its 9:16 shape and a Details panel opens beside it with the full top 5, Show the math and Share. Left and right arrow keys step, Space pauses, Escape closes.

1. Recap data. Written by the engine, stored in the server archive and cached on each device. The newest counted-through date wins. Two devices that built it from the same log produce identical bytes.

2. Progress. The card number and finish flag sync as one small field, last writer wins. Open it on the Mac and it resumes where the phone stopped. Start over is one tap.

3. Sound. The Sound toggle is per device and does not sync.

4. Seen. A "new" dot on Home clears on every device when the story is first finished.

5. Late plays. Plays synced after Dec 31 are counted by the time they happened. The recap is rebuilt quietly until Jan 8.

6. Clear listening history. Deletes the plays on every device and the server. A recap built from them is rebuilt from the day of the clear, and the intro says "since Aug 3". An archived past year stays unless the profile is deleted or the user chooses Delete past recaps in the same confirm sheet.

States that look different

Your 2026 · so far

No. 01Minutes

41,208

minutes with your music.

28 days · 15 hours

JFMAMJJASOND

Counted through Dec 12. So far, 12% more than 2025.

Preview
December. A "so far" tag and the counted-through date on every card.

Your 2026

So farYour year

1,206

minutes so far.

A short year so far. Your top song was Slow Burn Season.

Short version
10 plays or more but under 20 hours or 100 plays: three cards (minutes, top song, top artist) with a gentle line.

Your 2026

No. 02Top song

212 plays

Slow Burn Season

Marigold Ave

2Glasshouse 188

3Paper Lanterns 171

4Low Tide Radio 160

5Night Shift 149

Sound off, you are offline

Offline, art not cached
Cards use a plain block where a cover would sit. Sound is off, with a note.

Your 2026

No. 03Top artist

Lowlands

1,904 plays · 61 hours

2Marigold Ave 1,511

3The Quiet Hours 1,203

4Sable Coast 1,066

5Neon Orchard 948

Shown in a simpler look on this device

Older app
The pack's fallback look: plain type on paper, no motion, with a line saying so.

Joined mid-year

Counts from the first play and says "Since June 14". Imported history counts too.

Final

Jan 8 onwards. The tag is gone and the year goes to the archive.

Updated

A late sync adds plays. A quiet line: 12 more plays since you last looked.

Server off

The device builds it from its own copy of the log. Same result.

Kid profile

Explicit titles are filtered, and late-night beats are skipped.

Guest or Private

No recap. Private listening is never counted and says so on the detail page.

Recap off

A Settings switch. Nothing is shown on Home and no rollups are kept.

Very new profile

Under 10 plays: no recap card on Home yet. It appears with the short version after 10 plays.
