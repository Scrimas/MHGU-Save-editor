#!/usr/bin/env python3
"""Write data/skill-trees.csv, data/armor-skills.csv and data/decorations.csv: what each
skill tree activates, which armor pieces and decorations give its points. Pure stdlib;
reads a RomFS dump like build_assets.py. Numbers only: the names are in the asset pack.

    skill_tables.py [ROMFS]     default scratch/base_romfs.bin

Tables (loc/arc/resident.arc, f32 version, u32 count, packed records):
  skillTypeData  19 B per tree: u32 ID, u8 kind (1 deviant Soul, 2 Soul X), u16 x 7 the
                 skill (skillData record, named by skillData_eng entry 2k) active at
                 -20, -15, -10, +10, +15, +20, +25 points. The last tier never differs
                 from the one before. DERIVED: the thresholds are the series' known ones
                 (Attack Down L/M/S, Up S/M/L; Critical Eye -3 to +3; Health -30/-10/+20/+50).
  armorSeriesData  127 B per series: bytes 6-10 parts present; from byte 28, 15 B per part:
                 5 x (u16 tree, i8 points). DERIVED: Nargacuga Helm Evade Distance 3,
                 Expert 2, Dragon Res -2; its low-rank Faulds (Torso Up) hold none.
  decoData       5 B per decoration: u8 slots, (u8 tree, i8 points) x 2; record k = item
                 2638 + k (build_assets.py).

skill-trees.csv: tree, kind, then skill IDs at m20 m15 m10 p10 p15 p20 (the +25 tier
left out). armor-skills.csv: series, part (1 head … 5 legs), tree, points.
decorations.csv: item, slots, tree, points (one row per skill).
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc


def records(t, name, size):
    d = t['table\\' + name]
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + n * size, name
    return [d[8 + size * k:8 + size * (k + 1)] for k in range(n)]


def write(name, head, rows):
    with open(os.path.join(here, '..', 'data', name), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(head)
        w.writerows(rows)
    print('->', name, len(rows), 'rows')


def main(romfs):
    t = arc(RomFS(romfs).read('/nativeNX/loc/arc/resident.arc'))
    trees = []
    for k, r in enumerate(records(t, 'skillTypeData', 19)):
        ids = struct.unpack_from('<7H', r, 5)
        assert struct.unpack_from('<I', r)[0] == k and ids[6] == ids[5], k
        trees.append((k, r[4]) + ids[:6])
    write('skill-trees.csv', ['tree', 'kind', 'm20', 'm15', 'm10', 'p10', 'p15', 'p20'], trees)
    armor = []
    for k, r in enumerate(records(t, 'armorSeriesData', 127)):
        for p in range(5):
            if not r[6 + p]:
                continue
            for j in range(5):
                tree, pts = struct.unpack_from('<Hb', r, 28 + 15 * p + 3 * j)
                if tree and pts:
                    armor.append((k, p + 1, tree, pts))
    write('armor-skills.csv', ['series', 'part', 'tree', 'points'], armor)
    decos = []
    for k, r in enumerate(records(t, 'decoData', 5)):
        for j in range(2):
            tree, pts = struct.unpack_from('<Bb', r, 1 + 2 * j)
            if tree and pts:
                decos.append((2638 + k, r[0], tree, pts))
    write('decorations.csv', ['item', 'slots', 'tree', 'points'], decos)


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
