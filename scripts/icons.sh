#!/usr/bin/env bash
# Rebuild crates/xdevcmd/assets/xdevcmd.ico and about-logo.png from the SVGs in assets/.
# Needs `resvg` (cargo install --locked resvg).
# Both are committed, so nobody needs this to build xdevcmd; run it after changing an SVG.
# They live inside the crate because the build reads them and `cargo package` ships only
# what is inside the crate.
set -euo pipefail
cd "$(dirname "$0")/.."
out=crates/xdevcmd/assets
command -v resvg >/dev/null || { echo "resvg not found: cargo install --locked resvg" >&2; exit 1; }
tmp=$(mktemp -d)
trap 'rm -r -- "$tmp"' EXIT
# 16 px has its own drawing, snapped to whole pixels; the rest scale the main one.
resvg -w 16 -h 16 assets/icon-16.svg "$tmp/16.png"
for z in 24 32 48 64 128 256; do
  resvg -w "$z" -h "$z" assets/icon.svg "$tmp/$z.png"
done
# An .ico is a directory of images; PNG entries work on every Windows since Vista.
python3 -I - "$tmp" "$out/xdevcmd.ico" <<'PY'
import struct, sys
src, out = sys.argv[1], sys.argv[2]
sizes = [16, 24, 32, 48, 64, 128, 256]
pngs = [open(f"{src}/{z}.png", "rb").read() for z in sizes]
head = struct.pack("<HHH", 0, 1, len(sizes))
offset = len(head) + 16 * len(sizes)
entries, data = b"", b""
for z, png in zip(sizes, pngs):
    entries += struct.pack("<BBBBHHII", z % 256, z % 256, 0, 0, 1, 32, len(png), offset)
    offset += len(png)
    data += png
open(out, "wb").write(head + entries + data)
PY
echo "$out/xdevcmd.ico: $(wc -c < "$out/xdevcmd.ico") bytes"
# The logo About draws (14 x 7 cells, one colour each), built into the binary; see
# scripts/logo_pixels.py for why it is drawn by hand rather than shrunk from icon.svg.
python3 -I scripts/logo_pixels.py "$out/about-logo.png"
echo "$out/about-logo.png: $(wc -c < "$out/about-logo.png") bytes"
