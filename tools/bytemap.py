"""Byte-level coverage of the save from data/save-fields.csv. Read-only.

Expands the field map (structs embedded with @name), checks it (overlaps, sizes) and writes
data/save-unmapped.md: every byte range that is not CONFIRMED or DERIVED, i.e. fields marked U
(located, meaning UNRESOLVED) and gaps no field covers, with the values the save timeline holds.

    python3 tools/bytemap.py            # write data/save-unmapped.md
    python3 tools/bytemap.py --check    # only print the summary and problems
"""
import csv, hashlib, os, sys
import numpy as np

R = os.path.dirname(os.path.abspath(__file__)) + '/../'
sys.path.insert(0, R + 'tools/evidence')
FILE_SIZE = 5159100
RANK = {'U': 1, 'D': 2, 'C': 3}
NAME = {0: 'gap', 1: 'U', 2: 'D', 3: 'C'}


def num(s, d=None):
    s = (s or '').strip()
    return d if s == '' else int(s, 0)


def load(path=R + 'data/save-fields.csv'):
    defs = {}
    lines = [l for l in open(path, encoding='utf-8') if l.strip() and not l.startswith('#')]
    for i, r in enumerate(csv.DictReader(lines)):
        size = num(r['size'])
        row = dict(scope=r['scope'], off=num(r['offset']), size=size, count=num(r['count'], 1),
                   stride=num(r['stride'], size), conf=r['conf'].strip(), field=r['field'].strip(), n=i + 2)
        row['embed'] = row['field'][1:].split()[0] if row['field'].startswith('@') else None
        defs.setdefault(row['scope'], []).append(row)
    return defs


def extent(defs, scope):
    """Size of a struct: the largest size it is embedded with."""
    return max(r['size'] for rows in defs.values() for r in rows if r['embed'] == scope)


def instances(defs):
    """{scope: [(abs offset, limit)]} by expanding the embeds from the file scope."""
    out = {'file': [(0, FILE_SIZE)]}
    todo = ['file']
    while todo:
        s = todo.pop()
        for base, lim in out[s]:
            for r in defs[s]:
                if not r['embed']:
                    continue
                for k in range(r['count']):
                    o = r['off'] + k * r['stride']
                    if o >= lim:
                        break
                    out.setdefault(r['embed'], []).append((base + o, min(r['size'], lim - o)))
                if r['embed'] not in todo:
                    todo.append(r['embed'])
    return out


def scope_items(defs, scope, size):
    """Items of one scope: its U rows and its gaps, relative offsets; plus overlap problems."""
    cover = np.zeros(size, np.int16)
    items, problems = [], []
    for r in defs[scope]:
        for k in range(r['count']):
            o = r['off'] + k * r['stride']
            if o >= size:
                break
            e = min(o + r['size'], size)
            if 'overlay' not in r['field']:
                if cover[o:e].any():
                    problems.append('%s line %d: %s +0x%X overlaps another field' % (scope, r['n'], r['field'][:50], o))
                cover[o:e] += 1
        if r['conf'] == 'U':
            items.append(dict(kind='U', off=r['off'], size=r['size'], count=r['count'], stride=r['stride'], label=r['field']))
        elif r['conf'] not in RANK:
            problems.append('%s line %d: bad conf %r' % (scope, r['n'], r['conf']))
    free = np.flatnonzero(cover == 0)
    if len(free):
        runs = np.split(free, np.flatnonzero(np.diff(free) != 1) + 1)
        for run in runs:
            items.append(dict(kind='gap', off=int(run[0]), size=len(run), count=1, stride=len(run), label='no field'))
    items.sort(key=lambda it: it['off'])
    return items, problems


def status_map(defs, inst):
    """Per-byte status of the whole file (0 gap, 1 U, 2 D, 3 C); inner fields override embeds."""
    st = np.zeros(FILE_SIZE, np.uint8)
    order = ['file'] + [s for s in inst if s != 'file']
    depth = {'file': 0}
    changed = True
    while changed:
        changed = False
        for s, rows in defs.items():
            for r in rows:
                if r['embed'] and s in depth and depth.get(r['embed'], -1) < depth[s] + 1:
                    depth[r['embed']] = depth[s] + 1
                    changed = True
    for s in sorted(order, key=lambda s: depth.get(s, 0)):
        for base, lim in inst.get(s, []):
            for r in defs[s]:
                for k in range(r['count']):
                    o = r['off'] + k * r['stride']
                    if o >= lim:
                        break
                    e = min(o + r['size'], lim)
                    if r['embed']:
                        continue  # the struct's own rows set these bytes
                    st[base + o:base + e] = RANK[r['conf']]
    return st


