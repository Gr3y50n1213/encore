# ADR core-0007: Album palettes, the same bytes everywhere

Date: 7 November 2026
Status: accepted

## Context
Board 04 makes each album's background from its cover:
1. sample at 64 × 64 and find 5 clusters with k-means in OKLab;
2. rank them by population and chroma, drop near-duplicates, keep 4, and add a dark base tinted with the leading hue;
3. lower lightness until white text reaches 4.5:1. Light art flips to a light field with dark text at 7:1.

The same palette must come out on iOS, Android, Windows, Linux and the server: the year recap is built from it, and its data must be byte-identical (plan sections 10 and 25). Two things normally break that:
- platform maths libraries (`pow`, `cbrt`), which may differ in the last bit;
- image decoders that resample differently.

## Decision
- **The cover is fetched as a PNG at most 64 px** (`format=Png&maxWidth=64&maxHeight=64&tag=`). PNG is lossless, so every device decodes the same pixels. It is read with png 0.18.1 (MIT or Apache-2.0, approved).
- **No platform maths:**
  - sRGB to linear is a 256-entry table, written out and checked against the formula (0 difference);
  - linear back to 8-bit uses the same table;
  - the cube root is a fixed bit-trick guess plus 8 Newton steps;
  - everything else is IEEE `+ - * /` and `sqrt`, which every platform rounds the same way. Rust never fuses multiply and add on its own.
- **Fixed seeding and ties:**
  - k-means starts from the most populous 16-level colour bucket, then the point adding the most weighted distance;
  - the iteration count is capped;
  - every tie is broken by order.
- **Light art** (mean lightness at least `lightArtMinLightness`, 0.75) gets a soft field. Every stop is lifted to at least `lightFieldMinLightness` (0.85) with chroma capped at `lightFieldMaxChroma` (0.08), as board 04's light field shows, then checked at 7:1 against black.
- **Every number** is in `tokens.json` (`albumGradient`).
- **Caching.** Palettes are kept per album or playlist with the artwork tag they were made from: a new cover makes a new palette. They work offline once made. No cover gives None, and the UI uses the profile accent.

## Evidence
- **The four board 04 covers** (the project's own SVG art, rendered to 64 px PNGs) give the board's schemes, and every colour the board uses has one of ours within 0.038 OKLab. A just-noticeable difference is about 0.02. Nothing was tuned per cover; the light-field rule was added because the first version missed board 04's light field (Paper Moon was 0.078 off).
- **A golden JSON** locks the output. CI checks it on x86_64 (Linux, Windows) and on Apple silicon (macOS, arm64).
- **The owner's 38 real covers** (checked on the owner's PC only, never committed): 33 dark and 5 light, every one readable.

## Alternatives considered
- **Platforms make the palette:** different decoders and maths, so different colours on each device and no recap parity.
- **JPEG covers:** decoders differ (SIMD IDCT paths), and a JPEG decoder would be one more dependency.

## Consequences
- Changing the algorithm or a token changes every palette, the recap's included. The golden test fails until it is updated on purpose.
- A future source that can't send PNG will need a decoder and its own determinism check.
