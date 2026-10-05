"""Evidence checks: Guild Card manager, guest hunters, chat phrases.

CHECKS: LABELS entries (key = (scope, offset)). SPLIT_CHECKS: sub-labels (scope, offset, size).
"""
import sys, os, csv, zlib
from timeline import *

CARD = 0xC71BD                 # own Guild Card, 6328 B
L1, INFO1 = 0x2C6BD, 0xC8A75   # list 1 elements, list 1 card info (44 B)
GH, GH_REC, GH_ST, GH_N = 0x11B631, 0x11B693, 0x1D6, 13   # guest hunters: 98 B header, 13 x 470 B
CHAT_SLOTS, CHAT_AUTO = 0x11D089, 0x11EDC9


def _s(S, name): return [b for t, n, b in S if n == name][0]
def _str16(x): return x.decode('utf-16le', 'replace').split('\0')[0]
def _quests():
    q = {}
    for r in csv.DictReader(open(R + 'data/quest-index.csv')): q.setdefault(int(r['quest_id']), r['name'])
    return q


def _elems(b, B):
    """walk list 1: [(len, zlib, state, trailer)], bytes used"""
    o, out = B + L1, []
    for i in range(100):
        ln = u32(b, o); out.append((ln, b[o + 4:o + 4 + ln], u32(b, o + 4 + ln), b[o + 8 + ln:o + 44 + ln])); o += 44 + ln
    return out, o - (B + L1)


# ---------------------------------------------------------------- whole-label promotions

def chk_card_playtime(S):
    """Card play time (card +0x914) is a lagging copy of sGameControl play time: <= it in all saves/slots, never decreases, within 100 s after every quest; app wrote 560908, the next game session carried it to 561034 (pt 561059)"""
    prev, bad = None, []
    for t, n, b in S:
        for sl in (1, 2, 3):
            B = base(b, sl); pt, gc = u32(b, B + 0x2248B), u32(b, B + CARD + 0x914)
            if gc > pt: bad.append((n, sl, 'gc>pt'))
        B = base(b); pt, gc = u32(b, B + 0x2248B), u32(b, B + CARD + 0x914)
        if prev is not None and gc < prev: bad.append((n, 'decreased'))
        prev = gc
    after_quest = ['backup-20260919-125813-pre-arts', 'backup-20260919-142019-pre-capfix-awardtest', 'req-1-after-fatedfour',
                   'req-2-after-ludroth', 'complete-0-pre-full-completion', 'complete-2-after-quest']
    gaps = []
    for n in after_quest:
        b = _s(S, n); B = base(b); gaps.append(u32(b, B + 0x2248B) - u32(b, B + CARD + 0x914))
    w, g = _s(S, 'app-2026-10-04_235124'), _s(S, 'app-2026-10-05_124632')
    wv, gv, gp = u32(w, base(w) + CARD + 0x914), u32(g, base(g) + CARD + 0x914), u32(g, base(g) + 0x2248B)
    ok = not bad and max(gaps) <= 100 and wv == 560908 and wv < gv <= gp
    return ok, 'violations=%s; pt-gc after quests=%s; app 560908 -> game %d (pt %d)' % (bad[:3], gaps, gv, gp)


HIST = 0xC7AD5
QUEST_STEPS = [  # snapshot after the quest, expected quest ID, counted by the quest counter
    ('backup-20260919-125813-pre-arts', 11463, True), ('backup-20260919-142019-pre-capfix-awardtest', 814, True),
    ('req-1-after-fatedfour', 621, True), ('req-2-after-ludroth', 10220, True),
    ('complete-0-pre-full-completion', 1038, True), ('complete-2-after-quest', 201, False)]


def _hrec(b, B, i):
    r = b[B + HIST + 0xA0 * i:B + HIST + 0xA0 * (i + 1)]
    return dict(d=r[0], m=r[1], y=u16(r, 2), kind=u16(r, 4), qid=u16(r, 6), name=_str16(r[8:0x28]), hl=list(r[0x28:0x2B]), wt=r[0x9C], raw=r)


