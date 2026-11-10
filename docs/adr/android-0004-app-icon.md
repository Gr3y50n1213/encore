# ADR android-0004: The app icon is the wave e

Date: 9 November 2026
Status: accepted

## Context
The app shipped with a placeholder: a Material Symbols music note on the Iris accent. The owner asked for a real icon that looks premium, was shown four directions (a repeat sign, a wave e, a wave note, a wavy ring with a play triangle), chose the wave e, and asked for it to be made "more polished and professional".

## Decision
**The mark is a lowercase e drawn in one stroke, whose crossbar is the wavy line from Now Playing's progress bar.** It ties the icon to the name and to the part of the app people look at most.

How it is drawn, on Android's 108 dp adaptive canvas:

- **The bowl** is a circle of radius 18.7 about the centre. With the stroke the mark is 45 dp across, inside the 66 dp circle every launcher shape keeps.
- **The stroke** is 7.5, one fifth of the bowl's width, with round caps and joins.
- **The crossbar** sits at 53.5, a touch above the middle. It is one swell and one dip (`sin(2πu)·sin(πu)`, four cubic segments) that is level where it leaves the bowl and where it arrives, so both joins are clean.
- **The stroke ends** on the 45° line at the lower right, which opens the letter enough to read as an e at 24 dp.

**The finish is Iris:** white on a gradient from a light iris (`#7F60F4`) through the accent seed (`color.accents.iris.seed`) to a deep violet (`#4526B4`), with a soft light from the top left. The seed stays a colour resource so `ThemeTokensTest` keeps it equal to the token.

**The themed (monochrome) layer is the same drawable.** The mark is one stroke, so Android's tint needs nothing more.

## Consequences
- `res/drawable/ic_launcher_foreground.xml` (the mark), `res/drawable/ic_launcher_background.xml` (the gradient) and `res/mipmap-anydpi/ic_launcher.xml` are the whole icon. No bitmaps, and nothing from outside the project.
- A second finish was drawn and set aside: Midnight, near-black with the mark in lilac fading to rose. Switching means a new background drawable and a gradient stroke on the foreground; the monochrome layer would then need its own plain copy of the mark.
- The notification icon stays the system's media style and the download glyph. A splash mark and a small status icon from the same e can follow when they are wanted.
- The other platforms have no icon yet. They should start from this drawing.
