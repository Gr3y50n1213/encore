# Design

| Path | What it is |
| --- | --- |
| `tokens.json` | Design values: breakpoints, spacing grid, touch targets, radii, Android toggle-shape rule, Apple button behaviour, motion timing, haptics, album-gradient method, layout insets, smart-picks starting values |
| `wireframes/INDEX.md` | Start here for the 43 boards |
| `wireframes/png/` | One PNG per board, the visual reference |
| `wireframes/html/` | The same boards as standalone pages (open in a browser, keep `wf.css` beside them) |
| `wireframes/specs/` | One text file per board with the numbered behaviour rules, extracted from the boards |

## Using the tokens

- Every value in `tokens.json` names its source (a plan section or a board number) in a `$source` key. Values keep their units: points on Apple, dp and sp on Android, logical pixels in Flutter, milliseconds for time unless a key says Seconds.
- Phase 1 builds the codegen (Style Dictionary or a small Rust script, plan section 3) that turns this file into Swift constants, Kotlin constants and Dart constants. Until then, do not copy numbers into code by hand; ask for the codegen first.
- Two groups are intentionally unfinished and marked `"$status": "define-in-phase-1"`: the colour palette and the full type scale. The boards show an accent and text-over-art behaviour but no agreed palette. In phase 1, propose a palette and type scale that match the boards, in light and dark, and get the owner's approval before wiring it in.
- Anything under `androidToggleShape` is an Encore token. Material 3 Expressive documents shape slots, not numbers, so check them against the pinned Compose version and adjust the tokens, not the rule.
- Anything under `smartPicks` is a starting value, not a measurement.

## Reading a board

- The PNG shows the layout and drawn states. The spec lists each numbered rule in the same order as the marks on the PNG.
- Tags in square brackets: `[TAP]`, `[HOLD]` (long press), `[SWIPE]`, `[2×TAP]` (double tap), `[GUARD]` (protection against accidental presses), `[HAPTIC]` (what the device feels), `[MOTION]`, `[STATE]`.
- Album art colours in the boards are drawn placeholders. Real art takes one plain corner radius on every side, and the background gradient is built from the album's own cover by the method on board 04.
- Boards were drawn with IBM Plex for their own labels only. The apps use SF Pro on Apple devices (system font) and Inter on Android, Windows and Linux (plan section 3).
