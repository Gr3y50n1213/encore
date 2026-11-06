# Artwork fixtures

The four album covers drawn on board 04 (album gradients). They are the project's own original artwork, taken from the SVGs in `design/wireframes/html/wf.css` (`.cv-a` to `.cv-d`). The PNGs are those SVGs rendered at 64 x 64, the size the palette is sampled at. They were rendered by Chrome headless on 6 November 2026, and are committed so every test reads the same pixels.

| File | Album on the board | Expected scheme |
| --- | --- | --- |
| `golden-hour-static` | Golden Hour Static, Ember Cove | dark |
| `tidewater` | Tidewater, Salt & Lantern | dark |
| `brass-arches` | Brass Arches, Moth Parade | dark |
| `paper-moon` | Paper Moon, Quiet Press | light |

The expected palettes are in `design/tokens.json` (`albumGradient.exampleAlbums`) and on the board. `core/encore-core/src/palette/tests.rs` compares against them and prints the distances. Real album covers are never committed: they are copyrighted.
