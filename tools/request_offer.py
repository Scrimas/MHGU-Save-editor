#!/usr/bin/env python3
"""List the villager requests not accepted yet and what their NPC still waits for. Read-only.

The conditions in ../data/request-offer.csv come from the NPC talk data
(see ../docs/10-npc-talk.md). Conditions this tool cannot evaluate are printed as "?".
In the CSV, "a|b" means either condition holds.

Usage:  request_offer.py [--slot 1|2|3] [path/to/system]
"""
import sys, csv, pathlib

HR       = 0x28            # u16
CLEARED   = 0x2C77          # quest bitmap, index from quest-index.csv
VIL_STAR  = 0x2C4DA         # u16
HUB_STAR  = 0x2C4DC         # u16
FLAGS     = 0x2C56D         # event flag bitmap
NPC_HOLD  = 0x2C62D         # per-NPC bits, map A: no request offers until the next quest
PROGRESS  = 0x2F77          # u32: bit 20 = HR limit released, bit 31 = quest 10646 was listed
QUESTSETS = 0x3187          # bit N = every quest of set N (quest-index.csv, column sets) is cleared
POINTS    = 0x281B          # u32[4] village points Bherna/Kokoto/Pokke/Yukumo, G-rank set 16 bytes later
VILLAGES  = ["Bherna", "Kokoto", "Pokke", "Yukumo"]

DATA = pathlib.Path(__file__).resolve().parent.parent / "data"

def main():
    args = sys.argv[1:]
    slot = 1
    if "--slot" in args:
        i = args.index("--slot"); slot = int(args[i + 1]); del args[i:i + 2]
    path = args[0] if args else "system"
    buf = pathlib.Path(path).read_bytes()
    BASE = 0x24 + int.from_bytes(buf[0x34 + 4 * (slot - 1):0x38 + 4 * (slot - 1)], "little")
    index, groups, sets = {}, {}, {}
    for row in csv.DictReader(open(DATA / "quest-index.csv")):
        if int(row["quest_id"]) not in index:
            for n in row["sets"].split():
                sets.setdefault(int(n), []).append(int(row["index"]))
        index.setdefault(int(row["quest_id"]), int(row["index"]))
        if int(row["group"]):                     # key quest sets; quests sharing an alt value count once
            groups.setdefault(int(row["group"]), {}).setdefault(int(row["alt"]) or -int(row["index"]), []).append(int(row["index"]))

    def bit(off, i):
        return (buf[BASE + off + (i >> 3)] >> (i & 7)) & 1
    u16 = lambda off: int.from_bytes(buf[BASE + off:BASE + off + 2], "little")
    u32 = lambda off: int.from_bytes(buf[BASE + off:BASE + off + 4], "little")
    hr, vil, hub = u16(HR), u16(VIL_STAR), u16(HUB_STAR)

    done = {int(r["index"]): int(r["done_flag"]) for r in csv.DictReader(open(DATA / "request-index.csv")) if r["done_flag"]}

    offers = list(csv.DictReader(open(DATA / "request-offer.csv")))
    npc_bit = {r["npc_id"]: int(r["bit"]) for r in csv.DictReader(open(DATA / "npc-index.csv"))}

    def missing(c, row):
        """None if the condition holds, a description if not, '?...' if unknown."""
        kind, _, arg = c.partition(":")
        if kind == "flag":         return None if bit(FLAGS, int(arg)) else f"event flag {arg}"
        if kind == "notflag":      return None if not bit(FLAGS, int(arg)) else f"event flag {arg} must be clear"
        if kind == "cleared" and arg.isdigit() and int(arg) in index:
            return None if bit(CLEARED, index[int(arg)]) else f"clear {arg}"
        if kind == "notcleared" and arg.isdigit() and int(arg) in index:
            return None if not bit(CLEARED, index[int(arg)]) else f"{arg} must not be cleared"
        if kind == "village_eq":   return None if vil == int(arg) else f"Village star level exactly {arg}"
        if kind == "hub_eq":       return None if hub == int(arg) else f"Hub star level exactly {arg}"
        if kind == "village_max":  return None if vil <= int(arg) else f"Village star level at most {arg}"
        if kind == "hub_max":      return None if hub <= int(arg) else f"Hub star level at most {arg}"
        if kind == "hr_unlocked":  return None if bit(PROGRESS, 20) else "HR limit released"
        if kind == "listed" and arg == "10646" and bit(PROGRESS, 31): return None
        if kind == "listed":       return f"?quest {arg} listed on the board (see quest_unlock.py)"
        if kind in ("village_keys", "group_done"):
            g = int(arg) + (10 if kind == "group_done" else 0)
            ok = all(any(bit(CLEARED, i) for i in alt) for alt in groups.get(g, {}).values())
            return None if ok else f"all key quests of quest_group group {g}"
        if kind == "questset":
            if bit(QUESTSETS, int(arg)): return None
            left = sum(1 for i in sets.get(int(arg), []) if not bit(CLEARED, i))
            return f"quest set {arg}: {left} quests left" if left else f"quest set {arg}: all cleared, but bit {arg} at base+0x3187 is clear (the game sets it only when it sees the last clear)"
        if kind == "requests_done":
            rng, need = arg.split(":"); lo, hi = map(int, rng.split("-"))
            have = sum(bit(FLAGS, done[i]) for i in range(lo, hi + 1) if i in done)
            return None if have >= int(need) else f"{need} of requests {rng} completed (now {have})"
        if kind == "npc_idle":
            if arg not in npc_bit: return "?" + c
            held = any(bit(NPC_HOLD + 24 * m, npc_bit[arg]) for m in range(3))
            return None if not held else "NPC on hold until the next quest"
        if kind in ("footbath", "not_footbath"):
            return None                         # where the player stands while talking, not save state
        if kind == "points":
            name, need = arg.split(":"); v = VILLAGES.index(name)
            have = min(20000, u32(POINTS + 4 * v) + u32(POINTS + 16 + 4 * v))
            return None if have >= int(need) else f"{name} points {have}/{need}"
        if kind == "village_star": return None if vil >= int(arg) else f"Village star level {arg}"
        if kind == "hub_star":     return None if hub >= int(arg) else f"Hub star level {arg}"
        if kind == "hr":           return None if hr >= int(arg) else f"HR {arg}"
        return "?" + c

    print(f"character {slot}: {buf[BASE:BASE + 32].split(b'\0')[0].decode(errors='replace')!r}, "
          f"HR {hr}, Village star level {vil}, Hub star level {hub}")
    for row in offers:
        if bit(FLAGS, int(row["accept_flag"])):
            continue
        miss = []
        for group in (c for c in row["offer"].split(" AND ") if c):
            ms = [missing(c, row) for c in group.split("|")]      # a|b: either holds
            if all(ms):
                miss.append(" or ".join(ms))
        hold_map = {"8": 0, "9": 24}.get(row["talk_kind"])     # map A holds kind 8 blocks, map B kind 9
        hold = row["npc_bit"] and hold_map is not None and bit(NPC_HOLD + hold_map, int(row["npc_bit"]))
        need = "; ".join(miss) if miss else "ready"
        print(f"{row['index']:>4}  {row['quest_id']:>6}  {row['quest_name'] or '(delivery)':<34} "
              f"{row['npc_name']:<20} flag {row['accept_flag']:>4}  <- {need}{'  [NPC on hold]' if hold else ''}")

if __name__ == "__main__":
    main()
