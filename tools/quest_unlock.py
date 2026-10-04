#!/usr/bin/env python3
"""Evaluate the game's quest unlock rules against a save file. Read-only.

The rules in ../data/quest-unlock.csv are a transcription of the game script
`script\\check_quest_unlocked` (see ../docs/05-quests.md). For every quest this
prints whether the board would list it, and for locked quests what is missing.
"rotated" = the rule holds but the quest is one of the 51 rotating quests and its
bit at base+0x504B is clear.

Usage:  quest_unlock.py [--slot 1|2|3] [path/to/system] [quest_id ...]
"""
import sys, csv, pathlib

HR       = 0x28            # u16
CLEARED   = 0x2C77          # quest bitmaps, index from quest-index.csv
SEEN      = 0x2D77
HUB_STAR  = 0x2C4DC         # u16, 1-13
FLAGS     = 0x2C56D         # event flag bitmap
ROTATION  = 0x504B          # u64, bit from rotating-quests.csv

DATA = pathlib.Path(__file__).resolve().parent.parent / "data"

def main():
    args = sys.argv[1:]
    slot = 1
    if "--slot" in args:
        i = args.index("--slot"); slot = int(args[i + 1]); del args[i:i + 2]
    path = args.pop(0) if args and not args[0].isdigit() else "system"
    want = {int(a) for a in args}
    buf = pathlib.Path(path).read_bytes()
    BASE = 0x24 + int.from_bytes(buf[0x34 + 4 * (slot - 1):0x38 + 4 * (slot - 1)], "little")

    index = {}
    for row in csv.DictReader(open(DATA / "quest-index.csv")):
        index.setdefault(int(row["quest_id"]), int(row["index"]))   # first match wins

    def bit(off, i):
        return (buf[BASE + off + (i >> 3)] >> (i & 7)) & 1
    cleared = lambda q: bit(CLEARED, index.get(q, 0))
    hr = int.from_bytes(buf[BASE + HR:BASE + HR + 2], "little")
    hub = int.from_bytes(buf[BASE + HUB_STAR:BASE + HUB_STAR + 2], "little")

    def missing(pred):
        """Return None if the predicate holds, else a description of what is missing."""
        kind, _, arg = pred.partition(":")
        if kind == "always":   return None
        if kind == "flag":     return None if bit(FLAGS, int(arg)) else f"event flag {arg}"
        if kind == "cleared":  return None if cleared(int(arg)) else f"clear {arg}"
        if kind == "hr":       return None if hr >= int(arg) else f"HR {arg}"
        if kind == "hub_star": return None if hub >= int(arg) else f"Hub star level {arg}"
        if kind == "all":
            todo = [q for q in arg.split("|") if not cleared(int(q))]
            return None if not todo else "clear all of " + " ".join(todo)
        if kind == "atleast":
            n, _, qs = arg.partition(":")
            qs = qs.split("|")
            have = sum(cleared(int(q)) for q in qs)
            return None if have >= int(n) else f"clear {int(n) - have} more of " + " ".join(qs)
        raise ValueError(pred)

    rot = {int(r["quest_id"]): int(r["bit"]) for r in csv.DictReader(open(DATA / "rotating-quests.csv"))}

    print(f"character {slot}: {buf[BASE:BASE + 32].split(b'\0')[0].decode(errors='replace')!r}, HR {hr}, Hub star level {hub}")
    locked = 0
    for row in csv.DictReader(open(DATA / "quest-unlock.csv")):
        q = int(row["quest_id"])
        if want and q not in want:
            continue
        if not row["rule"]:                      # event quest: listed once downloaded
            if want: print(f"{q:>8}  event     {row['name']}")
            continue
        # alternatives are tried in order; the first one whose predicates all hold unlocks
        needs = [[m for m in map(missing, alt.split(" AND ")) if m] for alt in row["rule"].split(" OR ")]
        ok = any(not n for n in needs)
        # rotating quests also need their bit; the game re-rolls it after each quest
        out = ok and q in rot and not bit(ROTATION, rot[q])
        if ok and not out and not want:
            continue
        locked += not ok
        state = "rotated " if out else "unlocked" if ok else "LOCKED  "
        seen = "seen" if bit(SEEN, index.get(q, 0)) else "    "
        why = "  <- rule holds, not in the current rotation" if out else "" if ok else "  <- " + " | or ".join("; ".join(n) for n in needs)
        print(f"{q:>8}  {state}  {seen}  {row['name']}{why}")
    if not want:
        print(f"{locked} quests locked")

if __name__ == "__main__":
    main()
