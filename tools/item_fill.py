#!/usr/bin/env python3
"""Write data/item-fill.csv: the category of every item the item box can hold, for the
Items page's Fill box dialog. Pure stdlib; reads a RomFS dump like build_assets.py.

    item_fill.py [ROMFS]     default scratch/base_romfs.bin

From the game's item table (loc/arc/resident.arc table\\itemData, 44-B records after an
8-B header; record k = item ID k):
  +4   type: 0 item, 1 ammo, 3 decoration, 4 coating, 5 Kinsect extract
  +5   rarity - 1
  +9   quest items (Royal Honey, Unique Mushroom…)          ┐ none of these was ever in
  +10  supply items (First-aid Med, Map, Throwing Knife…)  │ the box of a save the game
  +13  appraised when a quest ends (charms, shards, "Item"s) │ wrote: left out
  +18  carried in both hands (eggs, heavy ores)             ┘
  +16  icon (ore, bone, herb, bug, fish, potion, ticket, Palico scrap…)
Decorations by size from table\\decoData (5-B records, record k = item 2638 + k).

Monster materials by rank: the lowest rank that gives the item, from
data/monster-carves.csv and the quest rewards (data/quest-rewards.csv, rewards.csv,
quest-index.csv: Village 1-6 and Hub 1-3 Low rank, Village 7-10 and Hub 4-7 High rank,
G1-G4 G rank, Special Permits deviant). Items only deviants give, or named after one,
are deviant; "Hyper ..." items hyper; the few without a source go by rarity (1-5 Low,
6-7 High, 8+ G).

Columns: item, category. DERIVED: categories are an editor grouping, not a game field.
"""
import collections, csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc

DATA = os.path.join(here, '..', 'data')
DEVIANTS = ('Dreadqueen', 'Dreadking', 'Deadeye', 'Stonefist', 'Grimclaw', 'Silverwind', 'Crystalbeard', 'Thunderlord',
            'Redhelm', 'Snowbaron', 'Drilltusk', 'Hellblade', 'Bloodbath', 'Rustrazor', 'Nightcloak', 'Boltreaver',
            'Soulseer', 'Elderfrost')
GATHERED = {9: 'ore', 14: 'bone', 1: 'plant', 3: 'plant', 8: 'plant', 31: 'plant', 93: 'plant', 94: 'plant', 6: 'bug',
            2: 'fish', 18: 'fish', 92: 'fish'}
HEAL = {22, 29, 33, 51}
TOOL = {7, 15, 16, 19, 20, 21, 23, 24, 25, 26, 27, 28, 30, 34, 35, 37, 38, 39, 42, 105}
MONSTER = {5, 10, 11, 12, 13, 17, 50}
TRADE = {40, 41, 43, 44, 52, 71}
RANKS = ['low', 'high', 'g']


def rows(name):
    with open(os.path.join(DATA, name), newline='') as f:
        return list(csv.DictReader(f))


def quest_rank(q):
    c, k = q['category'], q['rank']
    if c == 'Special Permit':
        return 3
    if not k:
        return None
    if k.startswith('G'):
        return 2
    k = int(k)
    return (0 if k <= 6 else 1) if 'Village' in c else (0 if k <= 3 else 1)


def main(romfs):
    R = RomFS(romfs)
    res = arc(R.read('/nativeNX/loc/arc/resident.arc'))
    reng = arc(R.read('/nativeNX/eng/arc/resident_eng.arc'))
    from build_assets import gmd
    names = gmd(reng['eng\\table\\itemData_eng'])[0::2]
    d = res['table\\itemData']
    recs = [d[8 + 44 * k:8 + 44 * k + 44] for k in range(struct.unpack_from('<I', d, 4)[0])]
    dd = res['table\\decoData']
    deco = {2638 + k: dd[8 + 5 * k] for k in range(struct.unpack_from('<I', dd, 4)[0])}

    monsters = {int(r['index']): r['monster'] for r in rows('monster-index.csv')}
    deviant_m = {k for k, v in monsters.items() if v and v.split()[0] in DEVIANTS}
    carve, by_deviant = {}, collections.defaultdict(set)
    for r in rows('monster-carves.csv'):
        i = int(r['item'])
        carve[i] = min(carve.get(i, 9), RANKS.index(r['rank']))
        by_deviant[i].add(int(r['monster']) in deviant_m)
    quests = {r['quest_id']: r for r in rows('quest-index.csv')}
    rem = collections.defaultdict(set)
    for r in rows('rewards.csv'):
        rem[r['rem']].add(int(r['item']))
    reward = {}
    for r in rows('quest-rewards.csv'):
        k = quest_rank(quests[r['quest_id']]) if r['quest_id'] in quests else None
        if k is not None:
            for i in rem.get(r['rem'], ()):
                reward[i] = min(reward.get(i, 9), k)

    def category(i, b, name):
        t, icon = b[4], b[0x10]
        if b[9] or b[10] or b[13] or b[18]:
            return None
        if t == 3:
            return 'deco%d' % deco[i] if deco.get(i) in (1, 2, 3) else None
        if t in (1, 4, 5):
            return {1: 'ammo', 4: 'coating', 5: 'extract'}[t]
        if icon == 49:
            return 'scrap'
        if icon in (36, 87):
            return 'ticket'
        if icon in TRADE:
            return 'trade'
        if icon in GATHERED:
            return GATHERED[icon]
        if icon in HEAL:
            return 'heal'
        if icon == 4:
            return 'food'
        if icon in TOOL:
            return 'tool'
        if icon in MONSTER or i in carve:
            if name.split()[0] in DEVIANTS or by_deviant.get(i) == {True} or (reward.get(i) == 3 and i not in carve):
                return 'deviant'
            if name.startswith('Hyper '):
                return 'hyper'
            k = min(carve.get(i, 9), reward.get(i, 9) if reward.get(i) != 3 else 9)
            if k == 9:
                r = b[5] + 1
                k = 0 if r <= 5 else 1 if r <= 7 else 2
            return RANKS[k]
        return 'other'

    out = []
    for i, b in enumerate(recs):
        name = names[i] if i < len(names) else ''
        if not 1 <= i <= 2990 or name in ('', 'DUMMY', '(None)', '(No Item)') or name.startswith('-'):
            continue
        c = category(i, b, name)
        if c:
            out.append((i, c))
    with open(os.path.join(DATA, 'item-fill.csv'), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['item', 'category'])
        w.writerows(out)
    print(len(out), dict(collections.Counter(c for _, c in out)))


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
