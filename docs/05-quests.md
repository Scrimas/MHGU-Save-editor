# 05 — Quests

Every quest in the game (Village, Hub, G-rank, Arena, Training, Special Permit,
Prowler and the built-in event quests) is tracked by **one list index**, shared by
three parallel bitmaps (cleared, seen, failed). The full index table is
[`data/quest-index.csv`](../data/quest-index.csv); its `group`, `alt` and `expansion`
columns are bytes `+4` … `+6` of the `quest_group` entry described below.

## Quest bitmaps — `base + 0x2C77`

| Bitmap | Offset (relative) | Slot 1 absolute | Meaning |
|---|---|---|---|
| Cleared | `base + 0x2C77` | `0x18F913` | quest cleared at least once |
| Seen | `base + 0x2D77` | `0x18FA13` | quest has been highlighted on the board (clears **NEW**) |
| Failed | `base + 0x2E77` | `0x18FB13` | quest failed at least once (end state 5), **DERIVED**, see [below](#third-bitmap--failed-quests) |

Each bitmap is 1509 bits (189 bytes, list indices 0–1508), LSB-first as elsewhere:

```
byte = bitmap + (index >> 3)
mask = 1 << (index & 7)
```

Index 0 is a placeholder and is never set. Nothing is stored past index 1508 up to
the next `0x100` boundary. The 19 bytes `0x18F900`–`0x18F912` hold something else and
are **not** part of the quest bitmap. Earlier revisions of this document put the
bitmap at `0x18F900`, so their bit numbers are 152 higher than the list indices.

### The index is a position in the game's quest list

**CONFIRMED.** The bit index is the quest's position in the game resource
`quest\quest_group` (type `rQuestGroup`, in `loc/arc/village/common.arc`), and has
no arithmetic relation to the quest ID. That is why every ID-based formula failed.

The resource layout:

```
0x00  u32   header (0x40800000)
0x04  u32   entry count (1509)
0x08  entry[count], 7 bytes each, packed:
        +0  u32  quest ID (entry 0 = 0, the placeholder)
        +4  u8   group: 0 = regular, 1-10 = key quests of Village ★1-★10,
                 11-17 = key quests of Hub ★1-★7, 19-22 = G★1-G★4,
                 24-33 = urgents that raise the Village star level to 1-10,
                 34-46 = urgents that raise the Hub star level to 1-13,
                 47-86 = twin quests in pairs (47/48, 49/50, ...), 127 event Hub,
                 128 event Arena, 129 Prowler
        +5  u8   alternative set inside a key group (quests sharing a value count once)
        +6  u8   1 = quest added by the XX / Generations Ultimate expansion, 0 = from
                 the first game. DERIVED: 1 on every G-rank and Village ★7-★10 quest,
                 0 on every Village ★1-★6 and Hub ★1-★3 quest
```

The executable's clear check (`0x523DA4` in the v1.4 NSO, ARM32) takes a bitmap
pointer and a quest ID. It linearly searches the loaded list from index 1 for the ID,
and tests `bitmap[index]`. An ID that is not found falls back to index 0. The setters
for the seen bitmap (`0x523E44`) and the third bitmap (`0x526B98`) address it as
`cleared + 0x100` and `cleared + 0x200`, using the same index.

The group byte is read by the key-quest counters `0x3b1964` / `0x3b1a18` (Village,
groups 1–10) and `0x3b1d70` (Hub, groups 11–23), which count total and cleared
quests of one group. For groups 47–86 the cleared setter (`0x523f30` via `0x526a38`)
also sets the partner quest of the pair (group ± 1), e.g. 308 ↔ 309. The urgent
groups are read by the star level update, see
[star levels](#star-levels--base--0x2c4da).

The list is identical in the base game and the v1.4 update.

Evidence that the index is correct, all from the analysed save:

- A controlled diff that cleared G-rank quest 11463 (*Seregios Scuffle*) moved
  exactly one bit, which is index 795.
- All six quests in the history log (814, 11234, 11302, 11303, 11357, 11463) are set.
- The 228 Special Permit quests occupy indices 947–1174. This is exactly the deviant
  level bitmap of [03 — Deviants](03-deviants.md), found independently: global bit
  `0x18F913*8 + 947` is `0x18F989` bit 3.
- Every cleared quest is also seen, and no bit is set outside the list, in any of
  the three bitmaps.

### Quest ID scheme

IDs are decimal, `E P T SS NN`:

| Digit(s) | Meaning |
|---|---|
| `E` (×1 000 000) | 1 = built-in event quest |
| `P` (×100 000) | 1 = Prowler-only quest |
| `T` (×10 000) | 0 Village, 1 Hub, 2 Arena, 3 Training, 4 Special Permit |
| `SS` | star level: Village 1–10, Hub 1–7, G-rank 11–14 (G★1–G★4). Special Permit: deviant 1–18, in the order of [03](03-deviants.md) |
| `NN` | sequence. Special Permit: level, 1–10 = Lv1–Lv10, 11–15 = G1–G5, 16 = EX. The 6 GU deviants only have 11–16 |

`rank` in the CSV comes from byte `0x12` of each quest's `questData` resource,
which matches `SS` for every regular quest and also gives the rank of event quests.

### List order

| Indices | Contents |
|---|---|
| 1–337 | Village (1049 and 1050 appear twice, see below) |
| 338–865 | Hub, Low → High → G-rank |
| 866–946 | Prowler Village, Prowler Hub |
| 947–1174 | Special Permit |
| 1175–1287 | Training (Prowler Training at 1182–1189) |
| 1288–1304 | Arena, Prowler Arena |
| 1305–1508 | Event quests (Hub, then Arena), 15 of them `DUMMY` placeholders |

| Category | Entries |
|---|---|
| Village | 335 (+2 duplicates) |
| Hub (incl. G-rank) | 528 |
| Special Permit | 228 |
| Training | 105 |
| Arena | 11 |
| Prowler (Village / Hub / Training / Arena) | 40 / 41 / 8 / 6 |
| Event (Hub / Prowler Hub / Arena / Prowler Arena) | 112 / 34 / 30 / 13 |

**Duplicates.** Village quests 1049 and 1050 appear at indices 313–314 and again at
315–316. The search stops at the first match, so 315–316 are never read. They are
flagged in the CSV and should be left alone.

### Third bitmap — failed quests

**DERIVED.** The only setter (`0x526b98`) is called from the quest result code
(`0x388d60`) on the branch where the quest did *not* succeed, and only if the quest
ended in end state 5 (`0x3a5ed0` case 4 tests byte `+0x50` of the quest object). The
success branch sets the cleared bit instead. No code clears the bit again.

The only reader (`0x526c84`) is the Hunter's Notes unlock `0x55515c`: a monster's
entry opens if its quest is cleared **or** has this bit, so a failed attempt still
counts as having met the monster.

13 bits are set in the analysed save: 10318, 11422, 11457, 11468, 40401, 41411,
41511, 41611, 41614, 41616, 1010150, 1011001, 1011030. That fits: all are hard
quests (Old Fatalis, Boltreaver EX …), and 11422 is failed but never cleared.
It is not board visibility: none of these quests is hidden.

End state 5 is a real failure, not an abandoned quest. The byte is written in two
places while the quest runs (state 2): the fail routine `0x3a5870` writes 5 for every
failure reason it is called with (its reason argument 0 / 1 / 2 only picks the
jingle), and the return-from-quest routine `0x3a93ac` writes 6. The predicate
`0x3a5ed0` groups them accordingly: case 3 "failed or abandoned" accepts 5 and 6,
case 4, the one used here, only 5. Success is 3 or 4.

### Controlled writes

**CONFIRMED — cleared.** Setting the cleared bit of an uncleared Village ★2 quest
(202, *Harvest Tour: Dunes*) made it display as cleared in-game.

**CONFIRMED — the second bitmap is "seen", not "unlocked".** Evidence:

- Setting it for a hidden Village ★6 quest (607, *The Perilous Pair*), together with
  the cleared bit, did **not** make the quest appear.
- A visible quest (612, *Break the Brachydios*) had the bit clear and showed
  **NEW**.
- With the bit restored to 0, Hub ★2 *Poisonous Pest* (10227) showed **NEW**. Moving
  the cursor onto it and saving flipped exactly one bit across all three bitmaps:
  its seen bit, index 392, 0 → 1. Opening a list without highlighting anything
  changes nothing.

**Board visibility is not stored in these bitmaps.** For 607 the reason is that it
is a villager request. See [Villager requests](#villager-requests--tableactivitydataatd).

## Quest sets — `base + 0x3187`

**DERIVED from code, checked against the save; bit 48 CONFIRMED by write.** A 16-byte
bitmap (`+0xc70` of the save object, file `0x18FE23` in slot 1): bit N is set when
**every quest of set N has been cleared**. There are 100 sets (bits 0–99). The column
`sets` of [`data/quest-index.csv`](../data/quest-index.csv) lists the sets of each
quest.

| Sets | Members |
|---|---|
| 0–5 / 54–57 | Village ★1–★6 / ★7–★10, hunter quests |
| 6–12 / 62–65 | Hub ★1–★7 / G★1–G★4, hunter quests |
| 13–18 / 58–61 | Village ★1–★6 / ★7–★10, hunter and Prowler quests |
| 19–25 / 66–69 | Hub ★1–★7 / G★1–G★4, hunter and Prowler quests |
| 38, 39, 40 / 70, 71 | Prowler quests: Village ★2–★6, Hub ★1–★3, Hub ★4–★7 / Village ★7–★10, Hub G |
| 26–37 | the 15 levels (I–X, G1–G5) of one of the 12 older deviants, Redhelm … Hellblade |
| 72–77 | the 5 G levels of Nightcloak, Rustrazor, Soulseer, Boltreaver, Elderfrost, Bloodbath |
| 80–97 | the EX quest of each of the 18 deviants, one quest per set |
| 41, 42, 43, 44 | low-rank Village quests whose [board byte](#where-a-quest-is-posted--questdata--0x11) is 4 or 8 (Bherna), 1 or 5 (Kokoto), 2 or 6 (Pokke), 3 or 7 (Yukumo): 11 / 19 / 18 / 26 quests, mostly villager requests |
| 48 | Village ★1–★6 without the *Advanced* quests and without the second quest of a twin pair: 153 quests |
| 99 | Village ★7–★10 without the *Advanced* quests: 142 quests |
| 45, 46, 47, 53 | the 10 low-rank Arena quests (7 hunter, 3 Prowler): all cleared / all with rank A or better / all with rank S / all with rank B or better |
| 98, 78, 79 | all 17 Arena quests: all cleared / all rank A or better / all rank S |
| 49–52 | no member found |

**How it is filled.** `0x3b85d8` walks the whole quest list, skips event quests and
repeated IDs, and keeps one counter of *uncleared* quests per set. The set of a quest
follows from its ID digits, the expansion byte of `quest_group`, a static table
(`0x162cdc5`) and, for sets 41–44, byte `0x11` of its `questData`. A set whose only
uncleared quest is the quest being played is written into eight slots
(`+0x1f3` of the quest manager). After the clear, `0x3f1950` sets the bits of those
slots here and, for a bit that was clear before, in two companion maps (`+0xc80`,
`+0xc90`). Only the second is saved, at `base + 0x3197`: it is the list of **set
notices still to show**. The quest result flow (`0x38fab0` onwards) queues one notice
per set bit through `0x1636a8` and clears the bit: sets 0–5 and 54–57 (Village levels),
6–12 (Hub), 62–65 (G), the deviant sets from 26 and the Prowler sets 38, 39, 40, 70,
71. A set without a notice keeps its bit for good, which is why bit 13 is still set
there in the analysed save. An edit can leave this map alone. For the rank sets the
setter also reads the rank of the Arena record (`+0x1244 >> 29`: 0 = S, 1 = A, 2 = B):
46 and 78 want A or better, 47 and 79 S, 53 B or better.

So the bit is written **only at the moment the game sees the last clear**. A set that
is completed by editing cleared bits keeps its bit clear until the bit is written too.

The membership was extracted by running `0x3b85d8` itself under an emulator
(`scratch/py/qsets.py`, unicorn) with every quest uncleared, and recording which
counter each quest raised.

**Readers.**

- Talk conditions 89, 90, 121, 161, 162 and 163 test bits 45, 46, 48, 98, 78 and 99,
  see [10 — NPC talk data](10-npc-talk.md#conditions). Bit 48 gates the four chiefs'
  last requests (*Advanced: Wrath of Rath* and its siblings): they want every ordinary
  low-rank Village quest cleared. Bit 99 gates the quest counter line that releases the
  last Village ★10 quests (flag 1073).
- The award check `0x3ec020` reads the map: bits 13 and 14 together grant award 0,
  *completed all 1★ and 2★ Village Quests*, bits 15 and 16 award 1, and so on. It
  writes the game-side award map at `base + 0x3157`, see
  [09 — Awards](09-awards.md).

**Evidence.** In the analysed save every set bit belongs to a set whose members are
all cleared (0, 13, 74–77, 80–97), and every set with an uncleared member has its bit
clear: 81 of the 96 sets with members agree in both directions. Bit 48 was set by hand for the
[controlled write](10-npc-talk.md#per-npc-bits--base--0x2c62d) with 75 of its 153
quests still open, and the chiefs offered their requests. The other 14 sets (26–37,
72, 73) are complete but their bit is clear. **UNRESOLVED**: the code gives no second
condition, and emulating `0x3b85d8` with 40115 as the last open quest of set 26 does
put 26 into the slots, so a normal last clear would have set the bit. The likely
reason is this save's history: all 228 Special Permit quests are cleared in every
snapshot while only a third of the Village and Hub quests are, and the
[history log](#quest-history-log--0x254771) shows four EX quests played back to back
in descending deviant order on one day. That fits deviant levels marked cleared by
an edit (no last clear seen, bit stays clear) and EX quests then played for
real (sets 80–97 set). The owner of the save confirmed it: the levels were unlocked
by the first byte edits of this project, before the quest bitmaps were understood.

## Where a quest is posted — `questData + 0x11`

Byte `0x11` of each quest's `questData` resource names the board that lists it
(field name from the [MHXX rQuestData notes](https://github.com/svanheulen/mhff/wiki/MHXX-rQuestData-Format)):

| Value | Board |
|---|---|
| 1 / 2 / 3 / 4 | Home village Kokoto / Pokke / Yukumo / Bherna of a quest that is *not* a villager request. **DERIVED** from the members: 1 holds the Verdant Hills tours and *Alas, Astalos Again*, 2 Arctic Ridge and Gammoth, 3 Misty Peaks and Mizutsune, 4 Glavenus. Same village order as 5–8. No board filters on it (below) |
| 5 / 6 / 7 / 8 | Kokoto / Pokke / Yukumo / Bherna |
| 9 | Prowler |
| 10 | Every board |
| 11 | Special Permit |

The 5–8 mapping comes from the quest names (Jurassic Frontier quests at 5, Popo and
Giaprey at 6, *The Yukumo Gal Special* at 7, Moofah quests at 8). It agrees with the
village field of the request table below.

Two predicates of the quest manager read the byte. `0x3a3470` is true for 5–8 and
12 (no quest uses 12): "this is a villager request quest". `0x3a3424` uses the same
mask in the quest result code (`0x388d60`): for such a quest the cleared bit is
written only if the quest is listed on this save (`0x54af8c`, which runs the unlock
script). A request quest played through another hunter's posting therefore does not
count. Values 1–4 are in neither mask. The other readers treat 1–4 like 5–8: the
[quest set](#quest-sets--base--0x3187) counter (`0x3b85d8`), and `0x3a9ea4`, which
picks the village whose contribution points the result screen shows (`0x38b124`;
4/8 Bherna, 1/5 Kokoto, 2/6 Pokke, 3/7 Yukumo, else none). The board icon tables of
`uUIQuestBoard` map the value to an icon.

**DERIVED — no board filters on the byte.** Which quests a board shows comes from
static per-board, per-category lists (`0x54b994` → `0x3bc96c`), chosen by the scene
index (table `0x159f0b0`): the Hunters Hub, the second hub, and one village board
shared by all four villages.

Almost every quest with a value of 1–8 is a villager request. It appears only after
the request has been accepted, and only on that village's board.

## Board visibility — `script\check_quest_unlocked`

**CONFIRMED.** Whether a board lists a quest is not stored per quest. Each time a
list is built the game runs a script with the quest ID and shows the quest only if
the script returns 1. The script reads state that *is* in the save: the
[event flags](#request-flags--base--0x2c56d), the cleared bitmap, HR and the Hub
star level. The full rule table is
[`data/quest-unlock.csv`](../data/quest-unlock.csv), and
[`tools/quest_unlock.py`](../tools/quest_unlock.py) evaluates it against a save and
prints what each locked quest is missing.

The script is the resource `script\check_quest_unlocked` (type `rCommonScript`) in
`loc/arc/village/common.arc`. It is byte-identical in the base game and v1.4.

### Rule syntax in the CSV

All predicates of a rule must hold (`AND`).

| Predicate | Meaning | Save field |
|---|---|---|
| `flag:N` | event flag N is set | bit N of `base + 0x2C56D` |
| `cleared:Q` | quest Q is cleared | cleared bitmap |
| `atleast:K:Q1\|Q2…` | at least K of the listed quests are cleared | cleared bitmap |
| `all:Q1\|Q2…` | all listed quests are cleared (the key quests of one Hub level, `quest_group` group 11–22) | cleared bitmap |
| `hr:N` | HR ≥ N | u16 `base + 0x28` |
| `hub_star:N` | Hub star level ≥ N | u16 `base + 0x2C4DC` |
| `always` | no condition | |
| *(empty)* | event quest (ID ≥ 1 000 000): not in the script, listed whenever it is installed | |

How the 1302 rules split: 762 quests need one event flag, 293 one cleared quest, 122
an `atleast`, 118 nothing, 96 an HR, 37 a full key-quest set, 25 a Hub star level.

The flags that open a whole star level:

| Board | Flags, in star order |
|---|---|
| Village ★1–★6 | 4, 12, 16, 25, 29, 34 |
| Village ★7–★10 | 1008 (+1069, 1070), 1025 (+1029), 1035 (+1050), 1053 |
| Hub ★1–★7 | 101, 105, 109, 113, 117, 121, 125 |
| G★1–G★4 | 1401, 1405, 1409, 1413 (+1415) |

For 148 of the villager-request quests the rule is exactly `flag:<accepted flag>` of
the request table, which was found independently. 19 chief requests use another flag.

**The G-rank deviant gate** of [03](03-deviants.md) is an ordinary rule: G1 needs
Lv10 cleared **and** one G-rank quest against the base monster, for example
`41111` (Grimclaw G1) = `cleared:41110 AND atleast:1:11405|11462`. EX is
`cleared:<G5> AND hr:100`. Lv1 needs one Hub quest against the base monster (10124
for Redhelm) or 10768.

### Evidence

1. Evaluated on the analysed save, the rules unlock 1116 quests and lock 186. All
   990 quests with the seen bit set are among the unlocked ones, with **zero
   violations**, and no locked quest is seen or cleared. One wrong field would show
   up here: a wrong offset for the Hub star level produced 25 violations at once.
2. The rule for 607 is `flag:396`, the bit whose controlled write made the quest
   appear ([above](#request-flags--base--0x2c56d)). On the snapshot taken before
   that write the tool reports 607 locked for that reason.
3. The deviant rules match the behaviour recorded in [03](03-deviants.md): G-rank
   Arzuros released Redhelm's G-rank levels.

4. Controlled write of a flag that no request uses: setting event flag 1059 alone
   (`0x1B928D` `0x00 → 0x08`, both slots) made the five Village ★10 quests whose rule
   is `flag:1059` appear: *Advanced: Wrong One Silver*, *Heart of Gold*, *Steel
   Yourself*, *Karma Chameleos* and *Empire of the Sun*. The last three are also in
   the rotating table below, and their bits (29–31) were set at the time.

5. Controlled write of cleared bits: setting the cleared bit of *Misty Opportunity?*
   (11404, `0x18F976` `0x4A → 0xCA`) made *Appeal from Authority* (11446) appear on the
   G★4 list. Its rule is `all:… AND atleast:1:11404 AND hr:13`, and 11404 was the
   only missing part.
6. The same write set the cleared bit of *The White Brute* (619, `0x18F926`
   `0x30 → 0x38`), the whole rule of *It's Electric* (618). 618 did **not** appear: it
   is a rotating quest and its rotation bit was clear. Setting that bit (next
   section) listed it. Rule and rotation are both required, as decoded.

`hub_star` has had no controlled write of its own; it rests on the code and on the
consistency test. `hr` was only part of the 11446 rule, where it already held.

### Script format

```
0x00  f32   1.0
0x04  u32   instruction count (5535)
0x08  instruction[count]:
        u32  line number
        u8   opcode
        cstr operand A
        cstr operand B
```

The interpreter (`0x3d17f8`, opcode table at `0x3d5874`) runs top to bottom with
`[WORK0]` = quest ID. A failed test skips forward to a marker opcode, there is no
other control flow.

| Opcode | Meaning |
|---|---|
| `0x20 [WORK0] N` | if quest ID ≥ N, skip to the next `0x24` (one section per ID range) |
| `0x0E [WORK0] Q` | label: if quest ID = Q, jump to the next `0x26`, the rule body |
| `0x13 [HR] N` | between labels: if HR < N, skip to `0x23`. The labels after it need HR ≥ N |
| `0x27` | no label matched: skip to `0x23` |
| `0x41 N` | `[WORK1]` = event flag N (`[0x1884c58] + 0x500`) |
| `0x76 Q` | `[WORK1]` = quest Q cleared (`0x523da4`) |
| `0x7E` | `[WORK1]` = Hub star level (`[0x18979f0] + 0x3e2`) |
| `0x9E N` | `[WORK1]` = all quests of `quest_group` group N + 10 cleared (`0x3b1d70`) |
| `0x52 [WORK2] 0`, `0x50 [WORK2] [WORK1]` | `WORK2 = 0`, `WORK2 += WORK1`: counts cleared quests |
| `0xA1 a b`, `0xA7 a b` | require a = b, a ≥ b. On failure the script returns 0 |
| `0x16 [HR] N` | if HR < N, skip to the next `0x26` (an alternative body) |
| `0x31 N`, `0x32 N` | set / clear event flag N. Not in this script; used by `script\debug_flag_control` |
| `0x33 Q` | mark quest Q cleared. Same |
| `0x03` | return 1 |
| `0x02` | end of section, return −1: not in the script, treated as locked |

`[HR]` is the u16 at `+0x554` of the player object. `[FESTA_HR]` appears in 14
alternative bodies for a convention build and is dropped from the CSV. One label has
an empty quest ID, a typo in the shipped script.

The board's list filler is `0x54b994`. For each quest of the star level it asks
`0x3b2174` → `0x3d7e90`, which runs the script and caches the answer. Event quests
skip the script (`0x3b9870`: ID / 1 000 000 mod 10 = 1). Which archives get loaded
(`0x3b8090`) depends only on the ID and the place, not on the save.

### Rotating quests — `base + 0x504B`

**CONFIRMED.** A second filter applies to 51 quests held in a table in the executable
(`0x162cde4`, 8 bytes each: quest ID and a u32 parameter), listed in
[`data/rotating-quests.csv`](../data/rotating-quests.csv). Such a quest is listed only
if its bit, the table index, is set in the u64 at `base + 0x504B` (file `0x191CE7`).
A quest of this table needs its script rule **and** its bit.

The game re-rolls the field in `0x54b1c4`. In the snapshots it changed after every
completed quest and never otherwise. Entries 0–7 are the four village pairs 308/309,
319/320, 324/325, 329/330, of which one each stays set; 8/9 are 618/619. The rest are
elder dragon and other repeating hunts (10329–10333, 10641–10643, 10756–10761,
11316–11318, 11412–11417, 11458–11460, …).

**DERIVED — the parameter** (roll `0x54b1c4`, mode switch `0x54b388`). Byte 0 is a
chance in percent (100, 80, 50, 40, 25 or 0), byte 1 a mode (low nibble, signed; a
negative mode is never listed), bytes 2–3 a u16 argument. Each re-roll draws
`rng % 100` per entry, clears the bit and sets it again when:

| Mode | Listed when |
|---|---|
| 0 | roll < chance |
| 1 / 2 | bit *arg* of the talisman-making quest map at `base + 0x2C6A9` (sMakeAmulet `+0x64`, test `0x14ce9c`) is clear / set. No roll. Entries 44, 47, 49, 51 of that map belong to quests 308, 319, 329, 324, so each village pair shows its first quest until that bit is set at quest end (`0x14cd64`), then its second |
| 3 | roll < chance and the quest of table entry *arg* is cleared (618: *arg* 9 = 619) |
| 4 | always until the quest is cleared, then roll < chance (619, 633–635) |
| 5 | group head, *arg* = number of entries that follow; one member is picked by cumulative chance. Followers (mode 5/7 with *arg* 0) are skipped by the loop (`0x54b318`) |
| 6 | only while the quest is not cleared (730) |
| 7 | like 5, but the head stays listed alone until it is cleared (1005, chance 0; then one of 1053, 1022, 1021, 1020 at 25 % each) |

An abandoned quest (`0x54b0d4`) re-evaluates modes 1 and 2 only.

Controlled write: with its rule satisfied, *It's Electric* (618, bit 8) stayed hidden
while the bit was clear. Setting it (`0x191CE8` `0xBE → 0xBF`, both slots) listed the
quest next to its partner 619. `tools/quest_unlock.py` reports such quests as
`rotated`.

### Star levels — `base + 0x2C4DA`

| Field | Offset | File offset | Type | Analysed save |
|---|---|---|---|---|
| Village star level | `base + 0x2C4DA` | `0x1B9176` | u16, 1–10 | 10 |
| Hub star level | `base + 0x2C4DC` | `0x1B9178` | u16, 1–13 | 13 |

They are fields `+0x3e0` / `+0x3e2` of the object serialized by `0x507e20`, the
block just before the event flags (149 bytes from `base + 0x2C4D8`; it also holds
four 32-byte pet names). The Hub value is **CONFIRMED** by the consistency test
above. The Village value is **CONFIRMED** the same way by the talk data: the
request offer blocks test it (`village_star` in
[`request-offer.csv`](../data/request-offer.csv)) and all accepted requests agree. The
talk data also sets the star-level flags from these two numbers, see
[10](10-npc-talk.md#star-level-flags).

**What raises them — CONFIRMED (code + controlled write).** `0x50cbd0` walks `quest_group`. For every urgent
group it counts the members and how many of them are cleared. If a group has members,
all of them are cleared, and the stored level is lower than the group's level, the
level is set to the group's level. It never lowers a level, and a level can be
skipped. Two exceptions: while the Village level is 5 only 601 counts in group 29,
and while it is 9 only 1005 counts in group 33, so the final bosses in those groups
do not hold the level back.

The function runs right after the cleared bit is set in the quest result code
(`0x388e54`), from talk condition 64 while its latch is clear (`0x3f12fc`), and from a
setup sequence at `0x6aa5d0` that an ordinary load does not go through.

Controlled write: the Hub level was lowered from 13 to 12 (`0x1B9178`
`0x0D → 0x0C`, both slots) with every urgent still cleared. After loading the
character, walking through four villages and saving, the file still held 12. After
one cleared quest (1038, not an urgent) it held 13 again. **CONFIRMED**: loading does
not recompute the level, clearing any quest does. For an editor: write the level
together with the urgents' cleared bits, or just the cleared bits and let the next
quest clear raise the level. The star-level *flags* still
need the two conversations of [10](10-npc-talk.md#star-level-flags), or a direct
edit.

| Level | Group | Urgents (the quest list shows an urgent under the level it unlocks) |
|---|---|---|
| Village 1 | 24 | — (no member, never applies) |
| Village 2 | 25 | 206 *Vaulting Outlaw* (Village 2) |
| Village 3 | 26 | 303 *Tusked Tantrum* (Village 3) |
| Village 4 | 27 | 402 *The Nocturnal Enchanter* (Village 4) |
| Village 5 | 28 | 501 *The Dark Age* (Village 5) |
| Village 6 | 29 | 601 *The Scorching Blade* (Village 6), 620 *Stop the Wheel* (Village 6) |
| Village 7 | 30 | 713 *Research Team's First Rodeo* (Village 7) |
| Village 8 | 31 | 806 *Primal Forest Arachnids* (Village 8) |
| Village 9 | 32 | 906 *Wish upon a...Gravios?* (Village 9) |
| Village 10 | 33 | 1005 *Beware the Comet of Disaster* (Village 10), 1026 *Grave Peril* (Village 10), 1017 *King of Hellfire* (Village 10), 1018 *The Seat of a God* (Village 10), 1019 *Stormlord* (Village 10), 1039 *Blazing Black of a Dark God* (Village 10) |
| Hub 1 | 34 | — (no member, never applies) |
| Hub 2 | 35 | 10216 *The New Tenant* (Hub 2) |
| Hub 3 | 36 | 10307 *A Shocking Scoundrel* (Hub 3) |
| Hub 4 | 37 | 10335 *Two-headed Carcass* (Hub 3) |
| Hub 5 | 38 | 10531 *A Plesioth in the Misty Peaks* (Hub 5) |
| Hub 6 | 39 | 10606 *A Bewitching Dance* (Hub 6), 10608 *The Unshakable Mountain God* (Hub 6) |
| Hub 7 | 40 | 10710 *Seer of Swords* (Hub 7) |
| Hub 8 | 41 | 10722 *Hellfire Star* (Hub 7) |
| Hub 9 | 42 | 10768 *Legendary Skills?* (Hub 7) |
| Hub 10 | 43 | 11204 *Dirty Deals* (Hub G2) |
| Hub 11 | 44 | 11319 *Giant Dragon Invasion* (Hub G3) |
| Hub 12 | 45 | 11401 *Sky Render* (Hub G4) |
| Hub 13 | 46 | 11432 *Castle on the Run* (Hub G4) |

Hub level 0 means the Hub is not joined yet: the constructor writes Village 1, Hub 0,
and the Guild Manager's first conversation sets Hub 1 (talk action 1 type 10,
`0x247bf0`). In the analysed save every group is fully cleared, which gives 10 / 13,
the stored values.

## Villager requests — `table/activityData.atd`

A standalone table in the romfs. Header: u32 `0x40A00000`, u32 count 184. It is followed by
184 records of 55 bytes. The full table is in [`data/request-index.csv`](../data/request-index.csv).

| Offset | Type | Meaning |
|---|---|---|
| `+0x00` | u32 | Record index |
| `+0x04` | u8 | Kind: 0 = quest, 1 = delivery, 2 = unknown, 3 = end marker |
| `+0x05` | u8 | Stage: 5–10 match Village ★1–★6, 11–14 Village ★7–★10, 15–21 Hub ★1–★7, 22–25 G1–G4, 26 late G4. Inferred from the stars of the quests. |
| `+0x06` | u8 | NPC. Records with the same NPC form one character's request list. |
| `+0x07` | u8 | Village: 0 Bherna, 1 Kokoto, 2 Pokke, 3 Yukumo |
| `+0x16` | u32 | Quest ID, for kind 0 |
| `+0x1A` | u32 | Prerequisite quest. Set on one record only: 625 needs 10730. |
| `+0x25` | u16 + u8 | Reward ID and count. Item IDs for tickets (Kokoto Ticket ×2 …); the Argosy Captain's 10220 reward, `0x6D9`, arrived in-game as a Poogie costume. |
| `+0x33` | u16 | **Accepted** flag index into the [request flag bitmap](#request-flags--base--0x2c56d) |
| `+0x35` | u16 | **Completed** flag index (always accepted + 1) |

Record 40 is *The Perilous Pair*: Argosy Captain (NPC `0x21`), Kokoto, stage 10.

**CONFIRMED — the posted-in village is not why 607 is hidden.** It was checked on the ★6 board in
Bherna and in Kokoto, and it was absent from both.

### Request flags — `base + 0x2C56D`

| Field | Offset | File offset | Size | Status |
|---|---|---|---|---|
| Event flag bitmap | `base + 0x2C56D` | `0x1B9209` | 192 bytes, 1536 bits, LSB-first | CONFIRMED |
| Per-NPC bits A / B / C | `base + 0x2C62D` | `0x1B92C9` | 3 × 24 bytes, one bit per NPC: on hold for request offers / kind 9 talk / story announcements. See [10](10-npc-talk.md#per-npc-bits--base--0x2c62d) | map B CONFIRMED by write, A and C from code |
| Two u32 | `base + 0x2C675` | `0x1B9311` | 8 bytes, change on every save. The first (`+0x608` of the object) is a random number: talk conditions 106–108 compare it modulo 10000 with a threshold, which makes a line appear with a fixed chance | from code |

The block is one object of the game (flags at `+0x500`, serializer `0x240ce4`). Each
request record names two bits in the first map:

- **accepted** (`record + 0x33`): the NPC's request is active. For kind 0 this is
  what **posts the quest on the village board**.
- **completed** (`record + 0x35`): reported back to the NPC, reward given. CONFIRMED
  by write (2026-10-06): clearing it on a cleared request (0, *On the Rebound*, flag
  301) made the game ask for the report again; reporting set it back.

The indices are not regular (`300 + 2·record` holds only for the first 17 records),
so take them from [`data/request-index.csv`](../data/request-index.csv).

**CONFIRMED**, three independent lines:

1. All 184 records agree with the quest bitmaps: accepted ⇔ quest seen (one
   exception, accepted but never hovered), completed ⇒ cleared. This offset is the
   only byte-aligned one in the character block with zero violations.
2. Reporting *Ahoy! Royal Ludroth!* (10220) to the Argosy Captain set exactly bit
   395, its completed flag, plus 41, 796 and "1569". The talk data explains two of
   them: 796 is set by the next line of the same conversation, and 1569 is past the
   end of this map, it is bit 33 (the Captain) of per-NPC map A.
3. Controlled write: setting bit 396 alone (`0x1B923A` `0x0C → 0x1C`, both slots)
   made *The Perilous Pair* (607) appear on the Kokoto ★6 board. Hovering it then
   set its seen bit as usual.

In the executable, the quest-list builder `0x79da8c` tests the accepted flag
(`record + 0x3a` in memory) and the request memo UI (`cUIOHNActivityMemo`) reads
both. The loader is `rActivityData` `0x3c9ce0` (float version 5.0, 64-byte records
in memory, parser `0x13c00`).

### What did not unlock 607 through play

Clearing *The Fated Four* (621), then clearing and reporting the Captain's other
request (10220), did not make him offer 607. The stage gate was already met (five
other stage-10 requests cleared), and per-NPC order is not the gate (607's stage is
lower than 10220's). The posted-in village is not the reason either.

Other flags those steps changed. The quest block serializer (`0x51d12c`) places them:
Fated Four set bit 10 of the u32 at `base + 0x2C5F` and award 34 in the game-side
[award map](09-awards.md#game-side-map--base--0x3157) at `base + 0x3157` and its
notice copy at `+0x316B`; the Captain report set award 83 in the same two maps and
bit 14 of the 8-byte words at `base + 0x2F9F` / `+0x2FA7`. Those words are the Poogie
costume map and its NEW copy ([11](11-save-map.md#unlock-maps)): the report gave a
costume, and award 83 is *Fine Poogie Ball*, "collected most of the Poogie costumes"
([09](09-awards.md#layout)). Such fields come in pairs, a state word and a copy that
drives a one-time notice.

**What makes an NPC offer a request** is in the NPC's talk data, see
[10 — NPC talk data](10-npc-talk.md) and [`data/request-offer.csv`](../data/request-offer.csv).
The Captain offers 607 when the Village star level is ≥ 6 and flag 395 is set, which
is 10220 **reported** to him. The report also puts him on hold (per-NPC map A) until
the next quest, so in that session he had nothing to offer. No code sets the accepted
flag from the request record: the talk action does (`0x247a00`), and conditions go
through the evaluator `0x2451c8`. `record + 0x31` (u16) is the Wycademy points the
report pays: it equals the parameter of talk action 2 type 4 in the report block on
all five records where it is not 0 (500 four times, 200 once). **DERIVED**

### Editing

To mark a quest cleared, set its cleared bit, and its seen bit if you don't want a
leftover NEW marker. Use OR, as always. This does not make a hidden quest appear by
itself, but it can satisfy the `cleared` / `atleast` / `all` rule of another quest.

To make a hidden quest appear, satisfy its rule from
[`data/quest-unlock.csv`](../data/quest-unlock.csv): set the event flag, or the
cleared bit of the prerequisite, or raise HR or the Hub star level.
`tools/quest_unlock.py <save> <quest id>` prints what is missing. Flag rules are
confirmed by two controlled writes (396 and 1059), `cleared` / `all` / `atleast` by
one (11404 → 11446, 619 → 618). A rotating quest also needs its bit at
`base + 0x504B`, which lasts until the next completed quest.

For a villager-request quest the flag is its **accepted** flag. Leave the completed
flag alone: the NPC sets it, and hands over the reward, when the cleared quest is
reported.

Setting a star-level flag lists the quests, it does not replay what the game does
when that level is reached in play (urgent notices, HR, cutscenes), so expect the
other progress fields to stay as they were.

## Quest history log — `0x254771`

The Guild Card's list of the 10 most recently completed quests, newest first.

| Field | Offset in record | Type |
|---|---|---|
| Date | `+0x00` | u8 day, u8 month, u16 year (`13 09 ea 07` = 19 September 2026), the console date of the clear |
| Record kind | `+0x04` | u16, 7 in all ten records. Kinds (message table `0x163129c`): 0 HR up (`+6` = HR), 1 MHGen transfer, 2 Welcome, 3 forged %s, 4 / 5 per-weapon upgrade / forge counts (`+6`), 6 "Earned all %s Awards", **7 / 8 / 9 quest completed / failed / abandoned**, 10 hired %s. 11 is the constructor's "unset" value and is never inserted ([11](11-save-map.md#card-layout-6328-b)) |
| Quest ID | `+0x06` | u16 |
| Quest name | `+0x08` | UTF-16LE, 16 characters, cut with `…` |
| Highlight codes | `+0x28` | u8 × 3, code 36 = empty. **DERIVED** from the writer `0x1636a8`: a new highlight replaces the slot with the highest code when its own code is lower, and the three are kept sorted ascending, so lower codes win. The quest end adds codes 20–22 for party results (`0x390cd0`). The card screen turns a code into a message through a 36-entry table (`0x16313b0`, `0x5a35e8`); codes [below](#highlight-codes) |
| Highlight values | `+0x2C`, `+0x38` | u32 × 3 and u32 × 3: the first and second value of each highlight, same slot order. In the four EX deviant records the code-3 highlight has `0x0402`, `0x0412`, `0x042d`, `0x0425` as its first value: the monster codes of Dreadking, Deadeye, Crystalbeard and Silverwind ([03](03-deviants.md#open-questions)) |
| Hunter and Palico names | from `+0x44` | UTF-16LE |
| Weapon types | `+0x9C` | u8 × 4, one per party slot, in the order of [04 — Weapon usage](04-weapon-usage.md): 13 = Charge Blade in eight records, 10 = Dual Blades in two, which matches the usage counters; 15 = a Palico, `ff` = empty slot. **DERIVED** from the values |

### Highlight codes

**DERIVED** from the quest-end builder `0x38f454` (called from `0x38ce60`) and the
renderer table `0x16313b0`. Messages are `GuildCardMsg` entries.

| Code | Message | Value 1 / value 2 |
|---|---|---|
| 0 | 103–108, 274–277: all N★ Village quests complete | star − 1 |
| 1 | 109–115: all Hub N★ quests complete (1–7★) | star index |
| 2 | 278–281: all Hub G1–G4★ quests complete | star index |
| 3 | 284: completed an EX Special Permit quest | deviant monster code |
| 4 | 116: completed a G5 Special Permit quest | deviant monster code |
| 5 | 260 / 282 / 261 / 262 / 283: Prowler "all complete" | 0–4 |
| 6 | 124: HR increased | HR |
| 7 | 125: abandoned %d quests | count, every 10 (`S+0xc2`) |
| 8 / 24 | 134: completed in under 5 minutes | 8 if a code-10 highlight exists, else 24 |
| 9 / 25 | 135: completed with under a minute left | same rule |
| 10 | 126: defeated a wild monster | monster name ID |
| 11 | 127: played %d hours | hours, multiples of 100 |
| 13 / 15 | 128: hunted %s %d times | monster / count milestone; 15 for deviants. 12 and 14 are never emitted |
| 16 | 129: %d quests in total | multiples of 100 |
| 17 | 130: obtained %s | item |
| 18 | 131: fainted %d times in total | rounded down to 100 (`S+0xd4`) |
| 19–22 | 132: %s fainted %d times | party slot / faints: 4+ → 19, 3 → 20, 2 → 21, 1 → 22 |
| 23 | 133: completed without armor | — |
| 26 | 136–149, 259: weapon used %d times | weapon index / count rounded to 100 |
| 27 / 28 | 150 / 151: largest / smallest size | monster / size |
| 29–35 | 154–160: Prowler party feats (traps, Barrel Bomb L, horn, attacks, gathering, rarely fainted) | party slot of the Prowler |
| 36 | — | empty |

The writer counts codes ≤ 8 as 8 when it picks the slot to replace in slot 0
(`0x1636c8`).

**Event quests.** For kinds 7–9 the card screen prints only the stored name at `+0x08`
(`0x5a2194`); the u16 at `+0x06` is never shown, so an event quest displays correctly
whatever it holds. Who writes `+0x06` (quest object `+0x2FE`) was not found; no store
to that offset exists in the binary besides the constructor. UNRESOLVED.

Record stride is `0xA0` bytes; ten records end exactly where the
[award field](09-awards.md) begins (`0x254DB1`). **DERIVED** from the values: the
dates match the days the quests were played. Earlier revisions of this document put
the record start at the ID (`0x2546D7` + 6 + n × `0xA0`), which splits each record in
two.

This log is the most useful entry point for anyone attacking the quest system: it
pairs numeric IDs with human-readable names in plaintext, so a partial quest ID table
can be harvested from a well-played save by walking the records with no prior
knowledge.

### Quest IDs observed

| ID | Quest |
|---|---|
| `0xA1C4` (41412) | Rustrazor Ceanataur G2 |
| `0xA228`–`0xA22C` (41512–41516) | Soulseer Mizutsune G2 … EX |
| `0xA2F1`–`0xA2F4` (41713–41716) | Elderfrost Gammoth G3 … EX |

These follow the Special Permit scheme above: `40000 + 100 × deviant + level`,
with the deviant numbered from 1.

## Counters

| Offset | Type | Behaviour |
|---|---|---|
| `0x192AEA` | u32 | Completed quests, Harvest Tours and Training excluded (`S+0x4418`) |
| `0x25476D` | u32 | Guild Card copy of the play time in seconds |

`0x192AEA` advanced by one over each of five quests between the snapshots (382 → 387)
and stayed at 387 over the Harvest Tour, which did get a
[history record](#quest-history-log--0x254771) and a weapon usage count.

**CONFIRMED** (code + save timeline: +1 at each of five counted quests, unchanged over a
Harvest Tour, talks and tool writes). `0x526f70` adds 1 (saturating) when a quest ends completed (sQuest `+0x50`
= 2; callers `0x39fef0`, `0x3a0810`, `0x3a94e4`). It skips an abandoned quest (6) and a
quest whose file has the skip bit set (`0x3a340c`, [below](#what-all-quests-completed-takes)):
all Harvest Tours and Training quests. The Courier reads it against the value at the
last talk (`base + 0x5E52`) for his quest-count gifts
([11](11-save-map.md#counters-and-other-fields)).

Earlier revisions listed a second u16 counter at `0x25476E`. That was a misreading:
the bytes are the middle of the u32 at `0x25476D`, which follows the play time at
`base + 0x20` (a second copy sits at `base + 0x2248B`) and is refreshed when a quest
ends, so its second byte happened to step by one between the first captures.
**CONFIRMED** from 36 saves: the value equals the play time in every save taken
right after a quest (175402 against 175408, 172975 against 172978), lags behind it
otherwise, and went on from an editor-written play time in the next game session.

## Open questions

- **The 20 bytes before the bitmap are not quest state** (`base + 0x2C63`, fields
  `+0xd78` / `+0xd84` of the save object). Both are Palico maps, **DERIVED** from code:
  `+0xd78` is a 96-bit map (test `0x5245f8`) that `0x25ec74` fills from the 84 owned
  Palicoes, 12 slots each, when the Palico's level reaches the entry's requirement
  (byte `+0xa`); `+0xd84` is a 57-bit map (test `0x52462c`) filled by `0x262560`
  (entries without a requirement) and `0x2625cc` (entries whose requirement is a
  cleared quest, `0x3a3930`). `+0xd78` is the **Palico skills** map: `ot_skl_eng.gmd`
  has exactly 96 skills (name and description each), and the test's callers pass a
  Palico's skill byte (`+0x264` of the Palico object, `0xe9e64`). `+0xd84` is therefore
  the **support moves** map; its callers pass the Palico's 16 support move slots
  (`+0x18`, `0xe877c`). **DERIVED.** The 3 × 24 bytes before them (`base + 0x2C13`) are the Hunter Arts
  bitmap of [08](08-progression.md) and two copies that drive notices. The same
  serializer walk also lands on the Canteen ingredients of 08 (`base + 0x2F8F`), which
  cross-checks the field map.
- **PARTLY RESOLVED — quest history record.** The highlight codes are named
  ([above](#highlight-codes)). Who writes the u16 quest ID is UNRESOLVED; it cannot
  hold event IDs (≥ 1 000 000) but is never shown. No record of the analysed save is
  an event quest.

## A caution on bulk edits

Setting every bit is still a bad idea. Index 0, the duplicate slots 315–316, the
`DUMMY` event entries, and the space past index 1508 are not real quests. Key
quests and urgents also drive state stored elsewhere: HR and the star-level event
flags, which a bitmap edit does not touch. The numeric star levels at
`base + 0x2C4DA` are recomputed from the urgents' cleared bits only when a quest is
cleared, not on load, see [star levels](#star-levels--base--0x2c4da).
Set the bits for real quests from the CSV. Rank progression then still needs either
the key quests cleared in play or those fields edited as well.
[`tools/complete_quests.py`](../tools/complete_quests.py) does the bulk edit with
these exclusions, see below.

### What "all quests completed" takes

The completion mark itself is fully mapped: the cleared bit of every real row of
[`quest-index.csv`](../data/quest-index.csv). Single bits were written and confirmed
(607, 619, 11404); **a bulk write was never tested.** State that a real clear also
moves, and that a bitmap edit leaves behind:

| State | Where | Effect if left as is |
|---|---|---|
| Star levels | `base + 0x2C4DA` / `+0x2C4DC` | fixed by the game after the next real clear; can be written directly (CONFIRMED) |
| Star-level event flags | [event flags](#request-flags--base--0x2c56d) | set by the chiefs' and quest counter Gals' talk lines once the numeric level is there |
| HR and the HR limit | HR `base + 0x28`, HR points `base + 0x280B`, bit 20 of `base + 0x2F77` | Hub rules with `hr:N` stay locked |
| Request flags | `accept_flag` / `done_flag` of [`request-index.csv`](../data/request-index.csv) | a cleared request without its done flag leaves the NPC waiting for a report whose reward was never queued |
| Quest set bits | [`base + 0x3187`](#quest-sets--base--0x3187) | the chiefs' last requests, the last Village ★10 quests and the completion awards stay closed: the game writes a set bit only when it sees the last clear |
| Rotating quests | `base + 0x504B` | none for completion; only decides what is listed |
| Pending set notices | `base + 0x3197` | none: only "you cleared every ★N quest" notices on the result screen |
| Rewards, Hunter's Notes, the two counters, the history log, Arena records, Guild Card awards | various | keep their old values; nothing depends on them for listing quests. Without Arena records the rank sets 46, 47, 53, 78, 79 stay open |

[`tools/complete_quests.py`](../tools/complete_quests.py) `<save> …` plans the edit and
prints it; with `--write` it changes the given files in place. It writes

1. the cleared and seen bit of every real row (placeholder rows, unused event slots
   and the second occurrence of 1049 / 1050 are skipped; `--no-events` leaves the event quests);
2. for every villager request with a quest that is not accepted yet, the accepted flag
   and whatever else its offer block sets or clears (column `also` of
   [`request-offer.csv`](../data/request-offer.csv): the chiefs' second flag, the
   Captain's idle-talk flag 796). The **completed flag is left clear** on purpose: the
   NPC's report block ("completed flag clear, quest cleared") then plays in the game
   and hands over the reward;
3. the bit of every quest set that is complete afterwards, except the rank sets.

It does not touch HR or the star levels (it reports them if they are below the
maximum), and it lists the story flags that still hide a quest. Those are set by talk
blocks whose conditions the edit has just made true, so a conversation sets them.

**Bulk write, 2026-09-19 — written and checked in the game.** On the analysed save
(HR 999, Village ★10, Hub ★13) the tool changed 294 bytes in both slots: 980 cleared
bits (Village 239, Hub 444, Arena 16, Training 113, Event 168), their seen bits, 63
requests marked accepted (33 flag bytes, flag 796 cleared), 66 set bits (everything
except 46, 47, 53, 78, 79 and the memberless 49–52). Afterwards
`quest_unlock.py` reports 18 quests not listed, all behind seven flags that a
conversation sets: 1054 → 1055 → 1056 → 1057, 1072, 1073 (Wyventurer and the quest
counter Gals, Village ★10), 686 (Bherna Chief) and the completed flags 333 and 439 of
two requests. 94 request reports are waiting. Snapshot before the write:
`complete-0-pre-full-completion`.

**After one round of conversations** (snapshot `complete-1-after-talks`, saved with
the game still open, no quest played): all seven flags above are set, so
`quest_unlock.py` reports 0 quests locked; 87 of the 94 reports went through (their
completed flags are set), which confirms that an accepted flag written by an edit is
enough for the report block. The cleared, seen, failed and set maps and the pending
notices did not change. The game-side award map gained award 39 (with its notice
bit); the quest set awards 0–12 were not granted by talking alone. Seven reports did
not fire:

- Six are Hunter Art lessons (requests 126, 128, 133, 141, 143, 150). Their report
  block has a third condition, talk types 46–53, 141, 142: a lesson of this teacher
  is due. The check (`0x3f0c3c`) skips every art whose bit is already set in the
  [Hunter Arts map](08-progression.md) (`0x524040`), and on this save all 178 arts
  were unlocked by an earlier edit, so no lesson is ever due. On such a save an edit
  has to write the completed flag itself (1283, 1287, 1297, 1313, 1317, 1331; the
  last block also clears 1398, the teacher's "request running" marker that its idle
  lines test). `complete_quests.py --lessons` does that for every lesson request that
  is accepted and not completed. Written to this save after the Request Log showed
  the six as pending (5 bytes per slot); the check in the game is still open.
- Request 50 (Jumbo Sweetheart, Pokke): its offer needs flag 415, the completed flag
  of delivery request 49 (Pokke Gal), which is still open on this save. The tool set
  the accepted flag 416 regardless; whether the NPC is present at all without 415
  was not checked.

The quest lists were then read off in the game, level by level (Village ★1–★10, Hub
★1–G4, hunter and Prowler quests counted apart): all 21 counts equal the number of
rows the unlock rules and the [rotation](#rotating-quests--base--0x504b) predict, and every quest
shows as completed. The three event lists gave 103 hunter and 22 Prowler quests,
the 125 downloaded ones. The other 23 event rows (`Hunter Low-rank 2`, `@`,
`Quest Template`, …) are slots no released quest uses; they carry the note
`unused event slot` in `quest-index.csv` and the tool skips them now. The bulk write
above still set their cleared and seen bits; nothing in the game showed them, and a
later write cleared the 46 bits again (snapshot before: `complete-3-pre-lesson-flags`). The Arena, Training and Special Permit lists
show every quest as completed too, the 16 Arena quests without a record time included.
The Request Log lists the six lesson requests as pending.

**After the next quest** (snapshot `complete-2-after-quest`; Harvest Tour 201, Village):
the game-side [award map](09-awards.md#game-side-map--base--0x3157) gained 18 awards
with their notice bits: 0–6 and 10, the quest set awards, and 59, 74, 89, 103,
109–114. Of the first thirteen only 7 and 8 (a count over the monster list,
`0x3f3624` / `0x3f3824`), 9 (`0x3f1b1c`, not read) and 11 and 12 (the Arena rank sets
46 and 47, left clear by the tool) are missing. So the set bits written by the tool
are enough: the award check runs after any quest, not only after the last clear of a
set. In the game the only visible result was new Guild Card titles; the Guild Card
award field was already full on this save. The cleared, seen, failed and set maps and
the pending notices did not change, the per-NPC hold maps were wiped as usual, and
ten event flags in 1537–1592, which no talk record uses, were cleared.

The [rotation field](#rotating-quests--base--0x504b) did **not** change over this
quest, the first such case in the snapshots. In the quest flow step `0x38b904` the
re-roll `0x54b1c4` sits in a block that is skipped when `0x3a340c` is true, and an
abandoned quest (end state 6) takes `0x54b0d4` instead.

**DERIVED — it was the skip bit.** `0x3a340c` tests bit 7 of `+0x10c` of the quest's
`rQuestData` record (sQuest `+0xa4`). That is file data, not a runtime field: the
deserializer `0x37539c` stores the last u16 of each `questData_*.ext` file there
(`0x376ef0`). Bit 7 is set in exactly 171 files: all 58 Harvest Tours (23 Village, 35
Hub) and the 113 Training quests. So tours and Training never re-roll the rotation and
never advance the quest counter. The other bits of that u16 are UNRESOLVED.

