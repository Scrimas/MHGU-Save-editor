"""savemap.py : turn the load log of runfull.py into a save map and a coverage report.

    savemap.py [FULLMAP] [SAVE]    -> data/save-map.csv, data/save-coverage.txt

Every byte or bit read of the game's loader becomes a row. Repeated reads (same loader
pc sequence at a constant stride) collapse into one array row. Rows are labelled from
LABELS (offsets relative to the character base or absolute, see docs/11-save-map.md).
"""
import os, sys, pickle, csv, collections

ROOT = os.path.dirname(os.path.abspath(__file__)) + '/../../'
FULLMAP = sys.argv[1] if len(sys.argv) > 1 else ROOT + 'scratch/fullmap.pkl'
SAVE = sys.argv[2] if len(sys.argv) > 2 else ROOT + 'scratch/work/system.ro'
OBJ, OBJ_SZ = 0x40000000, 0x1000000

# loader function -> manager. Singleton names are the MT class (DTI) name referenced in the
# same translation unit as the loader; '?' = nearest name, not certain.
MANAGERS = {
    0x52163c: 'sUserInfo (common)', 0x263298: 'sOtomo (common)', 0x2257e4: 'sBlackList', 0x16424c: 'sGuildCard (common)',
    0x3f8ef8: 'sGameControl (common)', 0x36bdc: 'sPrivilege',
    0x3e0db8: 'character header',
    0x19a4ec: 'sItemBox', 0x19e0ac: 'sItemPouch', 0x51dbb4: 'sUserInfo', 0x14b0b0: 'sEquipBox',
    0x3f8de0: 'sGameControl', 0x1979b0: 'sItem? (activity)', 0x2755f0: 'sPlayer', 0x2639ac: 'sOtomo',
    0x507d44: 'sVillage', 0x240d60: 'sNpcTalk', 0x1a4e64: 'sKitchen', 0x3f43d4: 'sFlagChecker',
    0x14c568: 'sEventCtrl?', 0x163e0c: 'sGuildCard', 0x15bd3c: 'sGuestHunter', 0x539120: 'sTutorial',
    0x1c9388: 'sMonNyan', 0x55d2c0: 'chat phrases (0x55d2c0)', 0x521398: 'sUserInfo fixup', 0x2282a8: 'tail 0x2282a8',
}

