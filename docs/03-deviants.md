# 03 — Deviants

Deviants (*Nyanta* variants — Redhelm, Dreadqueen, Hellblade, …) have three separate
pieces of save state: a **permit count**, a **cleared-level bitmap**, and a
**seen-level bitmap**.

## Deviant index order

**CONFIRMED** by exact fingerprint. The permit array was located by searching for the
player's full 18-value permit vector, including a distinctive outlier, and matched at
exactly one offset in the file. The order is therefore fixed and unambiguous:

| # | Deviant | Levels | | # | Deviant | Levels |
|---|---|---|---|---|---|---|
| 0 | Redhelm Arzuros | 16 | | 9 | Thunderlord Zinogre | 16 |
| 1 | Snowbaron Lagombi | 16 | | 10 | Grimclaw Tigrex | 16 |
| 2 | Stonefist Hermitaur | 16 | | 11 | Hellblade Glavenus | 16 |
| 3 | Dreadqueen Rathian | 16 | | 12 | Nightcloak Malfestio | 6 |
| 4 | Drilltusk Tetsucabra | 16 | | 13 | Rustrazor Ceanataur | 6 |
| 5 | Silverwind Nargacuga | 16 | | 14 | Soulseer Mizutsune | 6 |
| 6 | Crystalbeard Uragaan | 16 | | 15 | Boltreaver Astalos | 6 |
| 7 | Deadeye Yian Garuga | 16 | | 16 | Elderfrost Gammoth | 6 |
| 8 | Dreadking Rathalos | 16 | | 17 | Bloodbath Diablos | 6 |

Deviants 0–11 originate in Generations and have **16 levels**: Lv1–Lv10, then G1–G5,
then EX. Deviants 12–17 are GU-exclusive and have only **6 levels**: G1–G5, then EX.
This asymmetry drives the bitmap layout below.

## Permit counts — `0x18F4D8`

18 consecutive u8, one per deviant, in the index order above. Caps at 99.

```
permits[i] = u8 at 0x18F4D8 + i
```

## Level bitmaps

Two bitmaps of identical layout:

