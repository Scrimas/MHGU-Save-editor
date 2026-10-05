"""Evidence checks: sEquipBox (Palico box), sGameControl, sItem, sPlayer, sOtomo, sVillage.

CHECKS: LABELS entries (key = (scope, offset)); where only a sub-range is CONFIRMED the
docstring says so and LABELS has the sub-label. PROPOSED: sub-labels with an offset of their own.
"""
from timeline import *  # noqa: F401,F403

# ---------------------------------------------------------------- helpers
def _by(S): return {n: b for t, n, b in S}
def _names(S): return [n for t, n, b in S]
def _stream(b, o, n):
    v = int.from_bytes(b[o:o + (19 * n + 7) // 8], 'little')
    return [((v >> (19 * i)) & 0xFFF, (v >> (19 * i + 12)) & 0x7F) for i in range(n)]
def _box_ids(b, s=1):
    B = base(b, s)
    return {i for i, c in _stream(b, B + 0x278, 2300) + _stream(b, B + 0x27BF, 32) if i}
def _obtained(b, s=1):
    o = base(b, s) + 0x22497
    return {i for i in range(94 * 32) if bit(b, o, i)}
def _ent(b, s, i, o):
    p = base(b, s) + o + 36 * i
    return b[p:p + 36]
def _etype(e): return u16(e, 0) & 0x1F
def _equipped(b, s=1):
    B = base(b, s)
    return [u16(b, B + 0x23B39 + 2 * k) for k in range(7)]
def _myset(b, s=1):
    """the My Set (CONFIRMED, doc-07 offsets from base+0x208C8) whose box indices equal the equipped gear"""
    B = base(b, s); idx = b[B + 0x23B39:B + 0x23B39 + 14]
    for k in range(40):
        r = B + 0x208C8 + 0x88 * k
        if b[r + 0x30:r + 0x3E] == idx: return r
    return None
def _qc(b): return u32(b, base(b) + 0x5E4E)          # quest counter (S+0x44..), CONFIRMED
def _slots(b): return [s for s in (1, 2, 3) if b[base(b, s) + 0x23B7D]]   # slots holding a character
PT_WRITES = ('app-2026-10-04_235015', 'confirm-0-before-tests')   # saves right after the app wrote play time
def _game_pairs(S):
    """consecutive pairs (a, b, nameb) where the game ran (play time moved, not by an app write)"""
    out = []
    for (t0, n0, a), (t1, n1, b) in zip(S, S[1:]):
        if n1 in PT_WRITES: continue
        if u32(a, base(a) + 0x2248B) != u32(b, base(b) + 0x2248B): out.append((a, b, n1))
    return out
import csv as _csv
_ARTS = {int(x['id']): x for x in _csv.DictReader(open(R + 'data/hunter-arts.csv'))}
PK = 324
def _pal(b, s, lst, j): r = base(b, s) + lst + PK * j; return b[r:r + PK]

# ---------------------------------------------------------------- sGameControl
def chk_play_time(S):
    """play time: app wrote 560908 (10-04 23:50), next game session carried on to 561059 (+151 s); never moves on tool-only steps; each game step <= wall clock; slot header +0x20 equal in every save"""
    by = _by(S); B = base(S[0][2])
    w, g = by['app-2026-10-04_235015'], by['app-2026-10-05_124632']
    pre = by['app-2026-10-04_234914']
    ok = u32(pre, B + 0x2248B) == 175408 and u32(w, B + 0x2248B) == 560908 and 0 < u32(g, B + 0x2248B) - 560908 < 3600
    for n in ('unlock-1-pre-cleared-619-11404', 'unlock-2-pre-rotation-618', 'capture-B'):   # tool-only / identical
        i = _names(S).index(n); ok &= u32(S[i - 1][2], B + 0x2248B) == u32(S[i][2], B + 0x2248B)
    steps = 0
    for (t0, n0, a), (t1, n1, b) in zip(S, S[1:]):
        if n1 in PT_WRITES: continue
        d = u32(b, B + 0x2248B) - u32(a, B + 0x2248B)
        ok &= 0 <= d <= (t1 - t0) + 1
        steps += d > 0
    ok &= all(u32(b, base(b) + 0x2248B) == u32(b, base(b) + 0x20) for t, n, b in S)
    return ok, 'app 175408->560908, game 560908->%d; %d game steps all >=0 and <= wall clock; header copy equal in %d saves' % (
        u32(g, B + 0x2248B), steps, len(S))

def chk_play_rem(S):
    """f32 remainder: 18 distinct values, all in [0,60); changes in every game session (seconds moved) and never otherwise; untouched by the app's seconds write; 0 in the fresh slots"""
    vals, ok = set(), True
    for (t0, n0, a), (t1, n1, b) in zip(S, S[1:]):
        B = base(b)
        ra, rb = f32(a, B + 0x2248F), f32(b, B + 0x2248F)
        sa, sb = u32(a, B + 0x2248B), u32(b, B + 0x2248B)
        if n1 in PT_WRITES: ok &= ra == rb; continue
        ok &= (ra != rb) == (sa != sb)
    for t, n, b in S:
        r = f32(b, base(b) + 0x2248F); vals.add(r); ok &= 0 <= r < 60
    fresh = _by(S)['char2-2-after-scrimas3']
    ok &= f32(fresh, base(fresh, 2) + 0x2248F) == 0 and f32(fresh, base(fresh, 3) + 0x2248F) == 0
    return ok, '%d distinct values in [%.2f, %.2f]; moves iff the seconds move' % (len(vals), min(vals), max(vals))

# ---------------------------------------------------------------- sItem
def chk_items_obtained(S):
    """every item ID that newly entered the CONFIRMED box/pouch in game got its bit in the same save (9 IDs over 4 sessions); no bit ever cleared; 1178 of 1182 IDs held have their bit"""
    ok, new, prev = True, 0, None
    for t, n, b in S:
        ids, ob = _box_ids(b), _obtained(b)
        if prev:
            pids, pob = prev
            add = ids - pids
            ok &= add <= ob; new += len(add)
            ok &= pob <= ob
        prev = (ids, ob)
    b = S[-1][2]; ids = _box_ids(b); miss = ids - _obtained(b)
    ok &= new >= 9 and len(miss) <= 4
    return ok, '%d new IDs all flagged at once; %d/%d IDs held have the bit (missing %s)' % (new, len(ids) - len(miss), len(ids), sorted(miss))

def chk_g_accum(S):
    """G-rank accumulator = (old + G-rank contribution points gained) mod 800: complete-1 +1750 points, 755 -> 105 (3 wraps); unchanged in every other game session where the G points did not move"""
    ok = True; det = ''
    for a, b, n in _game_pairs(S):
        B = base(b)
        ga = sum(u32(a, B + 0x282B + 4 * k) for k in range(4)); gb = sum(u32(b, B + 0x282B + 4 * k) for k in range(4))
        xa, xb = u16(a, B + 0x2381B), u16(b, B + 0x2381B)
        if gb != ga:
            ok &= (xa + gb - ga) % 800 == xb
            det += '%s: +%d pts, %d -> %d; ' % (n, gb - ga, xa, xb)
        else:
            ok &= xa == xb
    return ok and det != '', det

def chk_staged0(S):
    """staged counter 0 (G1-4 star clears): +1 in the session that cleared Seregios Scuffle (Hub G4, first clear); unchanged at Village 6, Hub 2, Village 10 clears (split: 0x23835 1 B only)"""
    by = _by(S); B = base(S[0][2])
    a, b = by['backup-20260919-122446-pre-fashion'], by['backup-20260919-125813-pre-arts']
    ok = b[B + 0x23835] == a[B + 0x23835] + 1 and not bit(a, B + 0x2C77, 795) and bit(b, B + 0x2C77, 795)
    for p, q, quest in (('req-0-before-fatedfour', 'req-1-after-fatedfour', 157),
                        ('req-1-after-fatedfour', 'req-2-after-ludroth', 385),
                        ('unlock-3-pre-star-bit22-hold', 'complete-0-pre-full-completion', 308)):
        x, y = by[p], by[q]
        ok &= _qc(y) == _qc(x) + 1 and bit(y, B + 0x2C77, quest) and not bit(x, B + 0x2C77, quest)
        ok &= x[B + 0x23835] == y[B + 0x23835]
    return ok, 'Hub G4 clear: %d -> %d; 3 non-G clears (V6 #621, Hub2 #10220, V10 #1038): unchanged' % (a[B + 0x23835], b[B + 0x23835])

# ---------------------------------------------------------------- sPlayer
def chk_equipped(S):
    """equipped gear indices hit box entries of the slot's kind (weapon 7-21, head..legs 1-5, talisman 6) in all 36 saves and both fresh slots, and equal a CONFIRMED My Set's indices in every slot-1 save; 3 set loads seen in game"""
    ok, states = True, set()
    for t, n, b in S:
        slots = _slots(b)
        for s in slots:
            idx = _equipped(b, s)
            for k, i in enumerate(idx):
                if i == 0xFFFF: ok &= k == 6 and s != 1; continue
                ty = _etype(_ent(b, s, i, 0x62EE))
                ok &= (7 <= ty <= 21) if k == 0 else ty == k
        ok &= _myset(b) is not None
        states.add(tuple(_equipped(b)))
    return ok, '%d distinct gear states, all kinds match, all equal a My Set' % len(states)

def chk_weapon_class(S):
    """+0 weapon class == equipped weapon's box type - 7 in every save (14/CB, 11/DB) and in the fresh slots (1, Sword and Shield). Split: 0x23B47 1 B"""
    ok, seen = True, set()
    for t, n, b in S:
        slots = _slots(b)
        for s in slots:
            B = base(b, s); w = _equipped(b, s)[0]
            c = b[B + 0x23B47]; ty = _etype(_ent(b, s, w, 0x62EE))
            ok &= c == ty - 7; seen.add((s, c, ty))
    return ok, 'class,type pairs %s' % sorted(seen)

def chk_hunting_style(S):
    """weapon-class block +5 is the hunting style: equals the loaded My Set's style byte (5 Valor, 3 Adept) in every save, and each quest bumps exactly that style's use counter (S+0x11a); fresh slots 0 (Guild)"""
    ok, bumps = True, 0
    for t, n, b in S:
        B = base(b); r = _myset(b)
        ok &= r is not None and b[r + 0x88] == b[B + 0x23B4C]
    for (t0, n0, a), (t1, n1, b) in zip(S, S[1:]):
        B = base(b)
        ca = [u16(a, B + 0x2905 + 2 * k) for k in range(6)]; cb = [u16(b, B + 0x2905 + 2 * k) for k in range(6)]
        d = [y - x for x, y in zip(ca, cb)]
        if any(d):
            st = b[B + 0x23B4C]   # style in force at the save after the quest (req-1: set 1 loaded, then quest)
            ok &= st is not None and d[st] == _qc(b) - _qc(a) and sum(d) == d[st]; bumps += 1
    f = _by(S)['char2-2-after-scrimas3']
    ok &= f[base(f, 2) + 0x23B4C] == 0 and f[base(f, 3) + 0x23B4C] == 0
    return ok and bumps >= 5, '%d quests, each bumped the counter of the style in +5; fresh 0' % bumps

def chk_arts_block(S):
    """bytes 0-7 (3 equipped arts, SP bits) equal the loaded My Set's arts/SP byte in every save (179 with CB, 150/151 with DB), all arts unlocked in the CONFIRMED map; fresh slots 26 Round Force I + 1 with SnS. Split: 0x23A59 8 B"""
    ok, seen = True, set()
    for t, n, b in S:
        B = base(b); r = _myset(b)
        arts = [u16(b, B + 0x23A59 + 2 * k) for k in range(3)]; sp = u16(b, B + 0x23A59 + 6)
        ok &= r is not None and list(b[r + 0x89:r + 0x8C]) == arts and b[r + 0x8C] == sp
        for a in arts:
            if a:
                row = _ARTS[a]
                ok &= bit(b, B + 0x2C13 + int(row['bit_byte']), int(row['bit'])) == 1
        seen.add(tuple(arts))
    f = _by(S)['char2-2-after-scrimas3']
    for s in (2, 3):
        ok &= [u16(f, base(f, s) + 0x23A59 + 2 * k) for k in range(4)] == [26, 1, 0, 0]
    return ok, 'art states %s, all = loaded My Set, all unlocked; fresh [26,1,0,0]' % sorted(seen)

def chk_pigment(S):
    """first 20 B (5 x RGBA) equal the loaded My Set's pigment in every save; 3 different colour sets seen as sets were loaded in game. Split: 0x23B53 20 B"""
    ok, seen = True, set()
    for t, n, b in S:
        B = base(b); r = _myset(b)
        ok &= r is not None and b[r + 0x6A:r + 0x7E] == b[B + 0x23B53:B + 0x23B67]
        seen.add(b[B + 0x23B53:B + 0x23B67])
    return ok and len(seen) >= 3, '%d distinct pigments, all = loaded My Set' % len(seen)

def chk_default_flags(S):
    """bits 0-4 equal the loaded My Set's five default flags (0x1F with set 6 defaults 01x5, 0 with custom sets) in every save"""
    ok, seen = True, set()
    for t, n, b in S:
        B = base(b); r = _myset(b)
        want = sum(b[r + 0x83 + k] << k for k in range(5))
        ok &= (u16(b, B + 0x23B7B) & 0x1F) == want; seen.add(want)
    return ok and len(seen) == 2, 'flag states %s, all = loaded My Set' % sorted(seen)

def chk_hunter_name(S):
    """slot 2 becomes 'Scrimas2' exactly when created in game (char2-1) and slot 3 'Scrimas3' at char2-2; slot 1 'Scrimas' throughout"""
    by = _by(S)
    nm = lambda b, s: b[base(b, s) + 0x23B7D:base(b, s) + 0x23B9D].split(b'\0')[0]
    a, c, d = by['char2-0-pre-new-character'], by['char2-1-after-scrimas2'], by['char2-2-after-scrimas3']
    ok = nm(a, 2) == b'' and nm(c, 2) == b'Scrimas2' and nm(c, 3) == b'' and nm(d, 3) == b'Scrimas3'
    ok &= all(nm(b, 1) == b'Scrimas' for t, n, b in S)
    return ok, 'slot2 %r->%r, slot3 %r->%r' % (nm(a, 2), nm(c, 2), nm(c, 3), nm(d, 3))

# ---------------------------------------------------------------- sOtomo
def chk_palicoes(S):
    """record 2 (Suds) is the manager's buddy 1 and is the only Palico whose exp (+0x20) moves: +5150/+1440/+2000/+765 at 4 quest sessions (level byte 62->63); names/owners/greetings decode; gear indices hit Palico-box entries of the right kind"""
    ok, gains = True, []
    for a, b, n in _game_pairs(S):
        B = base(b)
        for j in range(84):
            ea, eb = u32(a, B + 0x23BB6 + PK * j + 0x20), u32(b, B + 0x23BB6 + PK * j + 0x20)
            if ea != eb:
                ok &= j == b[B + 0x23B9E] == 2 and eb > ea and _qc(b) == _qc(a) + 1; gains.append(eb - ea)
    b = S[-1][2]
    for j in range(84):
        p = _pal(b, 1, 0x23BB6, j)
        if not p[0]: continue
        for o in (0, 0x60, 0x9C): ok &= all(32 <= x < 127 for x in p[o:].split(b'\0')[0]) and p[o] != 0
        for k in range(3):
            g = u16(p, 0x100 + 2 * k)
            if g != 0xFFFF: ok &= _etype(_ent(b, 1, g, 0x17C2E)) == 22 + k
    return ok and len(gains) >= 4, 'buddy exp gains %s' % gains

def chk_palico_box(S):
    """every gear index of the 8 Palicoes hits a non-empty Palico-box entry of the slot's kind (22 weapon, 23 head, 24 body; 10 refs); box holds only types 22-24; fresh slots hold the 5 starter entries"""
    ok, refs = True, 0
    for t, n, b in S:
        types = {_etype(_ent(b, 1, i, 0x17C2E)) for i in range(1000) if any(_ent(b, 1, i, 0x17C2E))}
        ok &= types <= {22, 23, 24}
    b = S[-1][2]
    for j in range(84):
        p = _pal(b, 1, 0x23BB6, j)
        for k in range(7):
            g = u16(p, 0x100 + 2 * k)
            if p[0] and g != 0xFFFF: ok &= k < 3 and _etype(_ent(b, 1, g, 0x17C2E)) == 22 + k; refs += 1
    f = _by(S)['char2-2-after-scrimas3']
    for s in (2, 3):
        used = [_etype(_ent(f, s, i, 0x17C2E)) for i in range(1000) if any(_ent(f, s, i, 0x17C2E))]
        ok &= used == [22, 23, 24, 23, 24]
    return ok and refs >= 10,'%d gear refs all matching kind; fresh slots 5 starter entries' % refs

def chk_palico_list2(S):
    """same 324-B record: 24 names/greetings decode in all 3 slots, exp->level byte follows the main list's relation; rerolled (~1000 B) in exactly the 5 quest-clear sessions, untouched in talk/tour/tool sessions"""
    ok, rer = True, []
    for (t0, n0, a), (t1, n1, b) in zip(S, S[1:]):
        B = base(b)
        ch = a[B + 0x2A606:B + 0x2A606 + 24 * PK] != b[B + 0x2A606:B + 0x2A606 + 24 * PK]
        ok &= ch == (_qc(b) == _qc(a) + 1)
        if ch: rer.append(n1[:20])
    rel = {}
    f = _by(S)['char2-2-after-scrimas3']
    for s in (1, 2, 3):
        for lst, cnt in ((0x2A606, 24), (0x23BB6, 84)):
            for j in range(cnt):
                p = _pal(f, s, lst, j)
                if lst == 0x2A606:
                    for o in (0, 0x60): ok &= p[o] != 0 and all(32 <= x < 127 for x in p[o:].split(b'\0')[0])
                if p[0]: rel[u32(p, 0x20)] = p[0x24]
    ks = sorted(rel); ok &= all(rel[x] <= rel[y] for x, y in zip(ks, ks[1:]))
    return ok and len(rer) == 5, 'rerolled at %s; exp->level monotone over %d pairs' % (rer, len(ks))

def chk_buddy(S):
    """sOtomo +0x13849 (byte 1 of the 5-B manager field) = buddy: holds 2, and record 2 is the only Palico gaining exp after quests; fresh slots 0xFF"""
    ok, d = chk_palicoes(S)
    f = _by(S)['char2-2-after-scrimas3']
    ok &= all(b[base(b) + 0x23B9E] == 2 for t, n, b in S) and f[base(f, 2) + 0x23B9E] == 0xFF and f[base(f, 3) + 0x23B9E] == 0xFF
    return ok, 'buddy byte 2 in all saves; ' + d

# ---------------------------------------------------------------- sVillage
def chk_pet_names(S):
    """4 x char[32] decode to the game's default village pet names (Moofy = Bherna Moofah, Poogie x 3) in all 3 slots of every save"""
    want = [b'Moofy', b'Poogie', b'Poogie', b'Poogie']
    ok = True
    for t, n, b in S:
        slots = _slots(b)
        for s in slots:
            o = base(b, s) + 0x2C4E3
            ok &= [b[o + 32 * k:o + 32 * k + 32].split(b'\0')[0] for k in range(4)] == want
    return ok, 'names %s in every save/slot' % want

CHECKS = [
    (('char', 0x2248B), chk_play_time),
    (('char', 0x2248F), chk_play_rem),
    (('char', 0x22497), chk_items_obtained),
    (('char', 0x2381B), chk_g_accum),
    (('char', 0x23835), chk_staged0),          # split: 0x23835 1 B
    (('char', 0x23A59), chk_arts_block),       # split: 0x23A59 8 B
    (('char', 0x23B39), chk_equipped),
    (('char', 0x23B47), chk_weapon_class),     # split: 0x23B47 1 B
    (('char', 0x23B53), chk_pigment),          # split: 0x23B53 20 B
    (('char', 0x23B7B), chk_default_flags),
    (('char', 0x23B7D), chk_hunter_name),
    (('char', 0x23BB6), chk_palicoes),
    (('char', 0x2A606), chk_palico_list2),
    (('char', 0x17C2E), chk_palico_box),
    (('char', 0x2C4E3), chk_pet_names),
]
PROPOSED = [
    (('char', 0x23B4C), chk_hunting_style),    # new label: weapon-class block +5 = hunting style
    (('char', 0x23B9E), chk_buddy),            # split of 0x23B9D: byte 1 = hunting buddy 1
]

if __name__ == '__main__':
    S = saves()
    for (sc, o), f in CHECKS + PROPOSED:
        ok, d = f(S)
        print('%-5s %s %#x %-20s %s' % ('PASS' if ok else 'FAIL', sc, o, f.__name__, d))