# (scope, offset, size, label, confidence); scope 'char' = relative to the character base
LABELS = [
    ('abs', 0x0, 0x24, 'Switch header (nonce at 0x14)', 'CONFIRMED'),
    ('abs', 0x24, 0x1c, 'body header: version 0xc6, 1, block A/B offsets, 3 character offsets', 'CONFIRMED'),
    ('char', 0x0, 0x278, 'character header (name +0, HR +0x28, art slots +0x2C, equipped cache +0x110, pigment +0x24C)', 'DERIVED'),
    ('char', 0x278, 0x1557, 'item box: 2300 x (u12 item ID, u7 count), bit stream', 'CONFIRMED'),
    ('char', 0x17CF, 0xFF0, 'item loadouts: 24 x 170 B (name char[42], 32 x (u16 item, u16 count))', 'DERIVED'),
    ('char', 0x27BF, 0x4C, 'item pouch: 32 x (u12 item ID, u7 count), bit stream', 'CONFIRMED'),
    ('char', 0x280B, 0x400, 'S+0x20 talk state block (contribution points +0x10/+0x20)', 'DERIVED'),
    ('char', 0x281B, 0x10, 'Village contribution points, low rank', 'DERIVED'),
    ('char', 0x282B, 0x10, 'Village contribution points, G rank', 'DERIVED'),
    ('char', 0x283C, 0x12, 'deviant permit counts', 'CONFIRMED'),
    ('char', 0x2C13, 0x18, 'Hunter Arts unlocked', 'CONFIRMED'),
    ('char', 0x2C63, 0x14, 'Palico learned maps', 'DERIVED'),
    ('char', 0x2C77, 0x100, 'quests cleared bitmap', 'CONFIRMED'),
    ('char', 0x2D77, 0x100, 'quests seen bitmap', 'CONFIRMED'),
    ('char', 0x2E77, 0x100, 'quests failed bitmap', 'DERIVED'),
    ('char', 0x2F77, 0x4, 'progress word', 'DERIVED'),
    ('char', 0x2F8F, 0x6, 'Canteen ingredients', 'CONFIRMED'),
    ('char', 0x3157, 0x14, 'awards, game-side map', 'DERIVED'),
    ('char', 0x316B, 0x14, 'award notices', 'DERIVED'),
    ('char', 0x3187, 0x10, 'quest sets completed', 'CONFIRMED'),
    ('char', 0x3197, 0x10, 'quest set notices', 'DERIVED'),
    ('char', 0x5027, 0x10, "Hunter's Notes, large monsters", 'DERIVED'),
    ('char', 0x5037, 0x10, "Hunter's Notes, new marks", 'DERIVED'),
    ('char', 0x5047, 0x4, "Hunter's Notes, second map", 'DERIVED'),
    ('char', 0x504B, 0x8, 'rotating quests bitmap', 'CONFIRMED'),
    ('char', 0x5E4E, 0x2, 'quest counter', 'DERIVED'),
    ('char', 0x5EA6, 0x112, 'monster hunt tallies, u16 index 1-137', 'CONFIRMED'),
    ('char', 0x5FB8, 0x112, 'monster capture counts, u16 index 1-137', 'CONFIRMED'),
    ('char', 0x60CA, 0x224, 'monster size records, (u16 min, u16 max) index 1-137', 'CONFIRMED'),
    ('char', 0x62EE, 0x11940, 'equipment box: 2000 x 36 B', 'CONFIRMED'),
    ('char', 0x17C2E, 0x8CA0, 'Palico equipment box: 1000 x 36 B (types 22-24)', 'DERIVED'),
    ('char', 0x208CE, 0x1540, 'My Sets: 40 x 136 B (name at +0)', 'CONFIRMED'),
    ('char', 0x21E0E, 0x660, 'Palico equipment sets: 24 x 68 B (name char[42], 3 x u16 box index)', 'DERIVED'),
    ('char', 0x2381E, 0x17, 'activity: pending village rewards', 'DERIVED'),
    ('char', 0x23A58, 0x145, 'player record (loaded copy of the slot header fields)', 'DERIVED'),
    ('char', 0x23B53, 0x24, 'current pigment, 5 x RGBA + 16 B', 'DERIVED'),
    ('char', 0x23B7B, 0x2, 'current pigment default-colour flags', 'DERIVED'),
    ('char', 0x23B7D, 0x20, 'hunter name', 'DERIVED'),
    ('char', 0x23BB6, 0x6A50, 'Palicoes: 84 x 324 B (name char[32] +0, exp u32 +0x20, level u8 +0x24, greeting +0x60, owner +0x9C)', 'DERIVED'),
    ('char', 0x2A606, 0x1E60, 'Palicoes, second list: 24 x 324 B, same record', 'DERIVED'),
    ('char', 0x2C6BD, 0x9AB00, 'Guild Card list 1: 100 elements (u32 len, zlib card, u32 state, 36 B trailer) + padding', 'DERIVED'),
    ('char', 0xC71BD, 0x18B8, 'own Guild Card, 6328 B (name UTF-16 +0, HR +0x16, equipment 7 x 44 +0x54, Palicoes 3 x 580 +0x188, weapon usage +0x8BA, history +0x918, awards +0xF58)', 'DERIVED'),
    ('char', 0xC8A75, 0x1130, 'Guild Card manager +0x18D8, 4400 B', 'UNRESOLVED'),
    ('char', 0xC9BA5, 0x4D580, 'Guild Card list 2: 50 elements + padding', 'DERIVED'),
    ('char', 0x117125, 0x708, 'Guild Card manager +0x2A0C, 1800 B', 'UNRESOLVED'),
    ('char', 0x11782D, 0x114, 'Guild Card manager +0x3114, 276 B', 'UNRESOLVED'),
    ('char', 0x117941, 0x35E8, 'Guild Card manager +0x3228, 13800 B', 'UNRESOLVED'),
    ('char', 0x11AF29, 0x708, 'Guild Card manager +0x6810, 1800 B', 'UNRESOLVED'),
    ('char', 0x11B631, 0x1868, 'guest hunters: hunters met online (UTF-16 name, greeting, hired copies)', 'DERIVED'),
    ('char', 0x11CE99, 0xA0, 'tutorial flags (8 + 152 B)', 'DERIVED'),
    ('char', 0x11CF39, 0x107, 'Meownster Hunters (sMonNyan) state', 'DERIVED'),
    ('char', 0x11D040, 0x2883, 'chat phrases: 104 B slots (auto-chat lines)', 'DERIVED'),
    ('char', 0x2C4DA, 0x2, 'Village star level', 'CONFIRMED'),
    ('char', 0x2C4DC, 0x2, 'Hub star level', 'CONFIRMED'),
    ('char', 0x2C56D, 0xC0, 'event flags (villager requests)', 'CONFIRMED'),
    ('char', 0x2C62D, 0x48, 'per-NPC talk hold bits', 'CONFIRMED'),
    ('char', 0x2C675, 0x4, 'random word (talk conditions)', 'DERIVED'),
    ('char', 0x2C67D, 0xD, 'Canteen dishes', 'CONFIRMED'),
    ('char', 0x2C68D, 0xD, 'Canteen dishes copy', 'UNRESOLVED'),
    ('char', 0xC7A77, 0x1E, 'Guild Card weapon usage, Village', 'CONFIRMED'),
    ('char', 0xC7A95, 0x1E, 'Guild Card weapon usage, Hub', 'CONFIRMED'),
    ('char', 0xC7AB3, 0x1E, 'Guild Card weapon usage, Arena', 'CONFIRMED'),
    ('char', 0xC7AD1, 0x4, 'Guild Card play time (s)', 'DERIVED'),
    ('char', 0xC7AD5, 0x640, 'quest history log, 10 x 160 B', 'DERIVED'),
    ('char', 0xC8115, 0x11, 'Guild Card awards', 'CONFIRMED'),
    ('abs', 0x40, 0xE, 'sUserInfo common header', 'UNRESOLVED'),
    ('abs', 0x4E, 0x3F48, 'Palicoes, common pool: 50 x 324 B (DLC Palicoes, owner "Capcom")', 'DERIVED'),
    ('abs', 0x3F96, 0x2580, 'blacklist: 100 x 96 B (64 + 32)', 'DERIVED'),
    ('abs', 0x6516, 0x4D8C, 'sGuildCard common: 3 x 6616 B + 1 B', 'DERIVED'),
    ('abs', 0xB2A5, 0x6C, 'sPrivilege header: bitmaps (+0xf34 .. +0xf9c)', 'UNRESOLVED'),
    ('abs', 0xB311, 0x1450, 'DLC item pack list: 50 x 104 B', 'DERIVED'),
    ('abs', 0xC761, 0x3138, 'DLC Palico info, 12600 B', 'DERIVED'),
    ('abs', 0xF899, 0x118000, 'event quests: 160 x 0x1C00 (u32 ID, u32 size, ARC)', 'CONFIRMED'),
    ('abs', 0x127899, 0x4EC00, 'challenge quests: 45 x 0x1C00 (u32 ID, u32 size, ARC)', 'CONFIRMED'),
    ('abs', 0x176499, 0x16800, 'challenge quest records: 45 x 0x800 (u32 ID, u32 size, data)', 'DERIVED'),
]


