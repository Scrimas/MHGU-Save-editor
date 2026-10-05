"""Evidence checks for the editor's goals (app/gui/src/goals.rs) that a game session tested."""
import csv
from timeline import *

CROWN_AWARDS = {7: 'Miniature Crown', 8: 'Large Crown', 115: 'Miniature Master Crown', 116: 'Huge Master Crown'}


def _by(S): return {n: b for t, n, b in S}


def _crowns(b):
    """(small, gold) crown counts over the monsters that count for the crown awards"""
    B, mini, gold = base(b), 0, 0
    for r in csv.DictReader(open(R + 'data/monster-sizes.csv')):
        if r['crown_awards'] != 'yes': continue
        o = B + 0x60CA + 4 * (int(r['index']) - 1)
        lo, hi = u16(b, o), u16(b, o + 2)
        mini += bool(lo) and lo <= int(r['mini_le'])
        gold += hi >= int(r['gold_ge'])
    return mini, gold


def chk_crowns_goal(S):
    """the app's crowns goal (10-04 23:41) took the crown counts from 25 small / 13 gold to all 79; the next
    game session's award check granted Miniature / Large Crown and both Master Crowns (needs 66)"""
    D = _by(S)
    pre, post, game = D['app-2026-10-04_233922'], D['app-2026-10-04_234149'], D['app-2026-10-05_124632']
    a, c = _crowns(pre), _crowns(post)
    award = lambda b, i: bit(b, base(b) + 0x3157, i)
    granted = [i for i in CROWN_AWARDS if not award(post, i) and award(game, i)]
    ok = max(a) < 66 <= min(c) and granted == list(CROWN_AWARDS)
    return ok, 'crowns %s -> %s; granted in game: %s' % (a, c, ', '.join(CROWN_AWARDS[i] for i in granted))


CHECKS = [(('goal', 'crowns'), chk_crowns_goal)]
