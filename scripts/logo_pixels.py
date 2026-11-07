"""Draw the logo About shows as pixel art, one PNG per size, on whole pixels.

About draws the logo in half blocks, two pixels to a character cell, so each pixel is
large. A logo shrunk from icon.svg with smoothing has blended pixels along every edge,
and at that size they read as blur. Here every feature of icon.svg (drawn on a 64-unit
grid) is placed on whole pixels for the size asked, and each pixel is one of the logo's
five colours or transparent: no blending. Strokes keep one width throughout, and the two
bars stay equal.

Standard library only: python3 -I scripts/logo_pixels.py OUT_DIR SIZE...
"""

import math
import struct
import sys
import zlib

TILE = (0x1E, 0x25, 0x30)
ORANGE = (0xF2, 0x8C, 0x28)
WHITE = (0xE6, 0xED, 0xF3)
TEAL = (0x2D, 0xB5, 0xA3)


def draw(n):
    """The logo at n x n pixels, as rows of (r, g, b, a)."""
    k = n / 64
    px = [[(0, 0, 0, 0)] * n for _ in range(n)]

    # The tile: a square with rounded corners, a pixel in when its centre is inside.
    r = max(2, round(14 * k))
    for y in range(n):
        for x in range(n):
            cx, cy = x + 0.5, y + 0.5
            dx = max(r - cx, 0, cx - (n - r))
            dy = max(r - cy, 0, cy - (n - r))
            if dx * dx + dy * dy <= r * r:
                px[y][x] = TILE + (255,)

    # The chevron, drawn as pixel art draws a diagonal: each row a run of the same
    # length, one pixel further across than the row above, so the steps are even and
    # the slope exactly 45 degrees. icon.svg's arms are 15 across and 13 down: close.
    x0, y0 = round(15 * k), round(19 * k)
    half = round(13 * k)
    run = max(2, round(6 * math.sqrt(2) * k))  # a 6-unit stroke, measured across
    for i in range(half + 1):
        for y in (y0 + i, y0 + 2 * half - i):
            for x in range(x0 + i, x0 + i + run):
                if 0 <= x < n and 0 <= y < n:
                    px[y][x] = ORANGE + (255,)

    def rect(x0, y0, wd, ht, colour):
        for y in range(y0, y0 + ht):
            for x in range(x0, x0 + wd):
                if 0 <= x < n and 0 <= y < n:
                    px[y][x] = colour + (255,)

    # The cursor under the prompt.
    rect(round(34 * k), round(39 * k), max(3, round(15 * k)), max(2, round(6 * k)), WHITE)
    # The two bars: equal widths, the gap no narrower than a bar.
    bw = max(1, round(2.5 * k))
    gap = max(bw, round(2.5 * k))
    top, bottom = round(12 * k), round(30 * k)
    x0 = round(45 * k)
    rect(x0, top, bw, bottom - top, TEAL)
    rect(x0 + bw + gap, top, bw, bottom - top, TEAL)
    return px


def png(px):
    """A PNG of RGBA rows, standard library only."""
    h, w = len(px), len(px[0])
    raw = b"".join(b"\0" + bytes(c for p in row for c in p) for row in px)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(
            ">I", zlib.crc32(kind + data) & 0xFFFFFFFF
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


if __name__ == "__main__":
    out = sys.argv[1]
    for size in map(int, sys.argv[2:]):
        with open(f"{out}/icon-{size}.png", "wb") as f:
            f.write(png(draw(size)))