def chk_quest_history(S):
    """Quest history: each quest in the timeline pushed one record (date 19/09/2026, kind 7, quest ID whose quest-index name matches the stored name; Fated Four 621, Ludroth 10220, Harvest Tour 201 ...), older records shift down, weapon byte +0x9C = the weapon-usage counter that went +1; fresh Scrimas2/3: kind 2 Welcome dated 4/10/2026, rest kind 11 / highlights 36"""
    Q = _quests(); names = [n for t, n, b in S]; out = []
    for n, qid, counted in QUEST_STEPS:
        b = _s(S, n); a = S[names.index(n) - 1][2]; B = base(b)
        new, old0 = _hrec(b, B, 0), _hrec(a, base(a), 0)
        shifted = all(_hrec(b, B, i + 1)['raw'] == _hrec(a, base(a), i)['raw'] for i in range(9))
        nm_ok = Q.get(new['qid'], '').startswith(new['name'].rstrip('…'))
        qc = u32(b, B + 0x5E4E) - u32(a, base(a) + 0x5E4E)
        wa = [u16(a, base(a) + 0xC7A77 + 2 * i) for i in range(45)]; wb = [u16(b, B + 0xC7A77 + 2 * i) for i in range(45)]
        wd = [i % 15 for i in range(45) if wb[i] == wa[i] + 1]
        ok = (new['qid'] == qid and new['kind'] == 7 and (new['d'], new['m'], new['y']) == (19, 9, 2026) and shifted and nm_ok
              and qc == (1 if counted else 0) and wd == [new['wt']])
        out.append((qid, ok))
    fresh = []
    for sl in (2, 3):
        b = _s(S, 'char2-2-after-scrimas3'); B = base(b, sl); r0 = _hrec(b, B, 0)
        rest = [_hrec(b, B, i) for i in range(1, 10)]
        fresh.append(r0['kind'] == 2 and (r0['d'], r0['m'], r0['y']) == (4, 10, 2026) and all(r['kind'] == 11 and r['hl'] == [36, 36, 36] for r in rest))
    ok = all(o for q, o in out) and all(fresh)
    return ok, 'steps %s; fresh slots 2/3 Welcome+unset %s' % (out, fresh)


def chk_list1(S):
    """Guild Card list 1: in all saves slot 1 walks to 2 cards + 98 empties; each zlib stream inflates to exactly 6328 B (hunters Julien, Toadiddy', HR 148; 19 completed-quest records with IDs in quest-index, French names on Julien's card, the same 8 quests on both cards on 29/08); trailer HR/name/ID = the card's HR/name/+0x8B0, trailer u32 = the card's play time; slots 2/3: 100 empties"""
    Q = _quests(); bad = []; seen = set()
    for t, n, b in S:
        for sl in (1, 2, 3):
            el, used = _elems(b, base(b, sl))
            if used > 0x9AB00: bad.append((n, sl, 'overrun'))
            for ln, z, st, tr in el:
                if st == 1: continue
                if st != 3: bad.append((n, sl, 'state', st)); continue
                c = zlib.decompress(z)
                if len(c) != 0x18B8: bad.append((n, 'len', len(c))); continue
                # completed-quest records (kind 7); Julien's one kind-9 (abandoned) record holds 27972, not in the index
                h = [u16(c, 0x918 + 0xA0 * i + 6) for i in range(10) if u16(c, 0x918 + 0xA0 * i + 4) == 7]
                if not (u16(tr, 0) == u16(c, 0x16) and _str16(tr[2:24]) == _str16(c[:22]) and tr[24:32] == c[0x8B0:0x8B8]
                        and u32(tr, 32) == u32(c, 0x914) and all(q in Q for q in h)):
                    bad.append((n, sl, _str16(c[:22])))
                seen.add((sl, _str16(c[:22]), u16(c, 0x16)))
    return (not bad and seen == {(1, 'Julien', 148), (1, "Toadiddy'", 148)}), 'cards=%s bad=%s' % (sorted(seen), bad[:3])


CHECKS = [(('char', 0xC7AD1), chk_card_playtime), (('char', 0xC7AD5), chk_quest_history), (('char', 0x2C6BD), chk_list1)]


# ---------------------------------------------------------------- proposed split labels
# key (scope, offset, size) of a NEW label carved out of a DERIVED parent; the parent stays DERIVED.

