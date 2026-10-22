# -*- coding: utf-8 -*-
"""生成 icons/icon.ico（纯 Python，无 PIL 依赖，PNG-in-ICO 格式）"""
import os
import struct
import zlib

BG = (31, 36, 48, 255)
FG = (240, 244, 248, 255)


def draw(size):
    W = size
    px = [[(0, 0, 0, 0)] * W for _ in range(W)]
    rows = [[list(c) for c in row] for row in px]

    def in_rounded(x, y, x0, y0, x1, y1, r):
        cx0, cy0, cx1, cy1 = x0 + r, y0 + r, x1 - r, y1 - r
        dx = max(cx0 - (x + 0.5), (x + 0.5) - cx1, 0.0)
        dy = max(cy0 - (y + 0.5), (y + 0.5) - cy1, 0.0)
        return dx * dx + dy * dy <= r * r

    def rr(x0, y0, x1, y1, color, radius=0):
        for y in range(max(0, y0), min(W, y1)):
            for x in range(max(0, x0), min(W, x1)):
                if radius and not in_rounded(x, y, x0, y0, x1, y1, radius):
                    continue
                rows[y][x] = list(color)

    s = size / 256.0
    def sc(v):
        return int(round(v * s))

    rr(sc(20), sc(20), sc(236), sc(236), BG, sc(48))   # 底板
    rr(sc(108), sc(56), sc(148), sc(74), FG)           # 提手
    rr(sc(66), sc(74), sc(190), sc(90), FG)            # 盖
    rr(sc(80), sc(96), sc(176), sc(202), FG)           # 桶身
    rr(sc(102), sc(108), sc(111), sc(186), BG)         # 槽1
    rr(sc(145), sc(108), sc(154), sc(186), BG)         # 槽2
    return rows


def png_bytes(rows):
    H = len(rows)
    W = len(rows[0])
    raw = bytearray()
    for row in rows:
        raw.append(0)
        for c in row:
            raw.extend(bytes(c))

    def chunk(t, d):
        return (struct.pack(">I", len(d)) + t + d
                + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF))

    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
            + chunk(b"IEND", b""))


def main():
    sizes = [256, 128, 64, 48, 32, 16]
    entries = b""
    data = b""
    offset = 6 + 16 * len(sizes)
    for s in sizes:
        png = png_bytes(draw(s))
        entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(png), offset)
        data += png
        offset += len(png)
    ico = struct.pack("<HHH", 0, 1, len(sizes)) + entries + data
    out = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
    os.makedirs(out, exist_ok=True)
    path = os.path.join(out, "icon.ico")
    with open(path, "wb") as f:
        f.write(ico)
    print("written:", os.path.abspath(path), len(ico), "bytes")

    # 前端关于页用 256px PNG
    pub = os.path.join(os.path.dirname(__file__), "..", "public")
    os.makedirs(pub, exist_ok=True)
    png_path = os.path.join(pub, "icon.png")
    with open(png_path, "wb") as f:
        f.write(png_bytes(draw(256)))
    print("written:", os.path.abspath(png_path))


if __name__ == "__main__":
    main()
