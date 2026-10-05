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


def chk_palico_in_game(S):
    """read off in game 2026-10-05 (Palico Info of Suds, save confirm-0-before-tests): Lv 64, bias Gathering,
    comment "I'm a hoarder!", status Palico 1; the record holds level byte 63, bias 6, that greeting, and
    buddy 1 = its index"""
    b = _by(S)['confirm-0-before-tests']; B = base(b); j = 2
    r = B + 0x23BB6 + 324 * j
    got = (b[r:r + 32].split(b'\0')[0], b[r + 0x24] + 1, b[r + 0x25], b[r + 0x60:r + 0x9C].split(b'\0')[0], b[B + 0x23B9E])
    return got == (b'Suds', 64, 6, b"I'm a hoarder!", j), 'name, Lv, bias, greeting, buddy 1 = %s' % (got,)


def chk_body_in_game(S):
    """read off in game 2026-10-05: Scrimas, Scrimas2 and Scrimas3 are all male; body byte 0 in all three"""
    b = _by(S)['confirm-0-before-tests']
    v = [b[base(b, s) + 0x23B4B] for s in (1, 2, 3)]
    return v == [0, 0, 0], 'body bytes %s' % v


def chk_test_write(S):
    """2026-10-05 controlled write on Scrimas by the editor's code (confirm-1): Rathian's Hunter's Notes
    bit cleared, HR 999 -> 500 by HR points, a Hunter's Knife (type 8, ID 11) in box slot 557. Seen in game:
    Rathian gone from the Notes, HR 500, the knife in the box and equippable; the game's save (confirm-2)
    kept all three; the restore (confirm-3) wrote them back through the hr999 / notes goal code"""
    D = _by(S)
    w, g, r = D['confirm-1-test-write'], D['confirm-2-after-game'], D['app-2026-10-05_180822']
    B = base(w)
    notes = lambda b: bit(b, B + 0x32B7, 79)       # Rathian = monster 1, notes bit 79 (monster-sizes.csv)
    hr = lambda b: (u16(b, B + 0x28), u32(b, B + 0x280B), u16(b, B + 0xC71BD + 0x16))
    knife = lambda b: b[B + 0x62EE + 36 * 556:][:4].hex()
    after = D['confirm-3-restore-write']
    ok = (notes(r), notes(w), notes(g), notes(after)) == (1, 0, 0, 1) and hr(w) == hr(g) == (500, 2001420, 500) \
        and hr(after)[0] == 999 and knife(r) == '00000000' and knife(w) == knife(g) == '08000b00'
    return ok, 'Notes bit %d->%d->%d->%d, HR %s -> game %s -> %s, knife %s' % (
        notes(r), notes(w), notes(g), notes(after), hr(w), hr(g), hr(after)[0], knife(g))


CHECKS = [(('goal', 'crowns'), chk_crowns_goal), (('char', 0x23BB6), chk_palico_in_game),
          (('char', 0x23B9E), chk_palico_in_game), (('char', 0x23B4B), chk_body_in_game),
          (('char', 0x32B7), chk_test_write), (('char', 0x280B, 0x4), chk_test_write),
          (('goal', 'notes'), chk_test_write), (('goal', 'hr999'), chk_test_write)]
