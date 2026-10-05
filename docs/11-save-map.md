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
structure is `base + offset`, with base from the pointer table at `0x34`. **CONFIRMED**
with two newly created characters in slots 2 and 3: each creation wrote only its own
slot and the slot-use bytes, and the two fresh slots differ only in name, card owner ID,
the rolled Palico list and the card list padding ([07](07-equipment.md#character-slots)).

## Confirmed by the save timeline

A field named from the code is DERIVED. It becomes CONFIRMED when real saves agree with
it, independently of the code. The timeline is 36 saves of the same file, 2026-09-05 to
2026-10-05:
- the snapshots of [06](06-methodology.md);
- the editor's own pre-write snapshots;
- the live save.

Each step between two saves is a known in-game action or a tool write:
- a counted quest, the Harvest Tour, a round of NPC reports;
- the creation of Scrimas2 and Scrimas3;
- an editor write that a later game session carried on from.

[`tools/evidence/run.py`](../tools/evidence/run.py) re-runs every check. It needs the
local snapshots. In the CSV, a single loader row that holds labelled fields is cut at
their edges, so each field carries its own tag.

| Field | Independent evidence |
|---|---|
| slot header, player name, card name | Scrimas2 / Scrimas3 appear at their creation in all three copies. The header's gear cache equals the box entries at the equipped indices (264 of 264). Its pigment equals the loaded My Set's |
| play time, remainder, card play time, header copy | The editor wrote 560908 s. The next game session went on to 561059, and the card copy to 561034. No game step ran longer than the wall-clock gap. The remainder stays below 60 and moves only with the seconds |
| funds, Wycademy points, contribution points, G-rank accumulator | Funds and Wycademy points: see [the S+0x20 block](#the-block-s0x20--0x41f--base--0x280b). Contribution points: the accumulator arithmetic matches the point gains exactly |
| quest counter `base + 0x5E4E` | +1 at each of the five counted quests. Unchanged at the Harvest Tour, at talks and at tool writes |
| quest history log | Each quest pushed one record with its quest ID and shifted the other nine down. The weapon byte is the usage counter that rose. A fresh slot holds *Welcome* and nine unset records |
| card monster log | Equals the size records and the family sums of the CONFIRMED tallies, in all 36 saves |
| card equipment | Equals the CONFIRMED box entries at the equipped indices in the fresh slots, 28 of 28 |
| awards game-side map, award notices | Award 130, granted in game, appears here before the card copy. The notices gain exactly the newly granted awards and are emptied in game |
| crowns | A crowns write took the gold-crown count from 13 to 79. The next session's award check granted awards 7, 8, 115, 116 (needs 66) |
| items obtained | Every item that entered the box or pouch in game had its bit set in the same save |
| equipped gear, weapon class, hunting style, equipped arts, pigment, default flags | Always equal to the loaded My Set (CONFIRMED). The style's use counter rises at each quest |
| Palicoes, hunting buddy 1, Palico box | Only the buddy's record gained experience after quests. Gear indices point at Palico box entries of the right type |
| Palicoes for hire `base + 0x2A606` | Rerolled at exactly the five counted quests |
| title words, NEW copy, scenes | A fresh slot holds exactly the entries described "Available from the start." (79 words, 5 scenes). The NEW copy gained exactly the new words |
| Poogie costumes | Changed only at the Captain's reward (bit 14) and at the report round |
| delivery requests offered / delivered | Equal to the accepted / done flags of the kind-1 requests (CONFIRMED event flags) |
| item loadouts | Loadout 1 equals the pouch slot for slot |
| guest hunter records | Online hunters are exact copies of their stored cards (144 copies). The "Hired" hunters carry their weapon class's type code |
| Guild Card list 1 | Both cards inflate to 6328 B; trailer HR, name and ID match the card |
| chat phrases | 3 × 24 shortcut phrases and 3 × 9 auto-chat lines, decoded |
| block A / B: DLC lists, Palico pool, challenge records | Bitmaps, lists and pool agree entry for entry |
| HR points, Hunter's Notes map, a new box entry | Controlled write of 2026-10-05 through the editor's code (HR 999 → 500, Rathian's Notes bit cleared, a Hunter's Knife added). In game: HR 500, Rathian gone from the Notes, the knife in the box and equippable; the game's next save kept all three |
| Palico level, bias, target, greeting, hunting buddy; body type | Read off in game: Suds' Palico Info (Lv 64 = byte 63, Gathering = 6, Large First = 4, the comment, "Palico 1"); all three hunters male = 0 |

The same pass corrected labels the data contradicts:
- the two DLC bitmaps were swapped, and `base + 0x32DB` holds item packs;
- the challenge-quest bitmap follows the record store, and the event-quest bitmap is not indexed by slot;
- player record `+5` is the hunting style;
- the card's `+0x4C` arts copy does not follow the player block;
- the monster log's `+0` is the largest size;
- the chat layout, the guest-hunter layout and the list-1 trailer "version" (the card's play time).

Nine unlock maps had a "runtime NEW copy" counted in their size. That copy is not in the
file, so those labels now cover only the saved map.

## File layout

| Offset | Size | Block | Manager (loader) | Content |
|---|---|---|---|---|
| `0x00` | 36 | Switch header | — | nonce at `0x14` ([01](01-container.md)) |
| `0x24` | 28 | body header | — | u32 `0xC6`, u32 1, block A offset, block B offset, 3 slot offsets (all relative to `0x24`) |
| `0x40` | 14 | A | sUserInfo (`0x52163c`) | shared settings: bonus packs, TV brightness, rumble ([below](#block-a-header-and-shared-settings)) |
| `0x4E` | 16200 | A | sOtomo (`0x263298`) | shared Palico pool, 50 × 324 B (DLC Palicoes, owner "Capcom") |
| `0x3F96` | 9600 | A | sBlackList (`0x2257e4`) | 100 × (64 + 32) B, all zero here |
| `0x6516` | 19852 | A | sGuildCard (`0x16424c`) | 3 × 6616 B card copies, 1 B, 3 B unused; all zero here |
| `0xB2A2` | 3 | A | sGameControl (`0x3f8ef8`) | 3 × u8, the last is the text language |
| `0xB2A5` | 108 | B | sPrivilege (`0x36bdc`) | which downloads the save holds: 8 bitmaps and a stamp ([below](#downloads-held--block-b-header)) |
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
| `0x2246E` | 41 | sGameControl (`0x3f8de0`) | game options (29 B), play time, two u32 ([below](#smaller-managers)) |
| `0x22497` | 5569 | sItem (`0x1979b0`) | items obtained, Trader cargo, Alchemy requests, pending village rewards ([below](#the-item-manager-sitem)) |
| `0x23A58` | 325 | sPlayer (`0x2755f0`) | player record: the loaded copy of the header's appearance, pigment and name ([below](#player-record--base--0x23a58)) |
| `0x23B9D` | 35131 | sOtomo (`0x2639ac`) | Palicoes: 25 B of bit fields, 84 + 24 records of 324 B, 114 B tail |
| `0x2C4D8` | 149 | sVillage (`0x507d44`) | star levels at +2 / +4 ([05](05-quests.md#star-levels--base--0x2c4da)) |
| `0x2C56D` | 272 | sNpcTalk (`0x240d60`) | event flags, NPC hold bits, random word ([10](10-npc-talk.md)) |
| `0x2C67D` | 40 | sKitchen (`0x1a4e64`) | Canteen dishes and copy ([08](08-progression.md)) |
| `0x2C6A5` | 4 | sFlagChecker (`0x3f43d4`) | u32 flags ([below](#smaller-managers)) |
| `0x2C6A9` | 20 | sMakeAmulet (`0x14c568`) | 128-bit quest map, event scenes played ([below](#smaller-managers)) |
| `0x2C6BD` | 978804 | sGuildCard (`0x163e0c`) | Guild Cards ([below](#guild-card-manager)) |
| `0x11B631` | 6248 | sGuestHunter (`0x15bd3c`) | guest hunters: a 98-byte header, 13 records of 470 B, then 5 × 8-byte hunter IDs. A record is either a hunter met online, an exact copy of that hunter's stored Guild Card (name `+0`, title `+0x16`, HR `+0x1C`, greeting `+0x1E`, owner ID `+0x65`, appearance `+0x72`, pigment `+0x7E`, equipment 7 × 44 B `+0xA2`; u32 `+0x5C` = the card's Unity), or a "Hired *weapon*" hunter rerolled after every quest (CONFIRMED, [timeline](#confirmed-by-the-save-timeline)) |
| `0x11CE99` | 160 | sTutorial (`0x539120`) | 8 + 152 B |
| `0x11CF39` | 263 | sMonNyan (`0x1c9388`) | Meownster Hunters (Palico expeditions) state, bit fields |
| `0x11D040` | 10371 | `0x55d2c0` | chat phrases: a 0x49-byte header, then 104-byte text slots: 72 = three copies of the 24 shortcut phrases ("Let's do this!" … "I'm outta here.") at `0x11D089`, 27 = three copies of the 9 auto-chat lines ("I mounted it!", "Hunter Art 1 activated!", …) at `0x11EDC9` |
| `0x11F8C3` | 1 | — | alignment |

### Slot header — `base + 0x0`, 632 B

**CONFIRMED** (the writer `0x3e0ea4` and the [save timeline](#confirmed-by-the-save-timeline)).
The writer builds it on the stack from `sPlayer` and two globals. The loader `0x3e0db8` copies it to the stack and drops it. The header is
therefore a summary for the slot screen. Editing it alone does not change the
character, and the next save overwrites it (not tested in game).

| `base +` | Size | Source in the writer | Field |
|---|---|---|---|
| `0x000` | char[32] | sPlayer `+0x514` | hunter name |
| `0x020` | u32 | global `+0x34` | play time, seconds (the Guild Card copy is `+0x914` of the own card) |
| `0x024` | u32 | global `+0x24` | money (zenny); 9,999,999 in the analysed save |
| `0x028` | u16 | sPlayer `+0x554` | HR |
| `0x02A` | u8 | sPlayer `+0x4D4` / `+0x4D8` | 2 for a Prowler (weapon class 15), else 1 for female, 0 for male |
| `0x02B` | u8 | writer argument | |
| `0x02C` | 224 | sPlayer `+0x240` | the three equipped Hunter Arts ([08](08-progression.md#save-screen-art-slots--base--0x2c)), u16 each (get `0xe79c8`, set `0xe79b4`), then u16 SP Art bits: bit *i* = art slot *i* is an SP Art (set `0xe7a20`, test `0xe7a6c`; the arts screen `0x5490fc` toggles it, a My Set load copies it). The card builder copies these four u16 to card `+0x4C`, but in 36 saves card `+0x4C` kept 180, 0, 0, 1 while the arts went 179 → 150 → 151 and the card was rebuilt 11 times (UNRESOLVED). The class is shared with the Palicoes, which keep exp, level, moves, greeting and owner in the rest; for a hunter bytes 8–223 have no reader and are zero. Analysed save: 179 (*Energy Blade I*), 0, 0, bits 1; fresh slots 26, 1, 0, bits 0 |
| `0x10C` | 7 × 44 | sPlayer `+0x18 + 44k` | equipped-gear cache: u32 **vtable pointer** (runtime address, see below), 36-B box entry, u32 |
| `0x240` | 12 | sPlayer `+0x4D4` | `+0` current weapon class (15 = Prowler; a new character gets 1 with a Sword and Shield), `+1 … +8` character creation choices copied to the model by the title menu (`0x68894c`), `+4` = gender (read by the Smithy, the Armory and the talk conditions), `+5` = **hunting style** (0 Guild … 5 Valor; it always equals the style of the loaded My Set, and each quest raised the use counter of that style, `base + 0x2905`) |
| `0x24C` | 36 | sPlayer `+0x4E0` | current pigment, 5 × RGBA + 16 B ([07](07-equipment.md)) |
| `0x270` | u16 | sPlayer `+0x506` | default-colour flags |
| `0x272` | 2 | — | struct padding (stale bytes) |
| `0x274` | u32 | sPlayer `+0x508` | five 5-bit values, bits 5*i* … 5*i*+4 for pigment slot *i* (getter `0x26f194`, read with the colour by the pigment screen `0x5f1760`). Cleared when a slot gets an explicit or the default colour (`0x26f5b0`, `0x26f440`), set to 1 by the appearance menu (`0x26f0dc`). It is a colour mode: the swatch code (`0x56f714`) draws the plain RGBA for 0, takes a separate path for 1 (`0x54700c`), and uses preset *v* − 2 of a runtime colour-pair table (`0x560900`, shared with `cUIOAppearanceColor` and character creation) for 2 and up. My Sets keep the same five values (game record `+0x78`, [07](07-equipment.md#my-sets-saved-equipment-sets)). The Guild Card copies it to card `+0x48`. 0 in all three slots |

"sPlayer" here is the player data the getter `0x277454` returns: the loaded object
`+0x7C`. The loader's offsets below are therefore `0x7C` higher than the header
writer's.

The u32 before each equipped-cache copy is the in-memory address of the object's vtable,
written as is. It was `0x01E3A514` in the game and `0x11736514` in the emulator: vtable
`0x1736514` of the v1.4 image at two different load bases. It carries no save data.

### Player record — `base + 0x23A58`

**DERIVED** (loader `0x2755f0` and the header writer use the same sPlayer fields; every
shared field was byte-identical in the analysed save). This is the copy the game loads.
**CONFIRMED** by the [save timeline](#confirmed-by-the-save-timeline): the three equipped arts
and SP bits, the equipped gear, the weapon class (`+0`), the hunting style (`+5`), the five
pigment colours, the default-colour flags and the name.

| `base +` | Size | sPlayer field | Same as header |
|---|---|---|---|
| `0x23A58` | 1 | `+0x9C5C` | — |
| `0x23A59` | 224 | `+0x2BC` | `+0x02C` |
| `0x23B39` | 14 | `+0x84` | — **equipped gear**: 7 × u16 equipment box index (weapon, head, chest, arms, waist, legs, talisman; `0xFFFF` = none). The loader copies each entry from the box into the equipped cache (`0x275660`). 22, 45, 51, 57, 62, 69, 199 in the analysed save |
| `0x23B47` | 12 | `+0x550` | `+0x240`, weapon class (`+0`, equipped weapon type − 7), gender (`+4`), hunting style (`+5`) |
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

## The item manager sItem

`base + 0x22497`, 5569 bytes, loader `0x1979b0`. Earlier documents call it the
activity manager. Its methods are in the `sItem` unit (`0x193000 … 0x199000`);
**DERIVED** from the loader and the MT classes of the methods' callers. CONFIRMED by the
[save timeline](#confirmed-by-the-save-timeline): the items-obtained map, the G-rank
accumulator and staged counter 0.

| `base +` | sItem | Size | Content |
|---|---|---|---|
| `0x22497` | `+0x9c` | 94 × u32 | items obtained: bit = item ID. 1173 of the 1177 item IDs in the analysed item box have their bit set; the four without it (485, 503, 1246, 1841) were already in the oldest snapshot, probably put there by a tool. Every item that entered the box or pouch in game got its bit in the same save |
| `0x2260F` | `+0xea4` | 3 × 136 | the Trader's three cargo orders (`cUIOTradeCenterCargo`, `cUIOTradeCenterBox`) |
| `0x227A7` | `+0x103c` | 10 × 420 | Alchemy requests (`uUIAlchemy`, `cUIOAlchemyRequest`): u8, u8 (255 = empty), u16, three 36-byte equipment entries, then seven of (u32, u32, 36-byte equipment entry). All ten are empty in the analysed save. Stride in memory 500 |
| `0x2380F` | `+0x8c` | 10 | village tier bytes ([10](10-npc-talk.md)) |
| `0x23819` … `0x2381D` | `+0x6a` … `+0x6e` | 5 | counters that feed the pending rewards below. `+0x6a` u8: Trader cargo shipments (`0x195758`, at each quest result per active order); at 10, 40, 90, 140 it adds 5 to pending byte 1, then drops back to 90 (every 50 after that). `+0x6b` u8: low-rank contribution points (`0x523a80` → `0x1973b0`), every 250 add 2 to pending byte 3. `+0x6c` u16: G-rank contribution points (`0x1973ec`), every 800 add 2 to pending byte 22. `+0x6e` u8: Footbath uses (`uUIFootBath` → `0x197448`), every 15 add 3 to pending byte 4. Analysed save 60, 242, 105, 5 |
| `0x2381E` | `+0x6f` | 23 | pending village rewards ([10](10-npc-talk.md)), at most 99 each |
| `0x23835` | `+0x86` | 6 | staged quest-clear counters (`0x1974ac`, from the quest result `0x38f278`; table `0x1625aee`: pending byte, threshold, amount). Counter 0: G★1–4 quests cleared, every 10 add 1 to pending byte 18. Counters 1 → 2 and 3 → 4: villager-request quests cleared, a one-off payout at 10 (3 to pending byte 17), then every 20 / 30. Which of the two chains runs depends on a byte of another singleton (GOT `0x1838d28` `+0xb8`), UNRESOLVED. Analysed save `01 0a 06 01 00 00`. Counter 0 went 0 → 1 at the first clear of a Hub G★4 quest and stayed at three other clears (CONFIRMED); counters 1–4 stayed `0a 06 01 00` over two villager-request clears (*The Fated Four*, *Ahoy! Royal Ludroth!*), so the chain that fills them did not run there |
| `0x2383B` | `+0x50` | 25 | Combination List recipes combined successfully, bit = `itemPreData` record ID (`0x194c94` from the combine result `0x18b43c`). The list shows the real success rate only for these (`0x194474`). Award 27 *Sage's Tome* at 130 (`0x3eccc8`). A new character starts with 0, 1, 5; the analysed save has 9 |
| `0x23854`, `0x23954` | `+0x214`, `+0x314` | 256 each | the Combination List's user order, for the pouch list (`cMixListPouch`) and the item box list (`cMixListBox`). Byte 0 is flags: bit 1 = custom order in use (set by the move `0x194dd8`; `0x194d6c` returns the order only then), bit 0 = filter toggle (`0x194fa0`). Then one byte per position: the index of a recipe of `table/itemPreData.itp` (183 records of 24 B: u16 ID, u32 item A, u32 item B, u32 result, u32 rate %). Default `order[i] = i` (`0x193178`); all three slots hold the default |
| `0x23A54` | `+0x98` | u32 | Horns Coins traded in total, capped at 99,999,999 (`0x1978b0`, from `uUICrossCoin` and `uUICrossTicket`). The award check (`0x3f2810`) grants award 104 *Felicity's Picture Book* at 2000. 13 in the analysed save |

The equipment entries use the 36-byte box format of [07](07-equipment.md); the
helpers `0xdaba4` / `0xdac14` / `0xdadf0` construct, clear and copy them.

## Smaller managers

**DERIVED** from the loaders and the code that uses each field. CONFIRMED by the
[save timeline](#confirmed-by-the-save-timeline): play time and its remainder, hunting
buddy 1 and the pet names.

| `base +` | Owner | Content |
|---|---|---|
| `0x2246E` | sGameControl `+0x5e` | game options, 29 bytes: written by the Game, Chat and Network option windows (`cUIOOptionWindowFor…`), read by the quest camera and the players |
| `0x2248B` | `+0x34` | play time in seconds. The slot header (`+0x20`) and the own Guild Card (`+0x914`) are copies |
| `0x2248F` | `+0x38` | f32, the play-time remainder in frames: `0x3f83a8` (each frame, from `0x69fb24`) adds the frame delta and at 60.0 moves one second into `+0x34` (capped at 35,999,999). 28.19 in the analysed save, 0 in a fresh slot |
| `0x22493` | `+0x3c` | u32, copied to the own Guild Card `+0x86C` (`0x161ac8`) |
| `0x23B9D` | sOtomo `+0x13848` | five u8: the Palico played as Prowler, the two hunting buddies (Palico index 0–83, `0xFF` = none; `0x25f880`, `0x25f840`), Palico Dojo sessions completed (at most 100; award 53 at 50, `0x3ed174`), Palicoes hired in total (at most 200, `0x25e88c`; title words at 10, 30, 50, 80). Analysed save: none, none / 2, 0, 8; fresh slots `FF FF FF 00 00` |
| `0x23BA2` | `+0x138f6` | Palico service settings (`uUIOtomoService`): a 2-bit mode, seven small values (at most 9, 6, then 10 each, stored minus one at run time) and four u32, all bit-packed |
| `0x2C466` | `+0x13910` | 5 × 8 B: IDs of distinct Palicoes that reached level 50 (`0x262e1c`, at level-up after a quest). Five give award 50 *To the Best Hunter Ever* (`0x3f2084`). One in the analysed save |
| `0x2C48E` | `+0x48a78` | 16 B, the Palico Dojo teaching session: 4 Palico indices, a kind (1 = Palico skill from `rOtSkill`, else a support move), the skill or move ID, u16 in progress (the quest end `0x25c288` runs and clears it), two u32 UNRESOLVED |
| `0x2C49E` | `+0x48a88` | 3 × 16 B, Palico Dojo training slots: Palico index (`0xFF` = empty), type 0–3, sessions left (one per quest, `0x25c554`) |
| `0x2C4CE` | `+0x48ab8` | u8 count and 5 × u8 Palico indices: a team of up to five Palicoes (`0x261ebc`; `cMonNyanProcOtomoListWindow`, the Courier), probably the Meownster Hunters; each member gets experience after every quest (`0x26248c`) |
| `0x2C4D4` | `+0x4a5a0` | u32: bit 0 a Palico has reached level 20, bit 1 level 25 (`0x262db8`, the hire function). Tested by `0x262f68` for award-check conditions 148 and 16. 3 in the analysed save |
| `0x2C4D8` | sVillage `+0x472`, `+0x473` | Dark Piece and Dark Stone counts from the lottery of the cut Cave feature (`0x50636c`, run by the quest result and the title); the only reader is `uUICave`, whose text table holds only "NOT USED". 2, 1 in the analysed save |
| `0x2C4DE` | `+0x3e4` | village pets' affection, one u8 per village (0x50d728 adds, at most 10; reaching 6 unlocks a Guild Card title word). The Village Moofah NPCs read it |
| `0x2C4E2` | `+0x471` | times the Bherna Moofahs gave an item, at most 10 (`0x50d7e8`, after the gift of `S+0x3670`). Award 49 *Ball of Moofah Wool* at 10 (`0x3f31e4`) |
| `0x2C4E3` | `+0x3e8` | village pets' names, 4 × char[32]: Moofy, Poogie, Poogie, Poogie in the analysed save |
| `0x2C563` | `+0x468` | village pets' costumes, one u8 per pet, below 40 (the pet menu, `0x50d830`) |
| `0x2C567` | `+0x46c` | u32, bit *v* − 1: a village pet event seen in village *v* (1–4; set by `0x50d890` from the village script). Each bit unlocks a title word (*Fluffball*, *Hide-and-Seek*, *Snow Sprite*, *Guardian Pig*), all four *Pet Lover*. The trigger in play was not traced |
| `0x2C56B` | `+0x470` | the Housekeeper (Room Service, `0x50c988`): index into the NPC table `0x162bde8` = 10, 16, 17, 18, 19, 710, 711 |
| `0x2C56C` | `+0x474` | the village to start in on load (`0x6a6ea8`), set at the quest result. 1 (Bherna) in all three slots. It also changed in four game sessions without a counted quest (6 → 1, 1 → 2, 2 → 1, 1 → 6), so another writer exists |
| `0x2C679` | sNpcTalk `+0x60c` | a second random word, copied with the first and tested modulo 10000 the same way (`0x24217c`) |
| `0x2C6A5` | sFlagChecker `+0x20` | flags: `0x3f43e8` ORs in the bits set at `+0x1c` of another object, so they latch |
| `0x2C6A9` | sMakeAmulet `+0x64` | a 128-bit map (`+0x64 … +0x73`) over a 107-entry table of 12-byte records. The quest end (`aQuest`, `0x14c688`) tests each entry's condition and sets its bit; the rotation modes 1 / 2 read it ([05](05-quests.md#rotating-quests--base--0x504b)). Entries 104–106 are in the fourth word, `0x700` in the analysed save. The transfer copies only the first 96 bits |
| `0x2C6B9` | `+0x74` | u32, 24 one-time event scenes (table `0x15a0c78`: u16 event ID, u8 village, u8). `0x14f5d4` queues a scene whose bit is clear (airship, branch select, NPC 024, village load); `0x14f660` sets the bit after it plays. `0x00FDFFFF` in the analysed save: all but bit 17 (event 212) |

The block A bytes of `sGameControl` are in [Block A header](#block-a-header-and-shared-settings).

## Equipment manager additions

**DERIVED.**

- **My Sets start at `base + 0x208CE`**, 6 bytes later than [07](07-equipment.md#my-sets)
  states. The loader reads 40 × 136 B from there. The field offsets in 07 are relative to
  `0x208C8`. Subtract 6 to get offsets within the record: name `+0x00`, box index
  `+0x2A`, pigment `+0x64`, default flags `+0x7D`, hunting style `+0x82`, Hunter Arts
  `+0x83`, art flags `+0x86`. Both conventions point at the same
  bytes; only the record boundary moves.
- **Palico equipment box**, `base + 0x17C2E`: 1000 × 36 B, same entry format as the hunter
  box. Only types 22, 23, 24 occur (Palico weapon, head, body). CONFIRMED: every gear
  index of the Palicoes points at an entry of the right type, and a fresh slot holds the
  five starter entries.
- **Palico equipment sets**, `base + 0x21E0E`: 24 × 68 B. `+0x00` name char[42],
  `+0x2A` 3 × u16 box index (`0xFFFF` = empty), then flags.

## Palico records

**CONFIRMED** from the loader (`0x263390`: 32 + 224 + 14 + 12 + 36 + 4 + 2 bytes per
record) and the [save timeline](#confirmed-by-the-save-timeline): after each of four quests
only the hunting buddy's record gained experience. Checked in game (2026-10-05, Palico
Info of Suds): Lv 64, bias Gathering, the comment and "Palico 1" match level byte 63,
bias 6, the greeting and buddy 1 = its index.

| Offset | Size | Field |
|---|---|---|
| `+0x00` | char[32] | name |
| `+0x20` | u32 | experience |
| `+0x24` | u8 | **level − 1** (Lv 64 in game = 63). The level-up code `0x262e1c` counts 49 as level 50 (the level-50 list, award 50) and 98 as the top level, 99 (milestone bit of `S+0xd8c`) |
| `+0x25` | u8 | support bias (parameter block `+5`, see [StreetPass Palico record](#streetpass-palico-record-276-b)): 6 = Gathering in game |
| `+0x26` | u8 | parameter block `+6`, a 0–99 value capped per entry, UNRESOLVED (55–99 seen; 55 in all but eight records of slot 1). Suds: 89 with 4 of 5 Enthusiasm marks, so possibly Enthusiasm |
| `+0x27` | u8 | target (1–5 seen): 4 = Large First in game |
| `+0x60` | char[60] | greeting |
| `+0x9C` | char[32] | original owner name |

Lists: `base + 0x23BB6` 84 records (the Palicoes of the slot) and `base + 0x2A606` 24
records (same format: the Palicoes for hire, rerolled after each counted quest and at no
other step). Block A holds a shared pool of 50 records. In the
analysed save it contains DLC Palicoes with owner "Capcom".

## Guild Card manager

**DERIVED** (loader `0x163e0c`, writer `0x1641b0`, container load `0x3cf6d4` / save
`0x3cf928`). In `base +` order:

| `base +` | Size | Content |
|---|---|---|
| `0x2C6BD` | 633,600 | list 1: the stored Guild Cards, 100 elements, then padding |
| `0xC71BD` | 6328 | **own Guild Card** |
| `0xC8A75` | 4400 | list 1 card info: 100 × 44 B, [below](#card-info-records) (manager `+0x18D8`) |
| `0xC9BA5` | 316,800 | list 2: the Guild Card inbox, 50 elements, then padding |
| `0x117125` | 1800 | list 2 card info: 50 × 36 B (`+0x2A0C`) |
| `0x11782D` | 276 | the StreetPass Palico you send: one 276-byte record (`+0x3114`) |
| `0x117941` | 13800 | Palico inbox: 50 × 276 B, same record (`+0x3228`) |
| `0x11AF29` | 1800 | Palico inbox info: 50 × 36 B (`+0x6810`) |

**DERIVED.** List 2 and the two Palico lists belong to the Courier Service, which the
Courier describes in his talk file: received Guild Cards go to an inbox, received
Palicoes to a Palico inbox, and the player picks one StreetPass Palico to send. The Post
Office screen (`uUIPostOffice`) reads all three. The 276-byte record is built by
`0x165f44` from one of the player's Palicoes (`0x26e354` / `0x26e600`, the Palico code)
and the hunter name; `0x165a24` treats an inbox slot as empty when its first 8 bytes
match a constant, `0x165a9c` deletes slot i and moves the later ones up. List 2's card
info records start with the receive date (u16 year at `+2`, set by `0x165348`). All
three are empty in the analysed save.

### StreetPass Palico record (276 B)

**DERIVED** from the pack `0x526860` (called by `0x165f44`) and the unpack `0x5265b8`.
It is a cut-down copy of the owned-Palico record (324 B, sOtomo loader `0x263368`):

| Offset | Size | Content | 324-B record |
|---|---|---|---|
| `+0x00` | 8 | the sender's hunter ID (own card `+0x8B0`) | — |
| `+0x08` | 22 | Palico name, UTF-16 (11 characters) | `+0x00`, char[32], converted |
| `+0x1E` | 12 | appearance bytes; `+0x21` (colour preset) is sent and received as 0 | `+0x10E` |
| `+0x2A` | 2 | padding (stale stack bytes) | — |
| `+0x2C` | 9 × u32 | colours; slot 3 is replaced on send by the game's first default colour | `+0x11A` |
| `+0x50` | 196 | the first 196 bytes of the Palico's 224-byte parameter block: support bias `+5`, target `+7`, 8 equipped support moves `+8`, 16 learned move slots `+0x18` (57 = none). Byte `+6` (a 0–99 value capped per entry) is forced to 55 both ways, UNRESOLVED | `+0x20 … +0xE3` |

Not sent: the rest of the parameter block, the equipment (a received Palico gets
defaults, `0x26e964`), the u32 `+0x13E` and the u16 `+0x142`. Before packing,
`0x262838` resets the sender's copy's support-move and skill slots.

### Card info records

One 44-byte record per list 1 element. **DERIVED** from the receive code `0x16449c`
(called by `uUIReceive` with the console date and the sender's ID) and the list
screen `cUIOGuildCardList`:

| Offset | Size | Content | Analysed save |
|---|---|---|---|
| `+0x00` | 4 | date received: u8 day, u8 month, u16 year | 30 August 2026 for both cards |
| `+0x04` | 24 | the list's comment, UTF-16, up to 11 characters (*Add Comment*); cleared on receive | empty |
| `+0x1C` | u32 | **Unity** with the card's owner (sort *By Unity Level*, `0x59d580`; shown by the list row, `0x59c274`). The quest end (`0x38ce60`) adds 76 for each party member whose card ID matches, capped at 99,999; the own card's total is `+0x870`. 0 on receive | 7828, 3496 |
| `+0x20` | s8 | **card type** (*Change Card Type*, sort *By Type*, `0x59d47c`): 0–23 = *GuildCardMsg* 211–234 (Partner, Friend, Classmate, … Passerby, Sweetheart), 25 = StreetPass. A direct receive sets 0, a card moved from the inbox 25 (`0x164afc`) | 0 |
| `+0x21` | 8 | sender ID, unaligned | `fadefade fadefade` |
| `+0x29` | 3 | — | 0 |

A list element is:

1. u32 compressed length;
2. zlib stream of one 6328-byte card;
3. u32 state (3 = card, 1 = empty);
4. a 36-byte trailer: u16 HR (sort *By HR*), 11 UTF-16 characters of name (sort *By
   Name*), 8-byte ID, u32 version. The version is the card's play time (card `+0x914`:
   364261 and 231955 for the two stored cards), so a card arriving with the same name and
   ID replaces the stored one only if it has more play time (`0x164b08`, "Updated").

The list screen's sort modes (`0x59d30c`, *GuildCardMsg* 199–204) are slot order, HR,
date, Unity, type and name.

After the elements comes one padding block of Σ(`0x18B8` − length − 36) bytes. The
list size is therefore fixed: n × (`0x18B8` + 8), which is 633,600 B for list 1 and
316,800 B for list 2. The game fills the padding from an uninitialised heap buffer. It
contains stale strings and must not be read as data.

The analysed save holds two received cards in list 1. Each decompresses to 6328 bytes
with the same layout as the own card (history log at `+0x918` in both).

### Card layout (6328 B)

**DERIVED** from the card builders `0x1623b4` (hunter) and `0x162574` (Palico), the
award test `0x162720` and the history insert (`0x1628a0`). Checked on the own card and
a received card: HR 999 / 148, sensible equipment types, transmog IDs. CONFIRMED by the
[save timeline](#confirmed-by-the-save-timeline): name, equipment, weapon usage, play
time, quest history, awards and monster log.

The equipment and Palico blocks are a snapshot, not a mirror: over 36 saves the
equipped gear changed three times with quests in between and card `+0x54` never moved;
the card's buddy 2 is a Palico while sOtomo has none. Only play time, history, weapon
usage, awards and the monster log follow each quest end. What rebuilds the rest is
not identified.

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
| `+0x854` | 3 × u16 | title: the card editor's fields 0–2, word, connector (`GC_Title_2`), word. The own card has 140, 0 (none), 502 | cUIOGuildCardEdit |
| `+0x85A` | u8 | scene (editor field 4): 35 on the own card, of 136 | cUIOGuildCardEdit |
| `+0x85B` | u8 | pose (editor field 3; a change calls `0x1605b8`): 3, of 22 | cUIOGuildCardEdit |
| `+0x85C` | u16 | HR of a transferred save, 0xFFFF = none; copied from `S+0x41a` | `0x161ac8` |
| `+0x86C` | u32 | copied from `sGameControl +0x3c` | `0x161ac8` |
| `+0x878` | | greeting, UTF-16 | |
| `+0x8B0` | 8 | the card owner's ID (the arena records name hunters by it; the leaderboard reads it as 4 × u16) | `0x6e7800` |
| `+0x8B8` | u8 | flags (bit 1 from sPlayer `+0x9C5C`; bit 7 set = the award screen shows award 100 instead of 101, [09](09-awards.md#layout)) | `0x5a88f8` |
| `+0x8BA` | 3 × 15 × u16 | weapon usage, Village / Hub / Arena ([04](04-weapon-usage.md)) | |
| `+0x914` | u32 | play time ([05](05-quests.md)) | |
| `+0x918` | 10 × 160 | quest history ([05](05-quests.md)) | |
| `+0xF58` | 20 | awards, bits 0–159 (132 used, [09](09-awards.md)); copied from the game-side award map `S+0xc28` | `0x161b98` |
| `+0xF6C` | 87 × 8 | monster log, [below](#monster-log) | `0x161c38` |

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
1900), u16 record kind `+4`, u16 quest ID `+6`, name `+8`, and three highlights at
`+0x28` ([05](05-quests.md#quest-history-log--0x254771)). The ten records fill
`+0x918 … +0xF58`; the generic insert `0x162758` shifts them down by one.

Record kinds come from separate builders: 3 (`0x162888`, called by the Smithy at
`0x6fb160`), 4 (`0x162b54`, from `0x702e48`), 5 (`0x162e0c`, from the Smithy at
`0x6fb0dc`), 6 (`0x1630c4`, from the award checks), 7 (quests, built at quest object
`+0x2f8` and inserted by `0x390434`). `0x163454` (talk code) and `0x163aa4` build two more.

### Monster log

**CONFIRMED** from the builder loop `0x161c38` (87 entries from a monster table, stride
8, `+0xF6C … +0x1224`) and the save timeline: in all 36 saves every entry equals the
size records and the family sums of the hunt and capture tallies, and the totals moved
at exactly the quests:

| Offset | Size | Content |
|---|---|---|
| `+0` | u16 | largest size (`0x526f48` with argument 1) |
| `+2` | u16 | smallest size (`0x526f48` with argument 0) |
| `+4` | bits 0–13 | hunts, at most 9999 |
| | bits 14–27 | captures, at most 9999 |
| | bit 28 | the monster's Hunter's Notes entry is unlocked (`S+0x35b0`, `0x524728`) |
| | bits 29–30, 31 | crown marks derived from the size records (`0x67310`) |

### Arena log

`+0x1224 … +0x1378` is the Arena log: 17 entries of 20 bytes (5 × u32), one per Arena
quest of the Arena Counter's table (`0x164fdb4`, [above](#the-block-s0x20--0x41f--base--0x280b)).
The initialiser `0x161184` sets each u32 to `0x63800000` (Arena) or `0x61400000`
(Prowler Arena, test `0x3b5ac0`). **DERIVED** from the writer `0x3b123c` (caller
`0x6a1320`) and the leaderboard code (`0x6e47a4`, `0x6e7488`).

**The five u32 of a quest are its five best times, best first.** A new time goes in at
its rank and the lower entries move down (`0x3b1400` … `0x3b1528`); a time no better
than all five is dropped. Each u32:

| Bits | Content |
|---|---|
| 0–17 | time in 1/100 s, at most 180,000 (`0x3b1530`) |
| 18–25 | weapons, *w*: the hunter's is *w* mod 15 and the partner's (*w* / 15) mod 15, 14 = none. Prowler quests use 9 instead of 15 (support bias, 8 shown as 14). Written last (`0x3b181c`) |
| 26–28 | the quest's equipment set used, 0–4 (`0x3b1564`); the same index sets bit 5 × quest + set of `S+0xca0` |
| 29–30 | grade 0, 1, 2 when the time is within the quest's three thresholds, 3 = none. Readers (`0x3b8db4`, `0x3f1a64`) test ≤ 2 |
| 31 | always 0 |

The initial values decode as grade 3, set 0, no weapons. The partner's 8-byte ID is
at `+0x16D4` + 8 × quest and is copied only when the new time takes rank 1
(`0x3b166c`); the card owner's own ID is at `+0x8B0`. The own card has one entry:
quest 0, `0x0F682125` = 84.85 s, weapon 218 mod 15 = 8, set 3, grade 0, no partner.
It agrees with the save object's best time for that quest (`S+0x134`) and with bit 3
of `S+0xca0`. The rest of the card, `+0x1378 … +0x18B8` (partner IDs included), is zero
on the own card.

## The save object S

S is the `sUserInfo` object (global `0x1897f78`). The character block stores its
fields in loader order, not in object order, so a field's file offset cannot be
computed from its object offset: always go through the
[CSV](../data/save-map.csv) (column `object_field`, `obj17+X` = `S+X`). **Earlier
documents got this wrong once:** the Hunter's Notes maps are at `base + 0x32B7`, not
`base + 0x5027` (corrected below and in [10](10-npc-talk.md)).

**DERIVED** unless a row says CONFIRMED (the [save timeline](#confirmed-by-the-save-timeline)
backs those). Each field was named from the methods that use it: the bit
tests and setters of the `sUserInfo` unit (`0x51c900 … 0x528000`), every load of the
global followed through the function, and the MT class of the callers. A caller's
class comes from its vtable: entry 5 is `getDTI`, and the DTI object gets its name
from the static constructor call `0x7aedcc(DTI, "name")`.

### The block `S+0x20 … +0x41f` — `base + 0x280B`

The loader reads these 1024 bytes in one piece (`0x51d0a0`), so here the file keeps the
object order: `base + 0x280B + (X − 0x20)` for `S+X`. Its layout also shows in
`0x51e6d8`, which fills S field by field from an older save layout: four style
counters instead of six, no G-rank contribution points (most likely the MHXX data
transfer).

| `base +` | S field | Content |
|---|---|---|
| `0x280B` | `+0x20` | u32 HR points ([05](05-quests.md)). The transfer sets it to the points of HR min(HR, 7) (table `0x15973bc`). CONFIRMED by write: 2,001,420 points showed HR 500 in game and the game kept them |
| `0x280F` | `+0x24` | u32 funds (zenny); the adder `0x523150` keeps it within 0 … 9,999,999. CONFIRMED: a quest reward took it to exactly 9,999,999, a game session spent 960, and the slot header copy `+0x24` matched in all 36 saves |
| `0x2813` | `+0x28` | u32, a copy of sItem `+0x69`: the quest result (`0x195758`) draws it at random below n = 1, 2 or 3, n growing with the Village and Hub star levels. Meaning UNRESOLVED |
| `0x2817` | `+0x2c` | u32 Wycademy points, 0 … 9,999,999 (`0x523194`). CONFIRMED: +4110, +540, +3720, +1200, +2640 at the five counted quests, no change at talks, tool writes or the Harvest Tour |
| `0x281B` | `+0x30` | u32 × 4 contribution points, low rank, Bherna / Kokoto / Pokke / Yukumo ([10](10-npc-talk.md)). CONFIRMED: request rewards replaced a tool-written 9,999,999 with the cap 20000, village by village, while the low-rank accumulator (`base + 0x2381A`) rose |
| `0x282B` | `+0x40` | u32 × 4 contribution points, G rank. CONFIRMED: +1750 over the four villages in the report round, and the G-rank accumulator went 755 → 105 = (755 + 1750) mod 800 |
| `0x283B` | `+0x50` | u8. UNRESOLVED |
| `0x283C` | `+0x51` | u8 × 18 Special Permits held per deviant, at most 99 (`0x527a80`, see `S+0x4424` below) |
| `0x284F` | `+0x64` … `+0x7b` | 12 × u16, zero in the analysed save. UNRESOLVED |
| `0x2873` | `+0x88` … `+0xb3` | 44 bytes copied as one block by the transfer. UNRESOLVED |
| `0x28AB` | `+0xc0` | u8 counter, the quest result adds 1 (at most 255) when byte `+0x5b` of the quest data is set (`0x388b28`). 18 in the analysed save |
| `0x28AC` | `+0xc1` … `+0x10f` | u8, then u16 fields (8 at `+0xc2`, a u32 at `+0xd4`, 28 at `+0xd8`). UNRESOLVED |
| `0x28FB` | `+0x110` … `+0x117` | u8 and two u16 of talk condition 54 ([10](10-npc-talk.md)) |
| `0x2903` | `+0x118` | u8; the Start Menu raises a notice bit once it reaches 50 (`0x3f5200`). UNRESOLVED |
| `0x2905` | `+0x11a` | **hunting style use counts**, 6 × u16: Guild, Striker, Aerial, Adept, Alchemy, Valor. Talk condition 29 (sub-tests 56–67, `0x2493c8`) picks the most and least used; the Palico's lines name the style. The transfer fills only the first four. 0, 0, 1, 26, 0, 375 in the analysed save |
| `0x2913` | `+0x128` | 3 × (u8 day, u8 month, u16 year): the dates of the Arena Counter's *Latest Updates* (`Lb_ArenaCounterMsg` 13, 14) |
| `0x291F` | `+0x134` | **Arena best times**, 57 × 12 B: u32 time in 1/100 s, then the 8-byte Guild Card ID of the hunter who set it. Index = the Arena Counter's quest table `0x164fdb4`: 0–10 Arena quests 20001–20011, 11–16 Prowler Arena 120001–120006, 17 onwards the challenge quests 1020001 …. The leaderboard (`0x6e47a4`) compares it with the cards' Arena logs. The analysed save has entries 0 (84.85 s, own ID), 6, 8 and 16 (the ID of a card in list 1) |
| `0x2BCF` | `+0x3e4` … `+0x403` | fields set only by the initialisers `0x51cbe4` / `0x51ce44`. UNRESOLVED |
| `0x2BEF` | `+0x404` | u8 × 3, the quests of the three *Latest Updates*, index into the same table (17 = none) |
| `0x2BF3` | `+0x408` | u32 taken over by the transfer (old `+0x264`). UNRESOLVED |
| `0x2BF7` | `+0x40c` | u8 × 3: the quest counter's last three daily picks, newest first (255 = none; `0x5264f4`, `cUIOQuestCounterDailyInfo`) |
| `0x2BFB` | `+0x410` | u64, Unix time of the last daily pick (2026-09-19 17:25 UTC in the analysed save) |
| `0x2C03` | `+0x418` | u8 Jukebox song chosen; 0 = none, the village plays its own music (`sSoundControl`, `uUIJukeBox`) |
| `0x2C05` | `+0x41a` | u16 HR of the transferred save (`0x6b1eec`), 0xFFFF = no transfer. Copied to the Guild Card (`+0x85C`) |
| `0x2C07` | `+0x41c` | u32 HR points of the transferred save |

### Unlock maps

Most of S is unlock maps. A map comes with two copies of the same size:

| Copy | Saved | Role |
|---|---|---|
| U | yes | unlocked / owned |
| N1 | some | set with every newly set bit of U, cleared when the session's notice runs |
| N2 | yes | the NEW mark: set with every newly set bit of U, cleared when the player views the item |

A setter only touches N1 and N2 when the bit of U was clear. So an editor that unlocks
something should set U and N2 (the game then shows it as NEW), or U only (no NEW mark).

| `base +` | S field | Content |
|---|---|---|
| `0x2C0B` | `+0x424` | bonus packs announced. Bit N (1–4) = the Room Service showed the notice for privilege pack N (`0x78b6e4`) |
| `0x2C0F` | `+0x428` | bonus packs granted. Bit N = the contents of pack N were given (`0x522c64` → `0x522d60`). Owned packs are bits 1–4 of the shared `S+0x420` ([below](#block-a-header-and-shared-settings)). A pack is a list of records: type 5 unlocks a title word, type 6 a Guild Card scene |
| `0x2C2B`, `0x2C43` | `+0xd3c`, `+0xd54` | N1 and N2 of the Hunter Arts map at `0x2C13` |
| `0x2C5B` | `+0xd6c` | 31-bit map. `0x524580` ORs in `S+0xd74` when a quest ends, monster code (`uEm014`, `uEm022`, `uEm085` through `0x524534`, table `0x162c124`) sets `S+0xd74`. Alchemy counts it over a range (`0x5244c8`) |
| `0x2C5F` | `+0xd70` | u32 map tested by `0x5245d4`. The pending map `+0xd74` is not saved (the next field is `+0xd78`) |
| `0x2F77`, `0x2F7F` | `+0x958`, `+0x968` | the progress map is 64 bits (U `+0x958`, N1 `+0x960` unsaved, N2 `+0x968`). The progress word of [10](10-npc-talk.md) is its first half |
| `0x2F87`, `0x2F8B` | `+0x970` … `+0x978` | 32-bit map with copies: story events of the flagship monsters. The quest flow (`0x14d53c`) looks the current quest up in a 12-byte table (`0x15a06fc`: code, 1, quest ID) and sets bit 3 for 601 *The Scorching Blade* (Glavenus), 4 for 504 *The Thunderclaw Wyvern* (Astalos), 5 for 510 *The Entrancing Water Dancer* (Mizutsune), 6 for 508 *The Unwavering Colossus* (Gammoth), 8 for 1005 *Beware the Comet of Disaster* (Valstrax); bit 2 when quest 601 is cleared (`0x524064`). The village (`0x11e10`) reads it with its copies. Bits 0–13 set in the analysed save. Other bits UNRESOLVED |
| `0x2F97` | `+0x98c` | N2 of the Canteen ingredients |
| `0x2F9F`, `0x2FA7` | `+0x994`, `+0x9a4` | Poogie costumes, 64 bits. The award check sets award bit 68 (`GC_Medal` entry 68 is *Poogie Ball*, "collected some of the Poogie costumes") once 10 of bits 6–39 are set, bits 28, 35, 36 and 39 not counted (`0x524190`). The Trader's list builder also tests it |
| `0x2FAF`, `0x2FB3` | `+0x9ac` … `+0x9b4` | deviants, 18 bits: bit i is set when the first Special Permit quest of deviant i becomes available (`0x3eb1f4`, table `0x162a5f4` = 40101, 40201 … 41811); also raises event flags 185 and 962 |
| `0x2FB7`, `0x2FBB` | `+0x9b8` … `+0x9c0` | deviants the Courier hands out Special Permits for, 18 bits, same setter. `0x52779c` / `0x5278a8` lend the highest bit temporarily in two modes and keep what to restore at `S+0x43fc` / `S+0x4400` |
| `0x2FBF` | `+0x9c4` … `+0x9cc` | a 32-bit map with copies, vestigial: only the transfer converter `0x51e6d8` writes it (6 bits from the old save's `+0x790` … `+0x798`, the map after the old deviant maps). No reader in this game; 0 in all three slots |
| `0x2FC7`, `0x306B` | `+0x9d0`, `+0xb18` | Guild Card title words, first part: 1312 bits for the 1309 words of `GC_Title_1`. 79 are unlocked from the start (`0x162154`): a fresh slot has exactly the 79 words whose text says "Available from the start." (CONFIRMED with the NEW copy and the scenes map, [timeline](#confirmed-by-the-save-timeline)) |
| `0x310F`, `0x311F` | `+0xbbc`, `+0xbdc` | title words, second part: 121 words of `GC_Title_2`, 117 from the start |
| `0x312F`, `0x3143` | `+0xbec`, `+0xc14` | Guild Card scenes, the 136 backgrounds of `GC_background` |
| `0x317F`, `0x3183` | `+0xc64`, `+0xc6c` | Guild Card poses: 22 (Stand … Beam Fire). 17 from the start, bits 17–21 from the DLC map at sPrivilege `+0xf44` |
| `0x31A7`, `0x31CB` | `+0x2a0c`, `+0x2a54` | Smithy decorations listed / NEW, bit = `rDecoCreateData` entry |
| `0x31EF` … `0x32AB` | `+0x3488` … `+0x35a4` | the Trader (`uUITradeCenter`): seven maps, each followed by its N1 and N2. U at `+0x3488` (32 bits) and `+0x3494` (32): entries of the Trader's two item lists (UI byte `+0x3c` = 0 / 1), set by `0x7a9abc` once the entry's progress condition (`0x561b28`) holds. `+0x34a0` (448): Guild Card title words for sale (`tradeLimitedHonorList`, 442), `+0x3548` (160): Guild Card scenes for sale (`tradeLimitedPaperList`, 131), `+0x3584` (32): pet costumes for sale, the three set by the tabs of [block B's download test](#downloads-held--block-b-header). `+0x3590` (32): the coin-ticket trades (`rTradeCoinTicketList`, paid in *Horns Coin*); only the transfer converter sets it, the Cross ticket screen and the Trader read its N2 as NEW. `+0x359c` (32): delivery requests offered at the Trader, bit = `rTradeDeliveryList` entry, set by `0x79da8c` once the request's event flag is raised (`0x1971f0`); `0xff5` in the analysed save |
| `0x32AF` | `+0x35a8` | delivery requests delivered: bit *b* for kind-1 request *b* in [`request-index.csv`](../data/request-index.csv) order, 0–12 (CONFIRMED: bits 0, 4, 5, 7 = the done flags of those requests), tested by `0x524db8` for talk condition 41 ([10](10-npc-talk.md)), the Trader and the Start Menu |
| `0x32B3` | `+0x35ac` | Hunter's Notes tips read: a clear bit shows NEW (`cUIOHunterNoteTips`) |
| `0x32B7`, `0x32C7` | `+0x35b0`, `+0x35c0` | **Hunter's Notes, large monsters**: 123 bits and their NEW copy. Talk action 6 sets both (`0x247ae4`); condition 44 tests them (`0x245848`). The map is CONFIRMED by write: clearing bit 79 removed Rathian from the Notes in game (the NEW copy stays DERIVED) |
| `0x32D7` | `+0x35d0` | **Hunter's Notes, second list**: 30 bits, talk action 7 and condition 45 |
| `0x32DB` | `+0x35dc` | 64 bits: DLC item packs this character has taken from the Room Service (CONFIRMED; earlier revisions said Palicoes). Bit *b* pairs with bit *b* of sPrivilege `+0xf3c` (DLC item packs held). `uUIRoomService` sets it after the hand-over (`0x78ccb0`), buzzes on a second try (`0x78d01c`) and shows a notice while a held one is not taken (`0x78a39c`). `0x1ffff` (17) in the analysed save, the same as the held map; 0 in a fresh slot |
| `0x32E3` | `+0x3668` | u32 flags. Bit 0: a network-mode switch mirrored to sFestaNetwork `+0x1624e` (`0x228310`, set by `uUILobbyStartMenu`, copied back by the title menu; read by the room list and menu bar), probably local vs online, UNRESOLVED. Bit 1: today's quest-counter daily bonus received (set by `0x3bb96c` after the bonus is paid, cleared when new daily picks `+0x40c` are rolled, `0x3babf4`; read for the board icon). No other bit is used. `+0x366c` is a runtime field of the lobby code, not saved. 0 in all three slots |
| `0x32E7` … `0x3347` | `+0xd98` … `+0xe38` | two shop lists (`uUIGuildShop`, list index at UI `+0x8c`): 256 bits listed + NEW each, stride 96 |
| `0x3367` | `+0xe58` | Armory (equipment shop): 21 equipment types × (20 B listed, 20 B NEW), stride 60 with the N1 copy between. Bit = shop entry of the type |
| `0x36AF` | `+0x1344` | Smithy weapon lists, types 7–21: 15 × (20 B listed, 20 B NEW), stride 60 |
| `0x3907` … `0x4327` | `+0x16c8` … | Smithy armor lists, head … legs: 5 × (288 B listed, 288 B NEW), stride 864. The list builder `0x6f9438` sets an entry when it first lists it; the cursor clears NEW (`0x524a38`) |
| `0x4447` … `0x459B` | `+0x27a8` … | Palico smithy, weapons / helms / mail: 3 × (68 B listed, 68 B NEW), stride 204 |
| `0x45DF` … `0x4CDF` | `+0x2a78` … | 15 weapon types × 1024 bits, by weapon ID. Set by the Smithy list builder `0x6ffd40` for the entries it shows |
| `0x4D5F` | `+0x31f8` | the same for armor, 5248 bits by armor ID |
| `0x4FFB`, `0x5023` | `+0xca0`, `+0xcf0` | Arena: bit 5 × quest + set = the Arena quest was cleared with that of its five equipment sets (`0x3b13c8`, read by `uUIArenaCounter`) |
| `0x505B`, `0x505F` | `+0x35e4`, `+0x35ec` | Jukebox songs. `0x523b9c` unlocks the default ones |
| `0x5063`, `0x506F` | `+0x35f0`, `+0x3608` | Lab upgrades offered (96 bits, set by quest clears in `0x524e58`) |
| `0x507B`, `0x5087` | `+0x3614`, `+0x362c` | **Lab upgrades installed** (Soaratorium Lab, `researchReinforce`): bit = upgrade − 1. Bits 0–2 are the three Item Box expansions; `0x525878` counts them for the box size (202 callers) |
| `0x5093`, `0x509B` | `+0x3638`, `+0x3648` | supply drop sets of the Provision Division |
| `0x50A3`, `0x50AB` | `+0x3650`, `+0x3660` | Cross coin trades |
| `0x50B3`, `0x50B7` | `+0xd18`, `+0xd20` | deviants, 18 bits: bit i is set when quest 40000 + 100 (i + 1) + 16 is cleared, the deviant's EX level (`0x3f18b8`; deviant order of [03](03-deviants.md)) |

### Counters and other fields

| `base +` | S field | Content |
|---|---|---|
| `0x4FEF` | `+0xd8c` | 96-bit milestone map that award checks read. Bits 20–23: an unusual Moofy / Poogie moment seen in Bherna, Kokoto, Pokke, Yukumo (awards 48, 69, 84, 99, `0x3f31b8`); bit 34: every Footbath visitor talked to (set by `0x70eb98`; award 88); bits 35–41: a Palico of each support bias hired (award 51, `0x3eed68`); bits 57–64: support biases at their top level (award 128, `0x3f1da4`). Set in the analysed save: 0–4, 19, 24–35, 41, 56. The other bits UNRESOLVED |
| `0x5053` | `+0x3670` | bit 0: *Moofah Fleeceball* given since the last quest. Petting a Moofah (`0x6be478`) gives item 524 once while the bit is clear, sets it and unlocks title word 115 *Hugs*; the quest result (`0x38b948`) clears the u32 |
| `0x50BB` | `+0x3680`, `+0x3682` | u8, then u16: the large monsters hunted in a transferred save. Character creation (`0x6aa5ec`, the same routine that runs the transfer converter `0x51e6d8` and grants award 100 or 101) sums hunts + captures of the old save's monster list (`0x6b1e6c`, 159 entries, each capped at 9999). The common script and the Start Menu test it (> 199). 0 in the analysed save |
| `0x50BE` | `+0x3684` | 3444 bytes. Only the initialiser (`0x51cb08`, memset from `+0x3682` up to `+0x43f8`), the saver and the loader touch them; no indexed, method or added-base access reaches the range. Zero in all three slots. Reserved |
| `0x5057` | — | not S: the u32 `+0x2838` of the chat-phrase object, loaded inside the S stream (`0x55d450` → `0x1cab58`). A value ≥ 0 is replaced by `0xF8FC7E3F` at load |
| `0x5E32` | `+0x43f8` | 3 bytes, only the saver and loader touch them (not even the initialiser). Reserved |
| `0x5E35` | `+0x43fb` | control option byte: set by the Game options window, read by the player and the target camera. Cleared together with `+0x446c` (`0x3f7e1c`) |
| `0x5E36`, `0x5E3A` | `+0x43fc`, `+0x4400` | the Courier's lent bit, see `+0x9b8` |
| `0x5E3E` | `+0x4408` | u64: the Nintendo Account ID linked to the 3DS save-transfer server (getter `0x5279b4`, setter `0x5279cc`). Set by the link and transfer flows (`0x690640`, `0x6916f4`, …); the title menu (`0x690068`) compares it with the current user's account and shows *HD_DataTransfer* 43 ("different to the Nintendo Account linked to the server") when they differ. A shared copy sits in block B ([below](#downloaded-quests--block-b)). 0 in all three slots |
| `0x5E46` | `+0x4410` | u32, only the saver and loader touch it. Reserved |
| `0x5E4A` | `+0x4414` | Courier flags (`0x6d1e4c` and the Courier's talk code) |
| `0x5E4E` | `+0x4418` | u32 quest counter ([05](05-quests.md#counters)): completed quests except Harvest Tours and Training. `0x526f70` adds 1 when the quest ends completed (sQuest `+0x50` = 2), not when abandoned (6) or when the quest file's skip bit is set (`0x3a340c`) |
| `0x5E52` | `+0x441c` | the counter at the Courier's last talk (`0x6d19e8`). One-time gifts at 10, 30, 50, 80, 100 and 150 quests (flags `+0x4414` bits 0–5), periodic ones when a multiple of 3, 7 or 10 was crossed. `0x5279e0` stores it and, at 210 (lcm of 3, 7, 10) or more, folds both into 210 … 419. 387 / 382 in the analysed save |
| `0x5E56` | `+0x4420` | Courier points: `0x526f70` adds a per-quest amount from a table. Fell from 11600 to 10000 at the first quest of the timeline and stayed there, which looks like a cap of 10000 |
| `0x5E5A` | `+0x4424` | Special Permit points, 18 × u16, at most 9999. 100 points make one permit; the Courier (`0x527a80`, `0x527b2c`) hands them out up to 99 held, the held counts being the [deviant permit counts](#the-block-s0x20--0x41f--base--0x280b) at `base + 0x283C` (`S+0x51`) |
| `0x5E7E` | `+0x4448` | permit points waiting at the Courier, 18 × u16 |
| `0x5EA2` | `+0x446c` | control option bytes: the Game options window writes `+0`, the target camera reads `+0`, `+2`, `+3`, the Hunter Art gauge `+2`, `+3` |

## Block A header and shared settings

**DERIVED.** Block A starts with a second small serializer of S (`0x5215b8` writes,
`0x52163c` reads). Its fields are shared by all characters, so they sit in block A
and not in a slot. The title menu's *Game Settings* (TV brightness, rumble, language)
read and write them (`0x3e7f90`, `0x3e8254`).

| File | Size | S field | Content | Analysed save |
|---|---|---|---|---|
| `0x40` | u32 | `+0x420` | bonus packs loaded, bit N = privilege pack N (1–4). See the grant flags at `base + 0x2C0F` | `0x1E`: all four |
| `0x44` | 3 × u16 | `+0x3676` | vestigial: only the transfer `0x52171c` writes them (old `+0x28ea` … `+0x28ee`); no reader | 0 |
| `0x4A` | u8 | `+0x367c` | one-time title-menu notice shown: `0x67b544` shows *TitleMsg* 88 while it is 0, then sets it (`0x67de64`). Entry 88 is not in the base romfs table (an update text) | 1 |
| `0x4B` | u8 | `+0x367d` | transfer-server link made: set by `0x6917e8` when the link succeeds, cleared by the flows that rewrite the account ID at `+0x4408`; read by the title menu (`0x680714`) | 0 |
| `0x4C` | u8 | `+0x367e` | TV brightness: the game sets a scale of 0.4 + 0.025 × value (`0x5216c8`) | 24 (scale 1.0) |
| `0x4D` | u8 | `+0x367f` | rumble on (1) / off (0), copied to the pad object (`0x4e0ec8`) | 1 |

The three bytes at `0xB2A2` belong to `sGameControl` (`0x3f8ef8`): `+0x5c`, `+0x5d` and
`+0xa5`. `+0xa5` is the text language. When it is 0 the loader takes it from the system
language, and nearly every UI class reads it. `+0x5d` is the 3DS leftover "use the Circle
Pad Pro" flag (*CommonMsg* 141–146): the Game options window (`0x5e7000`) and the title
menu store 1 when ZL/ZR are seen, 0 otherwise, and the loader copies it to the runtime
byte `+0x72`. `+0x5c` is written only by the constructor, the reset and the transfer
(`0x3f8f90`, old `+0x24`); no reader. The per-character options are
the 29 bytes at `base + 0x2246E` (`sGameControl +0x5e`, written by the Game, Chat and
Network option windows and read by the quest camera).

## Downloads held — block B header

**DERIVED** from the download dispatcher `0x39394`, which sets one bit per stored
download by content type (the low nibble of the type, minus 2, indexes a jump table),
and from the "new content" checks at `0x3b72c` onwards, which compare each map with
the server catalog at `+0xed0` (runtime).

| File | Size | sPrivilege | Content | Analysed save |
|---|---|---|---|---|
| `0xB2A5` | 8 | `+0xf34` | DLC Palicoes received, 50 bits (catalog entries 11–60), the info at `0xC761`. CONFIRMED: bits 15–17 are exactly the occupied info entries and the three "Capcom" Palicoes of the block A pool (earlier revisions swapped the two maps) | 3 |
| `0xB2AD` | 8 | `+0xf3c` | DLC item packs received, 50 bits (catalog 507–556). The packs themselves are the list at `0xB311`. CONFIRMED: bits 32–48 are exactly the 17 occupied pack records, catalog IDs 539–555 = 507 + bit | 17 |
| `0xB2B5` | 4 | `+0xf44` | extras of download type 4, 9 entries (catalog 71–79). The Guild Card builder `0x161ac8` turns bits 0–3 into Guild Card poses 17, 20, 21 and 18 | 4 |
| `0xB2B9` | 12 | `+0xf48` | extras of download type 5, 80 entries (catalog 80–159) | 57 |
| `0xB2C5` | 4 | `+0xf54` | extras of download type 6, 10 entries (catalog 160–169) | 5 |
| `0xB2C9` | 40 | `+0xf58` | extras of download type 7, 300 entries (catalog 170–469) | 215 |
| `0xB2F1` | 8 | `+0xf80` | challenge quests stored, bit = slot of the 45 **records** at `0x176499` (CONFIRMED). The archive store `0x127899` is packed and the record store is not (record slot 25 is empty), so the two differ from slot 25 on: match them by quest ID, not by slot | 40 |
| `0xB2F9` | 20 | `+0xf88` | event quests stored: as many set bits as stored quests, but bit ≠ slot of the 160 at `0xF899` (slots 0–2 hold quests whose bits are clear, 23 set bits fall on empty slots). Probably the download catalog position, UNRESOLVED | 125 |
| `0xB30D` | u32 | `+0xf9c` | stamp compared with the catalog's `+0xe5c` (`0x3b714`); this is the u32 the writer round trip rewrites | |

The counts match block B: 40 challenge and 125 event quests are stored. The download
menu's extras are titles, Wycademy points, Trader wares, Guild Card backgrounds, pet
costumes and poses (`DLC_eng.gmd`).

**DERIVED — types 5–7 are Trader wares.** The Trader screen (`uUITradeCenter`,
`0x7a7f08`) has three tabs (UI byte `+0x3d`). Each tab skips what is already unlocked
and otherwise asks whether the download that sells it is held:

| Tab | Unlock map tested | Download test | Trader map set |
|---|---|---|---|
| 0, Guild Card title words | `S+0x9d0` | type 7, catalog − 170 (`0x7a84a0`) | `S+0x34a0` (448 bits) |
| 1, Guild Card scenes | `S+0xbec` | type 5, catalog − 80 (`0x7a85ac`) | `S+0x3548` (160 bits) |
| 2, pet costumes | `S+0x994` | type 6, catalog − 160 (`0x7a7d30`) | `S+0x3584` (32 bits) |

So type 5 = 80 Guild Card scenes, type 6 = 10 Poogie and Moofy costumes, type 7 = 300
title words; the sizes agree with `tradeLimitedPaperList` (131) and
`tradeLimitedHonorList` (442) for the two big Trader maps.
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

**Not all store bytes are quest data.** The full-file writer puts a shared copy of the
linked Nintendo Account ID (`S+0x4408`, 8 bytes) at file `0x12C2D6` (`0x3e7f4c`; read
back at block A load by `0x3e0b98`). That is offset `0x123D` of challenge store slot 2,
inside the unused tail of its `0x1C00` bytes (the quest there ends at `0x107A`). An editor
that rewrites a store slot must keep these 8 bytes.

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

- S ([above](#the-save-object-s)): what the old save meant by the vestigial map at
  `S+0x9c4`, and the unnamed bits of `S+0xd8c`. In the
  [`S+0x20` block](#the-block-s0x20--0x41f--base--0x280b): `S+0x28`, `S+0x50`,
  `S+0x64 … +0x117` apart from the counters named there, `S+0x118`, `S+0x3e4 … +0x403`
  and `S+0x408`.
- Guild Card: byte `+6` of the Palico parameter block (forced to 55 in a StreetPass
  record); who writes the u16 quest ID of a history record.
- Slot header / player record: what the 5-bit values of the u32 at `+0x274` select.
- Block A: the text of *TitleMsg* 88 (`S+0x367c`, an update string), and the old-save
  meaning of the vestigial `S+0x3676` and `sGameControl +0x5c`.
