# 07 — Equipment, transmog and dye

The equipment box, the equipped-gear cache, the saved equipment sets ("My Sets"), and
where transmog and armor pigment actually live. Everything here was confirmed by
writing bytes, loading the save in-game, and checking the result on screen.

Offsets in this document are **relative to the character base** (next section), not
absolute. The absolute values quoted are for the first character slot.

## Character slots

**CONFIRMED** with three real characters (below). The file starts with a 36-byte
(`0x24`) Switch header. The MHXX-layout body follows it, and its own header carries a
slot-use table and slot pointers:

| Absolute | Size | Field |
|---|---|---|
| `0x28` | 3 × u8 | slot in use (1 / 0) for characters 1–3 |
| `0x2B` | u8 | last loaded slot, 0-based |
| `0x34` | 3 × u32 | offset of character 1–3, **relative to `0x24`** |

```python
base = 0x24 + int.from_bytes(buf[0x34 + 4*slot : 0x38 + 4*slot], "little")
```

In the analysed save, slot 1 is `0x18CC78 + 0x24 = 0x18CC9C`. The unused slots still
have pointers (`0x2AC53C`, `0x3CBE00`), giving a stride of `0x11F8C4` bytes.
`base + 0x00` is the character name. This matches the MHXX `system` layout once the
`0x24` header is accounted for.

Every absolute offset in docs 02–05 falls inside slot 1's block, so those structures
are per-character too (for example, deviant permits are at `base + 0x283C`).

**Checked with two new characters** (Scrimas2, Scrimas3, created on 2026-10-04 and
saved before the introduction):

- Each creation changed only its own slot, `0x28 + slot` (in use) and `0x2B` (last
  loaded, 1 then 2). Blocks A and B and slot 1 stayed byte-identical.
- Slot 2 starts at `0x2AC560` and slot 3 at `0x3CBE24`, as the pointers say; the name is
  at `base + 0x00`.