def ranges(st, value):
    idx = np.flatnonzero(st == value)
    if not len(idx):
        return []
    runs = np.split(idx, np.flatnonzero(np.diff(idx) != 1) + 1)
    return [(int(r[0]), len(r)) for r in runs]


def item_spans(item, inst_list):
    for base, lim in inst_list:
        for k in range(item['count']):
            o = item['off'] + k * item['stride']
            if o >= lim:
                break
            yield base + o, min(item['size'], lim - o)


def main():
    check = '--check' in sys.argv
    defs = load()
    inst = instances(defs)
    sizes = {s: (FILE_SIZE if s == 'file' else extent(defs, s)) for s in defs}
    problems = ['struct %s is never embedded' % s for s in defs if s not in inst]
    all_items = {}
    for s in defs:
        items, probs = scope_items(defs, s, sizes[s])
        # a gap of the file scope inside an embedded slot is not a gap
        all_items[s] = items
        problems += probs
    st = status_map(defs, inst)
    tot = {NAME[v]: int((st == v).sum()) for v in NAME}
    print('bytes: C %(C)d  D %(D)d  U %(U)d  gap %(gap)d' % tot)
    for p in problems:
        print('PROBLEM', p)
    if check:
        return
    from timeline import saves
    tl = saves()
    live = np.frombuffer(tl[-1][2], np.uint8)
    arrs = [np.frombuffer(b, np.uint8) for _, _, b in tl]

    out = ['# Unmapped bytes of the save',
           '',
           'Generated by `tools/bytemap.py` from [`save-fields.csv`](save-fields.csv); do not edit by hand.',
           'A byte is **mapped** when a field of `save-fields.csv` covers it with confidence C (CONFIRMED)',
           'or D (DERIVED). Everything else is listed here: fields marked **U** (located, meaning',
           'UNRESOLVED) and **gaps** (no field). The work is finished when this file lists nothing.',
           '',
           'Offsets are relative to their scope: `file` = absolute, `slot` = character base (slot 1 base',
           '`0x18CC9C`, slot 2 `0x2AC560`, slot 3 `0x3CBE24`), a struct name = start of that record.',
           'For each item: instances = how many copies the file holds (all slots, all records);',
           '`nz` = instances with a non-zero byte in the live save; `timeline` = distinct contents',
           'over the %d saves of the timeline (1 = never changed); `sample` = bytes of the first' % len(tl),
           'non-zero instance in the live save (first 24 bytes), its absolute offset in brackets.',
           '',
           '## Totals (whole file, %d bytes)' % FILE_SIZE,
           '',
           '| Status | Bytes | Share |', '|---|---|---|']
    for k in ('C', 'D', 'U', 'gap'):
        out.append('| %s | %d | %.3f%% |' % (k, tot[k], 100.0 * tot[k] / FILE_SIZE))
    out.append('')
    n_items = sum(len(v) for v in all_items.values())
    out.append('Items to resolve: %d (in %d scopes).' % (n_items, sum(1 for v in all_items.values() if v)))
    out.append('')
    order = ['file', 'slot'] + sorted(s for s in defs if s not in ('file', 'slot'))
    for s in order:
        items = all_items[s]
        if not items:
            continue
        il = inst.get(s, [])
        out.append('## Scope `%s` (%d B, %d instance%s)' % (s, sizes[s], len(il), '' if len(il) == 1 else 's'))
        out.append('')
        for it in items:
            spans = list(item_spans(it, il))
            nbytes = sum(n for _, n in spans)
            nz = [(a, n) for a, n in spans if live[a:a + n].any()]
            h = set()
            for arr in arrs:
                m = hashlib.md5()
                for a, n in spans:
                    m.update(arr[a:a + n].tobytes())
                h.add(m.digest())
            rep = '+0x%X' % it['off'] if s != 'file' else '0x%X' % it['off']
            shape = '%d B' % it['size'] if it['count'] == 1 else '%d x %d B (stride %d)' % (it['count'], it['size'], it['stride'])
            line = '- `%s` %s, **%s** %s; instances %d (%d B), nz %d, timeline %d' % (
                rep, shape, it['kind'], it['label'], len(spans), nbytes, len(nz), len(h))
            if nz:
                a, n = nz[0]
                line += ', sample [0x%X] `%s`' % (a, live[a:a + min(n, 24)].tobytes().hex(' '))
            out.append(line)
        out.append('')
    open(R + 'data/save-unmapped.md', 'w').write('\n'.join(out))
    print('wrote data/save-unmapped.md, %d items' % n_items)


if __name__ == '__main__':
    main()
