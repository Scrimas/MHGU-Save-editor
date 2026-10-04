# 10 — NPC talk data

What an NPC says, when it offers a request, and which event flags the conversation
sets are all in one table per NPC. This is where the **accepted** flags of
[05](05-quests.md#request-flags--base--0x2c56d) and the star-level flags of the
[unlock rules](05-quests.md#board-visibility--scriptcheck_quest_unlocked) are set
during normal play.

## Files — `table/npc/script/npc_NNN_td.ntd`

171 NPCs, three files each, standalone in the romfs:

| File | Content |
|---|---|
| `table/npc/script/npc_NNN_td.ntd` | Talk data: one 37-byte record per text line |
| `<lang>/table/npc/script/npc_NNN_td_<lang>.gmd` | The text; string *i* belongs to record *i* |
| `table/npc/script/npc_NNN_is.nis` | `rNpcInitScript`: what the NPC does when it is placed, see [below](#init-scripts--npc_nnn_isnis) |

`NNN` is the NPC ID: 52 Argosy Captain, 1 Bherna Chief, 2 Bherna Gal, 520 Guild
Manager, 502 Wycademy Gal, 701 Wyventurer, 702 Soaratorium Gal, 801 Pub Manager,
802 Questender. The loader is `rNpcTalkData` (`0x23d808`, path format at
`0x155d130`), the selector is `0x243d00`, the condition evaluator `0x2451c8` and the
action executor the function around `0x2479b8`.

Header: f32 `1.0`, u32 record count. Record:

| Offset | Type | Meaning |
|---|---|---|
| `+0` | u32 | Line index |
| `+4` | u8 | Line type. 1 = text line. 2, 7, 8, 10, 14, 15 end a block (mask `0x3161` in the selector) |
| `+5` | u8 | **Talk kind** of the block, on its first line only (0 on the others). See below |
| `+6` | u8 | Direct-lookup ID (compared with a u16 of the request context), rarely used |
| `+7` | u8 | **Village star level test** |
| `+8` | u8 | **Hub star level test** |
| `+9` | 3 × {u16 type, u16 a, s16 b} | Conditions |
| `+27` | u16, u16 | Action 1: type 1 = set event flag, 2 = clear event flag; flag index |
| `+31` | u16, u16, u16 | Action 2: type, value, parameter |

A **block** is a run of lines up to the next end line. The selector collects the
block, tests the star levels and the conditions, and plays the first block of the
requested kind that passes.

Conditions are read slot by slot, line by line, and **reading stops at the first
empty slot** (type 0). A block that needs more than three conditions fills all three
slots of a line and continues on the next one; type 15 is a filler that is always
true. A condition on a later line behind an empty slot is never tested.

Star level test, value *v* against the level at [`base + 0x2C4DA` / `+0x2C4DC`](05-quests.md#star-levels--base--0x2c4da):
`1–20` level = *v*, `21–40` level ≥ *v* − 20, `41–60` level ≤ *v* − 40, `0` no test.

Talk kinds seen: 1 ordinary talk, 3 story announcement, **4 chief request offer**,
**5 request report**, 6 chief request report, **8 request offer**, 9 and 11–49
other events. A block of kind 3, 8 or 9 is skipped while the NPC is on hold for that
kind, see [per-NPC bits](#per-npc-bits--base--0x2c62d).

### Conditions

Every condition must hold, with one exception: types 16–21 (and 119, 147) are
**OR-ed with the condition before them**. The selector goes left to right and keeps
one result: a normal type after a false result fails the block, an OR type after a
true result passes it at once (`0x244138`–`0x244260`). So `A, B, C, OR D` reads
A and B and (C or D). [`request-offer.csv`](../data/request-offer.csv) writes this
as `C|D`.

For types 11, 12, 18, 19 and 118–120 the quest ID is *b* plus an offset chosen by
*a* (table at `0x1627b14`): 10002 adds 10000, 10003 adds 100000, 10004 adds 40000,
anything else adds nothing. That is how an s16 reaches 41811 or 111304.

| Type | Test | Status |
|---|---|---|
| 1 / 2 | event flag *a* set / not set | CONFIRMED (code + save) |
| 11 / 12 | quest cleared / not cleared (`0x523da4`) | CONFIRMED (code + save) |
| 13 / 14 | Village / Hub key quest set *b* all cleared (`0x526d28` / `0x526d48`) | from code |
| 16–21 | 1, 2, 11–14 again, OR-ed with the condition before | from code |
| 3–7, 9, 135, 136 | the player is in scene 1, 2, 3, 4, 0, 5, 6, 7: the u16 at `+2` of the talk request is the scene index, taken from byte `+0x4c` of the scene object (GOT `0x1838bb4`; the talk cache prebuild `0x50b4dc` loops over the eight scenes). Scenes: 0 Hunters Hub, 1 Bherna, 2 Kokoto, 3 Pokke, 4 Yukumo, 5 Palico Ranch, 6 Wycademy, 7 the second hub (Pub Manager, Questender). Requests built elsewhere hold `0xFFFF`, so all are false there. Used: 3 (Head Researcher in Bherna), 7 (NPC 701), 9 (NPC 613), 135 (NPC 9); 4, 5, 6, 136 unused | from code + NPC data |
| 22–25 | type of the quest just accepted (`0x3bd35c`): 0 hunt / 2 capture / 3 gathering / 1 (line text is a placeholder). Send-off lines of the quest counter Gals | from dialogue |
| 26 / 27 | byte `+0x4d8` of the player data clear / set: the hunter's gender, male / female. The slot header writer turns the same byte into its gender flag and the Smithy and the Armory read it ([11](11-save-map.md#slot-header--base--0x0-632-b), `base + 0x23B4B`) | from code |
| 28 | HR ≥ *b* (u16 `+0x554` of the player object) | from code |
| 29 | a statistic of the hunter, sub-test *b* (`0x2484c0`, jump table `0x2484f0`, 114 entries); only NPC 991, the house Palico. See [below](#condition-29--the-house-palicos-remarks) | from code + dialogue |
| 36–39 | how many Special Permits the Courier can hand out: word `+0x2cc` of the lobby-state object (GOT `0x1838fc0`) is 0 / 1 / 2 / 3. The Courier (`0x6d148c`) stores `0x50f440(0x527a80(S))`: the permits due, Σ over the 18 deviants of min((waiting `S+0x4448` + points `S+0x4424`) / 100, 99 − held), bucketed 0 / 1–50 / 51–100 / ≥ 101. True for all four without the object. Courier lines 78, 81, 84, 87 | from code + dialogue |
| 41 / 42 | 41: delivery request *b* (1–13, the kind 1 requests) has been delivered: `0x524db8` tests bit *b* of `S+0x35a8` (`base + 0x32AF`), on report lines. 42: at least *b* Hub ★1 quests (IDs 101xx) cleared, counted by `0x3b1b90` over the quest list. One use: NPC 502 line 20 (*b* = 1), the HR tutorial after the first Hub ★1 clear | from code + dialogue |
| 44 / 45 | a Hunter's Notes entry can be unlocked: large monsters (`0x554da4`, 123 entries) / the second list (`0x55515c`, 30 entries) | from code + text |
| 46–53, 141, 142 | a Hunter Art lesson of this teacher is due (`0x3f0ff0` … `0x3f1294` with 1 = ask): one of the teacher's arts is not yet in the [Hunter Arts map](08-progression.md) (`0x524040`) and its requirement holds. With every art unlocked by an edit the lesson is never due, and a request report that carries this condition never fires | from code + text; the blocked reports CONFIRMED on the save |
| 54 | a threshold is due (`0x197598`): with `S+0x110` clear and the Hub star level at least 8, the u32 at `S+0x20` has passed u16 `S+0x114` × 20000. The dialogue ties it to Wycademy points | from code + dialogue |
| 55–62, 102–105, 139, 151–156 | a byte of the [activity state](#other-save-state-the-talk-data-reads) is not 0: a reward is waiting. 102–105 / 151–154 are the village tickets of Bherna, Kokoto, Pokke, Yukumo in low and G rank, 139 the Soaratorium ticket | from code + text |
| 64 | quest 10646 is listed. Runs the [unlock script](05-quests.md#board-visibility--scriptcheck_quest_unlocked) for 10646 (`0x3f12c4`) and latches the result in bit 31 of the progress word | from code |
| 65 | the HR limit is released: bit 20 of the progress word. Set by talk action 1 type 5, the Pub Manager after 11432 | from code + text |
| 67–70 | contribution points of Bherna / Kokoto / Pokke / Yukumo ≥ *b* (`0x523b50`: low rank + G rank, at most 20000) | from code |
| 85, 93–97 | bits 29, 11, 12, 8, 9, 10 of the progress word. 93 / 94: the trader Neko has a second / third cart; 95–97: a new trading location opened. 85 occurs in no talk file | from dialogue |
| 86 / 87 | a state word is 1 / is not 1. The footbath visitors (NPC 420–428) use 87 for a one-line "have a seat, then we'll talk" and 86 for the real conversation, so this is "seated in the Yukumo footbath" | DERIVED |
| 89, 90, 121, 161, 162, 163 | bits 45, 46, 48, 98, 78, 99 of the [quest set map](05-quests.md#quest-sets--base--0x3187) at `base + 0x3187`: every quest of that set is cleared. 45 / 46 = the 10 low-rank Arena quests cleared / all at rank A, 98 / 78 = the same for all 17 Arena quests, 48 = the ordinary Village ★1–★6 quests (gates the four chiefs' last requests), 99 = the ordinary Village ★7–★10 quests | 121 CONFIRMED by write, the others from code and dialogue |
| 91 / 92 | NPC *b* has one / none of its three per-NPC bits set | from code |
| 98–101, 157, 158 | result of `0x1c8ca8` > 4, 1, 2, 5, 7, 8: the Meownster Hunters' progress (new destinations, the balloon; NPC 621 only) | from dialogue |
| 106 / 107 / 108 | random word modulo 10000 ≤ / ≥ / < *b*: a line with a fixed chance | from code |
| 109 / 110 | bit *b* of a bitmap at `+0x54` of another object set / clear. Bit 1 set = a delivery reward is waiting at the client (housekeeper lines) | from dialogue |
| 113 / 114 | of requests 99–105 (the footbath visitors), at least four / all seven are completed | from code |
| 116 / 127 | a quest is / is not selected (`0x3b9868`) | from code |
| 118 / 119 / 120 | the selected quest is / is (OR type) / is not the given quest | from code |
| 122 | quest 10644 is listed, like 64 but not latched in the save | from code |
| 123–126 | result of `0x1a57bc` + 1 compared with *b* (=, ≥, ≤, <): a shop level, used by "the shop just got an upgrade" lines | from dialogue |
| 143–146 | online lobby state, no save field. All need a session in modes 2, 7–11 with byte `+0x640` = 0 (`0x227a98`, object at GOT `0x1838d7c`). 143: also u32 `+0x644` = 0 (written by `uUILobbyStartMenu`), 144 its negation. 145: the session's `+0x41cc` equals `+0x41c8` (probably "this player is the host"), 146 its negation. Used by NPC 9's Hunter Call and boarding lines | structure from code; field meanings UNRESOLVED |
| 8, 10, 15, 30–35, 63, 71–74, 76–84, 137, 138, 147 | always true | from code |

The jump table of the evaluator is at `0x2451fc` (163 types). 58 types do not occur
in any file.

#### Condition 29 — the house Palico's remarks

NPC 991 picks a remark by testing one statistic; each line gives the group away.
**DERIVED** from the sub-test code and the lines it gates. Without a test, 0, 87, 89,
91, 93, 97, 99, 101, 103 and 104 are true (`0x2487e4`), the other values up to 114 false
(`0x248f40`).

| *b* | Test | Remark |
|---|---|---|
| 1–8 | the first Palico's support bias (`0xe8458`) is Charisma, Fighting, Protection, Assisting, Healing, Bombing, Gathering, Beast | "My Bias is …" |
| 9 | a Hub star-level test | Meownster Hunter duties |
| 10–12 | large monster hunts (`0x52308c`, tally + captures) in three bands | novice / mid-rank / expert |
| 13–16 | quests cleared (`0x526348`) in four bands | |
| 17–20 | a progress level (`0x523504`, starts from the Hub star level) in four bands, the first below 8 | "still got a lot to learn" … "you'll go down in history" |
| 21–24 | number of awards (`0x526564`), the first below 30 | "could I check the Awards on your Guild Card" |
| 25–27 | Palico equipment owned (`0x1418e0` − `0x142a38`) | |
| 28–41 | most used weapon (`0x249004`, Guild Card weapon usage); table `0x2486b8` maps *b* − 28 to the weapon class, in the on-screen order Great Sword … Bow | "You really like …" |
| 42–55 | least used weapon, same order | "Ever thought about trying …" |
| 56–61 | most used hunting style, Guild, Striker, Aerial, Adept, Alchemy, Valor (`0x2493c8`, the counters at `S+0x11a`, [11](11-save-map.md#the-block-s0x20--0x41f--base--0x280b)) | |
| 62–67 | least used style, same order | |
| 68–70 | Arena quests: none / some / all cleared | |
| 71–73 | Palicoes hired (sOtomo `+0x1384c`), the first below 13 | |
| 74–77 | how full the equipment box is | |
| 78–80 | Prowler weapon usage (index 14 summed over the venues, `0x166aac`): below 11, 11–30, above 30 | see [04](04-weapon-usage.md#open-questions) |
| 81–83 | Prowler quests cleared (`0x3b1e24`): none / some / all | |

### Actions

Action 1, jump table at `0x2479d8`:

| Type | Effect | Status |
|---|---|---|
| 1 | set event flag, then refresh through `0x3ef68c` | CONFIRMED (code + save) |
| 2 | clear event flag | from code |
| 3 | end the talk without the follow-up menu: sets byte `+6` of the talk result (`0x1884c5c`, reset by the executor `0x246b1c`). When `+6` or `+7` is set, the NPC base (`0x6ce8cc`) and `uUIOtomoService` skip their service step and close the talk. 107 intro, story and request lines; no save effect | from code; the menu reading from the lines |
| 4 | set event flag without the refresh (not used) | from code |
| 5 | release the HR limit: `0x5231d8` sets bit 20 of the progress word and recomputes HR. One line: Pub Manager, after 11432 | from code + text |
| 6 / 7 | unlock every Hunter's Notes entry that is due, in the large monster list / the second list | from code + text |
| 8 / 9 | `0x3d80a8` with 1 / 0 (not used) | — |
| 10 | join the Hub: Hub star level 0 → 1 and the HR setup `0x5232d8`. One line: Guild Manager's greeting | from code + text |

Action 2, jump table at `0x247cac`:

| Type | Effect | Status |
|---|---|---|
| 1 | a reward. *value* 30 / 31 pays *parameter* / 10 × *parameter* zenny (`0x523150`); 32–41, 54–57, 60–65 clear one activity state byte, that is, hand over the waiting tickets; any other *value* is an item ID with *parameter* as the count (10220's report: 1833 × 1, the Poogie costume), handled outside this function | from code, item case DERIVED |
| 2 / 3 / 26 | set the speaker's bit (or NPC *parameter*'s) in per-NPC map A / B / C | from code + save |
| 4 | add *parameter* Wycademy points (`0x523194`) | from code |
| 5 / 6 / 7 / 8 | add *parameter* contribution points of Bherna / Kokoto / Pokke / Yukumo, low rank (`0x523a80`) | from code + text |
| 9–16, 24, 25 | teach the Hunter Art whose lesson was due (conditions 46–53, 141, 142) | from code + text |
| 17–21 | unlock Canteen ingredient *parameter* of category 0–4 (`0x3f1380`, first index 0 / 9 / 19 / 29 / 39) | from code + text |
| 22 | unlock village feature *parameter* (`0x3ef97c`): Rife Roast, Poogie outfits … | from text |
| 23 | more jukebox tracks (`0x3f420c`) | from text |

## Request offers

A request is offered by a block of kind 8 (villagers) or 4 (chiefs). One of its lines
sets the request's **accepted** flag with action 1; the report block (kind 5 / 6)
sets the **completed** flag. The offer conditions of 182 of the 184 requests are in
[`data/request-offer.csv`](../data/request-offer.csv) (generator:
`scratch/py/mkoffer.py`, joins talk data to `request-index.csv` by the accepted flag).
Of the other two, record 183 is the end marker and record 117 is unused: a kind 2
record (a request without a quest, like 33 *Hot Heart* and 136) of the Kokoto Chief
that would pay 5 Kokoto Tickets. No talk line of any NPC sets or reads its flags 550
and 551, no script or init script names them, its log title is `dummy`, and both
flags are clear in the save. **DERIVED**: nothing in the data can offer it. Request 22 (639) is offered from a block of kind 9, not
8: the four chiefs hand out their last request in ordinary talk.

`tools/request_offer.py <save>` lists the requests not accepted yet with what their
NPC still waits for. Read-only.

| Column | Meaning |
|---|---|
| `index`, `quest_id`, `quest_name` | From `request-index.csv` |
| `npc_file` | NPC ID, the `NNN` of the talk file |
| `npc_bit`, `npc_name` | The NPC's bit in the per-NPC maps, which is also its index in `NpcName_<lang>.gmd` and the NPC byte `+0x06` of the request record |
| `talk_kind`, `talk_line` | Block kind (4 / 8, once 9) and its first line |
| `accept_flag` | Flag the block sets |
| `also` | Other event flags the block sets (`N`) or clears (`-N`) with action 1, its end line included: the second flag of a chief's request (150 … 160, 200 … 210, 250 … 260; 160 is what lists 639), the Captain's idle-talk flag 796. [`tools/complete_quests.py`](../tools/complete_quests.py) applies them together with the accepted flag |
| `offer` | Conditions besides "accepted flag not set yet", joined by ` AND `; `x\|y` means either. `village_star:N`, `hub_star:N` (≥), `village_eq` / `hub_eq`, `flag:N`, `notflag:N`, `cleared:Q`, `village_keys:N`, `group_done:N`, `hr:N`, `npc_idle:NPC`, `footbath`, `listed:Q` (64, 122), `hr_unlocked` (65), `points:Village:N` (67–70), `questset:N` (bit N of the [quest set map](05-quests.md#quest-sets--base--0x3187) at `base + 0x3187`), `pending:N` (activity state byte), `requests_done:FROM-TO:N` (113, 114), `condT:a:b` for the rest. Only what the selector really tests is listed: reading stops at the first empty slot |

The Argosy Captain (NPC 52), whose *The Perilous Pair* (607) started this:

| Line | Kind | Conditions | Sets |
|---|---|---|---|
| 9 | offer | Hub ★ ≥ 2, flag 394 clear | 394: 10220 accepted |
| 19 | report | 395 clear, 10220 cleared | 395, reward 1833, map A bit |
| 29 | offer | **Village ★ ≥ 6, 396 clear, 395 set** | 396: 607 accepted |
| 36 | report | 397 clear, 607 cleared | 397, reward 1834, map A bit |
| 45 | offer | Hub ★ ≥ 5, 398 clear, 397 set | 398 |
| 64 | offer | Hub ★ ≥ 7, 400 clear, 399 set, condition 64 | 400: 10732 accepted |

So 607 needs 10220 **reported**, not merely cleared. That is why clearing The Fated
Four changed nothing, and it matches the earlier notes that stage and posting village
are not gates.

**Evidence.** For the 116 rows whose request is accepted in the save, every condition
that can be read from the save holds: 190 condition groups (flags, cleared quests, key
quest sets, star levels, HR, contribution points, the two progress bits), zero
violations. `npc_bit`
equals the NPC byte of the request record in 175 of 186 rows (the other 11 are offered
from a different NPC's file). The Captain's report in snapshots `req-2 → req-3` set
exactly what line 19 says: flag 395, flag 796 (action 1 of the next line) and bit 33
of map A. Bit 33 of map A is "flag 1569" of the earlier diff, which is past the end of
the 1536-bit flag map.

## Per-NPC bits — `base + 0x2C62D`

Three 24-byte bitmaps follow the event flags in the same object (`+0x5c0`, `+0x5d8`,
`+0x5f0`; serializer `0x240ce4`):

| Map | Offset | File offset | Blocks talk kind |
|---|---|---|---|
| A | `base + 0x2C62D` | `0x1B92C9` | 8, request offers |
| B | `base + 0x2C645` | `0x1B92E1` | 9 |
| C | `base + 0x2C65D` | `0x1B92F9` | 3, story announcements |

The bit index is the NPC's position in the u16 table at `0x162793c` (188 entries,
lookup `0x23e640`): 1, 2, 3 … 12, 14, 15, 20 … so NPC 52 is bit 33. The same index
orders `NpcName_<lang>.gmd` (entry 33 is "Argosy Captain"). The table is in
[`data/npc-index.csv`](../data/npc-index.csv).

A set bit puts the NPC on hold for that kind. The context builder `0x243bd4` turns
bit A / B / C of the NPC into mask `0x100` / `0x200` / `0x8`, and the selector skips
blocks whose kind has its mask bit set. A request report sets the NPC's map A bit
(action 2). `0x38b904`, a step of the quest flow (called from `0x3866a4`; it also
re-rolls the rotating quests through `0x54b0d4`), passes map A and the length 72 to a
memclr-style import, which wipes all three maps. So an NPC that just took a
report offers its next request only after the next quest (**CONFIRMED**, see the
controlled write below). All maps are empty in every
snapshot taken right after a quest (`req-1`, `req-2`). This explains the observation
"reported 10220, got the costume, Captain offered nothing": bit 33 was set by that
report and is still set in the save.

**Controlled write — CONFIRMED.** Bit 48 of the [quest set map at `base + 0x3187`](05-quests.md#quest-sets--base--0x3187)
(condition 121, "all ordinary Village ★1–★6 quests cleared") was set, which made the last request of all four chiefs ready, and
bit 36 of map B (Kokoto Chief; his offer is a kind 9 block) was set in the same write
(`0x18FE29` `0x00 → 0x01`, `0x1B92E5` `0x00 → 0x10`, both slots). In the game the
Bherna Chief announced his new ★6 quests (flag 685), the Pokke and Yukumo chiefs
offered 640 and 641 (flags 210 / 412 and 260 / 456), and the Kokoto Chief had no
speech bubble and offered nothing: flags 160 / 346 stayed clear. No quest was played,
and bit 36 was still set in the save afterwards. After the next cleared quest (1038)
all three maps were empty, including the Captain's bit 33 of map A, which confirms the
wipe in the quest flow.

For an editor the maps only matter as a nuisance: a set bit delays an offer until the
next quest. Setting the accepted flag directly bypasses the conversation entirely.

## Star-level flags

The flags that list a whole star level (Village 4, 12, 16, 25, 29, 34, 1008, 1025,
1035, 1053; Hub 101 … 125; G 1401 … 1413) are set by two announcement blocks of
kind 3, not by quest code:

1. The chief (Bherna Chief 1; Wyventurer 701 for ★7+; Guild Manager 520; Pub Manager
   801 for G) has a block "star level = L, flag *N − 1* clear", often with "urgent quest cleared" (402, 501, 713,
   806, 906, 10768, 11204, 11319). It sets *N − 1* and hands over a reward.
2. The quest counter NPC (the four village Gals 2, 102, 202, 302; Soaratorium Gal
   702; Wycademy Gal 502; Questender 802) has a block "star level = L, *N − 1* set,
   *N* clear" that sets *N*.

Flag *N* is what the unlock script tests. The numeric star level at
`base + 0x2C4DA` / `+0x2C4DC` is the input of both blocks. It is raised first, by
`0x50cbd0`, as soon as all urgents of a level are cleared: see
[05](05-quests.md#star-levels--base--0x2c4da).

`script\debug_flag_control` (`arc/debug/dbgResident.arc`, same bytecode as the unlock
script) is a developer shortcut for the same state and confirms two more opcodes:
`0x31 N` sets event flag N, `0x32 N` clears it, `0x33 Q` marks quest Q cleared
(handlers `0x3d2cdc`, `0x3d2d7c`, `0x3d2e28`). Its G-rank entry sets
flags 131, 132, 1400, 1401, 1403, 1405, 1407, 1408, 1409, 1411, 1413 and clears the
urgents 10768, 11204, 11319, 11401. `script\related_flag_control` only sets flag 2
from a talk callback.

## Other save state the talk data reads

Found through the conditions and actions above. The character block copies
`+0x20 … +0x41f` of the game's save object to `base + 0x280B` in one piece
(`0x51d0a0`), which places the first three rows; they agree with funds at
`base + 0x280F` (`+0x24`) and Wycademy points at `base + 0x2817` (`+0x2c`). The whole
block is mapped in [11](11-save-map.md#the-block-s0x20--0x41f--base--0x280b).

| Field | Offset | Type | Status |
|---|---|---|---|
| Contribution points, low rank | `base + 0x281B` | u32 × 4: Bherna, Kokoto, Pokke, Yukumo. The adder `0x523a80` caps each at 20000 | DERIVED (code + plausible values) |
| Contribution points, G rank | `base + 0x282B` | u32 × 4, same order. 280 / 240 / 125 / 110 in the analysed save | DERIVED |
| Progress word | `base + 0x2F77` | u32. Bit 20 = HR limit released, bit 31 = quest 10646 was listed (latch of condition 64), bits 8–10 = trading locations opened, bits 11 / 12 = the trader's second / third cart (conditions 95–97, 93, 94), bit 29 read by the unused condition 85. `0x8A7FFFFF` in the analysed save | bits 20 and 31 from code |
| Quest set map | `base + 0x3187` | 16 bytes (`+0xc70`), bit N = every quest of set N is cleared, see [05 — Quests](05-quests.md#quest-sets--base--0x3187). Bit 48 (condition 121; byte `base + 0x318D` bit 0) was clear in the analysed save and none of the four requests it gates had been offered; setting it made the chiefs offer them | bit 48 CONFIRMED by write |
| Hunter's Notes, large monsters | `base + 0x32B7` | 123 bits (`S+0x35b0`), the NEW copy at `base + 0x32C7` (`S+0x35c0`). Earlier revisions gave `base + 0x5027`, computed as if the file kept the object order; see [11](11-save-map.md#the-save-object-s) | from code |
| Hunter's Notes, second list | `base + 0x32D7` | 30 bits (`S+0x35d0`; earlier revisions: `base + 0x5047`) | from code |
| Random word | `base + 0x2C675` | u32, see conditions 106–108 | from code |
| Activity state | `base + 0x2381E` | 23 bytes at `+0x6f` of the activity manager, written by its serializer `0x198120` between a 1-byte and a 6-byte field. Each byte counts rewards that are waiting for a conversation (at most 99). Bytes 7–10 and 13–16 are village tickets: `0x197208` adds to them when the contribution points pass a threshold. The manager's tail starts at `base + 0x2380F` with 10 tier bytes (`+0x8c`; `2a 2a 2a 2a` = tier 42 in all four villages in the analysed save). All 23 state bytes are 0 there, which agrees with no chief offering a ticket | DERIVED: offset from the sizes of the blocks up to the [star levels](05-quests.md#star-levels--base--0x2c4da); the Palico names sit exactly where the same count puts them |

## Init scripts — `npc_NNN_is.nis`

`rNpcInitScript`. Header: f32 `1.0`, u32 count. 20-byte records: u32 index, u8 phase
(0, 3–7), u8 command, u16 argument, three f32. Commands 1 and 2 carry coordinates,
many others a time in seconds. Commands 4, 5, 7, 8, 10 and 48 carry an event flag in
the u16 (128, 203, 415, 1042, 1222, 1400 …), so the placement of an NPC also depends
on the flags. The scripts only read flags. **DERIVED** from the values; the
interpreter was not read.

## Event flag coverage

Of the 1536 event flags, 1017 are referenced somewhere (talk conditions and actions,
init scripts, unlock rules, request records). All 589 flags set in the analysed save
are among them, and every flag that an unlock rule tests is set by some talk line.
Seven flags set in the save are read by talk lines but set by no talk line, so code
sets them: 237, 239 (shop stock upgrades), 677, 1502 (a weapon at its last level) and
1503–1505 (story scenes).

## Open questions

- Conditions 143–146: what the lobby fields `+0x644` and `+0x41cc` / `+0x41c8` mean.
  No save field, and none of them gates a request.
