#!/usr/bin/env python3
"""Write the Palico tables the editor checks support moves and skills against. Pure
stdlib; reads a RomFS dump like build_assets.py.

    palico_tables.py [ROMFS]     default scratch/base_romfs.bin

Tables (loc/arc/resident.arc; f32 version, u32 count, packed records):
  otomo/ot_lvl              cOtLevel, 15 B per level 1-99: u16 mVital, mAttack, mDefence,
                            u32 mTotalExp, u8 mFreeSupportSlot (+10), mMaxOtomoSkillSlot (+11),
                            mMaxTension, mMaxSupportGauge, mDonguriVital
  otomo/ot_skl              cOtSkill, 11 B: u32 ID, u8 ID, mSlotCost (+5), mLearnLevel, mSkillKind
  table/lobby/otLotOwnSupport  per forte: u16 forte, 3 x u16 moves: the first is the innate
                            move, the second innate move is drawn from the other two (none
                            for Charisma)
  table/lobby/otLotOwnSkill    per forte: u16 forte, 2 x u16 innate skills
  table/lobby/otSupportIni{1,2,3}pt, otSkillIni{1,2,3}pt   the moves and skills drawn
                            for the random part of the lists, worth 1, 2 or 3 points: u16
                            ID, 4 x u8 weights
A new Palico's move list (record +0x38, length +0x55) is its innate moves, Mini Barrel
Bombay and Herb Horn, random moves whose points follow one pattern of otSupportPoint
(otSupportPointSp for Charisma; pattern index +0x54), then 2 empty slots for taught moves
(3 for Charisma). Its skill list (+0x48, length +0x57) is its innate skills, random
skills by otSkillPoint (index +0x56), then 2 empty taught slots. 30 of the 32 Palicoes of
the analysed save fit this exactly; the other two are special Palicoes with no random
part. Equip limits, from the code (0xe8478, 0xe9338): mFreeSupportSlot + 2 support moves
(+1 with skill 21, Support Move +1); equipped skills' mSlotCost at most mMaxOtomoSkillSlot.

Looks. A Palico record keeps a character-creation block at +0x10E (12 B, the hunter's
layout: +0 weapon class 15 = Prowler) and 9 RGBA colours at +0x11A. A new Palico's looks
(0x25aee0, the stores from 0x25b5e0) are drawn from the 9 lottery rows of
table/lobby/otParamLot (16 weights each): coat +6, coat colour 0, clothing +3, clothing
colour 3, eyes +2, eye colour 1 (and 2: the right eye differs 5 times in 100), ears +7,
tail +8, voice +1 (drawn + 1). The models agree: otomo/mod/skin has 7 coats, otomo/mod/eye
6 eyes, otomo/vo 3 voices (01-03). Ears and tail are named by the order of the DLC Palico
fields (cDLCOtomoInfo: mOtomoHair, mOtomoVoiceType, mOtomoEye, mOtomoEar, mOtomoTail);
both have 5 choices. The colour palettes are runtime tables filled by 0x28a668 (copied
below, read by running it).

Writes:
  data/palico-levels.csv   level, support_slots (mFreeSupportSlot), skill_slots
  data/palico-actions.csv  kind (move, skill), id, points (1-3 when drawn at random, else
                           0), cost (skills: slot cost)
  data/palico-fortes.csv   forte, move (the innate move), moves2 (the second innate move's
                           choices), skills (the two innate skills)
  data/palico-looks.csv    look, slot (byte of the block, or colour:<n>), choices (values
                           0..choices-1; voice 1..choices), palette (RRGGBB colours)
"""
import csv, os, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from build_assets import RomFS, arc


# 0x28a668: the colours new Palicoes are drawn from (RGB; alpha always 0xFF)
COAT_COLOURS = 'f0f0f0 fa746a f4a024 fae96e a17345 7bfcb3 5a88e4 9dc6e7 aa96e6 f38dc0 6b6f8b e2e5d3 302e48'
CLOTHING_COLOURS = 'f0f0f0 c93036 e1711e ffec0f 96ff46 706541 2e627d 9569e2 7d1834 ff468e a1764f fae4d9 3d363e 8ba150'
EYE_COLOURS = '71deff edd400 6a491b 73c876 0b9397 ed8740 9f79e0 df99d6 ebf5ff'
# otParamLot row -> (look, slot, palette)
LOOKS = [('coat', '6', ''), ('coat_colour', 'colour:0', COAT_COLOURS), ('clothing', '3', ''),
         ('clothing_colour', 'colour:3', CLOTHING_COLOURS), ('eyes', '2', ''), ('eye_colour', 'colour:1', EYE_COLOURS),
         ('ears', '7', ''), ('tail', '8', ''), ('voice', '1', '')]


def records(d, size):
    n = struct.unpack_from('<I', d, 4)[0]
    assert len(d) == 8 + n * size, (len(d), n, size)
    return [d[8 + size * i:8 + size * (i + 1)] for i in range(n)]


def write(name, head, rows):
    out = os.path.join(here, '..', 'data', name)
    with open(out, 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(head)
        w.writerows(rows)
    print('->', out, len(rows), 'rows')


def main(romfs):
    t = arc(RomFS(romfs).read('/nativeNX/loc/arc/resident.arc'))
    lv = records(t['otomo\\ot_lvl'], 15)
    assert len(lv) == 99
    write('palico-levels.csv', ['level', 'support_slots', 'skill_slots'], [(k + 1, r[10], r[11]) for k, r in enumerate(lv)])

    points = {}
    for kind, name in (('move', 'otSupportIni%dpt'), ('skill', 'otSkillIni%dpt')):
        for p in (1, 2, 3):
            for r in records(t['table\\lobby\\' + name % p], 6):
                points[kind, struct.unpack_from('<H', r)[0]] = p
    moves = len(records(t['otomo\\support\\spt_act_base'], 24))
    skills = records(t['otomo\\ot_skl'], 11)
    assert [struct.unpack_from('<I', r)[0] for r in skills] == list(range(len(skills)))
    rows = [('move', k, points.get(('move', k), 0), '') for k in range(moves)]
    rows += [('skill', k, points.get(('skill', k), 0), r[5]) for k, r in enumerate(skills)]
    write('palico-actions.csv', ['kind', 'id', 'points', 'cost'], rows)

    om = records(t['table\\lobby\\otLotOwnSupport'], 8)
    os_ = records(t['table\\lobby\\otLotOwnSkill'], 6)
    rows = []
    for f in range(8):
        a, b, c = struct.unpack_from('<3H', om[f], 2)
        assert struct.unpack_from('<H', om[f])[0] == f and struct.unpack_from('<H', os_[f])[0] == f
        rows.append((f, a, ' '.join(str(x) for x in sorted({b, c}) if x), ' '.join(map(str, struct.unpack_from('<2H', os_[f], 2)))))
    write('palico-fortes.csv', ['forte', 'move', 'moves2', 'skills'], rows)

    rows = []
    for r, (look, slot, pal) in zip(records(t['table\\lobby\\otParamLot'], 16), LOOKS):
        n = max(k for k in range(16) if r[k]) + 1
        assert not pal or len(pal.split()) == n, (look, n)
        rows.append((look, slot, n, pal))
    write('palico-looks.csv', ['look', 'slot', 'choices', 'palette'], rows)


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'scratch', 'base_romfs.bin'))
