# Request: Library sorts by listening, and choosing which server libraries to include

- **From:** android
- **To:** core
- **Status:** open
- **Date:** 7 November 2026
- **Phase:** 2 (the setup step), 3 (the sorts)

## What I need

1. **Two more album sorts**, `AlbumSort::MostPlayed` and `AlbumSort::FewestSkips`, and the same for `SongSort`, read from this profile's play log. Ties break by title. Under Private listening and Guest they read the same log (they never write it).
2. **The server's music libraries, and a choice of which to sync.** For board 21's "Choose what to include" step:
   - `media_libraries(source_id)` async → `[MediaLibrary { id, name, song_count?, included }]`, the server's music libraries (Jellyfin: the user's views of collection type music);
   - `set_included_libraries(source_id, ids)`, saved with the source; the next `sync_library` mirrors only those, and removes what is no longer included from the mirror (history stays).
   All included by default, so a profile that never sees the step syncs everything, as today.

## Why

- Board 06, item 2: "Sort. TAP opens a menu: Recently added, Title, Artist, Most played, Fewest skips." Today `AlbumSort` has Title, Artist, Year and RecentlyAdded, so the Android menu offers those four and leaves Most played and Fewest skips out.
- Board 21, item 1 and "Next steps": "Choose what to include lists the server's music libraries with check rows, all ticked by default." No call lists the libraries or limits the sync. The Android setup shows "Step 2 of 2" until this exists, rather than a step that does nothing.

## What I did meanwhile

- The sort menu lists the orders the core offers (`android/app/src/main/kotlin/uk/gcartee/encore/app/ui/library/LibraryScreen.kt`, `SortChoice`).
- Setup goes from Connect straight to Home; the step counter counts the steps that exist (`ConnectScreen`, `EncoreRoot.Onboarding`).

## Answer

<Filled in by the core track.>
