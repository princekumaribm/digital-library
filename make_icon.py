"""Writes the app icon (open book on a gradient) as .png or .icns using only the stdlib and macOS tools."""

import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

SIZE = 1024
RADIUS = 225
TOP, BOTTOM = (99, 102, 241), (6, 182, 212)


def inside_rounded(x, y):
    cx = min(max(x, RADIUS), SIZE - 1 - RADIUS)
    cy = min(max(y, RADIUS), SIZE - 1 - RADIUS)
    return (x - cx) ** 2 + (y - cy) ** 2 <= RADIUS ** 2


def pixel(x, y):
    if not inside_rounded(x, y):
        return (0, 0, 0, 0)
    t = (x + y) / (2 * SIZE)
    bg = tuple(int(a + (b - a) * t) for a, b in zip(TOP, BOTTOM))
    # Two pages with a gap for the spine, and a few text lines on each page.
    for left, right in ((250, 497), (527, 774)):
        if left <= x <= right and 330 <= y <= 700:
            line = any(abs(y - ly) <= 9 for ly in (420, 490, 560, 630)) and left + 45 <= x <= right - 45
            return (*bg, 255) if line else (255, 255, 255, 255)
    if 230 <= x <= 794 and 712 <= y <= 740:
        return (255, 255, 255, 200)
    return (*bg, 255)


def write_png(path):
    raw = bytearray()
    for y in range(SIZE):
        raw.append(0)
        for x in range(SIZE):
            raw.extend(pixel(x, y))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as fh:
        fh.write(png)


def write_icns(path):
    tmp = Path(tempfile.mkdtemp())
    try:
        png = tmp / "icon.png"
        write_png(png)
        iconset = tmp / "AppIcon.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale, suffix in ((1, ""), (2, "@2x")):
                px = str(size * scale)
                subprocess.run(["sips", "-z", px, px, str(png), "--out", str(iconset / f"icon_{size}x{size}{suffix}.png")],
                               check=True, stdout=subprocess.DEVNULL)
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(path)], check=True)
    finally:
        shutil.rmtree(tmp)


if __name__ == "__main__":
    out = sys.argv[1]
    write_icns(out) if out.endswith(".icns") else write_png(out)
