"""Packs PNGs into a Windows .ico (PNG entries, which Windows Vista and later read): ico.py <out.ico> <png>...

Each PNG must be square, at most 256 px.
"""

import struct
import sys


def size(png: bytes) -> int:
    return struct.unpack(">I", png[16:20])[0]  # IHDR width


def main() -> None:
    out, pngs = sys.argv[1], [open(p, "rb").read() for p in sys.argv[2:]]
    entries, offset = b"", 6 + 16 * len(pngs)
    for png in pngs:
        n = size(png)
        entries += struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
    with open(out, "wb") as f:
        f.write(struct.pack("<HHH", 0, 1, len(pngs)) + entries + b"".join(pngs))


if __name__ == "__main__":
    main()
