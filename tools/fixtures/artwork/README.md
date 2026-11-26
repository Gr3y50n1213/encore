# Artwork fixtures

The four album covers drawn on board 04 (album gradients). They are the project's own original artwork, taken from the SVGs in `design/wireframes/html/wf.css` (`.cv-a` to `.cv-d`). The PNGs are those SVGs rendered at 64 x 64, the size the palette is sampled at. They were rendered by Chrome headless on 6 November 2026, and are committed so every test reads the same pixels.

| File | Album on the board | Expected scheme |
| --- | --- | --- |
| `golden-hour-static` | Golden Hour Static, Ember Cove | dark |
| `tidewater` | Tidewater, Salt & Lantern | dark |
| `brass-arches` | Brass Arches, Moth Parade | dark |
| `paper-moon` | Paper Moon, Quiet Press | light |

The expected palettes are in `design/tokens.json` (`albumGradient.exampleAlbums`) and on the board. `core/encore-core/src/palette/tests.rs` compares against them and prints the distances. Real album covers are never committed: they are copyrighted.

## JPEG versions (`jpeg/`)

Subsonic servers send covers as JPEG and can't be asked for a PNG, so the palette also reads JPEG (`core/encore-core/src/palette/jpeg.rs`, ADR core-0021). Made with Pillow on 25 November 2026 from the PNGs above:

- `<name>.jpg`: the PNG scaled to 512 x 512 (nearest neighbour) and saved as a baseline JPEG, quality 75, 4:2:0, as Navidrome scales covers. Read at one eighth, it gives the 64 x 64 picture back, so the board's palettes must come out again.
- `tidewater-restarts-444.jpg`: 200 x 136, 4:4:4, a restart marker every 5 blocks.
- `tidewater-grey.jpg`: one component (greyscale).
- `tidewater-progressive.jpg`: progressive, which the reader refuses (no palette, the profile accent shows).
