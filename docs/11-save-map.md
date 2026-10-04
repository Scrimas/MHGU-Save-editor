# 11 — Whole-file map (from the game's own loader)

Every byte of the save, assigned to the game object that reads it. Earlier documents were
found by diffing saves. This one comes from running the game's own load code on a real save
and logging every read. It accounts for the whole file, including regions nobody had
touched (item box, Palicoes, Guild Cards, downloaded quests).

Machine-readable: [`data/save-map.csv`](../data/save-map.csv) (one row per field or array)
and [`data/save-coverage.txt`](../data/save-coverage.txt) (bytes the loader never reads).
Tools: [`tools/emu/`](../tools/emu) (emulator, see [Method](#method)),
[`tools/items.py`](../tools/items.py) (item box, pouch, loadouts) and
[`tools/event_quests.py`](../tools/event_quests.py) (downloaded quests). Both tools are read only.

Offsets are absolute unless written `base + x`, where base is a character slot
([07](07-equipment.md#character-slots)). Slot 1 is at `0x18CC9C` in the analysed save.

## Method

**CONFIRMED (the code is the source).** The v1.4 executable is run under unicorn
(`tools/emu/emu.py`):

- The image is relocated to `0x10000000` so null pointers fault.
- The save managers are built by their own constructors (main init `0x3d8b10`).
- The full-file loader `0x3e7d10(this, file + 0x24)` then runs on the save.

Objects the game would have built at startup are allocated when the run faults on them.
For each `memcpy` from the file and each bit read (`0x3df22c`), the harness logs:

- the file offset;
- the size;
- the loader pc;
- the manager being loaded;
- the object field written.

`tools/emu/savemap.py` collapses repeated reads into arrays and labels them. The
singleton names (`sItemBox`, `sOtomo`, …) are the MT class names referenced in the
same translation unit as each loader. A `?` marks the nearest name where the unit is unclear.

Loaders whose counts or buffers come from objects filled at startup read too little when
those objects are empty. Two cases had to be seeded by hand (`runfull.py`):

| Where | What the game fills at startup | Without it |
|---|---|---|
| `S + 0x950/0x954` | list of (pointer, u16 count) for the monster size records | 548 bytes skipped, everything after shifted by `0x224` |
| `sPrivilege + 0xfa0 .. 0xfb0` | five heap buffers for block B | the 1.5 MB of block B is skipped (`0x3df144` seek) |

Anchors that must land: the equipment box at `base + 0x62EE`, the event flags at
`base + 0x2C56D`, and the star levels at `base + 0x2C4DA`. Each character chain must
also end at the next slot's base. All of them land.

**Result.** The loader reads 5,158,562 of 5,159,100 bytes. The rest is:

- the 36-byte Switch header and the body header (`0x0–0x40`);
- three alignment pads (1–3 bytes);
- a 469-byte zero tail after slot 3.

All three character slots are read by the same chain at a fixed stride of `0x11F8C4`,
empty slots included. Their layout is identical (checked by `savemap.py`). The only
exception is the element lengths inside the two Guild Card lists, whose totals are fixed.
This closes the README open question on multi-character layout: every per-character
structure is `base + offset`, with base from the pointer table at `0x34`.

## File layout

| Offset | Size | Block | Manager (loader) | Content |
|---|---|---|---|---|
| `0x00` | 36 | Switch header | — | nonce at `0x14` ([01](01-container.md)) |
| `0x24` | 28 | body header | — | u32 `0xC6`, u32 1, block A offset, block B offset, 3 slot offsets (all relative to `0x24`) |
| `0x40` | 14 | A | sUserInfo (`0x52163c`) | 4 + 6 + 4 × 1 bytes, UNRESOLVED |
| `0x4E` | 16200 | A | sOtomo (`0x263298`) | shared Palico pool, 50 × 324 B (DLC Palicoes, owner "Capcom") |
| `0x3F96` | 9600 | A | sBlackList (`0x2257e4`) | 100 × (64 + 32) B, all zero here |
| `0x6516` | 19852 | A | sGuildCard (`0x16424c`) | 3 × 6616 B card copies, 1 B, 3 B unused; all zero here |
| `0xB2A2` | 3 | A | sGameControl (`0x3f8ef8`) | 3 × u8 |
| `0xB2A5` | 108 | B | sPrivilege (`0x36bdc`) | 9 fields at object `+0xf34 .. +0xf9c`, mostly bitmaps (DLC / event state), UNRESOLVED |
| `0xB311` | 5200 | B | sPrivilege | DLC item pack list, 50 × 104 B (pack names) |
| `0xC761` | 12600 | B | sPrivilege | DLC Palico info (names, greetings, owner "Capcom") |
| `0xF899` | 1146880 | B | sPrivilege | event quests, 160 × `0x1C00` ([below](#downloaded-quests--block-b)) |
| `0x127899` | 322560 | B | sPrivilege | challenge quests, 45 × `0x1C00` |
| `0x176499` | 92160 | B | sPrivilege | challenge quest records, 45 × `0x800` |
| `0x18CC9C` | `0x11F8C4` | slot 1 | `0x3e0db8` | character, table below |
| `0x2AC560` | `0x11F8C4` | slot 2 | | same layout |
| `0x3CBE24` | `0x11F8C3` | slot 3 | | same layout, then 469 zero bytes to the end of the file |

Blocks A and B are shared by all characters. Each block starts 4-aligned relative to `0x24`.

## Character slot

| `base +` | Size | Manager (loader) | Content |
|---|---|---|---|
| `0x0` | 632 | header (`0x3e0db8`) | summary for the slot screen, [below](#slot-header--base--0x0-632-b). The loader discards it; the writer rebuilds it from live state |
| `0x278` | 5463 | sItemBox (`0x19a4ec`) | item box, 2300 slots ([below](#item-box-pouch-and-loadouts)) |
| `0x17CF` | 4080 | sItemBox | item loadouts, 24 × 170 B |
| `0x27BF` | 76 | sItemPouch (`0x19e0ac`) | item pouch, 32 slots |
| `0x280B` | 15075 | sUserInfo (`0x51dbb4`) | the save object S: talk state, quest bitmaps, Hunter Arts, awards, tallies, size records (docs 02–05, 08, 10) |
| `0x62EE` | 72000 | sEquipBox (`0x14b0b0`) | equipment box, 2000 × 36 B ([07](07-equipment.md)) |
| `0x17C2E` | 36000 | sEquipBox | Palico equipment box, 1000 × 36 B |
| `0x208CE` | 5440 | sEquipBox | My Sets, 40 × 136 B |
| `0x21E0E` | 1632 | sEquipBox | Palico equipment sets, 24 × 68 B |
| `0x2246E` | 41 | sGameControl (`0x3f8de0`) | 29 + 3 × 4 B; holds a copy of the play time (`base + 0x20`) |
| `0x22497` | 5569 | sItem? (`0x1979b0`) | activity manager: tickets, pending village rewards ([10](10-npc-talk.md)) |
| `0x23A58` | 325 | sPlayer (`0x2755f0`) | player record: the loaded copy of the header's appearance, pigment and name ([below](#player-record--base--0x23a58)) |
| `0x23B9D` | 35131 | sOtomo (`0x2639ac`) | Palicoes: 25 B of bit fields, 84 + 24 records of 324 B, 114 B tail |
| `0x2C4D8` | 149 | sVillage (`0x507d44`) | star levels at +2 / +4 ([05](05-quests.md#star-levels--base--0x2c4da)) |
| `0x2C56D` | 272 | sNpcTalk (`0x240d60`) | event flags, NPC hold bits, random word ([10](10-npc-talk.md)) |
| `0x2C67D` | 40 | sKitchen (`0x1a4e64`) | Canteen dishes and copy ([08](08-progression.md)) |
| `0x2C6A5` | 4 | sFlagChecker (`0x3f43d4`) | u32 |
| `0x2C6A9` | 20 | sEventCtrl? (`0x14c568`) | 5 × u32 |
| `0x2C6BD` | 978804 | sGuildCard (`0x163e0c`) | Guild Cards ([below](#guild-card-manager)) |
| `0x11B631` | 6248 | sGuestHunter (`0x15bd3c`) | hunters met online: UTF-16 name, greeting, records of 308 B, "Hired …" copies |
| `0x11CE99` | 160 | sTutorial (`0x539120`) | 8 + 152 B |
| `0x11CF39` | 263 | sMonNyan (`0x1c9388`) | Meownster Hunters (Palico expeditions) state, bit fields |
| `0x11D040` | 10371 | `0x55d2c0` | chat phrases: auto-chat lines in 104-byte slots ("Let's do this!", "Thanks!", …) |
| `0x11F8C3` | 1 | — | alignment |

### Slot header — `base + 0x0`, 632 B

**DERIVED** from the writer `0x3e0ea4`, which builds it on the stack from `sPlayer` and
two globals. The loader `0x3e0db8` copies it to the stack and drops it. The header is
therefore a summary for the slot screen. Editing it alone does not change the
character, and the next save overwrites it (not tested in game).

| `base +` | Size | Source in the writer | Field |
|---|---|---|---|
| `0x000` | char[32] | sPlayer `+0x514` | hunter name |
| `0x020` | u32 | global `+0x34` | play time, seconds (the Guild Card copy is `+0x914` of the own card) |
| `0x024` | u32 | global `+0x24` | money (zenny); 9,999,999 in the analysed save |
| `0x028` | u16 | sPlayer `+0x554` | HR |
| `0x02A` | u8 | sPlayer `+0x4D4` / `+0x4D8` | 2 if `+0x4D4` is 15, else `+0x4D8 != 0` |
| `0x02B` | u8 | writer argument | |
| `0x02C` | 224 | sPlayer `+0x240` | UNRESOLVED (character creation data?) |
| `0x10C` | 7 × 44 | sPlayer `+0x18 + 44k` | equipped-gear cache: u32 **vtable pointer** (runtime address, see below), 36-B box entry, u32 |
| `0x240` | 12 | sPlayer `+0x4D4` | UNRESOLVED |
| `0x24C` | 36 | sPlayer `+0x4E0` | current pigment, 5 × RGBA + 16 B ([07](07-equipment.md)) |
| `0x270` | u16 | sPlayer `+0x506` | default-colour flags |
| `0x272` | 2 | — | struct padding (stale bytes) |
| `0x274` | u32 | sPlayer `+0x508` | UNRESOLVED |

The u32 before each equipped-cache copy is the in-memory address of the object's vtable,
written as is. It was `0x01E3A514` in the game and `0x11736514` in the emulator: vtable
`0x1736514` of the v1.4 image at two different load bases. It carries no save data.

### Player record — `base + 0x23A58`

**DERIVED** (loader `0x2755f0` and the header writer use the same sPlayer fields; every
shared field was byte-identical in the analysed save). This is the copy the game loads.

| `base +` | Size | sPlayer field | Same as header |
|---|---|---|---|
| `0x23A58` | 1 | `+0x9C5C` | — |
| `0x23A59` | 224 | `+0x2BC` | `+0x02C` (character creation data?) |
| `0x23B39` | 14 | `+0x84` | — (7 × u16) |
| `0x23B47` | 12 | `+0x550` | `+0x240` |
| `0x23B53` | 36 | `+0x55C` | `+0x24C`, current pigment |
| `0x23B77` | 4 | `+0x584` | `+0x274` |
| `0x23B7B` | 2 | `+0x582` | `+0x270`, default-colour flags |
| `0x23B7D` | 32 | `+0x590` | `+0x000`, hunter name |

The 0x2C6BD–0x11B631 range alone is 0.98 MB per slot. That is why the file is 5 MB
while the character data proper is small.

## Item box, pouch and loadouts

**CONFIRMED** (loader bit widths, 1255 used slots with sensible names and counts 1–99,
loadout 1 equal to the pouch).

| `base +` | Layout |
|---|---|
| `0x278` | item box: 2300 slots, LSB-first bit stream, 19 bits per slot: item ID (12 bits), then count (7 bits). Slot *i* starts at bit 19·*i*. ID 0 = empty |
| `0x17CF` | item loadouts: 24 × 170 B. `+0x00` name char[42] ("Set 01", "---" = unused), `+0x2A` 32 × (u16 item ID, u16 count) |
| `0x27BF` | item pouch: 32 slots, same 19-bit format (24 items, then 8 ammo / coatings) |

Item names: string 2·ID of `table/itemData_<lang>.gmd` (romfs). Writer `0x19a55c` caps the
ID at `0xBAF`. Count is 7 bits, so 99 fits and 127 is the hard ceiling.

## Equipment manager additions

**DERIVED.**

- **My Sets start at `base + 0x208CE`**, 6 bytes later than [07](07-equipment.md#my-sets)
  states. The loader reads 40 × 136 B from there. The field offsets in 07 are relative to
  `0x208C8`. Subtract 6 to get offsets within the record: name `+0x00`, box index
  `+0x2A`, pigment `+0x64`, default flags `+0x7D`. Both conventions point at the same
  bytes; only the record boundary moves.
- **Palico equipment box**, `base + 0x17C2E`: 1000 × 36 B, same entry format as the hunter
  box. Only types 22, 23, 24 occur (Palico weapon, head, body).
- **Palico equipment sets**, `base + 0x21E0E`: 24 × 68 B. `+0x00` name char[42],
  `+0x2A` 3 × u16 box index (`0xFFFF` = empty), then flags.

## Palico records

**DERIVED** from the loader (`0x263390`: 32 + 224 + 14 + 12 + 36 + 4 + 2 bytes per
record) and from the values of the 32 Palicoes in the save: exp and level rise together,
and owner names are sensible.

| Offset | Size | Field |
|---|---|---|
| `+0x00` | char[32] | name |
| `+0x20` | u32 | experience |
| `+0x24` | u8 | level (raw; ordering matches experience) |
| `+0x25` | u8 | forte / bias (UNRESOLVED) |
| `+0x26` | u8 | UNRESOLVED (55–99 seen) |
| `+0x27` | u8 | target (1–5 seen) |
| `+0x60` | char[60] | greeting |
| `+0x9C` | char[32] | original owner name |

Lists: `base + 0x23BB6` 84 records (the Palicoes of the slot) and `base + 0x2A606` 24
records (a second list, same format). Block A holds a shared pool of 50 records. In the
analysed save it contains DLC Palicoes with owner "Capcom".

## Guild Card manager

**DERIVED** (loader `0x163e0c`, writer `0x1641b0`, container load `0x3cf6d4` / save
`0x3cf928`). In `base +` order:

| `base +` | Size | Content |
|---|---|---|
| `0x2C6BD` | 633,600 | list 1: 100 elements, then padding |
| `0xC71BD` | 6328 | **own Guild Card** |
| `0xC8A75` | 4400 | manager `+0x18D8`, UNRESOLVED |
| `0xC9BA5` | 316,800 | list 2: 50 elements, then padding |
| `0x117125` | 1800 | manager `+0x2A0C`, UNRESOLVED |
| `0x11782D` | 276 | manager `+0x3114`, UNRESOLVED |
| `0x117941` | 13800 | manager `+0x3228`, UNRESOLVED |
| `0x11AF29` | 1800 | manager `+0x6810`, UNRESOLVED |

A list element is:

1. u32 compressed length;
2. zlib stream of one 6328-byte card;
3. u32 state (3 = card, 1 = empty);
4. a 36-byte trailer: u16, 11 UTF-16 characters of name, 8-byte ID, u32.

After the elements comes one padding block of Σ(`0x18B8` − length − 36) bytes. The
list size is therefore fixed: n × (`0x18B8` + 8), which is 633,600 B for list 1 and
316,800 B for list 2. The game fills the padding from an uninitialised heap buffer. It
contains stale strings and must not be read as data.

The analysed save holds two received cards in list 1. Each decompresses to 6328 bytes
with the same layout as the own card (history log at `+0x918` in both).

### Card layout (6328 B)

**DERIVED** from the card builders `0x1623b4` (hunter) and `0x162574` (Palico), the
award test `0x162720` and the history insert (`0x1628a0`). Checked on the own card and
a received card: HR 999 / 148, sensible equipment types, transmog IDs.

| Card offset | Size | Field | Source |
|---|---|---|---|
| `+0x000` | 22 | hunter name, UTF-16 (11 characters) | sPlayer `+0x514` |
| `+0x016` | u16 | HR | sPlayer `+0x554` |
| `+0x018` | 12 | appearance bytes (same as slot header `+0x240`) | sPlayer `+0x4D4` |
| `+0x024` | 36 | pigment, 5 × RGBA + 16 B | sPlayer `+0x4E0` |
| `+0x048` | u32 | as slot header `+0x274` | sPlayer `+0x508` |
| `+0x04C` | 4 × u16 | first 8 bytes of the 224-byte block | sPlayer `+0x240` |
| `+0x054` | 7 × 44 | equipment, weapon … talisman (entry table below) | equipped gear |
| `+0x188` | 3 × 580 | Palicoes: main, buddy 1, buddy 2. Name UTF-16 `+0`, then the hunter-section layout | sOtomo |
| `+0x878` | | greeting, UTF-16 | |
| `+0x8B8` | u8 | flags (bit 1 from sPlayer `+0x9C5C`) | |
| `+0x8BA` | 3 × 15 × u16 | weapon usage, Village / Hub / Arena ([04](04-weapon-usage.md)) | |
| `+0x914` | u32 | play time ([05](05-quests.md)) | |
| `+0x918` | 10 × 160 | quest history ([05](05-quests.md)) | |
| `+0xF58` | 20 | awards, bits 0–159 (132 used, [09](09-awards.md)) | |

Card equipment entry (44 B), filled from the 36-byte box entry:

| Offset | Field |
|---|---|
| `+0x00` | u8 type (box `+0x00` bits 0–4) |
| `+0x02` | u16 equipment ID (box `+0x02`) |
| `+0x04` | u8 level − 1 (box `+0x00` bits 5–9) |
| `+0x08` | 3 × u16 decorations (box `+0x06`) |
| `+0x10` | 24 B (box `+0x0C`, talisman data) |
| `+0x28` | u16 transmog (box `+0x04`) |
| `+0x2A` | u8 box `+0x00` bits 10–14 |

The game converts names through a reused stack buffer, so bytes after a short name's
terminator can hold the tail of an earlier name. They are not data.

History record, from the insert code: day `+0`, month `+1`, u16 year `+2` (tm_year +
1900), u16 record kind `+4` (7 for the analysed quest records, 3 for a second record
type the code builds), u16 quest ID `+6`, name `+8`. The code that fills `+0x28` was not
traced.

Not yet labelled: `+0x854 … +0x8B8` around the greeting, `+0xF18 … +0xF58`, and
everything after `+0xF6C`.

## Downloaded quests — block B

**CONFIRMED** (loader `0x36bdc` reads each store as one block; the archives parse and
their titles match [`quest-index.csv`](../data/quest-index.csv)). Shared by all characters.

| Offset | Slots × size | Record |
|---|---|---|
| `0xF899` | 160 × `0x1C00` | u32 quest ID, u32 size, MT `ARC` archive (zlib entries) |
| `0x127899` | 45 × `0x1C00` | same, challenge (Arena event) quests 1020001–1020029, 1120001–1120012 |
| `0x176499` | 45 × `0x800` | u32 quest ID, u32 size, raw record (169–942 B) for the same challenge quests |

ID 0 = empty. Each archive holds the quest's files: `setEmMain`, `emSetList`, `rem`,
`supp`, `questPlus`, `questLink`, and `questData_<ID>_<lang>` per language (GMD, string 0
= title). The analysed save stores 125 event and 40 challenge quests. That is every real
(non-placeholder) event row of `quest-index.csv`, matching the rule in
[05](05-quests.md) that an event quest is listed whenever it is installed.

The raw challenge records are probably the fixed loadouts of the challenge quests. The
executable has `cTrialEquipOtomo`, `cTrialItemPoach` and `cTrialOtomoParam` classes. Not
decoded.

## Writer round trip

**CONFIRMED.** After the load, the game's full-file writer `0x3e798c(this, body)` runs
on the same objects (`runfull.py --write`). It produces a body of exactly `0x4EB6C3`
bytes, ending where the loader's slot 3 ends.

The slot loader fills the same singletons for every slot, and the writer serializes the
singletons into every slot. So slot 1 is reloaded last and only slot 1 is compared
(`RT_SLOT`). Slot 1 is byte-identical to the original except:

| Range | Bytes | Why |
|---|---|---|
| slot header (`base + 0x10D ..`) | 79 | rebuilt from live state on save. The emulator lacks state such as the equipped cache |
| Guild Card list 1 padding | 391,579 | uninitialised heap in the game, zeros in the emulator |
| Guild Card list 2 padding | 161,730 | same |

Every other byte of the slot matches: item box, pouch, S, equipment, Palicoes, event
flags, all Guild Card elements, own card, guest hunters, chat. Outside the slot, block A
matches in full. Two shared bytes are set from runtime state:

- block B differs only in the u32 at `0xB30D` (sPrivilege `+0xf9c`);
- the slot-in-use byte at `0x28` differs.

So the map is complete and consistent in both directions. An editor that writes a field
in place, where this map says it lives, writes what the game would write.

## Open questions

- Most bitmaps of S between `base + 0x2C0B` and `base + 0x5EA2` have exact boundaries
  (see the CSV) but no meaning. Each needs the code that tests it.
- The Guild Card manager's 4400 / 1800 / 276 / 13800 / 1800-byte members, and what
  list 2 holds.
- The 224 + 12 + 4 bytes of the slot header taken from sPlayer. Their source fields are
  known (table above), but not their meaning.
- Block A's first 14 bytes and the sPrivilege header bitmaps.
