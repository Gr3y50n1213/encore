# ADR core-0018: The core owns the sleep timer and writes the widget snapshot

Date: 11 November 2026
Status: accepted

## Context
Phase 9 (ROADMAP): "a widget set to one profile never shows another profile's history, the locked screen shows nothing from the listening history, the sleep timer ends with the fade-out, and no widget or Live Activity makes a network call." The specs are PLAN-UX section 20 and boards 31 (sleep timer), 32 and 34 (widgets). The owner answered on 11 November 2026: Glance may be added, `POST_PROMOTED_NOTIFICATIONS` may be added, and the Recap widget waits for Phase 11.

Two facts in the code shaped the design:
- `playlog.rs` closes an open play as a skip when the next play starts. A plain pause at the end of a timer would have been logged as a skip the next morning, and skips feed the recommendations.
- `home()` makes network calls and its set rotates daily. A widget that went through it would break the "no network call" line.

## Decision
**One timer per device, in the registry.** The timer lives in the device registry (migration R3), not in a profile. It survives a profile switch and a new `EncoreCore` on the same data folder. A handoff to another device clears it. An expired timer is cleared when it is read.

**The core owns the state.** The core keeps the mode (Minutes 1 to 1,440, End of song, End of album), the end time, the stop point and the play-log effect. The platform runs the clock, the fade and the card. Calls: `set_sleep_timer`, `extend_sleep_timer`, `cancel_sleep_timer`, `restore_sleep_timer` (Undo; an expired timer is refused), `sleep_timer`, `sleep_timer_ended`, `sleep_fade_plan`, and `set_sleep_timer_seconds` for tests only.

**Stop points follow the queue.** For End of song and End of album the timer names `stop_after_uid`, the queue entry after which playback pauses. It is recomputed when the queue changes, and it accounts for Repeat One, Repeat All, shuffle and Autoplay refills. The estimated end time follows it. The platform loads nothing past that entry.

**The play-log effect.** `sleep_timer_ended(listened_ms)` closes the open play as `endedBySystem` with the System signal, never as a skip. The queue stays where it is, and a resume starts a new play. A timer that runs out while the player is paused is closed the same way: `sleep_timer()` does it when it finds the timer run out, and so does the next `playback_started` (`the_end_of_a_timer_is_never_logged_as_a_skip` is the test; see the core review).

**One fade rule.** `sleep_fade_plan(position_ms, duration_ms)` is a pure function that every platform calls from its own tick. It answers KeepPlaying, FadeNow (fade over `fade_ms`, then pause and call `sleep_timer_ended`) or StopAtSongEnd (the song ends inside the last 10 seconds: full volume until `fade_from_ms`, fade to the song's end, then pause, so that play logs Finished). `fade_ms` comes from the token `sleepTimerFadeOutMs` (10,000). End of song and End of album never fade; they play to the end and pause.

**The widget snapshot.** The core writes `<data>/widgets/<profile>/snapshot.json` (schema 1) and about 256 px covers in `art/` beside it. Widgets read these files and nothing else. Details:
- **Atomic:** a temporary file, then a rename.
- **Debounced:** a burst of changes is one write, about a second later. `set_profile_pin`, `start_guest` and `clear_listening_history` write at once, so the history is gone before anything can read it.
- **Not on every tick:** play state is written on start, pause, resume, seek and song change only. The bar is an indicator, not a clock.
- **Local state only.** The snapshot is built from the library mirror, the play log, the pins and the day's set as Home last stored it. It never calls `home()` and never touches the network. The one exception is the cover fetch, which goes through the core's own client while the app runs (offline, a downloaded album's kept cover is copied).
- **Event:** `WidgetSnapshotChanged` with the profile ID, so the app can redraw.
- **Read call:** `widget_snapshot()` returns the open profile's snapshot. There is no call for another profile's; the core never reads a profile that isn't open.

**Privacy rules in the file.**
- A profile with a PIN has `privacy = locked` from the moment `set_profile_pin` returns: no song, no mixes, no albums, and an emptied `art/` folder.
- A Guest session has `privacy = guest` with the same emptiness.
- Private listening and Learning off record no plays, so last played does not change.
- Clearing history empties the file at once.
- Removing a profile from the device or deleting it everywhere deletes its widget folder.
- Profiles never share a file.
- Cover file names are a hash of album, artwork tag and size, so a name says nothing about the album.

**`play_mix(MixRef)`.** `MixRef` is `Id(mix_id)` or `NextUp`. A widget or the Quick Settings tile calls it. An `Id` that is gone (the daily set rotated, or it was hidden or unpinned) falls back to Next up. **Next up** is the first mix of today's set, in the set's order, that no play was started from since the set was made and that isn't playing now; when every mix has been played, the set's first. With no Next up, the first pinned mix plays; with neither, `NotFound`. Learning off has no set, as on Home, so no Next up. The call does what Play on a mix card does (`StartedFrom::Mix`, the mix ID as context, shuffle off) and never builds a set or calls the network.

**The reserved `recap` field.** The file carries `recap: null`. The Recap widget is Phase 11; reserving the field means the schema doesn't change shape then.

**Record-shaped specs.** `SleepTimerSpec { mode, minutes }` and `MixRef { kind, mix_id? }` are records with an enum and an optional field, not Rust enums with data, because Dart's generated code can't carry data in an enum the way Rust does (the API is shared with the Flutter app).

## Alternatives considered
- **The app owns the timer.** The app can't tell the play log "ended by the system", it doesn't know where an album ends once Autoplay and shuffle are on, and every later platform would write it again.
- **One timer per profile.** Board 31 item 5 says both "per profile" and (item 7) that switching profile needs no change. A person who switches profile at night expects the timer to carry on, so it is per device. Item 5's wording needs fixing (see the review).
- **Widgets that call the core, or read `home()`.** That means opening the database from a widget process, possibly a network call, and showing the day's set under the wrong profile. A file per profile makes "never another profile's history" a property of the folder.
- **Dropping the history rule from the file.** The file holds only what the profile may show. The lock-screen rules are the platform's, from `privacy` and `song.now_playing`.
- **A snapshot read by the Windows widget host later.** Board 34 item 10 has Windows read the same file; the format is plain JSON for that reason.

## Consequences
- **Easier:**
  - one tested fade rule and one play-log rule for every platform (CORE-API v7);
  - widgets on any platform read a file and can't reach the database or the network;
  - a timer is never a skip in the recommendations.
- **Harder:**
  - the file can lag by about a second, and a cover that isn't saved yet is null until the snapshot is written again;
  - the snapshot has to be kept up to date from every place that changes what it shows (the list is in CORE-API, "Widgets");
  - the timer can't be set per profile without a new decision.
- **To undo:** migration R3's table can stay unused. Removing the feature means removing the calls (a CORE-API version note) and deleting `<data>/widgets`.