def load(path):
    d = pickle.load(open(path, 'rb'))
    return d, [r for r in d['log'] if r['k'] in ('R', 'RB')]


def key(r): return (r['k'], r['pc'], r['n'], r.get('mgr'))
def pos(r): return r['off'] * 8 + r.get('bit', 0)
def nbits(r): return r['n'] * 8 if r['k'] == 'R' else r['n']


def compress(log):
    """[(first record, records per element, stride in bits, count)]"""
    out, i, N = [], 0, len(log)
    while i < N:
        best = None
        for p in range(1, 9):
            if i + 3 * p > N: break
            ks = [key(r) for r in log[i:i + p]]
            st = pos(log[i + p]) - pos(log[i])
            if st <= 0: continue
            j = i + p
            while j + p <= N and [key(r) for r in log[j:j + p]] == ks and pos(log[j]) - pos(log[j - p]) == st:
                j += p
            cnt = (j - i) // p
            if cnt >= 3 and (best is None or cnt * p > best[2] * best[0]): best = (p, st, cnt)
        if best:
            p, st, cnt = best
            out.append((log[i:i + p], st, cnt)); i += p * cnt
        else:
            out.append(([log[i]], pos(log[i + 1]) - pos(log[i]) if i + 1 < N else nbits(log[i]), 1)); i += 1
    return out


