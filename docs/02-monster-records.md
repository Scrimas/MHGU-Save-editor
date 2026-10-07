# 02 — Monster records

Three parallel arrays of 137 entries, all indexed by the same internal **monster
index**, valid range **1–137**.

| Structure | Base | First entry | Element | Applies to |
|---|---|---|---|---|
| Hunt tally | `0x192B40` | `0x192B42` | u16 | all monsters |
| Capture count | `0x192C52` | `0x192C54` | u16 | all monsters |
| Size record | `0x192D62` | `0x192D66` | (u16 min%, u16 max%), stride 4 | large monsters only |

```
tally[i]    = u16 at 0x192B40 + 2*i        i = 1..137
capture[i]  = u16 at 0x192C52 + 2*i
size_min[i] = u16 at 0x192D62 + 4*i
size_max[i] = u16 at 0x192D62 + 4*i + 2
```

The base addresses point one element *before* each array: **index 0 is not a slot.**
`0x192B40` belongs to whatever precedes the tallies, and `0x192D62`–`0x192D65` are
`capture[136]` and `capture[137]`, not a size record. Likewise "index 138" of the
tally array is `capture[1]`. The arrays abut with no gaps: `0x192B42 + 274 = 0x192C54`,
`0x192C54 + 274 = 0x192D66`.

