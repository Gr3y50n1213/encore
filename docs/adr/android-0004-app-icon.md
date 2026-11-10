# ADR android-0004: The app icon is the repeat sign

Date: 9 November 2026
Status: accepted

## Context
The app shipped with a placeholder: a Material Symbols music note on the Iris accent. The owner asked for a real icon that looks premium and was shown four directions: a repeat sign, a wave e, a wave note, and a wavy ring with a play triangle.

The owner first chose the wave e. Two redrawings followed:

- **A level crossbar meeting the bowl.** Rejected: "We don't need random bumps or sharp points. It's supposed to be flowy and round."
- **The first drawing.** The owner found it "looks too much like a yin-yang symbol".

The owner then had the e and the repeat sign developed side by side and chose the bold, fully rounded repeat sign.

## Decision
**The mark is a repeat sign.** In sheet music, two dots beside a double bar line mean "go back and play it again", which is an encore written in music's own notation. No other music app uses it.

How it is drawn, on Android's 108 dp adaptive canvas:

- **Round shapes only.** Two dots and two pill-shaped bars, matching the app's buttons and handles. There are no corners.
- **The dots** have a 9.4 diameter and sit 7.8 above and below the centre line. This keeps them visible at 24 dp, where they were the first thing to go in the first drawing.
- **The bars** are 38 tall. The thin bar is 4.8 wide and the thick bar 10.4.
- **Every gap is 4.6.**
- **Placement:** the whole sign is 34 by 38 and sits 0.4 right of centre, because the dots weigh less than the bars. It stays inside the 66 dp circle every launcher shape keeps.

**The finish is Iris:** a white mark on a gradient from a light iris (`#7F60F4`), through the accent seed (`color.accents.iris.seed`), to a deep violet (`#4526B4`), with a soft light from the top left. The seed stays a colour resource so `ThemeTokensTest` keeps it equal to the token.

**The themed (monochrome) layer is the same drawable.** The mark is solid shapes in one colour, so Android's tint needs nothing more.

## Consequences
- The whole icon is three files, with no bitmaps and nothing from outside the project:
  - `res/drawable/ic_launcher_foreground.xml`: the mark;
  - `res/drawable/ic_launcher_background.xml`: the gradient;
  - `res/mipmap-anydpi/ic_launcher.xml`: the adaptive icon that combines them.
- **Midnight was drawn and set aside:** near-black, with the mark in lilac fading to rose. Switching to it means a new background drawable and a gradient fill on the foreground. The monochrome layer would then need its own plain copy of the mark.
- **What stays as it is:** the notification icon keeps the system's media style and the download glyph. A splash mark and a small status icon from the same sign can follow when they are wanted.
- **Other platforms** have no icon yet. They should start from this drawing.
