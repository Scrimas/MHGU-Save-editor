#!/usr/bin/env python3
"""run.py : re-run the evidence behind the CONFIRMED tags that come from the save timeline.

    nice -n 19 python3 tools/evidence/run.py

Each check ties a field (a LABELS entry of tools/emu/savemap.py) to what the game did between two
saves of timeline.py: a quest, a talk, a character creation, or a tool write that a later game
session carried on from. The saves are local (snapshots/ and the editor's snapshots), so this only
runs on the machine that holds them. Read-only. Exit status 1 if a check fails or a checked
entry is not tagged CONFIRMED in LABELS.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)) + '/../emu')
from timeline import saves
import shared_and_s, managers, cards, goals
from savemap import LABELS


def main():
    S = saves()
    tagged = {(sc, o): c for sc, o, n, l, c in LABELS} | {(sc, o, n): c for sc, o, n, l, c in LABELS}
    lists = [shared_and_s.CHECKS, shared_and_s.SPLIT_CHECKS, shared_and_s.CORRECTED_CHECKS,
             managers.CHECKS, managers.PROPOSED, cards.CHECKS, cards.SPLIT_CHECKS, goals.CHECKS]
    bad = 0
    for lst in lists:
        for key, f in lst:
            ok, detail = f(S)
            tag = 'goal' if key[0] == 'goal' else tagged.get(key, 'no LABELS entry')
            ok = ok and tag in ('goal', 'CONFIRMED')
            bad += not ok
            k = ' '.join(x if isinstance(x, str) else '0x%X' % x for x in key)
            print('%-4s %-22s %-24s %s' % ('ok' if ok else 'FAIL', k, f.__name__, detail))
            if tag not in ('goal', 'CONFIRMED'): print('     tagged %s in LABELS' % tag)
    print('%d saves, %d checks failed' % (len(S), bad))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
