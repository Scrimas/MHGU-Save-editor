#!/usr/bin/env python3
"""Mark every quest of an MHGU save as cleared. Dry run unless --write is given.

What a real clear leaves behind is written too (see ../docs/05-quests.md, "What all
quests completed takes"):

  1. cleared + seen bit of every real row of ../data/quest-index.csv
     (placeholder rows, unused event slots and the second occurrence of a repeated
     ID are skipped);
  2. for every villager request with a quest that is not accepted yet: its accepted
     flag and the other flags its offer block sets or clears (column `also` of
     ../data/request-offer.csv). The completed flag is NOT written: the NPC's report
     line stays available and hands over the reward in the game. With --lessons the
     six Hunter Art lesson requests are the exception: their report line only comes
     up while the teacher has an art left to teach, so on a save with every art
     unlocked it never does, and the completed flag has to be written;
  3. the quest set bits at base+0x3187 of every set whose members are then all
     cleared. The Arena rank sets 46, 47, 53, 78, 79 need Arena records and are left.

Not touched: star levels and HR (reported if they are below the maximum), the failed
bitmap, Arena records, quest counters, the history log, awards, the pending set
notices at base+0x3197, and the story flags that talk lines set once their quests are
cleared (listed at the end of the run).

Usage:  complete_quests.py [--write] [--no-events] [--no-requests] [--lessons] [--slot 1|2|3]
                           system [system ...]
        --slot picks the character (default 1). With --write every given file is
        changed in place. Close the emulator and take a copy first; give both save
        slots (0/system and 1/system).
"""
import sys, csv, re, pathlib

HR       = 0x28
CLEARED   = 0x2C77
SEEN      = 0x2D77
VIL_STAR  = 0x2C4DA
HUB_STAR  = 0x2C4DC
FLAGS     = 0x2C56D
QUESTSETS = 0x3187
RANK_SETS = {46, 47, 53, 78, 79}
EVENT     = {"Event Hub", "Event Arena"}
# Hunter Art lesson requests 126, 128, 133, 141, 143, 150: completed flag (accepted = flag - 1)
# and the flags the report block clears
LESSONS   = {1283: (), 1287: (), 1297: (), 1313: (), 1317: (), 1331: (1398,)}

DATA = pathlib.Path(__file__).resolve().parent.parent / "data"

