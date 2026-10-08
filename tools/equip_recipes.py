#!/usr/bin/env python3
"""Write data/recipes.csv, data/weapon-tree.csv and data/provisions.csv: what the Smithy
asks to forge, upgrade and level up each weapon, armor piece and decoration. Pure stdlib;
reads a RomFS dump like build_assets.py. Numbers only: the names are in the asset pack.

    equip_recipes.py [ROMFS]     default scratch/base_romfs.bin

arc/facility/blacksmith.arc (f32 version, u32 count, packed records):
  weaponCreateWNN  38 B, NN = weapon class (no 05): +2 u16 weapon ID, +6 4 x (u16 item,
                   u8 count, u8 -), +22 provision (u16 group, u8 value), +26 scraps given
  weaponProcessWNN 32 B, one per weapon level past the forge: +0 u16 weapon ID, +2 u16
                   level (1: the upgrade from the parent weapon), +4 4 x (u16 item, u8
                   count), +16 provision (u16 group, u8 value), +20 scraps given
  armorCreateANN   42 B, NN = part - 1: +0 4 x u16 armor ID (smithy_lists.py), +10 4 x
                   (u16 item, u8 count, u8 -), +26 provision, +30 scraps given
  armorProcessA00  38 B, one per armor level past 1, all parts (the armor ID is the
                   series, the same in every part): +0 4 x u16 armor ID, +8 u16 level,
                   +10 4 x (u16 item, u8 count), +22 provision, +26 scraps given
  decoCreate       16 B: +0 u16 item, +4 4 x (u16 item, u8 count); some decorations
                   have two records (two recipes)
The first material's spare byte (create tables) is a flag, not part of the count.
loc/arc/resident.arc:
  weaponNNBaseData 32-34 B by class: +0 u32 weapon ID, last 10 B 5 x (u8 level, u8 weapon
                   ID): the weapons it upgrades into and the level it must reach first. DERIVED: Iron
                   Sword -> Santoku Reaver Lv 2, Buster Sword Lv 3, Ravager Blade Lv 4,
                   Lagiacrus Blade and Clero Blade Lv 5.
  itemData         44 B per item: +27 u8 provision value, +28 3 x u16 provision group.
                   A provision asks for materials of one group whose values add up to
                   the value asked ("Selected materials must add up to the required
                   value or higher", LbShop_EqMakeMsg). The game names no group. DERIVED:
                   group 4 holds the ores (Iron Ore 1, Earth Crystal 2, Disc Stone 3).

recipes.csv: kind (weapon:<class>, armor, deco), id (weapon ID, armor ID = series,
decoration item), level (0 forge, then the level reached), item1..4, count1..4 (0 none),
group, value (0 none). An armor row covers every part of its series; forge rows of
armor carry the part (armor:<part>).
weapon-tree.csv: class, weapon, parent, level. provisions.csv: item, value, group1..3.
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc

CLASSES = [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14]


def records(t, name, size):
    d = t['table\\' + name]
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + n * size, name
    return [d[8 + size * k:8 + size * (k + 1)] for k in range(n)]


def mats(r, at, stride):
    out = []
    for j in range(4):
        item, count = struct.unpack_from('<HB', r, at + stride * j)
        out += [item, count] if item else [0, 0]
    return out


def prov(r, at):
    group, value = struct.unpack_from('<HB', r, at)
    return [group, value] if group else [0, 0]


def write(name, head, rows):
    with open(os.path.join(here, '..', 'data', name), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(head)
        w.writerows(rows)
    print('->', name, len(rows), 'rows')


def main(romfs):
    rf = RomFS(romfs)
    b = arc(rf.read('/nativeNX/arc/facility/blacksmith.arc'))
    res = arc(rf.read('/nativeNX/loc/arc/resident.arc'))
    rows, tree = [], []
    for c in CLASSES:
        kind = 'weapon:%d' % c
        for r in records(b, 'weaponCreateW%02d' % c, 38):
            rows.append([kind, struct.unpack_from('<H', r, 2)[0], 0] + mats(r, 6, 4) + prov(r, 22))
        for r in records(b, 'weaponProcessW%02d' % c, 32):
            wid, lv = struct.unpack_from('<HH', r)
            rows.append([kind, wid, lv] + mats(r, 4, 3) + prov(r, 16))
        d = res['table\\weapon%02dBaseData' % c]
        size = (len(d) - 8) // struct.unpack_from('<I', d, 4)[0]
        for r in records(res, 'weapon%02dBaseData' % c, size):
            wid = struct.unpack_from('<I', r)[0]
            for j in range(5):
                lv, child = r[size - 10 + 2 * j], r[size - 9 + 2 * j]
                if child:
                    tree.append((c, child, wid, lv))
    # a weapon has one parent at most
    assert len({(t[0], t[1]) for t in tree}) == len(tree)
    for p in range(5):
        for r in records(b, 'armorCreateA%02d' % p, 42):
            for a in struct.unpack_from('<4H', r):
                if a:
                    rows.append(['armor:%d' % (p + 1), a, 0] + mats(r, 10, 4) + prov(r, 26))
    for r in records(b, 'armorProcessA00', 38):
        lv = struct.unpack_from('<H', r, 8)[0]
        for a in sorted(set(struct.unpack_from('<4H', r)) - {0}):
            rows.append(['armor', a, lv] + mats(r, 10, 3) + prov(r, 22))
    for r in records(b, 'decoCreate', 16):
        rows.append(['deco', struct.unpack_from('<H', r)[0], 0] + mats(r, 4, 3) + [0, 0])
    head = ['kind', 'id', 'level'] + [f'{k}{j}' for j in range(1, 5) for k in ('item', 'count')] + ['group', 'value']
    write('recipes.csv', head, rows)
    write('weapon-tree.csv', ['class', 'weapon', 'parent', 'level'], sorted(tree))
    provs = []
    for k, r in enumerate(records(res, 'itemData', 44)):
        value, *groups = struct.unpack_from('<B3H', r, 27)
        if value and groups[0]:
            provs.append([k, value] + groups)
    write('provisions.csv', ['item', 'value', 'group1', 'group2', 'group3'], provs)


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
