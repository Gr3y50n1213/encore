# Android review: Phase 9 (widgets, controls and live surfaces)

11 November 2026. The Android track's part of Phase 9:
- the sleep timer: the sheet, the fade and the card;
- three Glance widgets: Now Playing, Mixes and Recently played;
- the Quick Settings tile "Start a mix";
- Live Updates for the sleep timer and for downloads the person starts;
- Settings > Lock screen and widgets.

Checked against the "done when" line (*a widget set to one profile never shows another profile's history, the locked screen shows nothing from the listening history, the sleep timer ends with the fade-out, and no widget or Live Activity makes a network call*), the approved plan, PLAN-UX section 20, PLAN-ANDROID section 20, and boards 31, 32 and 34.

**New permission:** `POST_PROMOTED_NOTIFICATIONS`, approved by the owner on 11 November 2026; see PLAN-ANDROID section 19 and ADR android-0005.
**New libraries:** Glance 1.2.0, which brings WorkManager, Room and DataStore (all Google, Apache-2.0, in `THIRD_PARTY_LICENSES.md`). Approved by the owner the same day.

**Evidence:**
- **CI:** run 43087399927 (commit 694a93d) passed: lint, unit tests, debug and release builds, and every instrumented test on the API 28 phone, the API 36 phone (16 KB pages) and the API 36 tablet, including the release build starting. The API 37 emulator (the only one that can show a Live Update) has not yet run the tests: its image failed inside the emulator action before the script, so it is marked continue-on-error and Live Update promotion is **not checked**.
- **Emulators:** API 28, API 36 phone, API 36 tablet and the new API 37 phone, which is the only place a Live Update can be seen. Tests are in `android/app/src/test` (JVM) and `android/app/src/androidTest` (emulator). `device-check.sh` runs the emulator ones.
- **Not run on any real device.** Everything under "Not claimed" is for the owner.

## The "done when" line

| Item | Result | Evidence |
| --- | --- | --- |
| A widget set to one profile never shows another profile's history | **Passes on the emulators** | `WidgetProfileTest.aWidgetSetToOneProfileReadsOnlyThatProfilesFile`, `aFileInTheWrongFolderIsNotShown`, `aFixedProfileIsAlwaysItselfWhateverIsOpen`; `NowPlayingWidgetStatesTest.anotherProfilesFileIsNeverShown`; `CollectionWidgetStatesTest.recentNeverShowsListeningForLockedGuestRemovedOrAnotherProfile`. A widget for another profile or a PIN opens the switcher: `anotherProfileOrAPinMakesButtonsOpenTheSwitcher`, `anotherProfileOrAPinMakesATapOpenTheSwitcher`, `recentForAnotherProfileOrAPinOpensTheSwitcher`. |
| The locked screen shows nothing from the listening history | **Passes in the model and the declarations; the lock screen itself is for the owner** | Mixes and Recently played declare `not_keyguard`: `WidgetProviderXmlTest.mixesAndRecentlyPlayedAreNeverOnTheLockScreen`. Now Playing on the lock screen with "Hide listening history while locked" on never shows last played, paused or not: `NowPlayingWidgetStatesTest.onTheLockScreenWithHideOnTheLastPlayedSongNeverShows`, `onTheLockScreenWithHideOnAPausedSongDoesntShowEither`, `onTheLockScreenWithNoMixTheCardHasNoName`, `theLockScreenNeverOverridesLockedOrGuest`. A locked file never shows its song: `evenALockedFileWithASongNeverShowsIt`. The sleep timer card names no profile and nothing played (`SleepTimerNotifier`). Not seen on a lock screen: no Android 16 QPR1 phone. |
| The sleep timer ends with the fade-out | **Passes on the emulators** | `PlaybackFlowTest.theSleepTimerFadesOutPausesAndLogsTheEndAsEndedBySystem`: a 15 s timer, the volume ramps down, playback pauses, the volume is back, the play log says `endedBySystem`, the card is gone. `endOfSongPausesAtTheSongsEndWithTheNextSongLoaded`. `SleepFadeTest` (JVM) covers the ramp. Over Bluetooth: for the owner. |
| No widget or Live Update makes a network call | **Passes** | `WidgetSourceScanTest.onlyWidgetSyncTouchesTheCore`, `widgetSyncIsTheOneThatDoes` and `noWidgetFileUsesTheNetworkOrAFileAddress`. `NowPlayingWidgetTest.aBoundWidgetUpdatesWithoutStartingTheCore` and `CollectionWidgetsTest.boundMixesAndRecentWidgetsUpdateWithoutStartingTheCore` bind a widget in a fresh process and check the core doesn't start. The cover fetch is the core's, while the app runs (core review). |

## The phase's items

| Item | Result | Evidence |
| --- | --- | --- |
| Now Playing widget | **Passes on the emulators; Samsung sizes and colors are for the owner** | `NowPlayingWidgetStatesTest` covers playing, paused, idle, nothing yet, Next up with no song, unreachable, stale (24 h), locked, Guest and removed. The play button is round paused and a rounded square playing (PLAN-UX section 22). |
| Widget buttons with the app closed | **Passes on the emulators** | `NowPlayingWidgetTest.widgetButtonsPlayWithTheAppClosedAndKeepTheForegroundPromise`: Play with no activity open, no foreground-service error. `CollectionWidgetsTest.aMixesTileStartsPlaybackWithNoActivityOpen`. Under Samsung's battery management: for the owner. |
| The token and no profile id | **Passes** | `WidgetProfileTest.theTokenMustMatchExactly`, `buttonsPlayDirectlyOnlyForTheProfileTheServiceWouldOpen`. The service ignores a private action without the token. |
| Mixes widget | **Passes on the emulators** | `CollectionWidgetStatesTest`: Next up first, then pinned; at most four tiles, no mix twice; a pinned choice that is gone falls back to Next up. |
| Recently played widget | **Passes on the emulators** | `CollectionWidgetStatesTest.recentShowsUpToEightAlbumsAndOpensThem`, `theAlbumRequestRoundTripsAndRejectsBadOnes`, `recentStaleDimsAndOpensTheApp`. |
| Quick Settings tile | **Passes in its model; the tap is a best-effort emulator step** | `StartMixTileModelTest` (13 tests): first pinned mix, else Next up; Playing; Unlock Encore for a PIN on a locked phone; another profile opens the app; Android 9's label. `device-check.sh` runs `cmd statusbar add-tile` and `click-tile` as a best-effort step. Adding it on Android 13 and 14: for the owner. |
| Sleep timer sheet | **Passes** | `SleepTimerSheetTest.theSheetSetsShowsCancelsAndRestoresATimer` (set, running state, Cancel, Undo). |
| Sleep timer card and Live Update | **Passes as a notification; promotion not checked by a test** | The card uses the system countdown, a self-timeout and Add 15 min. `SleepFadeTest.theChipTextIsShort`. No test reads whether the timer card is promoted on API 37. |
| Live Updates for downloads | **Passes** | `LiveUpdatesTest`: only Android 16 QPR1 and later promotes, a person's own task is promoted, automatic work never, a toggle off means none, chip text under 7 characters. `OfflineAndCarTest` checks the download card is ongoing everywhere and promoted where the device allows it. |
| The lock-screen page | **Passes** | `LockScreenWidgetsPageTest.thePageShowsItsRowsAndATogglePersists`. |
| Download notification on the lock screen | **Passes by code; not read on a lock screen** | Private with a neutral public line when the history toggle is on (`DownloadNotifications.kt`). |
| Final CI | **Pending the last CI run** | Run id and result to be filled in. |

## Deviations from the plan

1. **The timer is per device, not per profile.** Board 31 item 5 says per profile and item 7 says switching profile needs no change. The plan chose per device (ADR core-0018).
2. **"Add 15 min" in song or album mode** switches to a time, 15 minutes after the song or album would have ended. The docs didn't say.
3. **"Hide listening history while locked" was kept.** The first idea was to drop it. That would have put the last played song on the lock screen.
4. **The first-sync Live Update was not built.** PLAN-ANDROID lists only the timer and downloads, and sync doesn't run as a job, so its card could outlive the app.
5. **The Recap widget is deferred to Phase 11,** as the owner said.
6. **`SleepTimerSpec` and `MixRef` are records**, since Dart enums can't carry data (core review).
7. **`org.json` was removed from the JVM tests.** The widget state tests read the core's JSON with `org.json`, which is a stub in JVM unit tests. They moved to the emulator (`NowPlayingWidgetStatesTest`, `CollectionWidgetStatesTest`, `WidgetProfileTest`), where it is real, so no new test library was added.
8. **EXPORT-FORMAT's `inProgress`.** The export writes `inProgress` for an open play, not `null`. `PlaybackFlowTest` reads it so, and `docs/core/EXPORT-FORMAT.md` is corrected.

## Found and fixed during this phase

1. **API 37 CI:** the emulator ran out of data space (a 6 GB data disk now), broke the install pipe right after boot (the workflow waits for the package service and retries), and aborted under `swiftshader_indirect` (it uses `swangle_indirect`).
2. **Lint `MissingPermission`:** the sleep card checks `POST_NOTIFICATIONS` before posting.
3. **A leaky playback test:** the new sleep tests left song 2 current, which the old gapless test found in History. `PlaybackFlowTest` now leaves no timer, fade or playing state behind.
4. **A double load at a sleep stop:** at an End of song stop the player is paused before the report, so the next song loads once, paused.

## Not claimed (for the owner)

- **Lock-screen widgets on an Android 16 QPR1 phone.** No device is available. A known gap.
- **Samsung's launcher:** widget sizes, colors and the picker's cell sizes.
- **Taps with the app closed under Samsung's battery management:** widget buttons, mix tiles and the Quick Settings tile.
- **The fade over Bluetooth.**
- **A timer running out while paused, with the process dead.**
- **Adding the tile on Android 13 and 14.**
- **Live Updates on a real phone:** only the API 37 emulator shows one.
- **Screenshots of the widgets against boards 31, 32 and 34.**

## Follow-ups

- ~~Board 31 item 5's per-profile wording needs fixing to "per device".~~ Fixed 12 November 2026.
- ~~The "Sleep timer set for ..." toast is drawn behind the open sheet.~~ Fixed 12 November 2026: the sheet's own running state shows when the music fades out, and an error is written under the title.
- ~~The tile's touch and hold opens the Mixes tab, not the specific mix.~~ Fixed 12 November 2026: it opens the pinned mix on top of the tab.
- Shuffle and repeat were left off the wide Now Playing widget.
- Recap widget: Phase 11.
