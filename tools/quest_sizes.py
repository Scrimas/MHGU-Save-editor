#!/usr/bin/env python3
"""Write data/quest-sizes.csv: the smallest and largest size each monster can have in the
game's quests, from the quest files and the size variation tables; with them
data/quest-monsters.csv (each quest's boss entries) and data/size-variation.csv (the
tables), which the Database's crown odds read. Pure stdlib; reads a RomFS dump like
build_assets.py.

    quest_sizes.py [ROMFS]     default scratch/base_romfs.bin

Quest files (loc/quest/questData/questData_NNNNNNN.ext, 329 B): 5 boss entries of 13 B
at +0x64: u16 monster code, u16 spawn set, u8 count, u8 x 3 stat tables, u8, u16 size %
(+9), u8 variation table (+0xB), u8. Code 0 = no entry; size 0 = no size (only on the
game's placeholder quests and one minigame).

Variation tables (loc/arc/resident.arc, enemy/resident/em_size_yure_data, XFS): 52
cEmSizeYureTbl, each an array of cEmSizeYureTblElement (f32 mScaleRate, u32
mProbability, probabilities summing to 100). The size rolled = size % x a scale rate of
the quest's table; rates of probability 0 never come up.

The size table em_size_scale_data (same arc; mMonsterIndex, mTrueSize, mSmallScale,
mBigScale, mKingScale, mQuestSizeMin, mQuestSizeMax) gives nearly the same bounds:
equal for every monster with a size record but Rathalos (130 in Paint It Gold, table
125), Kecha Wacha (123 / 115) and Gravios (79 in Gravios Backbreaker, table 88). Whether the
game clamps to it is not checked, so the wider quest bounds are kept: a warning then
never flags a size the game can make.

Monster code -> monster index: the executable's table 0x1597ea4 (137 x (u32 code, u32
index - 1)), copied below. Quests named "@", "DUMMY", "dummy" or "dummy data" are the
game's placeholders (009xxxx, never offered) and are left out.

Columns: index, min, max (whole percent; min rounded down, max up), entries (boss
entries seen). Monsters absent from every quest (small monsters, unused slots) have no
row. Folded variants keep their own rows (data/monster-sizes.csv family_of).

quest-monsters.csv: quest_id, monster (index), size (%), table, one row per boss entry
with a size, in quest then entry order. size-variation.csv: table, rate (hundredths;
every rate is a whole hundredth), chance (%), the rates of chance 0 left out.
"""
import csv, math, os, re, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc, gmd

# 0x1597ea4: monster code of index 1..137
CODES = [
    1, 513, 1025, 2, 514, 1026, 3, 8, 9, 10, 11, 14, 15, 16, 17, 18, 1042, 19, 1043, 20,
    22, 23, 1303, 24, 25, 27, 30, 32, 1056, 33, 36, 37, 1061, 38, 43, 1323, 45, 1069, 46, 47,
    49, 50, 55, 56, 57, 1081, 58, 60, 1084, 61, 1085, 62, 63, 65, 66, 1090, 67, 68, 69, 71,
    72, 76, 77, 79, 80, 1104, 81, 82, 83, 84, 85, 4097, 4098, 4099, 4100, 4101, 4102, 4103, 4104, 4105,
    4106, 4107, 4108, 4109, 4110, 4111, 4112, 4115, 4116, 4117, 4119, 4121, 4122, 4123, 4130, 4131, 4135, 4136, 4137, 4138,
    4140, 4141, 4142, 4143, 4144, 4196, 4197, 4198, 4199, 4200, 4201, 4202, 4, 5, 7, 1031, 12, 13, 269, 525,
    1044, 21, 34, 42, 44, 1343, 70, 1351, 1103, 1105, 1106, 1107, 86, 87, 88, 4113, 4118,
]
PLACEHOLDERS = {'@', 'DUMMY', 'dummy', 'dummy data'}


def yure(d):
    """(scale rate, chance %) of chance > 0, per table. XFS objects in file order: a table
    header (class 5) with its array (class 3) and element count, then the elements (class 7)."""
    tab = re.compile(rb'\x05\x00..(....)\x01\x00\x00\x00\x03\x00..(....)\x01\x00\x00\x00\x01(....)', re.S)
    el = re.compile(rb'\x07\x00..\x14\x00\x00\x00\x01\x00\x00\x00(....)\x01\x00\x00\x00(....)', re.S)
    heads = [(m.start(), struct.unpack('<I', m.group(3))[0]) for m in tab.finditer(d)]
    els = [(m.start(), struct.unpack('<f', m.group(1))[0], struct.unpack('<I', m.group(2))[0]) for m in el.finditer(d)]
    out = []
    for k, (o, n) in enumerate(heads):
        end = heads[k + 1][0] if k + 1 < len(heads) else len(d)
        e = [(s, p) for a, s, p in els if o < a < end]
        assert len(e) == n and sum(p for _, p in e) == 100, (k, n, len(e))
        out.append([(s, p) for s, p in e if p])
    assert len(out) == 52, len(out)
    return out


def write(name, head, rows):
    out = os.path.join(here, '..', 'data', name)
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(head)
        w.writerows(rows)
    print('->', out, len(rows), 'rows')


def main(romfs):
    R = RomFS(romfs)
    chances = yure(arc(R.read('/nativeNX/loc/arc/resident.arc'))['enemy\\resident\\em_size_yure_data'])
    tables = [[s for s, _ in t] for t in chances]
    assert all(abs(s * 100 - round(s * 100)) < 1e-4 for t in tables for s in t)
    index = {c: i + 1 for i, c in enumerate(CODES)}
    rng = {}
    entries = []
    for p in sorted(R.files):
        m = re.match(r'/nativeNX/loc/quest/questData/questData_(\d{7})\.ext$', p)
        if not m:
            continue
        if gmd(R.read('/nativeNX/eng/quest/questData/questData_%s_eng.gmd' % m.group(1)))[0] in PLACEHOLDERS:
            continue
        q = R.read(p)
        assert len(q) == 329, p
        for k in range(5):
            r = q[0x64 + 13 * k:0x64 + 13 * (k + 1)]
            code, size, table = struct.unpack_from('<H', r)[0], struct.unpack_from('<H', r, 9)[0], r[11]
            if not code or not size:
                continue
            i = index[code]
            lo = math.floor(size * min(tables[table]) + 1e-3)
            hi = math.ceil(size * max(tables[table]) - 1e-3)
            a, b, n = rng.get(i, (lo, hi, 0))
            rng[i] = (min(a, lo), max(b, hi), n + 1)
            entries.append((int(m.group(1)), i, size, table))
    write('quest-sizes.csv', ['index', 'min', 'max', 'entries'], [(i,) + rng[i] for i in sorted(rng)])
    write('quest-monsters.csv', ['quest_id', 'monster', 'size', 'table'], entries)
    write('size-variation.csv', ['table', 'rate', 'chance'],
          [(k, round(s * 100), p) for k, t in enumerate(chances) for s, p in t])


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
