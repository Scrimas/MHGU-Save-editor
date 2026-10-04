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
| 100–101 | no visible award, see below | 2 |
| 102–131 | Soaratorium | 30 |

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
| 7, 8 | `0x3f3624` / `0x3f3824` with 66: a count over the 87 monster list entries, one per crown size by the look of it (**UNRESOLVED**) |
| 9 | `0x3f1b1c`, not read |
| 10, 11, 12 | sets 45, 46, 47: every Arena quest cleared, all with rank A, all with rank S |

The check runs after every quest and needs no "last clear": on the first quest after
the [bulk completion write](05-quests.md#what-all-quests-completed-takes), a Harvest
Tour, this map and its notice copy gained 0–6, 10, 59, 74, 89, 103 and 109–114 at
once. 11 and 12 stayed clear with the rank sets, 7–9 do not depend on quests.

Whether the Guild Card copy is rebuilt from this map, and when, was not tested; it
was already full when these awards arrived.

## Open questions

- **UNRESOLVED — Bherna's 26th slot**, *A Pat on the Back* ("your first step into
  the world of monster hunting", crossed-swords icon, earned). Bherna's first 25
  slots fill bits 30–54 and Kokoto starts immediately at 55, so slot 26 is not in the
  Bherna run. It is **not** bit 100 or 101 either: clearing bit 101 and setting bit
  100 changed nothing on any of the six grids. Possibly granted unconditionally or
  stored elsewhere.
- **UNRESOLVED — bits 100–101.** No visible effect in either state. The analysed save
  has 100 = 0 and 101 = 1; leave them as found.
- **UNRESOLVED — award names/IDs.** Only positions are mapped; names come from the
  in-game descriptions.
