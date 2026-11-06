# Board 07: Search

PNG: `design/wireframes/png/07-search.png`  
HTML: `design/wireframes/html/Search.html` (open in a browser; styles in `wf.css` beside it)  

Phone screens (iOS on the left, Android on the right) are in the PNG; the text below is the numbered behaviour column that sits beside them, in the same numbering as the circled marks on the PNG. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

iOS 26 · Liquid Glass

Android 16 · Material 3 Expressive

Search

1. Search field. iOS puts it at the bottom in glass, within thumb reach; Android docks it at the top. [TAP] opens the keyboard and shows results as you type, matched against the on-device library index in under 50 ms, typo tolerant, lyrics included when the server has them. The return key saves the query. [SWIPE] down on results dismisses the keyboard. Android's scope chips: All, Library, Downloaded.

2. Recent searches. Saved per profile and never synced to anyone else. [TAP] fills the field and runs it. [SWIPE] left or tap Remove deletes one. [HOLD] on the header offers "Clear all" in a confirm sheet.

3. Browse by mood. Tiles come from the audio analysis, not genre tags. [TAP] opens a mix for that mood. Android's tiles alternate two leaf shapes, round at one pair of corners and tight at the other, so the grid reads as a pattern. Tiles are plain color, never artwork.

Results while typing. Sections: Top Result, Songs, Albums, Artists, Playlists. [TAP] song plays it and starts autoplay from it. [TAP] other types open them. [HOLD] item menu. [GUARD] the list is frozen from touch-down until release, so a row that re-ranks under the finger never launches the wrong song.

States. Idle (shown). Typing, results (above). No match: "Nothing matches [query] " with a tip to check spelling; if an online lyrics provider is on, "Search lyrics online" appears. Empty library: "Connect a server to search". Offline: search still works from the local index, and songs that are not downloaded dim.
