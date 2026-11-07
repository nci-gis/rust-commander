#!/usr/bin/env bash
# Rebuild assets/xdevcmd.ico and assets/icon-{24,32,48,64}.png from the SVGs. Needs `resvg` (cargo install --locked resvg).
# The .ico is committed, so nobody needs this to build xdevcmd; run it after changing an SVG.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v resvg >/dev/null || { echo "resvg not found: cargo install --locked resvg" >&2; exit 1; }
tmp=$(mktemp -d)
trap 'rm -r -- "$tmp"' EXIT
# 16 px has its own drawing, snapped to whole pixels; the rest scale the main one.
resvg -w 16 -h 16 assets/icon-16.svg "$tmp/16.png"
for z in 24 32 48 64 128 256; do
  resvg -w "$z" -h "$z" assets/icon.svg "$tmp/$z.png"
done
# An .ico is a directory of images; PNG entries work on every Windows since Vista.
python3 -I - "$tmp" assets/xdevcmd.ico <<'PY'
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
echo "assets/xdevcmd.ico: $(wc -c < assets/xdevcmd.ico) bytes"
# The logos About draws in half blocks, built into the binary: it shows the largest one
# the window has room for. Each is rendered at its own size, not shrunk from another.
for z in 24 32 48 64; do
  resvg -w "$z" -h "$z" assets/icon.svg "assets/icon-$z.png"
  echo "assets/icon-$z.png: $(wc -c < "assets/icon-$z.png") bytes"
done