def chk_card_name(S):
    """Own card +0 (22 B UTF-16) = the hunter name in every slot/save; 'Scrimas2' / 'Scrimas3' appeared exactly when those characters were created in game"""
    bad = []
    for t, n, b in S:
        for sl in (1, 2, 3):
            B = base(b, sl)
            if _str16(b[B + CARD:B + CARD + 22]) != b[B:B + 32].split(b'\0')[0].decode(): bad.append((n, sl))
    nm = lambda n, sl: _str16(_s(S, n)[base(_s(S, n), sl) + CARD:][:22])
    steps = [nm('char2-0-pre-new-character', 2), nm('char2-1-after-scrimas2', 2), nm('char2-1-after-scrimas2', 3), nm('char2-2-after-scrimas3', 3)]
    ok = not bad and steps == ['', 'Scrimas2', '', 'Scrimas3']
    return ok, 'mismatches=%d; slot2 %r->%r, slot3 %r->%r' % ((len(bad),) + tuple(steps))


def _boxconv(e):
    w = u16(e, 0)
    return (w & 0x1f, u16(e, 2), (w >> 5) & 0x1f, e[6:12], e[0xC:0x24], u16(e, 4), (w >> 10) & 0x1f)
def _cardconv(c):
    return (c[0], u16(c, 2), c[4], c[8:14], c[0x10:0x28], u16(c, 0x28), c[0x2A])


def chk_card_equipment(S):
    """Own card +0x54, 7 x 44 B: fresh Scrimas2/3 cards = the CONFIRMED box entries at the equipped indices (type, ID, level, decorations, transmog); slot 1's 5 armor + talisman entries equal box entries 38/49/55/61/67/179 field for field in all 36 saves (an older equipped set: the card is not refreshed on gear change)"""
    fresh = []
    for n in ('char2-2-after-scrimas3', 'LIVE'):
        b = _s(S, n)
        for sl in (2, 3):
            B = base(b, sl)
            for k in range(7):
                idx = u16(b, B + 0x23B39 + 2 * k); c = _cardconv(b[B + CARD + 0x54 + 44 * k:][:44])
                e = _boxconv(b[B + 0x62EE + 36 * idx:][:36]) if idx != 0xFFFF else None
                fresh.append(c == e if e else c[1] == 0)
    s1 = []
    for t, n, b in S:
        B = base(b); box = {_boxconv(b[B + 0x62EE + 36 * i:][:36]) for i in range(2000)}
        s1.append(all(_cardconv(b[B + CARD + 0x54 + 44 * k:][:44]) in box for k in range(1, 7)))
    return all(fresh) and all(s1), 'fresh slots 2/3 entries matching %d/%d; slot-1 armor+talisman found in box in %d/%d saves' % (sum(fresh), len(fresh), sum(s1), len(s1))


def chk_card_monster_log(S):
    """Own card +0xF6C, 87 x 8 B: in all 36 saves u16 +0 / +2 = CONFIRMED size record max / min and bits 0-13 / 14-27 = family sums of the CONFIRMED hunt / capture tallies (all 87 entries); the log total moved 375->376->377->381->383 at the quests, the crown bits follow the size thresholds"""
    M = {int(r['index']): r for r in csv.DictReader(open(R + 'data/monster-sizes.csv'))}
    heads = {int(r['card_pos']): i for i, r in M.items() if r['card_pos']}
    fam = {}
    for i, r in M.items(): fam.setdefault(int(r['family_of']) if r['family_of'] else i, []).append(i)
    bad, tot = 0, []
    for t, n, b in S:
        B = base(b); s = 0
        for p, h in heads.items():
            o = B + CARD + 0xF6C + 8 * p; w = u32(b, o + 4)
            hs = min(9999, sum(u16(b, B + 0x5EA4 + 2 * k) for k in fam[h])); cs = min(9999, sum(u16(b, B + 0x5FB6 + 2 * k) for k in fam[h]))
            mn, mx = u16(b, B + 0x60C6 + 4 * h), u16(b, B + 0x60C6 + 4 * h + 2)
            if (w & 0x3fff, (w >> 14) & 0x3fff, u16(b, o), u16(b, o + 2)) != (hs, cs, mx, mn): bad += 1
            s += (w & 0x3fff) + ((w >> 14) & 0x3fff)
        tot.append(s)
    steps = sorted(set(tot), key=tot.index)
    return bad == 0 and len(heads) == 87 and steps == [375, 376, 377, 381, 383], 'entry mismatches=%d over %d saves x 87; totals %s' % (bad, len(S), steps)


HIRED = {'Great': 7, 'Sword': 8, 'Hamme': 9, 'Lance': 10, 'Heavy': 11, 'Light': 13, 'Long ': 14, 'Switc': 15, 'Gunla': 16,
         'Bow': 17, 'Dual ': 18, 'Hunti': 19, 'Insec': 20, 'Charg': 21}   # weapon type code = class + 7 (equipment.rs)


