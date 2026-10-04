# 09 — Awards

The Guild Card's award grid is one LSB-first bitfield: one bit per award, set once
earned. Neither save editor maps it. Its MHXX 3DS offset (`0x1B8A`, 13 bytes) is
commented out in their `Offsets.cs` and reads as all zeros on Switch.

| Structure | Offset | Size | Valid bits | Confidence |
|---|---|---|---|---|
| Awards earned | `base + 0xC8115` (slot 1: `0x254DB1`) | 17 bytes | 0–131 (132) | CONFIRMED |

Offsets are **relative to the character base** (see
[07 — Equipment § Character slots](07-equipment.md#character-slots)). The field sits
inside the Guild Card block (`base + 0xC71BD`), directly after the last quest-history
record, and is followed by zero bits 132–135. The game's award test `0x162720` accepts
bit numbers below 160, so the card reserves 20 bytes (`+0xF58` of the 6328-byte card,
[11](11-save-map.md#card-layout-6328-b)); bits 132–159 are unused.

## Layout

The in-game award screen has one grid per location. Each grid is stored as a
contiguous run, in on-screen reading order (left to right, top to bottom):

| Bits | Grid | Slots |
|---|---|---|
| 0–29 | Wycademy | 30 |
| 30–54 | Bherna | 25 of 26, see below |
| 55–69 | Kokoto | 15 |
| 70–84 | Pokke | 15 |
| 85–99 | Yukumo | 15 |
| 100–101 | Bherna slot 26: one of the two, see below | 2 |
| 102–131 | Soaratorium | 30 |

**Names.** Bit *i* is entry *i* of `table/GC_Medal_eng.gmd` (132 names, then the 132
descriptions in the same order). Bit 0 is *Bherna Chief's Ornamental Belt*, as the
controlled write showed, bit 34 *Crest of Generations* (the Fated Four,
[05](05-quests.md)), bit 68 *Poogie Ball*. All 132:
[`data/awards.csv`](../data/awards.csv). **DERIVED.**

**Bits 100 and 101 share Bherna's 26th slot.** Bit 101 is *A Pat on the Back* ("your
first step into the world of monster hunting"), bit 100 *Veteran Hunter's Prize*, whose
description prints the total play time and Hunter Rank of a *Monster Hunter
Generations* save. The game grants one of them once (`0x3ea570`, called next to the
save transfer code): 101, or 100 when the source byte it tests is negative. The card
screen (`0x5a88f8`) shows bit 100 in that slot when the card's flag byte `+0x8B8` is
negative (bit 7 set) and bit 101 otherwise. **DERIVED** from code.

```
earned(bit) = (buf[base + 0xC8115 + (bit >> 3)] >> (bit & 7)) & 1
```

## Evidence

Known-plaintext match. Each of the six in-game grids was read as a bit vector
(lit = 1, empty = 0) and searched for in the save. All six appear, in grid order,
inside one 132-bit window. Every bit agrees, and the field's popcount (74) equals the
number of lit slots across all six screens. A 131-slot vector matching by chance is
not plausible.

**CONFIRMED** by a controlled write. Setting bit 0 made Wycademy slot 1 appear
in-game as *Bherna Chief's Ornamental Belt* ("completed all 1★ and 2★ Village
Quests"), and no other award appeared or vanished. The bit was then cleared again.

**Full-field write.** Setting bits 0–99 and 102–131 (bits 100–101 left as found)
lit every slot on all six grids, with no empty or `?` slot left. This checks the
layout for every slot, including those not earned in the analysed save:

```
before: 00e0ffe7697f90d1dffa67f5631e803f04   (74 bits)
after:  ffffffffffffffffffffffffefffffff0f   (131 bits)
```

**`?` slots are computed, not stored.** Once bit 0 was set, Wycademy slots 2 and 3
turned from empty to `?`. A `?` marks the next tier of an award series already
started, so it follows from the earned bits and has no bit of its own. Wycademy
slot 29 showed `?` before the write and stores 0.

## Game-side map — `base + 0x3157`

**DERIVED (code + values).** The Guild Card field above is not the only award map. The
game keeps its own at `base + 0x3157` (`+0xc28` of the save object, 20 bytes, same bit
order). The award check (`0x3ec020` onwards) sets a bit there and in two companion
maps (`+0xc3c`, `+0xc50`) when its condition holds. Of those only `+0xc50` is saved,
at `base + 0x316B`; it drives the "new award" notice. Its first tests read the
[quest set map](05-quests.md#quest-sets--base--0x3187): sets 13 and 14 complete give
award 0, *completed all 1★ and 2★ Village Quests*, sets 15 and 16 award 1.

In the analysed save this map holds the field's value from before the full-field
write, plus the two awards earned since (34 and 83), and those two are exactly the
bits set in the notice copy at `base + 0x316B`:

```
base+0x3157: 00e0ffe76d7f90d1dffa6ff5631e803f04 000000
base+0x316B: 0000000004000000000008000000000000 000000
```

The first thirteen awards, **DERIVED** from the code:

| Award | Condition |
|---|---|
| 0–5 | pairs and triples of the Village and Hub level sets 13–25 (0 = sets 13 and 14, 1 = 15 and 16, …) |
| 6 | awards 0–5 all earned (the check reads its own map: `& 0x7f == 0x3f`) |
| 7, 8 | *Miniature Crown* / *Large Crown*: `0x3f3624` / `0x3f3824` count the monsters of the 87-entry monster list whose size record carries the gold crown mark (`0x67310` returns 3), skipping a few codes; the award needs 66 |
| 9 | *Bionomical Report*, "captured" monsters: `0x3f1b1c`, a loop over monster codes, not read in detail |
| 10, 11, 12 | sets 45, 46, 47: every Arena quest cleared, all with rank A, all with rank S |

The check runs after every quest and needs no "last clear": on the first quest after
the [bulk completion write](05-quests.md#what-all-quests-completed-takes), a Harvest
Tour, this map and its notice copy gained 0–6, 10, 59, 74, 89, 103 and 109–114 at
once. 11 and 12 stayed clear with the rank sets, 7–9 do not depend on quests.

The Guild Card copy is not rebuilt from this map but OR-ed with it: the card update
`0x161ac8` sets every card bit whose game-side bit is set (`0x161b98`, bits 0–131) and
clears nothing. So a bit written to the card stays (the full-field write survived),
while a card bit cleared by an edit comes back if the game-side bit is set.

## Open questions

- **RESOLVED — Bherna's 26th slot is bit 101** (*A Pat on the Back*), or bit 100 on a
  transferred card, see [Layout](#layout). The earlier test, clearing bit 101 and
  setting bit 100 in the card copy, changed nothing for two reasons the code shows:
  the card update ORs the game-side map back in (bit 101 is set there), and the slot
  shows bit 100 only when the card's flag byte `+0x8B8` has bit 7 set. To remove the
  award, clear bit 101 in both maps (`base + 0x3157` and the card). **DERIVED**, not
  re-tested.
- **RESOLVED — award names**, from `GC_Medal_eng.gmd` ([`data/awards.csv`](../data/awards.csv)).
