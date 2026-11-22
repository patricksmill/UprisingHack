#!/usr/bin/env python3
"""Dump textures from Kid Icarus: Uprising's RomFS as PNG (stdlib only).

usage: kiu_textures.py <rom.cci> <out dir> <name prefix> [romfs dir prefix]

Scans every LZ11 `.zrc` archive (default under `us/menu/`) for CGFX
textures (TXOB) whose name starts with the prefix, decodes them and
writes `<name>.png` plus `<name>.txt` (format, size, source archive).
"""
import os, re, struct, subprocess, sys, zlib

FORMATS = {0: "RGBA8", 1: "RGB8", 2: "RGBA5551", 3: "RGB565", 4: "RGBA4",
           5: "LA8", 6: "HILO8", 7: "L8", 8: "A8", 9: "LA4", 10: "L4",
           11: "A4", 12: "ETC1", 13: "ETC1A4"}
BPP = {0: 32, 1: 24, 2: 16, 3: 16, 4: 16, 5: 16, 6: 16, 7: 8, 8: 8, 9: 8,
       10: 4, 11: 4, 12: 4, 13: 8}


def lz11(data):
    assert data[0] == 0x11
    size = int.from_bytes(data[1:4], "little")
    pos = 4
    if size == 0:
        size = int.from_bytes(data[4:8], "little"); pos = 8
    out = bytearray()
    while len(out) < size:
        flags = data[pos]; pos += 1
        for bit in range(8):
            if len(out) >= size: break
            if not flags & (0x80 >> bit):
                out.append(data[pos]); pos += 1; continue
            b = data[pos]; ind = b >> 4
            if ind == 0:
                n = (((b & 0xF) << 4) | (data[pos + 1] >> 4)) + 0x11
                disp = ((data[pos + 1] & 0xF) << 8 | data[pos + 2]) + 1; pos += 3
            elif ind == 1:
                n = (((b & 0xF) << 12) | (data[pos + 1] << 4) | (data[pos + 2] >> 4)) + 0x111
                disp = ((data[pos + 2] & 0xF) << 8 | data[pos + 3]) + 1; pos += 4
            else:
                n = ind + 1
                disp = ((b & 0xF) << 8 | data[pos + 1]) + 1; pos += 2
            for _ in range(n):
                out.append(out[-disp])
    return bytes(out)


def morton(x, y):
    m = 0
    for i in range(3):
        m |= ((x >> i) & 1) << (2 * i) | ((y >> i) & 1) << (2 * i + 1)
    return m


ETC_MODS = [(2, 8), (5, 17), (9, 29), (13, 42), (18, 60), (24, 80), (33, 106), (47, 183)]


def clamp(v): return 0 if v < 0 else 255 if v > 255 else v


def etc1_block(block):
    """4x4 RGB from one little-endian 64-bit ETC1 block, indexed [y][x]."""
    v = int.from_bytes(block, "little")
    hi, lo = v >> 32, v & 0xFFFFFFFF
    flip, diff = hi & 1, (hi >> 1) & 1
    t1, t2 = (hi >> 5) & 7, (hi >> 2) & 7
    if diff:
        def c5(s): return (hi >> s) & 0x1F
        def d3(s):
            d = (hi >> s) & 7
            return d - 8 if d & 4 else d
        b1 = [c5(27), c5(19), c5(11)]
        b2 = [b1[0] + d3(24), b1[1] + d3(16), b1[2] + d3(8)]
        e = lambda c: (c << 3) | (c >> 2)
        b1 = [e(c) for c in b1]; b2 = [e(c & 0x1F) for c in b2]
    else:
        b1 = [((hi >> s) & 0xF) * 17 for s in (28, 20, 12)]
        b2 = [((hi >> s) & 0xF) * 17 for s in (24, 16, 8)]
    px = [[None] * 4 for _ in range(4)]
    for x in range(4):
        for y in range(4):
            i = x * 4 + y
            second = (y >= 2) if flip else (x >= 2)
            base, table = (b2, t2) if second else (b1, t1)
            msb, lsb = (lo >> (16 + i)) & 1, (lo >> i) & 1
            m = ETC_MODS[table][lsb]
            if msb: m = -m
            px[y][x] = tuple(clamp(c + m) for c in base)
    return px


