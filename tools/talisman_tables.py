#!/usr/bin/env python3
"""Write data/talisman-tables.csv: the skills, points and slots each talisman tier can
have, from the game's charm tables. Pure stdlib; reads a RomFS dump like build_assets.py.

    talisman_tables.py [ROMFS]     default scratch/base_romfs.bin

Tables (loc/arc/resident.arc, f32 version, u32 count, packed records):
  amuletSkillDataNN  4-B records: u16 skill tree, i8 min points, i8 max points
  amuletSlotDataNN   4-B records: a key, then the chances of 1, 2 and 3 slots
Tier t (talisman entry +0x12: 97 Mystery, 98 Shining, 99 Timeworn, 100 Enduring) uses
skill tables 2(t-97) for the first skill and 2(t-97)+1 for the second (Mystery's is
empty: one skill), and slot table t-97. DERIVED: all 390 talismans of the analysed saves,
every one appraised in game, fit; the same tables shifted by one tier break 282-368.

Columns: kind (skill1, skill2, slots), tier, skill, min, max. A slots row gives the most
slots the tier's slot table can roll in max.
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc


def records(t, name):
    d = t['table\\' + name]
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + 4 * n, name
    return [d[8 + 4 * i:12 + 4 * i] for i in range(n)]


def main(romfs):
    t = arc(RomFS(romfs).read('/nativeNX/loc/arc/resident.arc'))
    rows = []
    for g in range(4):
        tier = 97 + g
        for k, kind in ((0, 'skill1'), (1, 'skill2')):
            for r in records(t, 'amuletSkillData%02d' % (2 * g + k)):
                skill, lo, hi = struct.unpack('<Hbb', r)
                rows.append((kind, tier, skill, lo, hi))
        top = max([j for r in records(t, 'amuletSlotData%02d' % g) for j in (1, 2, 3) if r[j]] or [0])
        rows.append(('slots', tier, '', 0, top))
    out = os.path.join(here, '..', 'data', 'talisman-tables.csv')
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['kind', 'tier', 'skill', 'min', 'max'])
        w.writerows(rows)
    print('->', out, len(rows), 'rows')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
