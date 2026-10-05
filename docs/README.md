# MHGU Switch Save Format — Reverse-Engineering Notes

Structural notes on the Monster Hunter Generations Ultimate save file as written by
the Nintendo Switch release, recovered by differential analysis against a live save.
The [save editor](../README.md) is built on them.

The goal of these documents is to describe **how the save is organised**, not to
describe any one player's data. Where a concrete value is quoted it is only ever as
evidence for a structural claim.

## Scope and provenance

| | |
|---|---|
| Title | Monster Hunter Generations Ultimate |
| Title ID | `0100770008DD8000` (EU / western release) |
| Platform | Nintendo Switch, observed through the Ryujinx emulator |
| File | `system` (and `system_backup`), 5,159,100 bytes |
| Method | Controlled before/after diffing of a real save (see [methodology](06-methodology.md)) |

All offsets are **absolute byte offsets into `system`**, little-endian, for character
slot 1. Per-character offsets move with the slot base; see
[Open questions](#open-questions).

## Documents

| Document | Covers |
|---|---|
| [01 — Container](01-container.md) | File layout, commit slots, encryption and integrity |
| [02 — Monster records](02-monster-records.md) | Hunt tallies, size records, the monster index table |
| [03 — Deviants](03-deviants.md) | Permit counts, level bitmaps, unlock gating |
| [04 — Weapon usage](04-weapon-usage.md) | Guild Card weapon usage counters |
| [05 — Quests](05-quests.md) | Quest bitmaps and the full quest index, history log, counters |
| [06 — Methodology](06-methodology.md) | How this was derived, and how to extend it |
| [07 — Equipment](07-equipment.md) | Character slots, equipment box, talismans, transmog, dye, My Sets |
| [08 — Hunter Arts and Canteen](08-progression.md) | Hunter Art unlocks, Canteen ingredients and dishes |
| [09 — Awards](09-awards.md) | Guild Card award bitfield |
| [10 — NPC talk data](10-npc-talk.md) | Talk tables: request offers, star-level flags, per-NPC bits |
| [11 — Whole-file map](11-save-map.md) | Every byte assigned to its game object by running the game's own loader: item box, Palicoes, Guild Cards, downloaded quests |

Machine-readable: [`data/monster-index.csv`](../data/monster-index.csv), [`data/quest-index.csv`](../data/quest-index.csv), [`data/request-index.csv`](../data/request-index.csv), [`data/quest-unlock.csv`](../data/quest-unlock.csv), [`data/request-offer.csv`](../data/request-offer.csv), [`data/rotating-quests.csv`](../data/rotating-quests.csv), [`data/npc-index.csv`](../data/npc-index.csv), [`data/hunter-arts.csv`](../data/hunter-arts.csv), [`data/save-map.csv`](../data/save-map.csv), [`data/save-coverage.txt`](../data/save-coverage.txt), [`data/offsets.json`](../data/offsets.json)

## Quick reference

| Structure | Offset | Layout |
|---|---|---|
| Header nonce | `0x000014` | u32, changes every write, **not** a checksum |
| Deviant permit counts | `0x18F4D8` | 18 × u8 |
| Quests cleared | `base + 0x2C77` | 1509 bits, index = position in `quest_group`, see [`quest-index.csv`](../data/quest-index.csv) |
| Quests seen (NEW cleared) | `base + 0x2D77` | same indexing, `+0x100` bytes |
| Quests failed | `base + 0x2E77` | same indexing, `+0x200` bytes; a failed quest counts as "met the monster" for the Hunter's Notes (DERIVED) |
| Villager request flags | `base + 0x2C56D` | 1536-bit event flag map; per-request accepted/completed bits in [`request-index.csv`](../data/request-index.csv). Accepted = quest posted on the board |
| Quest unlock rules | script, not a save field | the board runs `script\check_quest_unlocked` per quest; it reads event flags, cleared bits, HR and the Hub star level. Rules in [`quest-unlock.csv`](../data/quest-unlock.csv), evaluator [`tools/quest_unlock.py`](../tools/quest_unlock.py) |
| Request offer conditions | talk data, not a save field | per-NPC tables `table/npc/script/npc_NNN_td.ntd`; conditions in [`request-offer.csv`](../data/request-offer.csv), evaluator [`tools/request_offer.py`](../tools/request_offer.py) |
| Mark every quest cleared | several fields | [`tools/complete_quests.py`](../tools/complete_quests.py): cleared and seen bits, request accepted flags, quest set bits; dry run by default. See [05](05-quests.md#what-all-quests-completed-takes) |
| Rotating quests | `base + 0x504B` | u64, bit = row of [`rotating-quests.csv`](../data/rotating-quests.csv); 51 quests are listed only while their bit is set; re-rolled after each completed quest |
| Per-NPC talk hold bits | `base + 0x2C62D` | 3 × 24 bytes after the event flags, bit = row of [`npc-index.csv`](../data/npc-index.csv). Set = NPC skips request offers / kind 9 talk / announcements until the next cleared quest (CONFIRMED by write), see [10](10-npc-talk.md) |
| Village / Hub star level | `base + 0x2C4DA` / `+0x2C4DC` | u16 each, 1–10 / 0–13. The game raises them when a quest is cleared and all urgents of a level are cleared (`quest_group` groups 24–46); not recomputed on load, see [05](05-quests.md#star-levels--base--0x2c4da) |
| Village contribution points | `base + 0x281B` / `+0x282B` | 4 × u32 low rank, 4 × u32 G rank: Bherna, Kokoto, Pokke, Yukumo (CONFIRMED, [11](11-save-map.md#confirmed-by-the-save-timeline)) |
| Progress word | `base + 0x2F77` | u32; bit 20 = HR limit released, bit 31 = quest 10646 was listed (from code) |
| Quest sets completed | `base + 0x3187` | 100 bits, bit N = every quest of set N cleared (column `sets` of `quest-index.csv`); read by NPC talk and the award check. Bit 48 CONFIRMED by write |
| Pending village rewards | `base + 0x2381E` | 23 × u8 counters of the activity manager (DERIVED) |
| Deviant levels (cleared) | bit `0x18F989`.3 | quest indices 947–1174 (Special Permit), 228 bits |
| Deviant levels (seen) | bit `0x18FA89`.3 | same layout, `+0x100` bytes |
| Quest counter | `0x192AEA` | u16 |
| Monster hunt tallies | `0x192B40` | u16, index 1–137 (`0x192B40 + 2i`) |
| Monster capture counts | `0x192C52` | u16, index 1–137 (`0x192C52 + 2i`) |
| Monster size records | `0x192D62` | (u16 min%, u16 max%), stride 4, index 1–137 |
| Weapon usage — Village | `0x254713` | 15 × u16 |
| Weapon usage — Hub | `0x254731` | 15 × u16 |
| Weapon usage — Arena | `0x25474F` | 15 × u16 |
| Quest history log | `0x254771` | 10 × `0xA0` records: date, u16 quest ID at `+6`, name at `+8` |
| Character slot pointers | `0x34` | 3 × u32, relative to `0x24` |
| Equipment box | `base + 0x62EE` | 2000 × 36 bytes; transmog at `+0x04` |
| My Sets (dye lives here) | `base + 0x208C8` | stride `0x88`; pigment 5 × RGBA at `+0x6A` |
| Hunter Arts unlocked | `base + 0x2C13` | 24-byte bitfield, IDs 1–70 and 83–190 |
| Canteen dishes | `base + 0x2C67D` | 13-byte bitfield, 99 dishes |
| Canteen ingredients | `base + 0x2F8F` | 6-byte bitfield, 45 ingredients |
| Awards earned | `base + 0xC8115` | 132-bit bitfield, one run per location grid |
| Item box | `base + 0x278` | 2300 × 19-bit slots (u12 item ID, u7 count), LSB-first bit stream; [`tools/items.py`](../tools/items.py) |
| Item loadouts | `base + 0x17CF` | 24 × 170 B: name char[42], 32 × (u16 item, u16 count) |
| Item pouch | `base + 0x27BF` | 32 × 19-bit slots, same format; a slot holds at most the item's carry limit (`itemData` +6 in the game's tables) |
| Palico equipment box | `base + 0x17C2E` | 1000 × 36 B, equipment box format |
| Palicoes | `base + 0x23BB6` | 84 × 324 B (+ 24 at `base + 0x2A606`): name +0, exp u32 +0x20, level +0x24, greeting +0x60, owner +0x9C |
| Own Guild Card | `base + 0xC71BD` | 6328 B; weapon usage, history and awards live inside it |
| Downloaded quests | `0xF899` / `0x127899` | 160 / 45 × `0x1C00`: u32 ID, u32 size, ARC; shared by all slots; [`tools/event_quests.py`](../tools/event_quests.py) |

## Confidence levels

Every claim in these documents carries one of three tags:

- **CONFIRMED** — verified by at least two independent lines of evidence, normally a
  controlled write plus an observed in-game change, or two separate diffs agreeing.
- **DERIVED** — follows from a confirmed structure plus a single observation.
  Very likely correct, not independently cross-checked.
- **UNRESOLVED** — known to exist, layout or meaning not established.

Nothing here is from official documentation or leaked source. It is all inference
from observed bytes, and it is incomplete.

## Open questions

- **Multiple character slots — answered.** The game's loader reads all three
  slots with the same chain at a fixed stride, so every per-character offset is
  `base + const` with base from the pointer table at `0x34`
  ([11](11-save-map.md#method)). Confirmed with two more real characters
  ([07](07-equipment.md#character-slots)).
  **An editor should resolve the base through the pointer, not hard-code it.** The
  tools in [`tools/`](../tools) do, and take `--slot 1|2|3` (default 1).
- **Region portability.** Only the EU/western build was examined. Japanese builds
  may differ.
- **Monster indices 106–112** are unused (`dummy1`–`dummy7` in the game's name
  table), and index 134 is an empty slot. Names for 105 and 113–137 come from the
  game's name table and agree with the editor; they were not read back in-game — see
  [02 — Monster records](02-monster-records.md#the-games-name-table).

## Licence

Part of this repository, under the GNU General Public License v3.0 or later; see
[`LICENSE`](../LICENSE) and the [root README](../README.md#licence).
No warranty — verify against your own data before writing to anyone's save.
