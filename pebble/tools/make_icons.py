"""Draws the watchapp's action bar icons (pebble/resources/images/*.png).

Run from the repository root: python3 -I -B pebble/tools/make_icons.py
The PNGs are committed; this script is how they were made, so they can be redrawn.
Each is 18 x 18, 8-bit greyscale, opaque: a white glyph on black, the action bar's own
black. Opaque on purpose: the action bar draws a 1-bit icon with GCompOpAssign and any
other with GCompOpSet, and an opaque black-and-white icon looks the same either way, on the
black-and-white and the colour Pebbles.
"""

import os
import struct
import sys
import zlib

SIZE = 18
OUT = os.path.join("pebble", "resources", "images")


def triangle(px, py, a, b, c):
    """Whether the pixel centre (px + 0.5, py + 0.5) is inside triangle abc."""
    x, y = px + 0.5, py + 0.5

    def side(p, q):
        return (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0])

    s1, s2, s3 = side(a, b), side(b, c), side(c, a)
    return (s1 >= 0 and s2 >= 0 and s3 >= 0) or (s1 <= 0 and s2 <= 0 and s3 <= 0)


def bar(px, py, x0, x1, y0, y1):
    return x0 <= px < x1 and y0 <= py < y1


def glyph(name, px, py):
    if name == "play":
        return triangle(px, py, (5, 3), (5, 15), (15, 9))
    if name == "pause":
        return bar(px, py, 4, 8, 3, 15) or bar(px, py, 10, 14, 3, 15)
    if name == "prev":
        return bar(px, py, 3, 5, 3, 15) or triangle(px, py, (15, 3), (15, 15), (5, 9))
    if name == "next":
        return bar(px, py, 13, 15, 3, 15) or triangle(px, py, (3, 3), (3, 15), (13, 9))
    raise ValueError(name)


def png(rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + bytes(row) for row in rows)
    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 0, 0, 0, 0)  # 8-bit greyscale
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def main():
    os.makedirs(OUT, exist_ok=True)
    for name in ("prev", "play", "pause", "next"):
        rows = [[255 if glyph(name, x, y) else 0 for x in range(SIZE)] for y in range(SIZE)]
        with open(os.path.join(OUT, name + ".png"), "wb") as f:
            f.write(png(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
