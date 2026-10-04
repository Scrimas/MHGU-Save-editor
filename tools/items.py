#!/usr/bin/env python3
"""List the item box, the item pouch and the item loadouts of an MHGU save. Read only.

Layout (docs/11-save-map.md, from the game's loader 0x19a4ec / 0x19e0ac):
  base + 0x278   item box, 2300 slots
  base + 0x17CF  item loadouts, 24 x 170 B: name char[42], 32 x (u16 item, u16 count)
  base + 0x27BF  item pouch, 32 slots (24 items + 8 ammo / coatings)
A box or pouch slot is 19 bits of an LSB-first bit stream: item ID (12 bits), then
count (7 bits). Slot i starts at bit 19 * i. ID 0 = empty.

Item names come from the game's own text table if one is given:
romfs nativeNX/eng/table/itemData_eng.gmd (item name = string 2 * ID).

Usage:  items.py [--names itemData_eng.gmd] [--slot 1|2|3] system
"""
import sys, struct, pathlib

BOX, BOX_N = 0x278, 2300
LOADOUTS, LOADOUT_N, LOADOUT_SZ = 0x17CF, 24, 170
POUCH, POUCH_N = 0x27BF, 32


def gmd_strings(d):
    assert d[:4] == b'GMD\0'
    sc, ss = struct.unpack_from('<I', d, 0x18)[0], struct.unpack_from('<I', d, 0x20)[0]
    return [x.decode('utf-8', 'replace') for x in d[len(d) - ss:].split(b'\0')[:sc]]


def slots(buf, off, n):
    v = int.from_bytes(buf[off:off + (19 * n + 7) // 8], 'little')
    return [((v >> (19 * i)) & 0xfff, (v >> (19 * i + 12)) & 0x7f) for i in range(n)]


def main(argv):
    names, slot, path = None, 1, None
    it = iter(argv)
    for a in it:
        if a == '--names': names = gmd_strings(pathlib.Path(next(it)).read_bytes())
        elif a == '--slot': slot = int(next(it))
        else: path = a
    if path is None: sys.exit(__doc__)
    buf = pathlib.Path(path).read_bytes()
    base = 0x24 + struct.unpack_from('<I', buf, 0x34 + 4 * (slot - 1))[0]
    name = lambda i: names[2 * i] if names and 2 * i < len(names) else '#%d' % i
    hunter = buf[base:base + 32].split(b'\0')[0].decode(errors='replace')
    print(f'character {slot}: base 0x{base:X}  name {hunter!r}')

    box = slots(buf, base + BOX, BOX_N)
    used = [(i, a, c) for i, (a, c) in enumerate(box) if a]
    print(f'\n--- item box: {len(used)} of {BOX_N} slots used ---')
    for i, a, c in used:
        print(f'  {i:4d}  {a:4d}  x{c:<3d} {name(a)}')

    print('\n--- item pouch ---')
    for i, (a, c) in enumerate(slots(buf, base + POUCH, POUCH_N)):
        if a: print(f'  {i:4d}  {a:4d}  x{c:<3d} {name(a)}')

    print('\n--- item loadouts ---')
    for k in range(LOADOUT_N):
        o = base + LOADOUTS + LOADOUT_SZ * k
        items = [struct.unpack_from('<HH', buf, o + 42 + 4 * j) for j in range(32)]
        items = [(a, c) for a, c in items if a]
        if items:
            title = buf[o:o + 42].split(b'\0')[0].decode(errors='replace')
            print(f'  {k + 1:2d} {title!r}: ' + ', '.join(f'{name(a)} x{c}' for a, c in items))


if __name__ == '__main__':
    main(sys.argv[1:])