def chk_guest_records(S):
    """Guest hunters, 13 x 470 B records at +0x62: records carrying a list-1 card owner's ID (+0x65) copy that card's name, title, HR, greeting (+0x878), appearance, pigment and all 7 x 44 equipment, and +0x5C = that card's Unity (list 1 info +0x1C); one record is an older copy of Toadiddy' (HR 44); every 'Hired <weapon>' record's weapon entry has the type code of the named weapon (10 classes)"""
    full = older = hired = 0; bad = []
    for t, n, b in S:
        B = base(b); cards = {}
        el, _ = _elems(b, B)
        for i, (ln, z, st, tr) in enumerate(el):
            if st == 3:
                c = zlib.decompress(z); cards[c[0x8B0:0x8B8]] = (c, u32(b, B + INFO1 + 44 * i + 0x1C))
        for i in range(GH_N):
            r = b[B + GH_REC + GH_ST * i:][:GH_ST]; nm = _str16(r[:22])
            if nm.startswith('Hired '):
                hired += 1
                if HIRED.get(nm[6:11]) != r[0xA2]: bad.append((n, i, nm, r[0xA2]))
            elif r[0x65:0x6D] in cards:
                c, unity = cards[r[0x65:0x6D]]
                same = (nm == _str16(c[:22]) and r[0x16:0x1C] == c[0x854:0x85A] and u16(r, 0x1C) == u16(c, 0x16) and r[0x1E:0x56] == c[0x878:0x8B0]
                        and r[0x72:0x7E] == c[0x18:0x24] and r[0x7E:0xA2] == c[0x24:0x48] and r[0xA2:0x1D6] == c[0x54:0x188] and u32(r, 0x5C) == unity)
                if same: full += 1
                elif nm == _str16(c[:22]) and r[0x1E:0x56] == c[0x878:0x8B0] and u16(r, 0x1C) < u16(c, 0x16): older += 1
                else: bad.append((n, i, nm))
            elif any(r[:22]): bad.append((n, i, nm, 'unknown'))
    return not bad and full >= 3 * len(S) and hired == 6 * len(S), 'exact card copies=%d, older copies=%d, hired=%d, bad=%s' % (full, older, hired, bad[:3])


def chk_chat_slots(S):
    """Chat block +0x49: 72 x 104 B = 3 x the 24 shortcut phrases ("Let's do this!" ... "I'm outta here.") and +0x1D89: 27 x 104 B = 3 x the 9 auto-chat lines ("I mounted it!", "It has me pinned!", "Hunter Art 1-3 activated!"), char strings, same in all slots/saves"""
    bad = []
    for t, n, b in S:
        for sl in (1, 2, 3):
            B = base(b, sl)
            p = [b[B + CHAT_SLOTS + 104 * k:][:104].split(b'\0')[0].decode('latin1') for k in range(72)]
            a = [b[B + CHAT_AUTO + 104 * k:][:104].split(b'\0')[0].decode('latin1') for k in range(27)]
            if not (p[0] == "Let's do this!" and p[23] == "I'm outta here." and p[:24] == p[24:48] == p[48:]
                    and a[0] == 'I mounted it!' and a[8] == 'Hunter Art 3 activated!' and a[:9] == a[9:18] == a[18:]):
                bad.append((n, sl))
    return not bad, 'slots failing=%d of %d' % (len(bad), 3 * len(S))


SPLIT_CHECKS = [
    (('char', 0xC71BD, 0x16), chk_card_name),           # own card: hunter name, UTF-16
    (('char', 0xC7211, 0x134), chk_card_equipment),     # own card: equipment 7 x 44 B
    (('char', 0xC8129, 0x2B8), chk_card_monster_log),   # own card: monster log 87 x 8 B
    (('char', 0x11B693, 0x17DE), chk_guest_records),    # guest hunters: 13 x 470 B records
    (('char', 0x11D089, 0x1D40), chk_chat_slots),       # chat: 72 shortcut phrases
    (('char', 0x11EDC9, 0xAF8), chk_chat_slots),        # chat: 27 auto-chat lines
]

if __name__ == '__main__':
    S = saves()
    for (k, f) in CHECKS + SPLIT_CHECKS:
        ok, d = f(S)
        print('%-5s %-26s %s: %s' % ('PASS' if ok else 'FAIL', str(tuple(hex(x) if isinstance(x, int) else x for x in k)), f.__name__, d))