def decode(fmt, w, h, data):
    rgba = bytearray(w * h * 4)
    def put(x, y, p): rgba[(y * w + x) * 4:(y * w + x) * 4 + 4] = bytes(p)
    if fmt in (12, 13):
        pos = 0
        for ty in range(0, h, 8):
            for tx in range(0, w, 8):
                for sub in range(4):
                    sx, sy = tx + (sub & 1) * 4, ty + (sub >> 1) * 4
                    alpha = None
                    if fmt == 13:
                        alpha = int.from_bytes(data[pos:pos + 8], "little"); pos += 8
                    px = etc1_block(data[pos:pos + 8]); pos += 8
                    for y in range(4):
                        for x in range(4):
                            a = 255 if alpha is None else ((alpha >> ((x * 4 + y) * 4)) & 0xF) * 17
                            put(sx + x, sy + y, px[y][x] + (a,))
        return rgba
    bits = BPP[fmt]
    for ty in range(0, h, 8):
        for tx in range(0, w, 8):
            tile = ((ty // 8) * (w // 8) + tx // 8) * 64
            for i in range(64):
                x, y = i & 7, i >> 3
                n = tile + morton(x, y)
                if bits == 4:
                    b = data[n // 2]; v = (b >> 4) if n & 1 else (b & 0xF)
                else:
                    o = n * bits // 8; v = int.from_bytes(data[o:o + bits // 8], "little")
                if fmt == 0: p = ((v >> 24) & 255, (v >> 16) & 255, (v >> 8) & 255, v & 255)
                elif fmt == 1: p = ((v >> 16) & 255, (v >> 8) & 255, v & 255, 255)
                elif fmt == 2: p = (((v >> 11) & 31) * 255 // 31, ((v >> 6) & 31) * 255 // 31, ((v >> 1) & 31) * 255 // 31, 255 * (v & 1))
                elif fmt == 3: p = (((v >> 11) & 31) * 255 // 31, ((v >> 5) & 63) * 255 // 63, (v & 31) * 255 // 31, 255)
                elif fmt == 4: p = (((v >> 12) & 15) * 17, ((v >> 8) & 15) * 17, ((v >> 4) & 15) * 17, (v & 15) * 17)
                elif fmt == 5: p = (v >> 8, v >> 8, v >> 8, v & 255)
                elif fmt == 6: p = (v >> 8, v & 255, 0, 255)
                elif fmt == 7: p = (v, v, v, 255)
                elif fmt == 8: p = (255, 255, 255, v)
                elif fmt == 9: p = ((v >> 4) * 17,) * 3 + ((v & 15) * 17,)
                elif fmt == 10: p = (v * 17,) * 3 + (255,)
                else: p = (255, 255, 255, v * 17)
                put(tx + x, ty + y, p)
    return rgba


def png(path, w, h, rgba):
    raw = b"".join(b"\0" + bytes(rgba[y * w * 4:(y + 1) * w * 4]) for y in range(h))
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def textures(blob, prefix):
    for m in re.finditer(b"TXOB", blob):
        t = m.start() - 4
        if t < 0 or t + 0x58 > len(blob): continue
        name_at = t + 0xC + struct.unpack_from("<i", blob, t + 0xC)[0]
        if not 0 <= name_at < len(blob): continue
        name = blob[name_at:blob.find(b"\0", name_at)].decode("ascii", "replace")
        if not name.startswith(prefix): continue
        h, w, size = struct.unpack_from("<III", blob, t + 0x3C)
        fmt = struct.unpack_from("<I", blob, t + 0x34)[0]
        # in the file the pixels are a self-relative offset at +0x48; the
        # runtime address FINDINGS gives at +0x54 is only filled once loaded
        data_at = t + 0x48 + struct.unpack_from("<i", blob, t + 0x48)[0]
        if fmt not in FORMATS or not (8 <= w <= 1024 and 8 <= h <= 1024): continue
        if size != w * h * BPP[fmt] // 8 or data_at + size > len(blob): continue
        yield name, fmt, w, h, blob[data_at:data_at + size], data_at


def main():
    rom, out, prefix = sys.argv[1], sys.argv[2], sys.argv[3]
    folder = sys.argv[4] if len(sys.argv) > 4 else "us/menu/"
    here = os.path.dirname(os.path.abspath(__file__))
    listing = subprocess.run(["cargo", "run", "-q", "--release", "-p", "zakuro-fs", "--example", "romdump", "--",
                              os.path.abspath(rom), "files"], cwd=os.path.join(here, "../.."),
                             capture_output=True, text=True, check=True).stdout
    os.makedirs(out, exist_ok=True)
    seen = set()
    with open(rom, "rb") as f:
        for path, size, at in re.findall(r"^\s+(\S+\.zrc)\s+(\d+)\s+@0x([0-9A-F]+)$", listing, re.M):
            if not path.startswith(folder): continue
            f.seek(int(at, 16)); data = f.read(int(size))
            if data[0] != 0x11 or prefix.encode() not in (blob := lz11(data)): continue
            for name, fmt, w, h, pixels, data_at in textures(blob, prefix):
                if name in seen: continue
                seen.add(name)
                png(os.path.join(out, name + ".png"), w, h, decode(fmt, w, h, pixels))
                with open(os.path.join(out, name + ".txt"), "w") as t:
                    t.write(f"archive {path}\nformat {FORMATS[fmt]} ({fmt})\nsize {w}x{h}\npixels at 0x{data_at:X} in the decompressed archive\n")
                print(f"{name}: {FORMATS[fmt]} {w}x{h} from {path}")


if __name__ == "__main__":
    main()
