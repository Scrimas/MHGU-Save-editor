# 04 — Weapon usage

The Guild Card's Weapon Usage graph is backed by **three contiguous arrays of 15 u16**,
one per quest venue. The "All" tab is computed at display time, not stored.

| Array | Offset | Elements |
|---|---|---|
| Village | `0x254713` | 15 × u16 |
| Hub | `0x254731` | 15 × u16 |
| Arena | `0x25474F` | 15 × u16 |

```
usage[venue][w] = u16 at venue_base + 2*w
```

Arrays are `0x1E` (30) bytes apart — exactly 15 u16 — and directly adjacent. Note the
bases are **odd addresses**; the arrays are not 2-byte aligned, so a naive aligned
scan will miss them entirely.

## Weapon index order

**CONFIRMED.** Storage uses the classic internal Monster Hunter weapon order, which is
**not** the order the bars are drawn in on screen:

| # | Weapon | | # | Weapon |
|---|---|---|---|---|
| 0 | Great Sword | | 8 | Gunlance |
| 1 | Sword and Shield | | 9 | Bow |
| 2 | Hammer | | 10 | Dual Blades |
| 3 | Lance | | 11 | Hunting Horn |
| 4 | Heavy Bowgun | | 12 | Insect Glaive |
| 5 | Light Bowgun | | 13 | **Charge Blade** |
| 6 | Long Sword | | 14 | Prowler |
| 7 | Switch Axe | | | |

On-screen order is Great Sword, Long Sword, Sword and Shield, Dual Blades, Hammer,
Hunting Horn, Lance, Gunlance, Switch Axe, Charge Blade, Insect Glaive, Light Bowgun,
Heavy Bowgun, Bow, Prowler. Confusing the two is the most likely way to write a
count into the wrong weapon.

### Charge Blade, worked example

```
Village  0x254713 + 2*13 = 0x25472D
Hub      0x254731 + 2*13 = 0x25474B
Arena    0x25474F + 2*13 = 0x254769
```

## Verification

The layout was validated end to end. All 15 weapons were read from the three venue
arrays, summed, and compared against the "All" tab as rendered by the game — **15 of
15 matched exactly**, including four weapons with non-zero counts split across
venues in different proportions.

Independently, `0x25474B` (Hub, Charge Blade) was observed incrementing by exactly 1
after each of two separate quests completed with a Charge Blade, in two unrelated
diffs captured days apart.

## Semantics

Counts are **quests completed** with that weapon equipped, not hunts and not monsters
killed. The sum across all weapons and venues therefore tracks completed quests, not
the monster tallies in [02 — Monster records](02-monster-records.md). Editing hunt
tallies does not and should not change these.

The game derives "most used weapon" from the totals; there is no separate stored
field for it. To change the displayed main weapon, make that weapon's total the
largest.

## Open questions

- **RESOLVED — venue array count: three.** The arrays are card fields `+0x8BA`
  (3 × 15 × u16) of the own Guild Card at `base + 0xC71BD`
  ([11](11-save-map.md#card-layout-6328-b)), and the next card field, `+0x914`, is the
  play time (`0x25476D`, see [05 — Quests](05-quests.md#counters)). There is no room
  for a fourth venue.
- **RESOLVED — Prowler is index 14.** `0x166aac` sums index 14 over the three venues
  (card `+0x8D6`, `+0x8F4`, `+0x912`), and talk condition 29 compares that sum
  (sub-tests 78 / 79 / 80: below 11, 11–30, above 30;
  [10](10-npc-talk.md#conditions)). Its only user, the Palico NPC 991, then says
  "how far I have to go as a Palico", "even Palicoes can be a real help" and "it's
  okay to let us Prowlers handle large meownsters". **DERIVED** (code + dialogue); the
  value itself was zero throughout.
