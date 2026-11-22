#!/usr/bin/env python3
"""Tile kiutrace top-screen shots into one PNG. Standard library only.

usage: pnm_grid.py <out dir> <dest.png> <columns> <shot> [<shot> ...]
A shot name is what the script passed to `shot`; this reads <shot>_top.ppm.
Optional: --crop x,y,w,h --zoom n to enlarge one region (words, glyphs).
"""
import struct, sys, zlib


def ppm(path):
    data = open(path, 'rb').read()
    _, w, h, _, pixels = data.split(maxsplit=4)
    w, h = int(w), int(h)
    return w, h, pixels[:w * h * 3]


def png(path, w, h, px):
    raw = b''.join(b'\0' + bytes(px[y * w * 3:(y + 1) * w * 3]) for y in range(h))
    chunk = lambda t, d: struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d))
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def main(argv):
    crop, zoom = None, 1
    while argv and argv[0].startswith('--'):
        flag, value, argv = argv[0], argv[1], argv[2:]
        if flag == '--crop': crop = [int(v) for v in value.split(',')]
        if flag == '--zoom': zoom = int(value)
    out, dest, cols, names = argv[0], argv[1], int(argv[2]), argv[3:]
    tiles = []
    for name in names:
        w, h, p = ppm(f'{out}/{name}_top.ppm')
        x0, y0, cw, ch = crop or (0, 0, w, h)
        rows = []
        for y in range(y0, y0 + ch):
            row = b''.join(p[(y * w + x) * 3:(y * w + x) * 3 + 3] * zoom for x in range(x0, x0 + cw))
            rows.extend([row] * zoom)
        tiles.append((cw * zoom, ch * zoom, b''.join(rows)))
    tw, th = tiles[0][0], tiles[0][1]
    rows_n = (len(tiles) + cols - 1) // cols
    W, H = cols * (tw + 4), rows_n * (th + 4)
    sheet = bytearray(b'\xff' * W * H * 3)
    for n, (_, _, t) in enumerate(tiles):
        ox, oy = n % cols * (tw + 4), n // cols * (th + 4)
        for y in range(th):
            sheet[((oy + y) * W + ox) * 3:((oy + y) * W + ox + tw) * 3] = t[y * tw * 3:(y + 1) * tw * 3]
    png(dest, W, H, sheet)


if __name__ == '__main__':
    main(sys.argv[1:])
