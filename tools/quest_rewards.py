#!/usr/bin/env python3
"""Write data/quest-rewards.csv and data/rewards.csv: the reward tables each quest uses.
Pure stdlib; reads a RomFS dump like build_assets.py.

    quest_rewards.py [ROMFS]     default scratch/base_romfs.bin

Each quest's arc (loc/arc/quest/qNNNNNNN.arc) holds questLink_NNNNNNN, which names its
resources: 5 boss sets, 3 small-monster sets, then 5 reward tables "rem_XXXXXX"
(rem_000000 = none), and the reward tables themselves (quest/rem/rem_XXXXXX):
  +0   f32 1.0, u32 1
  +8   u8 flag, u8 n, 14 B 0
  +24  n x (u16 item, u8 count, u8 chance %), chances summing to 100 (all but one
       table, which sums to 104; written as it is)
The 5 slots, by what they hold: 0 main rewards (the target's parts), 1 main rewards
(general items), 2 and 3 extra rewards (Distinction, Horns Coins…; flag 3 or more),
4 the subquest's rewards. How many draws each slot gets is not in these files.
Placeholder quests (as in quest_sizes.py) and the debug ones without text are left out.

quest-rewards.csv: quest_id, slot, rem. rewards.csv: rem, item, count, chance, in file
order (an item can come twice, with different counts).
"""
import csv, os, re, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc, gmd
from quest_sizes import PLACEHOLDERS


def table(d):
    n = d[9]
    return [struct.unpack_from('<HBB', d, 24 + 4 * k) for k in range(n)]


def main(romfs):
    R = RomFS(romfs)
    links, tables = [], {}
    for p in sorted(R.files):
        m = re.match(r'/nativeNX/loc/arc/quest/q(\d{7})\.arc$', p)
        if not m:
            continue
        text = '/nativeNX/eng/quest/questData/questData_%s_eng.gmd' % m.group(1)
        if text not in R.files or gmd(R.read(text))[0] in PLACEHOLDERS:
            continue
        q = arc(R.read(p))
        rems = re.findall(rb'rem_(\d{6})', q['loc\\quest\\questLink\\questLink_%s' % m.group(1)])
        assert len(rems) == 5, (p, rems)
        for slot, r in enumerate(rems):
            if r == b'000000':
                continue
            r = int(r)
            links.append((int(m.group(1)), slot, r))
            t = table(q['quest\\rem\\rem_%06d' % r])
            assert tables.setdefault(r, t) == t, r
    out = os.path.join(here, '..', 'data')
    with open(os.path.join(out, 'quest-rewards.csv'), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['quest_id', 'slot', 'rem'])
        w.writerows(links)
    with open(os.path.join(out, 'rewards.csv'), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['rem', 'item', 'count', 'chance'])
        w.writerows((r,) + row for r in sorted(tables) for row in tables[r])
    print('->', len(links), 'quest slots,', len(tables), 'tables')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