| Bitmap | Base bit address | Meaning |
|---|---|---|
| Cleared | byte `0x18F989`, bit 3 | levels the player has completed |
| Seen | byte `0x18FA89`, bit 3 | level highlighted on the board, clears **NEW**. Earlier revisions read this as "unlocked"; see [05](05-quests.md#controlled-writes) |

The second is exactly `0x100` bytes after the first. Bits are **LSB-first** within
each byte: global bit `g` lives at byte `g >> 3`, mask `1 << (g & 7)`.

Note the base is **not** byte-aligned — it starts three bits into `0x18F989`. Any
implementation must address bits globally rather than walking bytes.

**CONFIRMED — these are not separate bitmaps.** They are indices 947–1174 of the
quest bitmaps in [05 — Quests](05-quests.md), one bit per Special Permit quest
(quest ID `40000 + 100 × (deviant + 1) + level`). The mixed-width layout is simply
the quest list containing 16 permit quests for each Generations deviant and 6 for
each GU deviant. The odd start bit is list index 947, counted from the quest bitmap
base at `0x18F913`.

### Block layout

Blocks are **mixed-width**, packed with no padding:

```
def block(i):
    if i < 12:  return (i * 16,             16)   # Gen deviants
    else:       return (192 + (i - 12) * 6,  6)   # GU deviants
```

Total: `12 × 16 + 6 × 6 = 228` bits per bitmap.

Within a block, bit `j` is level `j` in progression order, and the **final** bit
(`j == width - 1`) is EX. So for a Gen deviant bit 0 is Lv1 and bit 15 is EX; for a
GU deviant bit 0 is G1 and bit 5 is EX.

```
bit_index(deviant, level) = base + block(deviant).offset + level
```

A uniform 6-bit stride was tried first and produces impossible states — clears at G3
and above with G1 and G2 unset, which sequential progression forbids. That
contradiction is the tell that the layout is mixed-width. With the correct model
every set bit in the analysed save was accounted for exactly, with none left over.

### Reference implementation

```python
NAMES = ["Redhelm Arzuros","Snowbaron Lagombi","Stonefist Hermitaur",
         "Dreadqueen Rathian","Drilltusk Tetsucabra","Silverwind Nargacuga",
         "Crystalbeard Uragaan","Deadeye Yian Garuga","Dreadking Rathalos",
         "Thunderlord Zinogre","Grimclaw Tigrex","Hellblade Glavenus",
         "Nightcloak Malfestio","Rustrazor Ceanataur","Soulseer Mizutsune",
         "Boltreaver Astalos","Elderfrost Gammoth","Bloodbath Diablos"]

PERMITS  = 0x18F4D8
CLEARED  = 0x18F989 * 8 + 3          # global bit address
SEEN     = CLEARED + 0x100 * 8

def block(i):
    return (i * 16, 16) if i < 12 else (192 + (i - 12) * 6, 6)

def get_bit(buf, g):
    return (buf[g >> 3] >> (g & 7)) & 1

def set_bit(buf, g):
    buf[g >> 3] |= 1 << (g & 7)

def read_levels(buf, i):
    """Return the list of cleared level bits for deviant i."""
    off, width = block(i)
    return [get_bit(buf, CLEARED + off + j) for j in range(width)]

def clear_all_but_ex(buf):
    """Set every level except EX, for every deviant, in both bitmaps."""
    for base in (CLEARED, SEEN):
        for i in range(18):
            off, width = block(i)
            for j in range(width - 1):        # width-1 skips the EX bit
                set_bit(buf, base + off + j)
```

Use OR rather than assignment so existing EX clears are preserved.

## Unlock gating

Two gates, and they are independent. Only the first lives in these bitmaps.

**EX requires G5 cleared and HR 100.** Satisfied by setting the level bits.

**CONFIRMED — G-rank deviant levels additionally require hunting the base monster in
G-rank.** This gate is *not* stored in the deviant bitmaps. Setting every level bit
leaves affected deviants displaying "Lv10 cleared" with no G1 or higher quest
offered. Hunting the corresponding base monster once at G-rank immediately releases
them; this was verified by hunting G-rank Arzuros, after which Redhelm's EX quest
appeared without any further save edit.

**Where it is recorded: nowhere of its own.** The board asks the unlock script of
[05 — Quests § Board visibility](05-quests.md#board-visibility--scriptcheck_quest_unlocked)
for every level, and the script reads the ordinary cleared bitmap:

| Level | Rule |
|---|---|
| Lv1 (12 Gen deviants) | one Hub quest against the base monster cleared, or 10768 |
| Lv2–Lv10, G2–G5 | the previous level cleared |
| G1 (12 Gen deviants) | Lv10 cleared **and** one G-rank quest against the base monster |
| G1 (GU deviants) | one G-rank quest against the base monster; Bloodbath: event flag 1226 |
| EX | G5 cleared and HR ≥ 100 |

The G-rank quests that release G1:

| Deviant | Any one of |
|---|---|
| Redhelm | 11125, 11104 |
| Snowbaron | 11108 |
| Stonefist | 11113, 11111 |
| Dreadqueen | 11248, 11210, 11226, 11352, 11306 |
| Drilltusk | 11128, 11110 |
| Silverwind | 11216, 11234 |
| Crystalbeard | 11303 |
| Deadeye | 11302, 11347 |
| Dreadking | 11352, 11306, 11357, 11359 |
| Thunderlord | 11308, 11355 |
| Grimclaw | 11405, 11462 |
| Hellblade | 11467 |
| Nightcloak | 11206, 11235, 11348 |
| Rustrazor | 11214, 11250 |
| Soulseer | 11310, 11356 |
| Boltreaver | 11311, 11358 |
| Elderfrost | 11312, 11354 |
| Bloodbath | event flag 1226 |

So an editor releases the gate by setting the **cleared bit of one listed quest**
(index from [`data/quest-index.csv`](../data/quest-index.csv)). This follows from the
decoded script and matches the Arzuros observation above, but the write itself has
**not been tested** yet. That EX appeared only after the hunt, although its own rule
does not mention the base monster, suggests the permit list stops at the first locked
level. **DERIVED.**

## Open questions

- **Not yet tested on a deviant — releasing the G-rank gate by a bitmap write.** No
  gate is locked in the analysed save. The predicate kinds the gate uses (`cleared`,
  `atleast`) are confirmed by a controlled write on other quests, see
  [05](05-quests.md#evidence).
- **RESOLVED — deviant tally indices.** The executable maps each tally index to a
  monster code (table `0x1597ea4`, 137 × (u32 code, u32 index), read by `0x50bdc`). A
  code is the base monster in the low byte and a variant in the high byte; the 18
  deviants are exactly the variant-4 codes, each next to its base monster's code:

  | Tally index | Deviant | Code | | Tally index | Deviant | Code |
  |---|---|---|---|---|---|---|
  | 3 | Dreadqueen Rathian | `0x401` | | 51 | Snowbaron Lagombi | `0x43D` |
  | 6 | Dreadking Rathalos | `0x402` | | 56 | Drilltusk Tetsucabra | `0x442` |
  | 17 | Deadeye Yian Garuga | `0x412` | | 66 | Hellblade Glavenus | `0x450` |
  | 19 | Stonefist Hermitaur | `0x413` | | 116 | Bloodbath Diablos | `0x407` |
  | 29 | Grimclaw Tigrex | `0x420` | | 121 | Rustrazor Ceanataur | `0x414` |
  | 33 | Silverwind Nargacuga | `0x425` | | 129 | Nightcloak Malfestio | `0x44F` |
  | 38 | Crystalbeard Uragaan | `0x42D` | | 130 | Boltreaver Astalos | `0x451` |
  | 46 | Thunderlord Zinogre | `0x439` | | 131 | Soulseer Mizutsune | `0x452` |
  | 49 | Redhelm Arzuros | `0x43C` | | 132 | Elderfrost Gammoth | `0x453` |

  The names follow from the base monster (the low byte), whose tally index is known;
  each base monster has one deviant. Earlier revisions put Soulseer at 122, which
  [02](02-monster-records.md) had already corrected to 131. Variant 5 is the four
  special forms: Furious Rajang (23), Savage Deviljho (36), Raging Brachydios (126),
  Chaotic Gore Magala (128). The history records' highlight values `0x402`, `0x412`,
  `0x42d`, `0x425` ([05](05-quests.md#quest-history-log--0x254771)) are these codes:
  Dreadking, Deadeye, Crystalbeard and Silverwind, matching the four EX quests 40916,
  40816, 40716, 40616 they belong to. **DERIVED.**
