# Board 04: System: album gradients

PNG: `design/wireframes/png/04-system-album-gradients.png`  
HTML: `design/wireframes/html/System-Gradients.html` (open in a browser; styles in `wf.css` beside it)  

This board is a reference or layout board. The text below is every label, rule and caption on it; the PNG shows the layout and drawn examples. Keyword tags in square brackets mean: TAP, HOLD (long press), SWIPE, 2xTAP (double tap), GUARD (accidental-press protection), HAPTIC (what the device feels), MOTION, STATE.

---

Album gradients

As on Apple Music, the background is made from the playing album's own cover, so every album gets its own colors. The four covers below are original artwork, and the swatches under each are real: they were sampled from the cover image with the method described further down. The demos run about three times faster than production so the motion is easy to judge.

Golden Hour Static

Ember Cove

Ember · dark scheme, white text · lowest contrast 4.7:1

Sampled (5 clusters)

E65D45

C52957

1E1231

FED07A

673942

Used (base + 4 stops, contrast clamped)

380000

D02E10

D00053

1F1034

9B6A00

Tidewater

Salt & Lantern

Tidewater · dark scheme, white text · lowest contrast 4.52:1

Sampled (5 clusters)

0D3857

61C6BF

267B8D

C2EFE6

1C506A

Used (base + 4 stops, contrast clamped)

041827

00385C

00827C

007D91

4F7F77

Brass Arches

Moth Parade

Brass Arches · dark scheme, white text · lowest contrast 4.51:1

Sampled (5 clusters)

1F3D2B

E8C053

86AD66

D8ED92

D8ED92

Used (base + 4 stops, contrast clamped)

0B1A11

1A3E29

986D00

527D29

6D7E00

Paper Moon

Quiet Press

Paper Moon · light art, dark text · lowest contrast 9.92:1

Sampled (5 clusters)

F3EBDD

D94C3A

D4C1B5

E59383

87837C

Used (base + 4 stops, contrast clamped)

E9E8E5

F0EBE3

FFBDAD

E1D5CD

FEC9BE

Building the palette

1. Sample. Shrink the artwork to 64 × 64 and cluster it into five colors (k-means in OKLab). Cache the result per album.

2. Choose four stops. Rank clusters by population and chroma, drop near-duplicates, keep the top four, then add a dark base tinted with the dominant hue.

3. Keep text readable. Lower lightness on every stop until white text reaches 4.5:1. Light art, like the fourth cover, flips to a light field with dark text at 7:1.

4. Change tracks smoothly. Cross-fade the colors over 600 ms when the album changes. Songs on the same album keep the same field. Never snap, never restart the drift.

Where it appears

Now Playing and Lyrics. Animated field behind a soft blur. Each blob follows its own slow loop of 40 to 70 s, with about 6 to 10% of the width as travel and ±6° of hue shift.

Album and playlist pages. The cover sits on one tint taken from the palette, fading to the dark base. The field is pinned, only the list scrolls.

Mini player. Glass on iOS, a tonal container on Android, both lightly tinted by the dominant stop.

Recap cards. Same field at higher chroma, seeded by profile and year.

Building it

iOS. MeshGradient with a 4 × 4 grid of control points, animated from a TimelineView. Below iOS 18, layered radial gradients.

Android. AGSL RuntimeShader from API 33. Below that, blurred radial brushes. Compose has no built-in mesh gradient.

Fallbacks. Reduce Motion: static. Reduce Transparency or battery saver: solid tint, no blur. Missing art: the profile accent.

Reference. The open-source AMLL project has a fluid background worth studying. Confirm its license before copying any code.
