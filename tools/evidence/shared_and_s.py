"""Evidence checks: block A/B shared data, character header, S up to base+0x62EE.

CHECKS          : LABELS entries (key = (scope, offset)) CONFIRMED by the check.
SPLIT_CHECKS    : sub-fields of the S+0x20 block (funds, Wycademy points).
CORRECTED_CHECKS: entries relabelled because the data contradicted the code reading.
Game text (title words, scenes) comes from scratch/gtxt.pkl or the RomFS dump in scratch/.
"""
import os, sys, csv, json, re, pickle
from timeline import *

NAMES = json.load(open(R + 'app/assets/gen/names.json'))['items']


def _by(S):
    return {n: b for t, n, b in S}


def _bits(b, o, n):
    return {i for i in range(n * 8) if bit(b, o, i)}


def _title_text():
    """GC_Title_1 / GC_background (eng) from the romfs; cached in scratch/ (game text, never committed)."""
    p = R + 'scratch/gtxt.pkl'
    if os.path.exists(p):
        T = pickle.load(open(p, 'rb'))
        k1, k2 = '/nativeNX/eng/table/GC_Title_1_eng.gmd', '/nativeNX/eng/table/GC_background_eng.gmd'
        if k1 in T and k2 in T:
            return T[k1], T[k2]
    sys.path.insert(0, R + 'scratch/py')
    from romfs import RomFS
    from gmd import gmd_strings
    r = RomFS(R + 'scratch/base_romfs.bin')
    return (gmd_strings(r.read('/nativeNX/eng/table/GC_Title_1_eng.gmd')),
            gmd_strings(r.read('/nativeNX/eng/table/GC_background_eng.gmd')))


def _pouch(b, slot=1):
    o = base(b, slot) + 0x27BF; n = int.from_bytes(b[o:o + 76], 'little')
    return [((n >> (19 * i)) & 0xFFF, (n >> (19 * i + 12)) & 0x7F) for i in range(32)]


# ---------------------------------------------------------------- character header

def chk_header(S):
    """Slot header = summary of live state: names of the two characters created in game (Scrimas2/3); equipped-cache entries = the CONFIRMED box entries at the equipped indices (types weapon, head..legs, talisman); pigment = the CONFIRMED My Set wearing the same gear; art slots in the CONFIRMED Hunter Arts map (fresh: Round Force I + Absolute Evasion); play time follows the app write 560908 and the game's +151 s"""
    D = _by(S); live = S[-1][2]
    names = [live[base(live, s):base(live, s) + 32].rstrip(b'\0') for s in (1, 2, 3)]
    ok = names == [b'Scrimas', b'Scrimas2', b'Scrimas3']
    ncache = nset = nsets = 0
    for t, n, b in S:
        for s in ((1, 2, 3) if n == 'LIVE' else (1,)):
            B = base(b, s)
            gear = [u16(b, B + 0x23B39 + 2 * k) for k in range(7)]
            for k, g in enumerate(gear):
                if g == 0xFFFF: continue
                c = bytearray(b[B + 0x110 + 44 * k:B + 0x110 + 44 * k + 36])
                e = b[B + 0x62EE + 36 * g:B + 0x62EE + 36 * g + 36]
                c[1] &= 0x7F                       # bit 15 of the type word: a cache-only flag
                ok &= bytes(c) == e and (e[0] & 31) == ([None, 1, 2, 3, 4, 5, 6][k] or (e[0] & 31))
                ok &= k == 0 or (e[0] & 31) == k   # armor pieces in order head..legs, talisman 6
                ncache += 1
            if s == 1:                              # My Set with the same gear has the same pigment
                for m in range(40):
                    M = B + 0x208CE + 136 * m
                    if [u16(b, M + 0x2A + 2 * k) for k in range(7)] == gear:
                        nsets += 1
                        if b[M + 0x64:M + 0x64 + 20] == b[B + 0x24C:B + 0x24C + 20]: nset += 1
            arts = [u16(b, B + 0x2C + 2 * i) for i in range(3)]
            ok &= all(bit(b, B + 0x2C13, a) for a in arts if a)
    ok &= nsets > 0 and nset == nsets
    fa = [u16(live, base(live, 2) + 0x2C + 2 * i) for i in range(3)]
    ok &= fa == [26, 1, 0] and live[base(live, 2) + 0x240] == 1      # SnS start: Round Force I, Absolute Evasion
    pt = [u32(D[n], base(D[n]) + 0x20) for n in ('app-2026-10-04_235124', 'app-2026-10-05_124632')]
    ok &= pt == [560908, 561059] and all(u32(b, base(b) + 0x20) == u32(b, base(b) + 0x2248B) for t, n, b in S)
    ok &= all(u32(b, base(b) + 0x24) == u32(b, base(b) + 0x280F) for t, n, b in S)
    return ok, 'names %s; %d cache entries = box entries; %d/%d My Set pigment matches; fresh arts %s; play time %s' % (
        names, ncache, nset, nsets, fa, pt)


