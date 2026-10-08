#!/usr/bin/env python3
"""Write data/monster-carves.csv: what carving a large monster gives, per rank, from the
game's carve tables. Pure stdlib; reads a RomFS dump like build_assets.py.

    monster_carves.py [ROMFS]     default scratch/base_romfs.bin

Carve tables (arc/enemy/emXXX_YY.arc, enemy/hagi/hagi_sN_emXXX_YY, 346 B): the arc of
monster code YY << 8 | XXX (monster index: quest_sizes.CODES). N = 1 Low, 2 High, 4 G
rank (the items say so: Rathian Scale, Scale+, Shard); 0 and 3 are left out (3 holds
filler, 0 an older Low rank list on a few monsters).
  +0   u8[8]   header (03 01 12 14, u16 size of the used part, ff ff)
  +8   u8[18]  table index per kind and set: kind k, set j at byte 3k + j; ff = none.
               Kinds: 0 tail, 1 broken part (Barroth, Crystalbeard, Bloodbath),
               2 shiny drop (Wyvern Tear…), 3 other (Nightcloak), 4 dropped or mined
               (Gypceros, Basarios, Uragaan). The body table of set 1 is table 0.
  +26  u16[2] x 80  (count << 8 | chance %, item ID) pairs; ffff 0000 ends a table.
                    Chances of a table sum to 100.
A quest picks a set; most monsters have one, deviants and a few others up to 3 (higher
sets are the stronger quests). Only set 1 is written, with the number of sets.

Columns: monster, rank (low/high/g), sets, kind (body/tail/shiny/other), item, count,
chance. Kinds 1, 3 and 4 are written as "other".
"""
import csv, os, re, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc
from quest_sizes import CODES

RANKS = {1: 'low', 2: 'high', 4: 'g'}
KINDS = {0: 'tail', 1: 'other', 2: 'shiny', 3: 'other', 4: 'other'}


def tables(d):
    """The (count, chance, item) lists of a carve file, by table index."""
    v = struct.unpack_from('<160H', d, 26)
    pairs = list(zip(v[0::2], v[1::2]))
    out, cur, start = {}, [], 0
    for k, (x, item) in enumerate(pairs):
        if x == 0xFFFF:
            if cur:
                out[start] = cur
            cur, start = [], k + 1
        else:
            cur.append((x >> 8, x & 0xFF, item))
    for t in out.values():
        assert sum(c for _, c, _ in t) == 100, t
    return out


def main(romfs):
    R = RomFS(romfs)
    index = {c: i + 1 for i, c in enumerate(CODES)}
    rows = []
    for p in sorted(R.files):
        m = re.match(r'/nativeNX/arc/enemy/em(\d{3})_(\d{2})\.arc$', p)
        if not m:
            continue
        i = index[int(m.group(2)) << 8 | int(m.group(1))]
        a = arc(R.read(p))
        for n, rank in RANKS.items():
            d = a['enemy\\hagi\\hagi_s%d_em%s_%s' % (n, m.group(1), m.group(2))]
            assert d[:4] == b'\x03\x01\x12\x14', p
            head, t = d[8:26], tables(d)
            sets = max([j % 3 + 1 for j in range(18) if head[j] != 0xFF] + [1])
            kinds = [('body', 0)] + [(KINDS[k], head[3 * k]) for k in sorted(KINDS) if head[3 * k] != 0xFF]
            if t[0] != [(1, 100, FILLER)]:
                for kind, b in kinds:
                    rows += [(i, rank, sets, kind, item, count, chance) for count, chance, item in t[b]]
    out = os.path.join(here, '..', 'data', 'monster-carves.csv')
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['monster', 'rank', 'sets', 'kind', 'item', 'count', 'chance'])
        w.writerows(rows)
    print('->', out, len(rows), 'rows,', len({r[0] for r in rows}), 'monsters')


# a rank the monster is not in holds one First-aid Med (item 1716) at 100 %
FILLER = 1716

if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
