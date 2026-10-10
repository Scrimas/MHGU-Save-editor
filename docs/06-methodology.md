# 06 — Methodology

How these structures were found, including the approaches that failed. The techniques
transfer to any plaintext save format; the failures are worth as much as the successes.

## Preconditions

Everything here depends on two properties established first, before any structural
work: the file is **plaintext**, and it has **no content checksum**. Establish both
before anything else — they decide whether the whole approach is viable.

- **Plaintext?** Measure Shannon entropy. ~3 bits/byte means plaintext; ~8 means
  encrypted or compressed, and the work becomes a different, harder problem.
- **Checksummed?** Compare the save against its in-game backup copy immediately after
  a normal save. If two files with identical body content differ only in a header
  field, no checksum covers the body.

## The controlled diff

The core loop, and the source of almost every confirmed fact here:

1. Close the game completely. Snapshot the save file.
2. Perform **exactly one** well-understood in-game action.
3. Save, close completely, snapshot again.
4. Diff.

Discipline in step 2 is what makes step 4 readable. Useful constraints, all of which
were applied:

- Hunt one monster type, in one quest.
- Collect nothing. Gather nothing.
- Sell every reward immediately, so inventory and money churn is minimised rather
  than left to smear across the diff.

A clean single-action diff is a handful of byte ranges. A sloppy one is thousands,
and is close to useless.

```bash
# snapshot the live save
snap() {
  newest=$(ls -t "$SAVE"/{0,1}/system | head -1)
  cp "$newest" "$1.bin"
  md5sum "$1.bin"
}
```

Label snapshots sequentially (`A`, `B`, `C`, …) and never delete them. Several
findings here came from re-diffing pairs captured days earlier for other reasons —
the Charge Blade offset was confirmed by an increment visible in two unrelated
captures.

## Filtering noise

Even a clean diff carries churn: RNG seeds, timestamps, play counters, uninitialised
padding. Two filters do most of the work.

**Cross-session intersection.** Take two diffs of the *same* action. Structures
relevant to that action change in both; noise rarely does.

**Expected-delta matching.** Search for the shape of the change rather than its
location. Hunting two monsters means looking for `+2`, not for "something that
changed". This is how the size records were found: a diff restricted to values that
moved by a plausible size delta pointed straight at `0x192D62`.

## Known-plaintext fingerprinting

Where the player can read a vector of values off a UI screen, search the file for
that exact vector. This is the highest-yield technique available and it is nearly
free.

It found the deviant permit array in a single pass — an 18-value vector, containing
one distinctive outlier, matched at exactly one offset in 5 MB.

It also found the weapon usage arrays, but only after a correction worth
internalising: **the on-screen order is not necessarily the storage order.** A
positional search for the weapon vector failed outright. Re-running it as a
*multiset* match — same values, any order — found all three arrays immediately, and
the permutation between screen order and storage order then fell out by inspection.
When a positional search fails, try an unordered one before concluding the structure
is absent.

## Self-identifying marker writes

The strongest technique used, and the one that broke the monster index problem open
after every analytical approach had failed.

Rather than deducing which array slot corresponds to which entity, **write each slot's
own index into it**, then read the values back off the in-game UI. The game reports
its own mapping.

```python
BASE, N = 0x192B40, 105
for i in range(N):
    buf[BASE + 2*i : BASE + 2*i + 2] = (100 + i).to_bytes(2, 'little')
# in game, a monster displaying 148 is index 48
```

Choose the marker base so markers cannot be mistaken for real data — offset well
above any plausible real value, and use a distinct range (`1000 + i`) for a second
pass over a different index range.

Two caveats, both encountered:

- **Slots outside the written range keep their real values.** That is a feature: it
  reveals the array's true extent. Entries showing non-marker values are outside the
  marked range, which is itself a finding.
- **Transcribing the readout by hand is the weak link.** Fifteen screens read
  manually produced seven duplicated entries. The technique was sound; the
  transcription was not. Prefer OCR, or fewer entries per pass.

Always snapshot immediately before a marker write. The markers destroy real data, and
the snapshot is what restores it afterwards.

## Reading the game itself

Some layouts have no arithmetic relation to anything visible in the save. The quest
bitmap is the clearest case: its index is a position in a game data table, so diffing
alone would need one controlled clear per quest. When a structure resists diffing,
read the game's own code and data instead:

1. Dump your own copy of the game and its update, and apply the update's patch
   layer. Do not trust a tool's patched output without checking that files the
   update did not change match the base game byte for byte.
2. Find the code that reads the structure. Save-field names and resource type names
   survive in the executable as strings (`rQuestGroup`, `cQuestLink`, …), which
   narrows the search.
3. MT Framework archives (`.arc`) identify each entry by a type hash,
   `~crc32(name) & 0x7FFFFFFF`. This locates the resource behind a type name, and a
   brute scan of every archive for two known IDs finds it too.
4. Validate the recovered table against the save exactly as you would a diffed
   structure. For quests: the controlled-diff anchor, every history-log quest, and
   the independently derived deviant block all had to land on set bits.

## Failed approaches

Recorded because each cost real time and each is an easy trap to re-enter.

**Assuming ID equals index.** The cleared-quest bitmap was assumed to be indexed by
quest ID. Tested against a quest known to be cleared, every linear mapping put it on
a zero bit. Compact internal indices, dense over entities that exist, are the norm in
this format — the deviant, monster and quest structures all use them.

**Assuming uniform record width.** The deviant level bitmap was first decoded at a
uniform 6-bit stride. It produced states that sequential progression makes impossible
— G3 cleared with G1 unset. *An impossible decode is evidence about the layout, not
noise.* The contradiction is what pointed at a mixed-width structure, and the correct
model accounted for every set bit with none left over.

**Assuming parallel arrays stay in step.** The monster tally and size arrays share an
index, so walking both together seems safe. It is not: small monsters occupy tally
slots but have no size records. Everything after index 72 desynchronises. Confirm
that two arrays cover the same entity set before pairing them.

**Trusting a single UI reading.** One reported count never reconciled with the save
and consumed effort on the assumption the model was wrong. The model was right. Weigh
aggregate agreement — 16 of 16 counts matching — over any individual data point.

## Validation

Before trusting a mapping, make it predict something it was not derived from.

Every structure here was checked by predicting a value, then comparing against a
figure read independently off the UI. The monster index table reproduces 16 of 16
independently reported hunt counts; the weapon arrays reproduce 15 of 15 totals. Two
structures that cross-validate each other — the tally and size arrays independently
agreeing that index 121 is one specific deviant — is stronger evidence than either
alone.

Where a prediction failed, it is recorded as UNRESOLVED rather than smoothed over.

## Safety

- Back up the entire save directory before the first write, and keep it.
- Write both commit slots identically.
- Verify the emulator is not running before every write, programmatically, not by
  memory.
- Prefer OR over assignment when setting flags, to preserve state you have not
  modelled.
- Re-read and diff after writing, rather than assuming the write did what was
  intended.
