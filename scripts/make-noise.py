#!/usr/bin/env python3
"""Writes assets/onboarding/noise.png: 256x256 grey film grain, tiled behind onboarding.

Standard library only; seeded, so the file is reproducible.
"""
import random
import struct
import zlib
from pathlib import Path

SIZE = 256
random.seed(12)
rows = b"".join(
    b"\x00" + bytes(v for _ in range(SIZE) for g in [random.randint(0, 255)] for v in (g, g, g, 255))
    for _ in range(SIZE)
)


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(rows, 9))
    + chunk(b"IEND", b"")
)
out = Path(__file__).resolve().parent.parent / "assets/onboarding/noise.png"
out.write_bytes(png)
print(f"wrote {out} ({len(png)} bytes)")