def chk_loadouts(S):
    """Item loadout 1 ('Set 01') is the CONFIRMED pouch slot for slot: its 20 items (valid names, counts) sit in the same pouch slots in all 36 saves, the pouch being equal to it except for a few extra stacks in empty slots in the six oldest saves (gathered material); loadouts 2-24 and both fresh slots are '---' and empty"""
    ok = True; det = ''; equal = 0
    for t, n, b in S:
        for s in ((1, 2, 3) if n == 'LIVE' else (1,)):
            L = base(b, s) + 0x17CF
            ent = [b[L + 170 * k:L + 170 * (k + 1)] for k in range(24)]
            its = [[(u16(e, 0x2A + 4 * i), u16(e, 0x2C + 4 * i)) for i in range(32)] for e in ent]
            p = _pouch(b, s)
            if s == 1:
                ok &= ent[0][:42].rstrip(b'\0') == b'Set 01'
                ok &= all(a == q for a, q in zip(its[0], p) if a[0]) and all(not a[0] for a, q in zip(its[0], p) if a != q)
                equal += its[0] == p
                ok &= all(e[:42].rstrip(b'\0') == b'---' and not any(x for x, c in it) for e, it in zip(ent[1:], its[1:]))
                det = '%d items, first %s x%d' % (sum(1 for x, c in its[0] if x), NAMES[its[0][0][0]], its[0][0][1])
            else:
                ok &= all(e[:42].rstrip(b'\0') == b'---' for e in ent) and not any(x for x, c in p)
    ok &= equal == len(S) - 6
    return ok, 'loadout 1 items in the same pouch slots in all saves, identical in %d of %d: %s' % (equal, len(S), det)


# ---------------------------------------------------------------- S+0x20 block

def chk_contrib_low(S):
    """Low-rank contribution points: tool-era 9,999,999 snapped to exactly 20000 (the adder's cap) when the game added points: Kokoto at req-2 (Kokoto request 39 'Ahoy! Royal Ludroth!'), Pokke and Yukumo at complete-1 (87 request reports); the activity counter at base+0x2381A rose at the same steps (+20, +50, +150)"""
    D = _by(S)
    v = lambda n: [u32(D[n], base(D[n]) + 0x281B + 4 * i) for i in range(4)]
    c = lambda n: D[n][base(D[n]) + 0x2381A]
    M = 9999999
    ok = v('req-1-after-fatedfour') == [20000, M, M, M] and v('req-2-after-ludroth') == [20000, 20000, M, M]
    ok &= v('complete-0-pre-full-completion') == [20000, 20000, M, M] and v('complete-1-after-talks') == [20000] * 4
    ok &= [c(n) for n in ('req-1-after-fatedfour', 'req-2-after-ludroth', 'req-3-after-captain', 'complete-1-after-talks')] == [22, 42, 92, 242]
    l = S[-1][2]
    ok &= all(u32(l, base(l, s) + 0x281B + 4 * i) == 0 for s in (2, 3) for i in range(4))
    return ok, 'req-1 %s -> req-2 %s -> complete-1 %s; counter 22->42->92->242' % (
        v('req-1-after-fatedfour'), v('req-2-after-ludroth'), v('complete-1-after-talks'))