def objfield(r):
    d = r.get('dst')
    if d is None or not (OBJ <= d < OBJ + 160 * OBJ_SZ): return ''
    return 'obj%d+0x%x' % ((d - OBJ) // OBJ_SZ, (d - OBJ) % OBJ_SZ)


def main():
    save = open(SAVE, 'rb').read()
    d, log = load(FULLMAP)
    bases = [0x24 + int.from_bytes(save[0x34 + 4 * i:0x38 + 4 * i], 'little') for i in range(3)]
    a_off = 0x24 + int.from_bytes(save[0x2c:0x30], 'little'); b_off = 0x24 + int.from_bytes(save[0x30:0x34], 'little')

    def block(off):
        for i in (2, 1, 0):
            if off >= bases[i]: return 'char%d' % (i + 1), off - bases[i]
        if off >= b_off: return 'B', off - b_off
        if off >= a_off: return 'A', off - a_off
        return 'header', off

    labs = []
    for scope, o, n, lab, conf in LABELS:
        for b in (bases if scope == 'char' else [0]):
            labs.append((b + o, b + o + n, lab, conf))
    labs.sort()

    def label(lo, hi):
        """smallest label that contains the row, else the labels inside it"""
        hits = [(a, b, l, c) for a, b, l, c in labs if a < hi and lo < b]
        cont = [h for h in hits if h[0] <= lo and hi <= h[1]]
        if cont:
            a, b, l, c = min(cont, key=lambda h: h[1] - h[0])
            return l, c
        inside = [h for h in hits if lo <= h[0]]
        if inside:
            return 'contains: ' + '; '.join(h[2] for h in inside), min((h[3] for h in inside), key=['UNRESOLVED', 'DERIVED', 'CONFIRMED'].index)
        return '', ''

    rows = []
    for recs, st, cnt in compress(log):
        r0 = recs[0]
        start = pos(r0)
        if cnt > 1: end = start + st * cnt
        else: end = start + nbits(r0)
        lo, hi = start // 8, (end + 7) // 8
        blk, rel = block(lo)
        lab, conf = label(lo, hi)
        fields = ' '.join(('%dB' % r['n']) if r['k'] == 'R' else ('%db' % r['n']) for r in recs)
        rows.append({'offset': '0x%X' % lo, 'bit': start % 8, 'size': hi - lo, 'block': blk, 'rel': '0x%X' % rel,
                     'manager': MANAGERS.get(r0.get('mgr'), r0.get('mgr') and hex(r0['mgr'])),
                     'count': cnt, 'stride_bits': st if cnt > 1 else '', 'element': fields,
                     'loader_pc': '0x%x' % r0['pc'], 'object_field': objfield(r0), 'label': lab, 'confidence': conf})
    # slots 2 and 3 are read by the same chain: check they repeat slot 1, then write slot 1 only.
    # The two Guild Card lists hold zlib elements of variable length (fixed total size): not compared.
    var = [(0x2C6BD, 0xC71BD), (0xC9BA5, 0x117125)]
    fixed = lambda r: not any(a <= int(r['rel'], 16) < b for a, b in var)
    shape = lambda c: [(r['rel'], r['bit'], r['size'], r['manager'], r['count'], r['element']) for r in rows if r['block'] == c and fixed(r)]
    same = shape('char1') == shape('char2') == shape('char3')
    print('character slots 2 and 3 repeat the layout of slot 1:', same)
    if same:
        rows = [r for r in rows if r['block'] not in ('char2', 'char3')]
    with open(ROOT + 'data/save-map.csv', 'w', newline='') as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader(); w.writerows(rows)

    cov = d['cov']
    gaps, i = [], 0
    while i < len(save):
        if cov[i]: i += 1; continue
        j = i
        while j < len(save) and not cov[j]: j += 1
        nz = sum(1 for b in save[i:j] if b)
        gaps.append((i, j, nz)); i = j
    with open(ROOT + 'data/save-coverage.txt', 'w') as f:
        read = sum(1 for b in cov if b)
        f.write('bytes read by the loader: %d of %d (%.1f%%)\n' % (read, len(save), 100 * read / len(save)))
        f.write('unread ranges (start, end, size, non-zero bytes in the analysed save, block):\n')
        for a, b, nz in gaps:
            if b - a >= 1:
                f.write('0x%07X 0x%07X %8d %8d %s+0x%X\n' % ((a, b, b - a, nz) + block(a)))
    print(len(rows), 'rows;', len(gaps), 'unread ranges')


if __name__ == '__main__':
    main()
