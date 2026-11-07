"""Draw the logo About shows, one colour per character cell.

The logo is 14 cells wide and 7 tall, and every cell is a single colour: About paints
it as background colour only, with no character drawn. Half blocks (two pixels to a
cell, the upper as the foreground of a half-block character) depend on the font: in
macOS Terminal.app, with any line spacing, the character does not fill its half of the
cell, and the logo came out striped and stepped. A background colour fills the whole
cell in every terminal.

A cell is about twice as tall as it is wide, so a step of two columns per row is a
45-degree line; the chevron's rows are three cells long, so each overlaps the next by
one and the stroke stays joined. The PNG is 14 x 14, each row of cells written twice, so it is drawn by
the same code as a picture and every cell's two halves are equal.

Standard library only: python3 -I scripts/logo_pixels.py OUT_FILE
"""

import struct
import sys
import zlib

TILE = (0x1E, 0x25, 0x30)
ORANGE = (0xF2, 0x8C, 0x28)
WHITE = (0xE6, 0xED, 0xF3)
TEAL = (0x2D, 0xB5, 0xA3)

# The tile with its corners cut, the chevron, the cursor under the prompt and the two
# bars of icon.svg; one character per cell. "." is transparent.
CELLS = [
    ".TTTTTTTTTTTT.",
    "TTTTTTTTTTTTTT",
    "TTOOOTTTTBTBTT",
    "TTTTOOOTTBTBTT",
    "TTOOOTTWWWTTTT",
    "TTTTTTTTTTTTTT",
    ".TTTTTTTTTTTT.",
]
COLOURS = {"T": TILE, "O": ORANGE, "W": WHITE, "B": TEAL}


def draw():
    """The logo as rows of (r, g, b, a), each row of cells twice."""
    rows = []
    for line in CELLS:
        row = [COLOURS[c] + (255,) if c in COLOURS else (0, 0, 0, 0) for c in line]
        rows += [row, list(row)]
    return rows


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
    with open(sys.argv[1], "wb") as f:
        f.write(png(draw()))