def chk_contrib_g(S):
    """G-rank contribution points rose by 550/250/400/550 in the complete-1 talk session (87 request reports); the total +1750 is exactly what the G-rank activity counter at base+0x2381B (every 800 -> tickets) moved: 755 + 1750 = 2505 = 105 mod 800; unchanged at the five quest steps"""
    D = _by(S)
    a, c = D['complete-0-pre-full-completion'], D['complete-1-after-talks']
    va = [u32(a, base(a) + 0x282B + 4 * i) for i in range(4)]; vc = [u32(c, base(c) + 0x282B + 4 * i) for i in range(4)]
    ca, cc = u16(a, base(a) + 0x2381B), u16(c, base(c) + 0x2381B)
    d = sum(vc) - sum(va)
    ok = va == [280, 240, 125, 110] and vc == [830, 490, 525, 660] and (ca + d) % 800 == cc
    ok &= all([u32(b, base(b) + 0x282B + 4 * i) for i in range(4)] == va for t, n, b in S[:S.index(next(s for s in S if s[1] == 'complete-1-after-talks'))])
    return ok, '%s -> %s (+%d); counter %d -> %d' % (va, vc, d, ca, cc)


# ---------------------------------------------------------------- S maps

def chk_awards_game(S):
    """Game-side award map equals the CONFIRMED Guild Card award copy (73 bits) before any tool write; award 130 granted in game appears here first, then in the card copy; the tool's write of the card copy did not touch it; fresh slots hold only award 101 in both (granted at creation)"""
    D = _by(S)
    g = lambda n, s=1: _bits(D[n], base(D[n], s) + 0x3157, 20)
    c = lambda n, s=1: _bits(D[n], base(D[n], s) + 0xC8115, 17)
    f = 'backup-20260919-113617-pre-db-builds'
    ok = g(f) == c(f) and len(g(f)) == 73
    ok &= g('backup-20260919-130939-pre-dish76') - g('backup-20260919-130346-pre-ingredients') == {130}
    ok &= 130 in c('capture-A')
    ok &= g('backup-20260919-151405-pre-questtest') == g('backup-20260919-143350-pre-allawards')  # tool wrote the card copy only
    ok &= len(c('backup-20260919-151405-pre-questtest')) == 131
    ok &= g('LIVE', 2) == c('LIVE', 2) == g('LIVE', 3) == {101}
    lag = [n for t, n, b in S if not _bits(b, base(b) + 0xC8115, 17) >= _bits(b, base(b) + 0x3157, 20)]
    ok &= lag == ['backup-20260919-130939-pre-dish76']   # card copy rebuilt one save later
    return ok, 'first save G == card (73); +130 in game then card; fresh {101}'


def chk_award_notices(S):
    """Award notices get exactly the awards the game newly grants (130; 34; 83; 39; the 18 after the Harvest Tour) and are emptied in later game sessions (38 -> 0, 21 -> 0); the tool's/app's writes of awards do not set them; fresh slots: {101}, the creation award"""
    g = lambda b: _bits(b, base(b) + 0x3157, 20)
    nt = lambda b: _bits(b, base(b) + 0x316B, 20)
    grants = clears = 0; ok = True
    for (t0, n0, a), (t1, n1, b) in zip(S, S[1:]):
        new = g(b) - g(a)
        if n1 in ('app-2026-10-05_124632', 'app-2026-10-04_234149'):
            continue                                # cleared in that session / app write
        if new:
            ok &= nt(b) - nt(a) == new; grants += 1
        elif nt(b) != nt(a):
            ok &= not nt(b) and n1 == 'backup-20260919-151405-pre-questtest'; clears += 1
    D = _by(S)
    ok &= not nt(D['app-2026-10-05_124632']) and len(nt(D['app-2026-10-04_235124'])) == 21
    ok &= 100 not in nt(D['app-2026-10-04_234149'])
    l = S[-1][2]
    ok &= _bits(l, base(l, 2) + 0x316B, 20) == {101}
    return ok, '%d game grant steps mirrored, cleared at 151405 and 10-05' % grants