def plan(buf, events=True, requests=True, lessons=False, slot=1):
    """Return ({absolute offset: new byte}, report lines)."""
    BASE = 0x24 + int.from_bytes(buf[0x34 + 4 * (slot - 1):0x38 + 4 * (slot - 1)], "little")
    new = {}
    def get(off):
        return new.get(BASE + off, buf[BASE + off])
    def bit(off, i):
        return (get(off + (i >> 3)) >> (i & 7)) & 1
    def put(off, i, v=1):
        o = off + (i >> 3)
        b = get(o) | (1 << (i & 7)) if v else get(o) & ~(1 << (i & 7))
        if b != buf[BASE + o]: new[BASE + o] = b
        else: new.pop(BASE + o, None)

    out = [f"character {slot}: {bytes(buf[BASE:BASE + 32]).split(b'\0')[0].decode(errors='replace')!r}"]
    rows = list(csv.DictReader(open(DATA / "quest-index.csv")))
    first, real = {}, []
    for r in rows:
        q = int(r["quest_id"])
        if r["notes"] in ("placeholder entry", "unused event slot") or q in first: continue
        first[q] = int(r["index"])
        if events or r["category"] not in EVENT: real.append(r)

    # 1. cleared + seen
    n = {}
    for r in real:
        i = int(r["index"])
        if not bit(CLEARED, i): n[r["category"]] = n.get(r["category"], 0) + 1
        put(CLEARED, i); put(SEEN, i)
    out.append(f"cleared bits to set: {sum(n.values())}  " + ", ".join(f"{k} {v}" for k, v in sorted(n.items())))

    # 2. requests: accepted flag and the rest of the offer block
    if requests:
        done = set(); todo = []
        for r in csv.DictReader(open(DATA / "request-offer.csv")):
            if not r["quest_id"] or r["index"] in done: continue      # first offer block of a request
            done.add(r["index"])
            if bit(FLAGS, int(r["accept_flag"])): continue
            todo.append(r)
            put(FLAGS, int(r["accept_flag"]))
            for f in map(int, r["also"].split()): put(FLAGS, abs(f), f > 0)
        out.append(f"requests to mark accepted: {len(todo)}")
        if lessons:
            n = 0
            for d, clear in LESSONS.items():
                if not bit(FLAGS, d - 1) or bit(FLAGS, d): continue
                n += 1; put(FLAGS, d)
                for f in clear: put(FLAGS, f, 0)
            out.append(f"lesson requests to mark completed: {n}")
        reqs = {r["index"]: r for r in csv.DictReader(open(DATA / "request-index.csv"))}
        waiting = [r for r in reqs.values() if r["quest_id"] and not bit(FLAGS, int(r["done_flag"]))]
        out.append(f"request reports left for the game: {len(waiting)}")

    # 3. quest sets
    members = {}
    for r in rows:                                # the game counts a repeated ID once, at its first row
        if first.get(int(r["quest_id"])) != int(r["index"]): continue
        for s in map(int, r["sets"].split()): members.setdefault(s, []).append(int(r["index"]))
    sets = [s for s, m in sorted(members.items())
            if s not in RANK_SETS and not bit(QUESTSETS, s) and all(bit(CLEARED, i) for i in m)]
    for s in sets: put(QUESTSETS, s)
    out.append(f"quest set bits to set: {len(sets)}  {' '.join(map(str, sets))}")
    early = [s for s, m in sorted(members.items()) if bit(QUESTSETS, s) and not all(bit(CLEARED, i) for i in m)]
    if early: out.append(f"quest set bits set although the set is incomplete: {early}")

    # what is left to the game
    u16 = lambda off: int.from_bytes(buf[BASE + off:BASE + off + 2], "little")
    hr, vil, hub = u16(HR), u16(VIL_STAR), u16(HUB_STAR)
    out.append(f"HR {hr}, Village star level {vil}, Hub star level {hub}" +
               ("" if vil >= 10 and hub >= 13 else "  <- below the maximum: some quests stay unlisted until the levels are raised"))
    flags = {}
    for r in csv.DictReader(open(DATA / "quest-unlock.csv")):
        alts = [re.findall(r"\bflag:(\d+)", a) for a in r["rule"].split(" OR ")] if r["rule"] else []
        if alts and all(any(not bit(FLAGS, int(f)) for f in a) for a in alts):
            for f in alts[0]:
                if not bit(FLAGS, int(f)): flags.setdefault(int(f), []).append(r["quest_id"])
    if flags:
        out.append("event flags still clear that list a quest (set by an NPC conversation in the game):")
        out += [f"  flag {f}: quests {' '.join(q)}" for f, q in sorted(flags.items())]
    return new, out

def main():
    args = sys.argv[1:]
    slot = 1
    if "--slot" in args:
        i = args.index("--slot"); slot = int(args[i + 1]); del args[i:i + 2]
    opts = {a for a in args if a.startswith("--")}
    paths = [a for a in args if not a.startswith("--")]
    if not paths or opts - {"--write", "--no-events", "--no-requests", "--lessons"} or not 1 <= slot <= 3:
        sys.exit(__doc__)
    for p in paths:
        buf = bytearray(pathlib.Path(p).read_bytes())
        if len(buf) != 5159100: sys.exit(f"{p}: not an MHGU Switch save (size {len(buf)})")
        if not buf[0x28 + slot - 1]: sys.exit(f"{p}: character slot {slot} is not in use")
        new, out = plan(buf, "--no-events" not in opts, "--no-requests" not in opts, "--lessons" in opts, slot)
        print(p); print("\n".join("  " + l for l in out))
        print(f"  bytes to change: {len(new)}")
        if "--write" in opts:
            for o, b in new.items(): buf[o] = b
            pathlib.Path(p).write_bytes(buf)
            print("  written")
    if "--write" not in opts: print("dry run, nothing written (--write to apply)")

if __name__ == "__main__":
    main()
