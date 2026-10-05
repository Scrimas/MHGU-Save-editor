# 08 — Hunter Arts and Canteen

Unlock flags for Hunter Arts, Canteen ingredients and Canteen dishes. All three are
plain LSB-first bitfields inside the character block, and all three were edited and
checked in-game.

Offsets are **relative to the character base** (see
[07 — Equipment § Character slots](07-equipment.md#character-slots)). In the analysed
save, slot 1's base is `0x18CC9C`.

| Structure | Offset | Size | Valid bits | Confidence |
|---|---|---|---|---|
| Hunter Arts unlocked | `base + 0x2C13` | 24 bytes | IDs 1–70, 83–190 (178) | CONFIRMED |
| Canteen ingredients | `base + 0x2F8F` | 6 bytes | 0–44 (45) | CONFIRMED |
| Canteen dishes | `base + 0x2C67D` | 13 bytes | 0–98 (99) | CONFIRMED |
| Dishes viewed (NEW marks) | `base + 0x2C68D` | 13 bytes | 0–98 | DERIVED |

## Hunter Arts — `base + 0x2C13`

**CONFIRMED.** 192 bits, one per Hunter Art ID. Bit `n` = art ID `n` is unlocked.

Where the layout comes from: an MHXX 3DS memory cheat ("unlock all Hunter Arts")
writes the six u32 values below through the same pointer that its money and
Wycademy-point cheats use. Those two writes land 8 bytes apart, like funds
(`base + 0x280F`) and academy points (`base + 0x2817`) in the save. The RAM struct is
padded differently from the save, so the arts offset could not be shifted across
directly. Instead, the save was searched for a 24-byte window that fits the mask.

```
FFFFFFFE FFFFFFFF FFF8007F FFFFFFFF FFFFFFFF 7FFFFFFF   (u32 LE)
```

- Bit 0 is unused (ID 0 = no art).
- IDs 71–82 are an unused gap (`FFF8007F`).
- Bit 191 is unused.
- 178 bits remain, which is exactly the number of Hunter Arts in MHGU.

Only one window in the file fit the mask. It also passed three independent checks:

- All 56 weapon-art "I" tiers were set. Those unlock through normal story progress.
- The unused bits (0, 71–82, 191) were clear.
- The save-screen art slot (`base + 0x2C`, u16) held ID 150. Under this numbering
  that is Wolf's Maw II, a Dual Blades art, and the weapon-type byte at `base + 0x240`
  pointed to Dual Blades.

Writing the full mask unlocked every art in-game, including II/III tiers that were
not unlocked before.

**ID order — DERIVED.** Kiranico's Hunter Arts list, read in order, maps onto the IDs:
the first 70 entries are IDs 1–70, and the rest are IDs 83–190. Grouped by weapon,
that order follows the weapon usage storage order in [04](04-weapon-usage.md)
(Great Sword, Sword and Shield, Hammer, Lance, Heavy Bowgun, Light Bowgun, …, Charge
Blade). The table is in [`data/hunter-arts.csv`](../data/hunter-arts.csv).

Only the whole-mask result and the one ID-150 match have been checked. Individual
names were not checked in-game one by one.

### Save-screen art slots — `base + 0x2C`

**CONFIRMED** (save timeline: in every save the arts equal those of the loaded My Set and
are unlocked; a fresh slot holds 26 and 1). 3 × u16 art IDs, as in the MHXX notes, then a u16 of SP Art bits (bit *i*
= art slot *i* is an SP Art). Only slot 1 was non-zero in the analysed save. The slot
header is a save-screen summary; the copy the game loads is the player record at
`base + 0x23A59` ([11](11-save-map.md#player-record--base--0x23a58)), which holds the
equipped arts themselves. A new character starts with arts 26 and 1.

## Canteen ingredients — `base + 0x2F8F`

**CONFIRMED.** 45 bits, one per ingredient. Bits 45–47 of the last byte are unused.

How it was found:

- Between two snapshots taken days apart, the only change in the area was bit 44 being
  set.
- The field had 39 bits set, and the game showed 39 ingredients.
- The recipe table uses 45 distinct ingredients, which matches the bit range 0–44.
- Setting the six clear bits in 0–44 made the game show 45 ingredients.

**DERIVED — bit order.** Bit *i* is ingredient *i* of the recipe table
(`table/kitchenListMenu.kcm`), grouped by kind with the GU additions at the end of
each group: meat 0–8, grain 9–18, vegetables 19–28, fish 29–38, drinks 39–44. The
code confirms the groups: `0x1a6598` counts the bits 0–8, 19–28, 29–38 and 39–44 of
`S+0x97c` one group at a time. Names come from `Kitchen_eng.gmd` (entries 103–133, then
251–264 for the GU ones), and every recipe's two ingredients agree with its dish
name. The full list is [`data/canteen.csv`](../data/canteen.csv). The bit 44 seen
being set between two snapshots is *Chat-eau de Nya*.

**Setting ingredients does not add dishes.** The dish list was unchanged after the
write. The dishes had to be made in-game by combining ingredients at the Canteen.

## Canteen dishes — `base + 0x2C67D`

**CONFIRMED.** 99 bits, one per dish (13 bytes; bits 99–103 unused). The two
starter meals are included.

How it was found:

- The player combined ingredients in-game, and the save was diffed before and after.
- This field went from 81 bits set to 98, matching the in-game count of 98 dishes.
- The one clear bit (76) was set by hand. The game then showed 99 dishes.

**DERIVED — bit order.** The field is `sKitchen +0x108` (the loader `0x1a4e64` reads
40 bytes to `+0x108`). Bit *j* is record *j* of `kitchenListMenu.kcm` (99 records of
35 bytes: u32 ingredient A, u32 ingredient B, …): the Bherna seller's "concoct" step
looks the two chosen ingredients up in that table and sets the record's bit
(`0x1a7288`, at most 98), and the initialiser sets bits 0 and 1, the two starter
meals (`0x1a5068`). Five records have no ingredients (14, 27, 40, 55, 98: Ardent
Crockpot, Spirited Chowder, Affectionate Soup, Exquisite Rice, Ultimate Rice). Names
are `Kitchen_eng.gmd` 134–210, then 265–286, and bit 76 is indeed *Jumbo Fried
Dragon* (Jumbo Fish + Wyvern Amber Ale). List: [`data/canteen.csv`](../data/canteen.csv).

### Dishes viewed — `base + 0x2C68D`

**DERIVED.** `sKitchen +0x118`, same size and bit order, directly after the dish
field: the dishes already viewed in the Canteen list (`cUIOKitchenUp`). `0x736438`
shows NEW for a learned dish whose bit is clear, and `0x736498` sets the bit when the
cursor rests on the row (`0x5c03cc`). The initialiser `0x1a49ec` clears it. In the
analysed save it holds 86 of the 99 learned dishes, the 13 others were never
highlighted (the dish written by hand among them). It gained bits when dishes were made
in-game because the list was browsed then. Leave it alone, or set a bit to drop a NEW
mark.

## Editing checklist

1. Resolve the base through the slot pointer at `0x34`.
2. Set bits with OR. Never set the unused bits listed above.
3. Hunter Arts: OR the six-u32 mask above at `base + 0x2C13`.
4. Canteen: OR `(1 << 45) - 1` into the 6 bytes at `+0x2F8F`, and `(1 << 99) - 1`
   into the 13 bytes at `+0x2C67D`.
5. Write both commit slots with the emulator closed (see
   [01 — Container](01-container.md)).