def chk_poogie(S):
    """Poogie costumes: the only change at req-3 (Captain request 10220 reported, costume received in game) is bit 14; +4 bits at the complete-1 report session; never otherwise; CONFIRMED award 68 (Poogie Ball, >=10 of bits 6-39 w/o 28,35,36,39) is held with 19-23 qualifying bits"""
    D = _by(S)
    u = lambda n: _bits(D[n], base(D[n]) + 0x2F9F, 8)
    ok = u('req-3-after-captain') - u('req-2-after-ludroth') == {14}
    ok &= u('complete-1-after-talks') - u('complete-0-pre-full-completion') == {4, 15, 22, 25}
    ch = [n for (t0, n0, a), (t1, n, b) in zip(S, S[1:]) if a[base(a) + 0x2F9F:base(a) + 0x2FA7] != b[base(b) + 0x2F9F:base(b) + 0x2FA7]]
    ok &= ch == ['req-3-after-captain', 'complete-1-after-talks']
    q = [i for i in u('LIVE') if 6 <= i <= 39 and i not in (28, 35, 36, 39)]
    ok &= len(q) >= 10 and bit(D['LIVE'], base(D['LIVE']) + 0xC8115, 68) == 1
    return ok, 'changes only at %s; qualifying %d' % (ch, len(q))


def chk_title1(S):
    """Title words part 1: fresh slots hold exactly the 79 words whose game text says 'Available from the start.'; the game's 10-05 session unlocked 17 words whose descriptions are the completion feats of the bulk quest completion ('Completed all 1* and 2* Village Quests', ...)"""
    t1, bg = _title_text()
    D = _by(S); l = S[-1][2]
    start = {i for i in range(1309) if 'from the start' in t1[1309 + i]}
    ok = _bits(l, base(l, 2) + 0x2FC7, 164) == start == _bits(l, base(l, 3) + 0x2FC7, 164)
    ok &= all(_bits(b, base(b) + 0x2FC7, 164) >= start for t, n, b in S)
    a, b = D['app-2026-10-04_235124'], D['app-2026-10-05_124632']
    new = _bits(b, base(b) + 0x2FC7, 164) - _bits(a, base(a) + 0x2FC7, 164)
    feats = [i for i in new if 'Completed' in t1[1309 + i]]
    ok &= len(new) == 17 and len(feats) >= 14 and {124, 125, 126} <= new
    return ok, 'fresh = %d start words; 10-05 +%d words, %d "Completed ..."' % (len(start), len(new), len(feats))


def chk_title1_new(S):
    """Title words part 1 NEW: gained exactly the newly unlocked words both times (+98 at capture-A, +17 in the 10-05 session), always a subset of the unlocked map, equal to it in the fresh slots, 553 of 867 in the played slot"""
    ok = True; steps = []
    for (t0, n0, a), (t1_, n1, b) in zip(S, S[1:]):
        ua, ub = _bits(a, base(a) + 0x2FC7, 164), _bits(b, base(b) + 0x2FC7, 164)
        na, nb = _bits(a, base(a) + 0x306B, 164), _bits(b, base(b) + 0x306B, 164)
        ok &= nb <= ub
        if ub != ua or nb != na:
            ok &= (ub - ua) == (nb - na) and na <= nb; steps.append((n1, len(ub - ua)))
    l = S[-1][2]
    ok &= all(_bits(l, base(l, s) + 0x306B, 164) == _bits(l, base(l, s) + 0x2FC7, 164) for s in (2, 3))
    ok &= [s[1] for s in steps] == [98, 17]
    return ok, 'steps %s; slot 1 NEW %d of %d' % (steps, len(_bits(l, base(l) + 0x306B, 164)), len(_bits(l, base(l) + 0x2FC7, 164)))