**DERIVED** from the Switch editor
([MHXXSwitchSaveEditor](https://github.com/Dawnshifter/MHXXSwitchSaveEditor)),
whose `Constants.cs` sizes the arrays at 274 / 274 / 548 bytes and reads them
0-based from the first entry, so **editor index = index − 1**. Both of this document's
controlled diffs (indices 48 and 121) agree with that alignment.

## Index space

| Range | Contents | Size records? |
|---|---|---|
| 1–71 | Large monsters | yes, except a few with no size variation |
| 72–104 | Small monsters | **no** |
| 105 | Moofah | no |
| 106–112 | Unused: the game's name table calls them `dummy1`–`dummy7` | — |
| 113–135 | Additional large monsters, incl. GU-exclusive deviants; 134 is an empty slot | yes, with exceptions |
| 136–137 | Great Thunderbug, Conga | no |

The small-vs-large split is **CONFIRMED** by cross-referencing which indices carry
size records against which carry only tallies. Names for 105 and 113–137 come from
the game's monster name table, see [below](#the-games-name-table).

The small-monster block at 72–104 is the structural detail that defeats naive
alignment attempts. Small monsters have hunt counts but **no** size record, so any
code that walks both arrays in lockstep desynchronises at index 72 and every
subsequent mapping is wrong. This cost considerable effort to discover.

A handful of large monsters also lack size records — those with no size variation in
game, such as Nakarkos. Absence of a size record is therefore not proof of a small
monster.

## Hunt tallies — `0x192B40`

u16, one per monster, counting monsters **slain**. Captures are counted separately
(next section); the in-game *Hunted (Capts)* line shows `tally + capture` as the
first number.

**CONFIRMED — deviant hunts do not roll up into the base monster.** Killing two
Rustrazor Ceanataur incremented index 121 (Rustrazor) from 1 to 3 while leaving
index 20 (Shogun Ceanataur) at 2 and index 90 (small Ceanataur) at 18. A displayed
base count and a displayed deviant count are independent totals; the family total is
their sum.

## Size records — `0x192D62`

Two u16 per large monster, the smallest and largest specimen seen, as a percentage
of the base size. Crown thresholds are derived from these by the game.

**CONFIRMED** twice by controlled hunts:

| Hunt | Offset touched | Change | Index |
|---|---|---|---|
| 2 × G-rank Arzuros | `0x192E22` / `0x192E24` | min 100→91, max 113→120 | 48 |
| Deviant Ceanataur | `0x192F46` / `0x192F48` | min 100→97, max 106→113 | 121 |

Both land exactly on `0x192D62 + 4*index`, and both indices independently agree with
the tally array — the two structures cross-validate each other.

`(0, 0)` means never hunted. To mark a monster hunted, a non-zero tally **and** a
non-zero size record are both needed for large monsters; small monsters need only
the tally.

Writing min/max within roughly 90–115% marks a monster as hunted without fabricating
gold or silver crown records. **Write indices 1–137 only.** Writing "index 0" puts
the pair into `capture[136..137]` — the analysed save carried exactly this artefact,
Great Thunderbug and Conga reading 90 and 115 captures against 23 and 28 hunts.

### Sizes the game can roll

A quest names each large monster with a size % and a variation table; the monster
spawns at that size times a scale rate drawn from the table. Taken over every quest of
the game, this bounds what a size record can hold:
[`data/quest-sizes.csv`](../data/quest-sizes.csv), written by
[`tools/quest_sizes.py`](../tools/quest_sizes.py) (file layouts in its header).

- Quest files (`questData_NNNNNNN.ext`): 5 boss entries of 13 B at `+0x64`, monster
  code `+0`, size % `+9` (u16), variation table `+0xB`. Monster code to index: the
  executable's table `0x1597ea4`.
- Variation tables (`em_size_yure_data`): 52 tables of (scale rate, chance in 100).
- Most monsters range 88–125 %; a few reach 130 % (Rathalos in *Paint It Gold*,
  Deviljho and Savage Deviljho), the fixed-size ones stay at 100.

**DERIVED**: every size record of the analysed save that no editor wrote falls inside
its range, and every crown threshold too. Out of range were Bulldrome 169 % and
Basarios 160 %, which no quest gives, and the 90 / 115 pairs written by an editor on
monsters whose quests give 95–110 % or a fixed 100.

The size table `em_size_scale_data` holds per monster `mQuestSizeMin` / `mQuestSizeMax`,
equal to the quest bounds except for Rathalos (125 against 130), Kecha Wacha (115
against 123) and Gravios (88 against 79 in *Gravios Backbreaker*). Whether the game clamps to it
is not checked; the editor keeps the wider quest bounds, so it never flags a size the
game can make.

## Capture counts — `0x192C52`

u16, one per monster: the `M` in the Monster List's *Hunted (Capts)* `N(M)`.

**CONFIRMED** — located via the editor's `MonsterCaptures` offset, then matched
against the in-game Monster List, where the displayed `N` is **tally + capture**:

| # | Monster | Tally | Capture | Displayed | |
|---|---|---|---|---|---|
| 1 | Rathian | 10 | 2 | `12(2)` | ✓ |
| 63 | Seregios | 11 | 4 | `15(4)` | ✓ |
| 122 | Congalala | 3 | 0 | `3(0)` | ✓ |
| 131 | Soulseer Mizutsune | 2 | 1 | `3(1)` | ✓ |

Other non-zero captures in the analysed save: Nargacuga (32), Lagombi (50),
Glavenus (65), Great Maccao (71), and 115, 116, 121, 129–132. The alternative
alignment (repo index = editor index) fails at once: it assigns index 0, tally 0,
two captures.

A controlled capture diff confirms that a capture touches **only** the capture array.
Capturing one Nargacuga (index 32):

| Array | Offset | Change |
|---|---|---|
| capture[32] | `0x192C92` | 1 → 2 |
| tally[32] | `0x192B80` | unchanged |
| size[32] | `0x192DE2` | unchanged (specimen within the recorded range) |

The only other monster-array change in that diff was tally[78] (Felyne) 23 → 25,
from Felynes killed on the same quest.

## Monster index table

Recovered by writing self-identifying markers — each slot `i` set to `100 + i`, then
read back off the in-game Monster List, so the game itself reports its own indices.
See [methodology](06-methodology.md).

Confidence:
- **[C]** CONFIRMED — controlled write or hunt diff, two independent lines
- **[V]** DERIVED, count-validated — marker readout, and the stored value matches an
  independently reported hunt count
- **[E]** DERIVED from the editor's `MonsterHuntNames` list (editor index + 1) and the
  game's own name table (entry index − 1, [below](#the-games-name-table)), which agree
- unmarked — DERIVED from the marker readout alone

Over 1–104 the editor list agrees with every marker-derived entry, and its names
fill exactly the gaps the marker pass left.

### Large monsters, 1–71

| # | Monster | | # | Monster |
|---|---|---|---|---|
| — | — | | 36 | Savage Deviljho **[E]** |
| 1 | Rathian **[E]** | | 37 | Uragaan |
| 2 | Gold Rathian | | 38 | Crystalbeard Uragaan |
| 3 | Dreadqueen Rathian **[V]** | | 39 | Lagiacrus |
| 4 | Rathalos | | 40 | Royal Ludroth **[V]** |
| 5 | Silver Rathalos | | 41 | Agnaktor |
| 6 | Dreadking Rathalos | | 42 | Alatreon |
| 7 | Khezu | | 43 | Duramboros |
| 8 | Yian Kut-Ku **[V]** | | 44 | Nibelsnarf |
| 9 | Gypceros **[V]** | | 45 | Zinogre |
| 10 | Plesioth | | 46 | Thunderlord Zinogre |
| 11 | Kirin | | 47 | Amatsu |
| 12 | Velocidrome **[V]** | | 48 | **Arzuros [C]** |
| 13 | Gendrome **[V]** | | 49 | Redhelm Arzuros |
| 14 | Iodrome **[V]** | | 50 | Lagombi **[E]** |
| 15 | Cephadrome **[V]** | | 51 | Snowbaron Lagombi **[V]** |
| 16 | Yian Garuga | | 52 | Volvidon **[V]** |
| 17 | Deadeye Yian Garuga | | 53 | Brachydios |
| 18 | Daimyo Hermitaur **[V]** | | 54 | Kecha Wacha **[V]** |
| 19 | Stonefist Hermitaur | | 55 | Tetsucabra **[V]** |
| 20 | Shogun Ceanataur | | 56 | Drilltusk Tetsucabra |
| 21 | Blangonga | | 57 | Zamtrios |
| 22 | Rajang **[E]** | | 58 | Najarala |
| 23 | Furious Rajang **[E]** | | 59 | Seltas Queen |
| 24 | Kushala Daora | | 60 | Gore Magala |
| 25 | Chameleos | | 61 | Shagaru Magala |
| 26 | Teostra | | 62 | Seltas **[V]** |
| 27 | Bulldrome **[V]** | | 63 | Seregios **[E]** |
| 28 | Tigrex | | 64 | Malfestio **[E]** |
| 29 | Grimclaw Tigrex | | 65 | Glavenus **[E]** |
| 30 | Akantor | | 66 | Hellblade Glavenus **[V]** |
| 31 | Lavasioth | | 67 | Astalos |
| 32 | Nargacuga **[E]** | | 68 | Mizutsune **[V]** |
| 33 | Silverwind Nargacuga **[V]** | | 69 | Gammoth |
| 34 | Ukanlos | | 70 | Nakarkos |
| 35 | Deviljho **[E]** | | 71 | Great Maccao **[E]** |

### Small monsters, 72–104

| # | Monster | | # | Monster | | # | Monster |
|---|---|---|---|---|---|---|---|
| 72 | Aptonoth | | 83 | Cephalos | | 94 | Altaroth |
| 73 | Apceros | | 84 | Bullfango | | 95 | Jaggi |
| 74 | Kelbi | | 85 | Popo | | 96 | Jaggia |
| 75 | Mosswine | | 86 | Giaprey | | 97 | Ludroth |
| 76 | Hornetaur | | 87 | Anteka | | 98 | Uroktor |
| 77 | Vespoid | | 88 | Remobra | | 99 | Slagtoth |
| 78 | Felyne | | 89 | Hermitaur | | 100 | Gargwa |
| 79 | Melynx | | 90 | Ceanataur | | 101 | Zamite |
| 80 | Velociprey | | 91 | Blango | | 102 | Konchu |
| 81 | Genprey | | 92 | Rhenoplos | | 103 | Maccao |
| 82 | Ioprey | | 93 | Bnahabra | | 104 | Larinoth |

### Moofah and unused slots, 105–112

| # | Monster |
|---|---|
| 105 | Moofah **[E]** |
| 106–112 | unused — `dummy1`–`dummy7` in the game's name table, *Unknown* in the editor |

### GU block, 113–137

| # | Monster | | # | Monster |
|---|---|---|---|---|
| 113 | Basarios **[E]** | | 126 | Raging Brachydios **[E]** |
| 114 | Gravios **[E]** | | 127 | Nerscylla **[E]** |
| 115 | Diablos **[E]** | | 128 | Chaotic Gore Magala **[E]** |
| 116 | Bloodbath Diablos **[E]** | | 129 | Nightcloak Malfestio **[E]** |
| 117 | Lao-Shan Lung **[E]** | | 130 | Boltreaver Astalos **[E]** |
| 118 | Fatalis **[E]** | | 131 | **Soulseer Mizutsune [C]** |
| 119 | Crimson Fatalis **[E]** | | 132 | Elderfrost Gammoth **[E]** |
| 120 | White Fatalis **[E]** | | 133 | Valstrax **[E]** |
| 121 | **Rustrazor Ceanataur [C]** | | 134 | *empty slot* (code `0x57`, empty name in the game's table) |
| 122 | **Congalala [C]** | | 135 | Ahtal-Ka **[E]** |
| 123 | Giadrome **[E]** | | 136 | Great Thunderbug **[E]** (small) |
| 124 | Barioth **[E]** | | 137 | Conga **[E]** (small) |
| 125 | Barroth **[E]** | | | |

Index 121 is confirmed twice over — the tally diff and the size-record diff from the
same controlled hunt both point at it; the editor agrees (its *Shredclaw Ceanataur*
is Rustrazor's Japanese-derived name). Editor names above are translated to their
western GU names.

**Resolved conflict at 122 / 131.** The marker readout put Soulseer Mizutsune at
122; the editor puts Congalala there and Soulseer (*Divinesight Mizutsune*) at 131.
The Monster List settles it for the editor: Congalala `3(0)` = index 122 (tally 3,
capture 0) and Soulseer `3(1)` = index 131 (tally 2, capture 1). The marker entry
was a transcription slip whose "count validation" matched by coincidence (both show
3). The [V] tag is only as good as the counts it was checked against: those were
displayed counts, which equal the stored tally only when captures are zero.

## Reliability of the table

The mapping reproduces **16 of 16** independently reported hunt counts, spread across
large monsters, small monsters and a GU-block deviant. That is far beyond
coincidence and the table can be relied on in bulk.

Individual entries are weaker than the aggregate. The readout was transcribed by hand
from 15 screens, and seven entries came back duplicated. Five were adjacent rows on
the same screen sharing one value — the signature of a transcription slip, not a
structural feature. Where the stored value could be checked against a known hunt
count the ambiguity resolved cleanly, which is what the **[V]** tag records; those
are the entries listed above. Their partners were consequently unplaced, which is
why several very common monsters — **Rathian, Nargacuga, Lagombi, Glavenus,
Seregios, Great Maccao, Malfestio** — were missing from the marker table. The
editor list later filled every one of those gaps (the **[E]** entries).

**Anyone extending this work should redo the marker pass and transcribe it
mechanically** — screenshot OCR, or reading fewer entries per pass. The technique is
sound; only the hand transcription was lossy.

## Open questions

- **RESOLVED — index 134: an empty slot.** The game's name table has an empty string
  for it in every language, and the index-to-monster-code table (`0x1597ea4`, see
  [03](03-deviants.md#open-questions)) gives it code `0x57`, between Valstrax (`0x56`,
  index 133) and Ahtal-Ka (`0x58`, index 135): a code reserved for a monster that is
  not in the game. **DERIVED.**
- **DERIVED — [E] names, now from the game.** See the next section. The code table
  agrees as well wherever a code can be checked: Fatalis, Crimson and White Fatalis
  are `0x0D`, `0x10D`, `0x20D` (118–120), every deviant is the variant-4 code of its
  base monster ([03](03-deviants.md#open-questions)). Indices 106–112 are codes
  `0x1064`–`0x106A`. A marker pass over 105–137 is no longer needed to name them; it
  would only add the in-game readout as a second line.

### The game's name table

`GUI/06_msg/monsterName_eng.gmd` (romfs) holds 268 names. Its first 137 entries are the
tally order: **entry i − 1 is index i**, with no gaps. Checked against every index of
this document: all names agree (the four controlled rows 48, 121, 122, 131, every
marker-derived row and every editor row), except for a typo here (92 was spelled
*Thenoplos*). Variants share the base name (22/23 *Rajang*, 118–120 *Fatalis*,
126 *Brachydios*, 128 *Gore Magala*); the variant is in the code table. Entries
105–111 are `dummy1`–`dummy7` (indices 106–112) and entry 133 (index 134) is empty.
Entries 137–267 repeat the large monsters for another list. **DERIVED.**
- **RESOLVED — captures vs kills.** The `M` of `N(M)` lives in the capture array at
  `0x192C52`, directly after the tallies; `N` = tally + capture, and a capture
  increments only the capture array.
