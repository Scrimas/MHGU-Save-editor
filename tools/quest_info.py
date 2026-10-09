#!/usr/bin/env python3
"""Write data/quest-info.csv: each quest's locale, money and Hunter Rank Points, as the
quest board shows them. Pure stdlib; reads a RomFS dump like build_assets.py.

    quest_info.py [ROMFS]     default scratch/base_romfs.bin

Quest files (loc/quest/questData/questData_NNNNNNN.ext, 329 B, packed little-endian):
  +0x0C u32  quest id
  +0x12 u8   stars (5, 2 and 14 = G4 on the quests below; 0 and 101-116 also occur)
  +0x14 u8   stage (below)
  +0x16 u8   time limit, minutes
  +0x17 u8   carts allowed
  +0x1F u8   main objective type (1 hunt, 2 capture, ...), +0x20 u16 its target
  +0x31 u32  contract fee (z)
  +0x35 u32  not identified (0-45, nonzero on 203 quests)
  +0x39 u32  reward (z)
  +0x3D u32  subquest reward (z), 0 = no subquest
  +0x41 u32  not identified: reward / 10 (Village, Low/High), reward x 0.08 (G rank)
  +0x45 u32  always 0
  +0x49 u32  not identified: subquest reward / 10, or x 0.08 in G rank
  +0x4D u32  HRP (also set on Village quests, which the game does not show)
  +0x51 u32  subquest HRP
Checked against the game on 501 (3900z, HRP 250, fee 400z, sub 600z / 30 HRP), 10207
(4800z, 200, 500z, 600z / 20) and 11402 (20700z, 980, 2100z, 4500z / 100).

Stage ids (+0x14) and their name in GUI/06_msg/questMessage_<lang>.gmd (the board's
"Locale", short names; 101-104 are the night versions):
"""
import csv, os, re, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, gmd
from quest_sizes import PLACEHOLDERS

# stage id -> questMessage entry (eng short name)
STAGE_TEXT = {
    1: 4,      # J. Frontier (D)
    101: 5,    # J. Frontier (N)
    2: 6,      # V. Hills (D)
    102: 7,    # V. Hills (N)
    3: 8,      # A. Ridge (D)
    103: 9,    # A. Ridge (N)
    4: 10,     # M. Peaks (D)
    104: 11,   # M. Peaks (N)
    5: 12,     # Dunes
    6: 13,     # D. Island
    7: 14,     # Marshlands
    8: 15,     # Volcano
    9: 26,     # Arena
    10: 27,    # V. Slayground
    11: 16,    # A. Steppe
    12: 19,    # V. Hollow
    13: 17,    # Primal Forest
    14: 18,    # F. Seaway
    15: 28,    # F. Slayground
    16: 21,    # Sanctuary
    17: 20,    # Forlorn Arena
    18: 24,    # S. Pinnacle
    19: 22,    # Ingle Isle
    20: 23,    # Polar Field
    21: 25,    # Wyvern's End
    22: 114,   # Desert
    23: 115,   # Jungle
    24: 116,   # Ruined Pinnacle
    25: 117,   # Castle Schrade
    26: 118,   # Fortress
    27: 119,   # Forlorn Citadel
}
__doc__ += ''.join('  %3d -> entry %d\n' % kv for kv in STAGE_TEXT.items()) + """
Matched by the locale the quest descriptions name (several per stage) and the monsters
(10: Kecha Wacha, Nerscylla...; 15: Plesioth, Lagiacrus..., the floating one).

Quest text (quest/questData/questData_NNNNNNN_<lang>.gmd, lang eng fre ger ita spa,
the folder under nativeNX named the same), 7 entries: 0 title, 1 client, 2 description,
3 other monsters, 4 main objective, 5 failure conditions, 6 subquest ("None" / "Rien" /
"-" / "Nessuno" / "Ninguno" when there is none; "@" on 5 Italian ones). No subquest <=>
subquest reward 0 (677 quests); the two fishing trainings have a subquest worth 50z and
0 HRP. Objectives and subquests can hold a line break (\\r\\n).

Placeholder quests (as in quest_sizes.py) and the debug ones without text are left out,
as in quest_rewards.py. Columns: quest_id, stage, reward, hrp, fee, sub_reward, sub_hrp.
"""


def info(q):
    assert len(q) == 329
    u = lambda o: struct.unpack_from('<I', q, o)[0]
    stage, fee, reward, sub, hrp, sub_hrp = q[0x14], u(0x31), u(0x39), u(0x3D), u(0x4D), u(0x51)
    assert stage in STAGE_TEXT, stage
    assert u(0x45) == 0
    assert fee < reward or reward == 0, (fee, reward)
    assert sub or not sub_hrp
    return stage, reward, hrp, fee, sub, sub_hrp


def main(romfs):
    R = RomFS(romfs)
    rows = []
    for p in sorted(R.files):
        m = re.match(r'/nativeNX/loc/quest/questData/questData_(\d{7})\.ext$', p)
        if not m:
            continue
        text = '/nativeNX/eng/quest/questData/questData_%s_eng.gmd' % m.group(1)
        if text not in R.files:
            continue
        t = gmd(R.read(text))
        if t[0] in PLACEHOLDERS:
            continue
        q = R.read(p)
        assert struct.unpack_from('<I', q, 0x0C)[0] == int(m.group(1)), p
        r = info(q)
        assert len(t) == 7 and (t[6] == 'None') == (r[4] == 0), (p, t[6], r)
        rows.append((int(m.group(1)),) + r)
    out = os.path.join(here, '..', 'data', 'quest-info.csv')
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['quest_id', 'stage', 'reward', 'hrp', 'fee', 'sub_reward', 'sub_hrp'])
        w.writerows(rows)
    print('->', out, len(rows), 'rows')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