def chk_scenes(S):
    """Guild Card scenes: fresh slots hold exactly the 5 scenes whose game text says 'Available from the start.' (Rath-of-Meow, Courier Catnap, World Warrior, Wycademy Emblem, Bherna Emblem); the played slot has 120 incl. those and the own card's scene 35"""
    t1, bg = _title_text()
    l = S[-1][2]
    start = {i for i in range(136) if 'from the start' in bg[136 + i]}
    ok = _bits(l, base(l, 2) + 0x312F, 20) == start == _bits(l, base(l, 3) + 0x312F, 20) and len(start) == 5
    u = _bits(l, base(l) + 0x312F, 20)
    ok &= u >= start and l[base(l) + 0xC71BD + 0x85A] in u and max(u) < 136
    return ok, 'fresh %s; slot 1 %d' % (sorted(start), len(u))


def _kind1(b):
    rq = [r for r in csv.DictReader(open(R + 'data/request-index.csv')) if r['kind'] == '1']
    B = base(b)
    acc = {j for j, r in enumerate(rq) if bit(b, B + 0x2C56D, int(r['accept_flag']))}
    done = {j for j, r in enumerate(rq) if bit(b, B + 0x2C56D, int(r['done_flag']))}
    return acc, done, len(rq)


def chk_delivery_offered(S):
    """Trader delivery requests offered: bit j = j-th kind-1 (delivery) request of data/request-index.csv accepted (CONFIRMED event flags): {0,2,4..9} from the start; bits 10, 11 set by the game exactly when requests 120 and 146 were accepted in the complete-1 talk session"""
    D = _by(S)
    res = []
    for n in ('backup-20260919-113617-pre-db-builds', 'complete-0-pre-full-completion', 'complete-1-after-talks'):
        b = D[n]; acc, done, k = _kind1(b)
        res.append(_bits(b, base(b) + 0x32A7, 4) == acc)
    a = D['complete-0-pre-full-completion']; b = D['complete-1-after-talks']
    new = _bits(b, base(b) + 0x32A7, 4) - _bits(a, base(a) + 0x32A7, 4)
    ok = all(res) and new == {10, 11}
    return ok, 'offered == accepted at 3 points %s; complete-1 +%s' % (res, sorted(new))


def chk_delivery_done(S):
    """Delivery requests delivered: bits {0,4,5,7} = exactly the kind-1 requests whose CONFIRMED done flag the game had set (23, 74, 83, 92), counted from 0 in request-index order; the app's later flag write (234914) did not set more (they were not delivered)"""
    D = _by(S)
    b = D['backup-20260919-113617-pre-db-builds']; acc, done, k = _kind1(b)
    d = _bits(b, base(b) + 0x32AF, 4)
    ok = d == done == {0, 4, 5, 7} and k == 13
    ok &= all(_bits(x, base(x) + 0x32AF, 4) == d for t, n, x in S)
    return ok, 'delivered %s == done %s' % (sorted(d), sorted(done))


