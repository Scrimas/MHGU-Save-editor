#!/usr/bin/env python3
"""Write data/unlock-lists.csv: the entries of the character's unlock maps (Soaratorium
Lab, Jukebox, Provision Division, Cross coin trades, Poogie and Moofy costumes, the
Trader, the Market and Guild Store, the Armory) and what unlocks each in game. Pure
stdlib; reads a RomFS dump like build_assets.py.

    unlock_lists.py [ROMFS]     default scratch/base_romfs.bin

Tables (f32 version, u32 count, packed records; docs/11-save-map.md, S fields):
  table/researchReinforce.ots   25-B records: u32 index, u16 ID (= bit + 1), u16 category,
                                u16, u32 cond 1, u32 cond 2 (0x525040), u32 Wycademy
                                points, u16 item, u8 count. Offered when cond 1 or cond 2
                                holds; installing needs the entry offered (0x66cac8)
  table/coinTradeList.ctl       11-B records: u32 item, u8 Horns Coins, u32 cond
                                (0x5614c0), u16 event flag (0 = none)
  table/otodokeSetList.ots      33-B records; set 2 is unlocked when the Provision
                                Division first opens (0x191374), set T[i] follows Lab
                                upgrade 3 + i (0x1914a0, T = 1, 3, 4 ... 24 at 0x1625328)
  table/lobby/tradePointItemList.tpil, tradePointItemSpList.tpil
                                8-B records: u16 item, u16 cond (0x561b28), u32 points
  table/lobby/tradeLimitedHonorList.tlil, tradeLimitedPaperList.tlil,
  tradeLimitedClothList.tlil    8-B records: u16 title word / scene / costume ID, u16
                                kind (0 always, 1 a download, 2 Hub star 9 for words;
                                scenes have progress kinds), u32 price
  table/lobby/tradeDeliveryList.trdl
                                36-B records: u32 NPC, u32 request index, ...
  loc/arc/village/common.arc    shopList00 (Market), shopList01 (Guild Store): 16-B
                                records u32 index, u32 item, u32 cond 1, u32 cond 2;
                                equipShopListWNN (class NN) / A00 (all five armor parts):
                                10-B records u16 weapon ID / armor series, u32 cond 1,
                                u32 cond 2 (0x5614c0). The shops list an entry while its
                                condition holds, whatever its listed bit (0x72ef60,
                                0x6eb9fc): those maps only record NEW marks
  loc/arc/resident.arc          itemPreData: the Combination List, one bit per recipe
Jukebox songs (14, 0x7304d8), Poogie / Moofy costumes (40, 0x50d830), the Housekeeper's
Gallery (14, 0x1650ae8), the Hunter's Notes second list (30, 0x162d5d0) and tips (16,
0x1631e74), the one-time event scenes (24, 0x15a0c78) and the milestone map the award
checks read (S +0xd8c) have no table in RomFS: they are copied below from the code.

Columns: map, bit, id (the item, word, scene, costume, equipment or request the entry
stands for; empty when the bit is the ID), need: what unlocks it in game, tokens joined
by '|' (any of) and '&' (all of):
  start           from the start        never          the game never sets it
  v<N> / h<N>     Village / Hub star N  hr             the HR limit released
  q<ID>           quest ID cleared      f<N>           event flag N raised
  dlc             a download held       ex             a deviant's EX level (and a
                                                       Hunter Armor Fusion upgrade)
  lab<B>          Lab upgrade bit B installed          open    opening the facility
  adopt           adopting the pet      talk           a villager's gift
  item<ID>        item obtained         trader         bought at the Trader
  visit           the first visit       view           viewing it in game
  combine         combining it once     progress       story progress the editor
                                                       does not decode further
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc, gmd


def table(d, stride):
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + n * stride, (len(d), n, stride)
    return [d[8 + stride * i:8 + stride * (i + 1)] for i in range(n)]


def any_of(*needs):
    """'a|b' of the needs that can hold; 'start' wins, 'never' drops out."""
    if 'start' in needs:
        return 'start'
    out = [n for n in needs if n != 'never']
    return '|'.join(dict.fromkeys(out)) or 'never'


def star(c):
    """The common condition (0x5614c0)."""
    if c == 0 or c > 25:
        return 'start'
    if c == 1:
        return 'never'
    if c <= 11:
        return 'v%d' % (c - 1)
    if c <= 24:
        return 'h%d' % (c - 11)
    return 'hr'


# Lab upgrade conditions (0x525040)
LAB = {0: 'start', 1: 'never', 2: 'f1008', 3: 'f1033', 4: 'f130', 5: 'q806', 6: 'q906',
       7: 'q1005', 8: 'q11204', 9: 'q11319', 10: 'q11401', 11: 'hr&f1303', 12: 'ex',
       13: 'q806&q10768', 14: 'q906&q11204', 15: 'f1303'}

# Trader item list conditions (0x561b28)
TRADER = {0: 'start', 1: 'never', 2: 'v3|h2', 3: 'v4|h2', 4: 'v5|h3', 5: 'v8|h5',
          6: 'v9|h6', 7: 'h8', 8: 'h8&f33', 9: 'h8&f711', 10: 'f39', 11: 'f161',
          12: 'f211', 13: 'f261', 14: 'h9', 15: 'h10', 16: 'h11', 17: 'f1051',
          18: 'f1051&f1418'}

# Jukebox: bits 0-4 and 9 from the start (0x523b9c), the rest by the Mewstress's talk
# action 23 (0x3f420c) once its condition holds
SONGS = ['start'] * 5 + ['q908', 'q909', 'q907', 'q910', 'start', 'f1051', 'f1051',
                         'f1217|f1219', 'f1217|f1219']

# Costumes: 0-5 Moofy, 6-39 Poogie. Adoption gives the pet's default one (0x6bf3c8),
# talk action 2 type 22 gives one (0x3ef97c), item 2634 obtained gives 18 (0x75e0cc),
# the Trader sells the rest (tradeLimitedClothList)
COSTUMES = {0: 'adopt', 6: 'adopt', 19: 'adopt', 23: 'adopt', 18: 'item2634'}
for b in [1, 4, 5, 7, 8, 9, 10, 11, 13, 14, 15, 17, 20, 21, 22, 24, 25, 29, 32, 33, 37, 38]:
    COSTUMES[b] = 'talk'

# Gallery movies: 0, 1, 7, 11 at character creation (0x5221c4); the monster movies with
# their quest (0x14d53c); 2 after quest 601 is cleared (0x524064); 9, 10, 12, 13 with the
# ending (0x12074)
GALLERY = ['start', 'start', 'q601', 'q601', 'q504', 'q510', 'q508', 'start', 'q1005',
           'progress', 'progress', 'start', 'progress', 'progress']

# Hunter's Notes second list: entry -> large-list Notes bit (0x162d5d0) and the quest whose
# clear lets talk action 7 unlock it (0x55515c; first alternative only)
NOTES2 = [(38, 'q40801'), (44, 'q40201'), (46, 'q40101'), (53, 'q40501'), (59, 'q40301'),
          (72, 'q40701'), (76, 'q41201'), (78, 'q40601'), (80, 'q40401'), (83, 'q40901'),
          (87, 'q41101'), (94, 'q41001'), (95, 'q618'), (96, 'q620'), (97, 'q633'),
          (98, 'q634'), (99, 'q635'), (100, 'q1039'), (101, 'q1019'), (102, 'f126'),
          (105, 'q41611'), (106, 'q41511'), (107, 'q41711'), (108, 'q41811'),
          (109, 'q41411'), (110, 'q41311'), (119, 'q11319'), (120, 'q11454'), (103, 'q1005'),
          (104, 'q11432')]

# Hunter's Notes tips: bit -> tip ID (HN_HunterTipsMsg); the bit is set once read
TIPS = [64, 70, 71, 72, 85, 99, 108, 160, 139, 140, 154, 102, 143, 152, 86, 87]

# One-time event scenes (0x15a0c78): bit -> event ID (arc/ev/vNNN.arc) and when it plays
# (0x14eb08): first visits; Village star 4; five of the first urgent quests; the Meownster
# Hunters' stages; the Wycademy after quests 716 and 901. Event 212's trigger is unknown
EVENTS = [(1, 'visit'), (2, 'visit'), (3, 'visit'), (4, 'visit'), (5, 'visit'), (6, 'visit'),
          (100, 'progress'), (101, 'progress'), (102, 'progress'), (103, 'v4'), (104, 'v4'),
          (105, 'v4'), (106, 'v4'), (107, 'progress'), (108, 'progress'), (109, 'progress'),
          (110, 'progress'), (212, 'progress'), (7, 'visit'), (8, 'visit'), (111, 'progress'),
          (112, 'visit'), (113, 'q716'), (114, 'q901')]

# Milestones the award checks read (S +0xd8c): 0-4 an armor part (id 1-5) and 5-19 a
# weapon class (id 0-14, class 5 unused) upgraded to its max level (0x70301c; awards 29,
# 28); 20-23 a Moofy / Poogie moment in village id 1-4 (awards 48, 69, 84, 99); 24-34 a
# Footbath guest, id = NPC (0x162900c; award 88); 35-41 a Palico of support bias id hired
# (award 51); 42-56 a weapon class id upgraded past a second level threshold (no
# reader); 57-64 one of bias id at level 99 (award 128)
MILESTONES = ([(b, b + 1) for b in range(5)] + [(5 + c, c) for c in range(15) if c != 5]
              + [(20 + v, v + 1) for v in range(4)]
              + list(zip(range(24, 35), [323, 331, 420, 421, 422, 423, 424, 425, 426, 428, 427]))
              + [(35 + k, k) for k in range(7)] + [(42 + c, c) for c in range(15) if c != 5]
              + [(57 + k, k) for k in range(8)])

# Trader title word entries of a download the catalog does not list (EXE 0x1596500):
# nothing can set them
WORDS_UNSET = {390, 391}

DUMMY = ('', 'DUMMY', '@', '---')

WEAPON_CLASSES = [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14]


def main(romfs):
    R = RomFS(romfs)
    t = lambda p: R.read('/nativeNX/' + p)
    rows = []

    for r in table(t('table/researchReinforce.ots'), 25):
        i, ident, _, _, c1, c2 = struct.unpack_from('<IHHHII', r)
        assert ident == i + 1
        rows.append(('lab', i, '', any_of(LAB[c1], LAB[c2])))

    for b, need in enumerate(SONGS):
        rows.append(('song', b, '', need))

    sets = table(t('table/otodokeSetList.ots'), 33)
    lab_of = {1: 3}
    lab_of.update({s: 1 + s for s in range(3, len(sets))})   # T = 1, 3, 4 ... 24
    for b in range(1, len(sets)):
        rows.append(('supply', b, '', 'open' if b == 2 else 'lab%d' % lab_of[b]))

    for b, r in enumerate(table(t('table/coinTradeList.ctl'), 11)):
        item, _, c, flag = struct.unpack_from('<IBIH', r)
        need = star(c)
        if flag:
            need = 'f%d' % flag if need == 'start' else '%s&f%d' % (need, flag)
        rows.append(('coin', b, item, need))

    words = gmd(t('eng/table/GC_Title_1_eng.gmd'))
    scenes = gmd(t('eng/table/GC_background_eng.gmd'))
    cloth = table(t('table/lobby/tradeLimitedClothList.tlil'), 8)
    sold = {struct.unpack_from('<HH', r)[0]: struct.unpack_from('<HH', r)[1] for r in cloth}
    for b in range(40):
        need = COSTUMES.get(b) or ('dlc' if sold.get(b) == 1 else 'trader')
        assert b in COSTUMES or b in sold, b
        rows.append(('costume', b, '', need))

    for k, name in enumerate(['tradePointItemList', 'tradePointItemSpList']):
        for b, r in enumerate(table(t('table/lobby/%s.tpil' % name), 8)):
            item, c = struct.unpack_from('<HH', r)
            rows.append(('trader:items%d' % k, b, item, TRADER[c]))
    for b, r in enumerate(table(t('table/lobby/tradeLimitedHonorList.tlil'), 8)):
        w, kind = struct.unpack_from('<HH', r)
        need = 'never' if words[w] in DUMMY or b in WORDS_UNSET else ['start', 'dlc', 'h9'][kind]
        rows.append(('trader:words', b, w, need))
    for b, r in enumerate(table(t('table/lobby/tradeLimitedPaperList.tlil'), 8)):
        s, kind = struct.unpack_from('<HH', r)
        need = 'never' if scenes[s] in DUMMY else {0: 'start', 1: 'dlc'}.get(kind, 'progress')
        rows.append(('trader:scenes', b, s, need))
    for b, r in enumerate(cloth):
        c, kind = struct.unpack_from('<HH', r)
        rows.append(('trader:costumes', b, c, ['start', 'dlc'][kind]))
    for b, r in enumerate(table(t('table/lobby/tradeDeliveryList.trdl'), 36)):
        _, req = struct.unpack_from('<II', r)
        rows.append(('trader:delivery', b, req, 'progress'))

    for b, need in enumerate(GALLERY):
        rows.append(('gallery', b, '', need))
    for b, (notes, need) in enumerate(NOTES2):
        rows.append(('notes2', b, notes, need))
    for b, tip in enumerate(TIPS):
        rows.append(('tips', b, tip, 'view'))
    for b, (ev, need) in enumerate(EVENTS):
        rows.append(('events', b, ev, need))
    for b, ident in MILESTONES:
        rows.append(('milestones', b, ident, 'progress'))
    pre = arc(t('loc/arc/resident.arc'))['table\\itemPreData']
    for b in range(struct.unpack_from('<I', pre, 4)[0]):
        rows.append(('combos', b, '', 'combine'))

    v = arc(t('loc/arc/village/common.arc'))
    for k in range(2):
        for b, r in enumerate(table(v['table\\shopList%02d' % k], 16)):
            i, item, c1, c2 = struct.unpack_from('<4I', r)
            assert i == b
            rows.append(('shop:%d' % k, b, item, any_of(star(c1), star(c2))))
    armory = {p: 'equipShopListA00' for p in range(1, 6)}
    armory.update({w + 7: 'equipShopListW%02d' % w for w in WEAPON_CLASSES})
    for typ in sorted(armory):
        for b, r in enumerate(table(v['table\\' + armory[typ]], 10)):
            eid, c1, c2 = struct.unpack_from('<HII', r)
            rows.append(('armory:%d' % typ, b, eid, any_of(star(c1), star(c2))))

    out = os.path.join(here, '..', 'data', 'unlock-lists.csv')
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['map', 'bit', 'id', 'need'])
        w.writerows(rows)
    print('->', out, len(rows), 'rows')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