- The two fresh slots differ only in the name (slot header `+0x00`, player record
  `+0x23B7D`, own Guild Card `+0xC71BD`), the own card's 8-byte owner ID (card `+0x8B0`),
  the hireable Palico list at `+0x2A606` (rolled at creation) and the Guild Card list
  padding (heap leftovers, [11](11-save-map.md#guild-card-manager)).
- The slot-1 formats parse in the new slots: both card lists hold 100 / 50 empty
  elements (state 1), the equipment box starts with ID-1 weapons, HR, HR points and
  funds are 0.

## Equipment box

**CONFIRMED.** `base + 0x62EE` (absolute `0x192F8A`): 2000 entries × 36 bytes, in the
same order as the in-game box. An empty entry is all zero.

| Offset | Size | Field |
|---|---|---|
| `+0x00` | u16 | type (bits 0–4), level − 1 (bits 5–9), bits 10–15 see below |
| `+0x02` | u16 | equipment ID (armor: armor ID; weapon: index into the weapon-tree table) |
| `+0x04` | u16 | **transmog appearance ID** (armor only; 0 = own look) |
| `+0x06` | 3 × u16 | decoration item IDs, one per slot, 0 = empty |
| `+0x0C` | 24 | armor/weapon: zero in every entry observed; talisman: see below |

Type codes seen: 1 head, 2 chest, 3 arms, 4 waist, 5 legs, 6 talisman, 7–21 weapon
classes (18 = Dual Blades). The level field stores the in-game level minus one.
Writing max level produced the right raw and element values on the status screen.

**DERIVED — bits 10–14 of `+0x00`: level − 1 of the transmog source.** When a look
is applied, the transmog screen (`uUICoordinate`, `0x148f00`) writes the source
piece's equipment ID to `+0x04` and its level field (bits 5–9) to bits 10–14. The
equipment detail window (`0x56e810`) reads the pair together: with the appearance
shown it swaps (ID, level) for (`+0x04`, bits 10–14). The Guild Card keeps the field
as a byte of its own (card equipment entry `+0x2A`). So 0 means the source piece was
at level 1, which fits every transmogged entry of the analysed save. The MHXX notes'
name, "transmog level", is right.

**UNRESOLVED — bit 15.** The equipped cache copy of one transmogged helm had it set
when its box source did not. It is a declared one-bit field: the entry clear and
construct helpers (`0xdab04`, `0xdaba4`, `0xdac14`) and the builder `0x159b18` keep it
(`and #0x8000`), so it is metadata that survives a clear. A whole-binary scan for a
tester (`tst #0x8000`, `lsr #15`, a byte test of `+0x01` bit 7) found none. Leaving it
at zero is safe.

### Talisman fields

**CONFIRMED.**

| Offset | Size | Field |
|---|---|---|
| `+0x0C` | u8 ×2 | skill-tree ID 1, 2 |
| `+0x0E` | i8 ×2 | skill points 1, 2 |
| `+0x10` | u8 | slot count |
| `+0x12` | u8 | tier code: 97 Mystery, 98 Shining, 99 Timeworn, 100 Enduring |
| `+0x13` | u8 | 1 on every talisman observed |

## Transmog

**CONFIRMED.** Transmog is per box entry: bytes `+0x04..+0x05` hold the armor ID whose
model is displayed. Writing an ID there changes the look of that piece in every set
that uses it. Skills, slots and defense are unchanged. The in-game "Col" column shows
the transmog marker for these pieces. The target must be the same body part and a
class the hunter can wear. No ownership check was triggered on load.

## Armor pigment (dye)

**Not stored per armor piece.** In every box entry of a save containing dyed,
transmogged gear, bytes `+0x0C..+0x23` were zero. Pigment belongs to the *outfit*:
one colour per body part, kept in two places.

### Current outfit

**DERIVED.**

| Offset | Size | Field |
|---|---|---|
| `base + 0x24C` | 5 × RGBA | pigment per body part |
| `base + 0x270` | u8 | bits 0–4: 1 = use the armor's default colour for that part, 0 = custom |

Changing sets rewrites these, so they are just a copy of the equipped set's values.
Default colours are written out explicitly (e.g. `fa f5 e6 ff`), not stored as zero.

**The copy the game loads is elsewhere** ([11](11-save-map.md#player-record--base--0x23a58)).
`base + 0x24C` / `+0x270` sit in the 632-byte slot header, which the loader discards.
The live values are in the player record: pigment at `base + 0x23B53` (5 × RGBA + 16 B)
and flags at `base + 0x23B7B` (u16). Both copies were byte-identical in the analysed save.
An editor that dyes the current outfit should write both.

### My Sets (saved equipment sets)

**CONFIRMED** for the box indices, name and pigment. Set 1 is at `base + 0x208C8`
(absolute `0x1AD564`), stride `0x88`. The menu shows 5 pages × 8 sets, so there are
most likely 40 records. That count is **DERIVED** from the UI.

**Record boundary (from the loader, [11](11-save-map.md#equipment-manager-additions)).**
The game reads the sets as 40 × 136 B starting at `base + 0x208CE`, 6 bytes later than
the offsets below assume. So the 6 "varying" bytes at `+0x00` are the last 6 bytes of the
previous set. For set 1 they are the end of the Palico equipment box. The count of 40 is
confirmed by the loader. The table keeps the original offsets; subtract 6 for offsets
within the game's record.

| Offset | Size | Field |
|---|---|---|
| `+0x00` | 6 | the previous set's last 6 bytes, see below |
| `+0x06` | 24 | set name, single-byte text, NUL-padded (`---` when unused) |
| `+0x30` | 7 × u16 | box index for weapon, head, chest, arms, waist, legs, talisman; `0xFFFF` = empty |
| `+0x3E` | 7 × 3 × u16 | copy of each piece's decorations (**DERIVED**) |
| `+0x6A` | 5 × RGBA | pigment per body part |
| `+0x7E` | 5 × u8 | per part, a colour mode handed to the dye call with the colour and the flag (`0x26f958`): 0 = the RGBA, 2 and up = preset colour *v* − 2, 1 = a third path ([11](11-save-map.md#slot-header--base--0x0-632-b), header `+0x274`). Zero in every set of all three slots |
| `+0x83` | 5 × u8 | per-part default flag: 1 = default colour, 0 = custom RGBA |
| `+0x88` | u8 | hunting style: 0 Guild, 1 Striker, 2 Aerial, 3 Adept, 4 Alchemy, 5 Valor (same order as the style counters of [11](11-save-map.md#the-block-s0x20--0x41f--base--0x280b)) |
| `+0x89` | 3 × u8 | the three Hunter Arts (IDs of [`hunter-arts.csv`](../data/hunter-arts.csv)) |
| `+0x8C` | u8 | bits 0–2: art slot *i* is an **SP Art**, copied to the player's SP Art bits ([11](11-save-map.md#slot-header--base--0x0-632-b), header `+0x32`) |
| `+0x8D` | u8 | padding |

**DERIVED — the last six bytes (`+0x88 … +0x8D`, the game's record `+0x82 … +0x87`).**
Loading a set (`0x72d0f8`, from the item box screen) writes the style byte into the
player data and the three arts with `0xe79b4` to player `+0x240` (the arts the save
screen shows at slot header `+0x2C`), the three SP Art bits with `0xe7a20`. The copy
routine `0x14709c` moves the same bytes. In the analysed save set 1 has style 5 and art
179 (*Energy Blade I*, a Charge Blade art, the one equipped), sets 2–6 style 3 and art
151 (*Wolf's Maw III*). The style numbering is **DERIVED** from these values and the
counters (Valor used 375 times).

The five RGBA values are probably ordered chest, arms, waist, legs, head, as in the
MHXX Guild Card. That order is **DERIVED** only: every test used the same colour on
all five parts.

To dye a set, write the RGBA five times at `+0x6A` and clear the five flags at `+0x83`.
Confirmed in-game: a colour written this way appears on the hunter when the set is
loaded.

Alpha was `0xFF` in every colour the game wrote. A forum report says it controls the
specular "shine". That is untested here.

## Equipped-gear cache

**DERIVED.** `base + 0x110`: 7 × 44-byte records (weapon, head, chest, arms, waist,
legs, talisman). Each record starts with a copy of the 36-byte box entry. A u32 that
was constant across all seven slots (`14 a5 e3 01` in the analysed save) sits 4 bytes
before each copy. It is a runtime vtable pointer written as is, not data
([11](11-save-map.md#slot-header--base--0x0-632-b)). The cache lives in the 632-byte
slot header, which the loader discards and the writer rebuilds. Patching it only
changes the slot-select screen until the next save, so an editor can leave it alone.

## Decorations, slots and armor classes (game tables)

**DERIVED** from the game's tables (`resident.arc`, read by `tools/build_assets.py`) and
checked against every equipment box entry of 79 saves; not yet written in game.

- **Decoration fields are packed.** A decoration takes one of the three `+0x06` fields
  whatever its size, filled from the first: a 3-slot Attack Jwl 3 is `[id, 0, 0]`.
- **Decoration sizes:** `table\decoData`, 5-byte records `[size, skill, points, skill,
  points]`; record *k* is item `2638 + k`. Every "Jwl N" name has size N.
- **Armor slots:** `armorSeriesData` byte `108 + part − 1` of the series record. All 38
  decorated armor pieces fill exactly that many slots.
- **Weapon slots** grow with the level: `weaponNNLevelData` records (stride per class)
  hold the weapon ID at `+4`, the level at `+5` and the slots in their last byte. Every
  decorated weapon fits; the Dual Blades and the Gunlance ones fill them exactly.
- **Blademaster / Gunner:** `armorSeriesData` bytes 15 and 16 (Hunter's Helm 1/0,
  Hunter's Cap 0/1, Leather 1/1), next to male/female at 13/14. The editor offers a
  look only of the same part and class, wearable by the character's body type.
- **Palico gear:** box types 22 weapon, 23 head, 24 body; the box ID is the record of
  `otWeaponData` / `otArmorData`, named by `otWeaponData_eng` entry 2*k* and
  `otArmorData_eng` entries 4*k* (head) and 4*k* + 1 (body). A new save's box reads
  Bone Wedge, Acorn Helm and Mail, Bherna Staff, Hood and Mail.

## Editing checklist

1. Resolve the character base through the pointer at `0x34`. Don't hard-code it.
2. Only write to empty box slots, or to entries you have just read and matched.
3. Transmog: write `+0x04`. Dye: write the My Set pigment (and the current-outfit
   pigment if that set is worn).
4. Write both commit slots identically, with the emulator closed (see
   [01 — Container](01-container.md)).