def chk_quest_counter(S):
    """Quest counter +1 at each of the five quest steps (382 -> 387), unchanged at the Harvest Tour (complete-2), talk sessions and tool writes; 0 in the fresh slots"""
    qc = [u32(b, base(b) + 0x5E4E) for t, n, b in S]
    steps = [S[i][1] for i in range(1, len(S)) if qc[i] != qc[i - 1]]
    exp = ['backup-20260919-125813-pre-arts', 'backup-20260919-142019-pre-capfix-awardtest',
           'req-1-after-fatedfour', 'req-2-after-ludroth', 'complete-0-pre-full-completion']
    ok = steps == exp and all(qc[i] - qc[i - 1] in (0, 1) for i in range(1, len(S))) and qc[0] == 382 and qc[-1] == 387
    l = S[-1][2]
    ok &= u32(l, base(l, 2) + 0x5E4E) == 0 == u32(l, base(l, 3) + 0x5E4E)
    return ok, '%d -> %d, steps %s' % (qc[0], qc[-1], len(steps))


# ---------------------------------------------------------------- shared data

def chk_challenge_records(S):
    """Challenge quest records: 40 records (u32 ID, u32 size 169/942 <= 0x7F8); the IDs are exactly the 40 challenge quests of the CONFIRMED archive store 0x127899 (that store is packed, the records leave slot 25 = 1020020 empty)"""
    b = S[-1][2]
    rec = [(u32(b, 0x176499 + 0x800 * i), u32(b, 0x176499 + 0x800 * i + 4)) for i in range(45)]
    arc = {u32(b, 0x127899 + 0x1C00 * i) for i in range(45)} - {0}
    ids = [i for i, s in rec if i]
    ok = set(ids) == arc and len(ids) == 40 and all(0 < s <= 0x800 - 8 for i, s in rec if i)
    ok &= all(not any(b[0x176499 + 0x800 * k:0x176499 + 0x800 * (k + 1)]) for k, (i, s) in enumerate(rec) if not i)
    ok &= len({s[2][0x176499:0x176499 + 92160] for s in S}) == 1
    return ok, '%d records, sizes %s' % (len(ids), sorted({s for i, s in rec if i}))


def chk_dlc_item_list(S):
    """DLC item pack list: 17 records (entries 32-48) of 104 B: pack name ('Starter Pack', 'Guild Provisions', 'Trick or Treat Pack', ...), u32 index 0-16 at +0x28, u16 catalog 539-555 at +0x2C (= 507 + entry, the catalog range of sPrivilege +0xf3c), then (u16 item, u16 count) pairs that name real items, counts 1-99"""
    b = S[-1][2]
    ok = True; used = []
    for i in range(50):
        r = b[0xB311 + 104 * i:0xB311 + 104 * (i + 1)]
        if all(x in (0, 0xFF) for x in r): continue
        used.append(i)
        nm = r[:40].split(b'\0')[0].decode('utf-8', 'replace')
        pairs = [(u16(r, 0x2E + 4 * k), u16(r, 0x30 + 4 * k)) for k in range(14)]
        pairs = [p for p in pairs if p[0]]
        ok &= 'Pack' in nm or 'Provisions' in nm
        ok &= u32(r, 0x28) == i - 32 and u16(r, 0x2C) == 507 + i
        ok &= len(pairs) > 0 and all(0 < it < len(NAMES) and NAMES[it] and 0 < c <= 99 for it, c in pairs)
    ok &= used == list(range(32, 49))
    return ok, 'entries %d-%d decode (names, catalog 539-555, items)' % (used[0], used[-1])


def chk_dlc_palico_info(S):
    """DLC Palico info: entries 15-17 (stride 252) are the DLC Palicoes Ranger 2, Dahdrai, Araujo with owner 'Capcom' and greeting 'Happy Hunting!' - the same names at the same indices as the shared Palico pool at 0x4E; all other entries empty"""
    b = S[-1][2]
    used = [i for i in range(50) if any(b[0xC761 + 252 * i:0xC761 + 252 * (i + 1)])]
    nm = [b[0xC761 + 252 * i:0xC761 + 252 * i + 32].split(b'\0')[0] for i in used]
    pool = [b[0x4E + 324 * i:0x4E + 324 * i + 32].split(b'\0')[0] for i in used]
    ok = used == [15, 16, 17] and nm == [b'Ranger 2', b'Dahdrai', b'Araujo'] and nm == pool
    ok &= all(b'Capcom' in b[0xC761 + 252 * i:0xC761 + 252 * (i + 1)] for i in used)
    return ok, 'entries %s = %s, pool agrees' % (used, nm)


