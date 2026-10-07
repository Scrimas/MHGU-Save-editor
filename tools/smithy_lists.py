#!/usr/bin/env python3
"""Write data/smithy-lists.csv: the entries of every Smithy list, from the game's create
tables. Pure stdlib; reads a RomFS dump like build_assets.py.

    smithy_lists.py [ROMFS]     default scratch/base_romfs.bin

The tables are in arc/facility/blacksmith.arc (f32 version, u32 count, packed records):
  weaponCreateWNN  38-B records, NN = weapon class (no 05); +2 u16 weapon ID
  armorCreateANN   42-B records, NN = part - 1; +0 4 x u16 armor IDs (0 = none): one ID,
                   a Blademaster / Gunner pair, or type 1 Blademaster, type 1 Gunner,
                   type 2 Blademaster, type 2 Gunner
  decoCreate       16-B records; +0 u16 decoration item ID
  otWeaponCreate   36-B records, Palico weapons; otArmorCreate the same for helms and mail

The save keeps two bitmaps per list, listed and NEW (docs/11-save-map.md, S fields). Bit =
record index; armor bit = 4 x record + ID slot. The Smithy shows an entry whose listed
bit is set (0x5248c0 weapons, 0x5249f4 armor, read by 0x6fa928); when it first lists one
it sets listed and NEW (0x524980). A type 1 hunter of the analysed save has slots 0-1 of
every four-ID record listed and never slots 2-3.

Columns: list (weapon:<class>, armor:<part>, deco, palico:<weapon|helm|mail>), records,
ids (armor only: per record, how many ID slots it fills, 1, 2 or 4).
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc


def records(t, name, stride):
    d = t['table\\' + name]
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + n * stride, name
    return [d[8 + stride * i:8 + stride * (i + 1)] for i in range(n)]


def main(romfs):
    t = arc(RomFS(romfs).read('/nativeNX/arc/facility/blacksmith.arc'))
    rows = []
    for w in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14]:
        rows.append(('weapon:%d' % w, len(records(t, 'weaponCreateW%02d' % w, 38)), ''))
    for p in range(5):
        r = records(t, 'armorCreateA%02d' % p, 42)
        ids = ''.join(str(sum(1 for k in range(4) if struct.unpack_from('<H', x, 2 * k)[0])) for x in r)
        assert set(ids) <= set('124'), ids
        rows.append(('armor:%d' % (p + 1), len(r), ids))
    rows.append(('deco', len(records(t, 'decoCreate', 16)), ''))
    rows.append(('palico:weapon', len(records(t, 'otWeaponCreate', 36)), ''))
    n = len(records(t, 'otArmorCreate', 36))
    rows += [('palico:helm', n, ''), ('palico:mail', n, '')]
    out = os.path.join(here, '..', 'data', 'smithy-lists.csv')
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['list', 'records', 'ids'])
        w.writerows(rows)
    print('->', out)


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
