#!/usr/bin/env python3
"""Write data/arena.csv: the grade times and equipment sets of the Arena quests, from
the game's Arena equipment tables. Pure stdlib; reads a RomFS dump like build_assets.py.

    arena_tables.py [ROMFS]     default scratch/base_romfs.bin

Tables (arc/village/v00/v00.arc, quest/ac_equip; f32 version, u32 count, packed records):
  ac_pl_equip  942-B records, Arena quests 20001-20011
  ac_ny_equip  169-B records, Prowler Arena quests 120001-120006
  +0 u32 quest ID, +4 u8, +5 3 x u32 grade times in seconds, +0x11 3 x u32, then the
  five equipment sets from +0x1D: 182 B each for hunters (+0 equipment type 7-21),
  28 B for Prowlers (+0xA support bias 0-7).
The quest loader (0x3c1568) copies the times into the quest (+0xd3c) and the clear
(0x3b123c) grades a time 0, 1 or 2 when it is at most the first, second or third, else
3; the Guild Card's Arena log keeps it with the set and weapon (docs/11-save-map.md,
Arena log). Index = the Arena Counter's quest table 0x164fdb4, which lists these 17 in
this order.

Columns: index, quest_id, prowler (yes/no), grade_a, grade_b, grade_c (seconds), sets
(five values: the weapon in Guild Card storage order for hunters, the support bias for
Prowlers).
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc

# equipment type 7-21 -> Guild Card weapon storage order (docs/04); 12 is unused
WEAPON = {7: 0, 8: 1, 9: 2, 10: 3, 11: 4, 13: 5, 14: 6, 15: 7, 16: 8, 17: 9, 18: 10, 19: 11, 20: 12, 21: 13}


def records(d, size):
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + n * size, size
    return [d[8 + size * i:8 + size * (i + 1)] for i in range(n)]


def main(romfs):
    t = arc(RomFS(romfs).read('/nativeNX/arc/village/v00/v00.arc'))
    rows = []
    for name, size, prowler, ids in (('ac_pl_equip', 942, False, range(20001, 20012)), ('ac_ny_equip', 169, True, range(120001, 120007))):
        recs = records(t['quest\\ac_equip\\' + name], size)
        assert [struct.unpack_from('<I', r)[0] for r in recs] == list(ids), name
        for r in recs:
            times = struct.unpack_from('<3I', r, 5)
            assert times[0] < times[1] < times[2], times
            if prowler:
                sets = [r[0x1D + 28 * k + 0xA] for k in range(5)]
                assert all(b < 8 for b in sets), sets
            else:
                sets = [WEAPON[r[0x1D + 182 * k]] for k in range(5)]
            rows.append([len(rows), struct.unpack_from('<I', r)[0], 'yes' if prowler else 'no', *times, ' '.join(map(str, sets))])
    out = os.path.join(here, '..', 'data', 'arena.csv')
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['index', 'quest_id', 'prowler', 'grade_a', 'grade_b', 'grade_c', 'sets'])
        w.writerows(rows)
    print('->', out, len(rows), 'quests')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