def chk_palico_pool(S):
    """Shared Palico pool: 5 records at stride 324 decode with name +0, greeting ~+95, owner +156 ('Capcom' x4, "Calicap'n"); records 15-17 are the DLC Palicoes of the DLC Palico info list (same names, same indices); unused records are filler"""
    b = S[-1][2]
    rec = [b[0x4E + 324 * i:0x4E + 324 * (i + 1)] for i in range(50)]
    used = [i for i, r in enumerate(rec) if r[:1] not in (b'\0', b'\xff') and re.match(rb'[ -~]{4,}', r)]
    own = [rec[i][156:188].split(b'\0')[0] for i in used]
    gre = [bool(re.search(rb'[A-Za-z][ -~]{6,}', rec[i][90:156])) for i in used]
    ok = used == [0, 1, 15, 16, 17] and own.count(b'Capcom') == 4 and all(gre)
    ok &= [rec[i][:32].split(b'\0')[0] for i in (15, 16, 17)] == [b[0xC761 + 252 * i:0xC761 + 252 * i + 32].split(b'\0')[0] for i in (15, 16, 17)]
    return ok, 'records %s owners %s' % (used, own)


CHECKS = [
    (('char', 0x0), chk_header),
    (('char', 0x17CF), chk_loadouts),
    (('char', 0x281B), chk_contrib_low),
    (('char', 0x282B), chk_contrib_g),
    (('char', 0x3157), chk_awards_game),
    (('char', 0x316B), chk_award_notices),
    (('char', 0x2F9F), chk_poogie),
    (('char', 0x2FC7), chk_title1),
    (('char', 0x306B), chk_title1_new),
    (('char', 0x312F), chk_scenes),
    (('char', 0x32A7), chk_delivery_offered),
    (('char', 0x32AF), chk_delivery_done),
    (('char', 0x5E4E), chk_quest_counter),
    (('abs', 0x176499), chk_challenge_records),
    (('abs', 0xB311), chk_dlc_item_list),
    (('abs', 0xC761), chk_dlc_palico_info),
    (('abs', 0x4E), chk_palico_pool),
]


# ---------------------------------------------------------------- split labels of ('char', 0x280B)

def chk_funds(S):
    """Funds u32 base+0x280F: the quest of the 125813 step raised 9,999,039 to exactly the cap 9,999,999; the 10-05 game session spent 960 (9,999,999 -> 9,999,039); the game-built slot header money (+0x24) equals it in every save"""
    v = [u32(b, base(b) + 0x280F) for t, n, b in S]
    ch = [(S[i][1], v[i - 1], v[i]) for i in range(1, len(S)) if v[i] != v[i - 1]]
    ok = ch == [('backup-20260919-125813-pre-arts', 9999039, 9999999), ('app-2026-10-05_124632', 9999999, 9999039),
                ('app-2026-10-05_173839', 9999039, 9999999)]
    ok &= all(u32(b, base(b) + 0x24) == u32(b, base(b) + 0x280F) for t, n, b in S)
    return ok, str(ch)


def chk_wycademy(S):
    """Wycademy points u32 base+0x2817: +4110, +540, +3720, +1200, +2640 at exactly the five quest steps (quest counter +1), unchanged at talks, tool writes and the Harvest Tour; app wrote 9,999,999 and the 10-05 session kept it at the cap"""
    w = [u32(b, base(b) + 0x2817) for t, n, b in S]
    q = [u32(b, base(b) + 0x5E4E) for t, n, b in S]
    st = [(S[i][1], w[i] - w[i - 1]) for i in range(1, len(S)) if w[i] != w[i - 1]]
    qs = [S[i][1] for i in range(1, len(S)) if q[i] != q[i - 1]]
    ok = [s[0] for s in st[:5]] == qs and all(d > 0 for n, d in st[:5])
    ok &= st[5:] == [('app-2026-10-04_235015', 9999999 - 9793209)] and w[-1] == 9999999
    return ok, str(st)


