#!/usr/bin/env python3
"""Write data/combinations.csv: the game's item combination list. Pure stdlib; reads a
RomFS dump like build_assets.py. Numbers only: the names are in the asset pack.

    item_combos.py [ROMFS]     default scratch/base_romfs.bin

loc/arc/resident.arc table\\itemPreData (f32 version, u32 count, 24-B records):
  +0 u16 index, +2 u32 item, +6 u32 item, +10 u32 result, +14 u8 success chance %.
  Herb + Blue Mushroom = Potion at 95 %. Left out: +15 (0-15; ammo and the Slickaxe
  records set it, maybe how many it makes), +19 (1 traps and meats, 2 dung, 3 bombs,
  5 bait) and +23 (1 on three of the first records).

combinations.csv: item1, item2, result, chance.
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc


def main(romfs):
    t = arc(RomFS(romfs).read('/nativeNX/loc/arc/resident.arc'))
    d = t['table\\itemPreData']
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + 24 * n
    rows = []
    for k in range(n):
        idx, a, b, c = struct.unpack_from('<HIII', d, 8 + 24 * k)
        assert idx == k
        rows.append((a, b, c, d[8 + 24 * k + 14]))
    with open(os.path.join(here, '..', 'data', 'combinations.csv'), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['item1', 'item2', 'result', 'chance'])
        w.writerows(rows)
    print('-> combinations.csv', len(rows), 'rows')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