SPLIT_CHECKS = [(('char', 0x280F), chk_funds), (('char', 0x2817), chk_wycademy)]


# ---------------------------------------------------------------- corrected wording

def chk_dlc_palico_bits(S):
    """CORRECTED: 0xB2A5 (sPrivilege +0xf34) = DLC Palicoes received: bits {15,16,17} = the occupied entries of the DLC Palico info 0xC761 and the DLC records of the Palico pool (Ranger 2, Dahdrai, Araujo); item pack entries 15-17 are empty"""
    b = S[-1][2]
    bits_ = _bits(b, 0xB2A5, 8)
    pal = {i for i in range(50) if any(b[0xC761 + 252 * i:0xC761 + 252 * (i + 1)])}
    items = {i for i in range(50) if any(x not in (0, 0xFF) for x in b[0xB311 + 104 * i:0xB311 + 104 * (i + 1)])}
    return bits_ == pal == {15, 16, 17} and not (bits_ & items), 'bits %s, Palico info %s' % (sorted(bits_), sorted(pal))


def chk_dlc_item_bits(S):
    """CORRECTED: 0xB2AD (sPrivilege +0xf3c) = DLC item packs received: bits {32..48} = the 17 occupied item pack records (catalog 539-555 = 507 + bit); the per-character map base+0x32DB (taken from the Room Service, paired bit for bit with +0xf3c) holds the same 17 bits, so it records item packs collected"""
    b = S[-1][2]
    bits_ = _bits(b, 0xB2AD, 8)
    items = {i for i in range(50) if any(x not in (0, 0xFF) for x in b[0xB311 + 104 * i:0xB311 + 104 * (i + 1)])}
    taken = _bits(b, base(b) + 0x32DB, 8)
    return bits_ == items == taken == set(range(32, 49)), 'bits %d-%d == pack records == taken' % (min(bits_), max(bits_))


def chk_challenge_bits(S):
    """CORRECTED: 0xB2F1 bit i = record slot i of 0x176499 in use (40 bits, slot 25 clear, bit 40 set); it does NOT follow the packed archive store 0x127899 (archive slot 25 = 1020021, slot 40 empty)"""
    b = S[-1][2]
    rec = [u32(b, 0x176499 + 0x800 * i) for i in range(45)]
    arc = [u32(b, 0x127899 + 0x1C00 * i) for i in range(45)]
    cb = [bit(b, 0xB2F1, i) for i in range(64)]
    ok = all(cb[i] == (rec[i] != 0) for i in range(45)) and not any(cb[45:])
    mism = [i for i in range(45) if cb[i] != (arc[i] != 0)]
    return ok and mism == [25, 40], 'matches records; differs from archive slots at %s' % mism


CORRECTED_CHECKS = [(('abs', 0xB2A5), chk_dlc_palico_bits), (('abs', 0xB2AD), chk_dlc_item_bits),
                    (('char', 0x32DB), chk_dlc_item_bits), (('abs', 0xB2F1), chk_challenge_bits)]


if __name__ == '__main__':
    S = saves()
    for name, lst in (('CHECKS', CHECKS), ('SPLIT_CHECKS', SPLIT_CHECKS), ('CORRECTED_CHECKS', CORRECTED_CHECKS)):
        print('==', name)
        for (sc, o), f in lst:
            ok, d = f(S)
            print('%-5s %-4s 0x%-6X %-24s %s' % ('PASS' if ok else 'FAIL', sc, o, f.__name__, d))
