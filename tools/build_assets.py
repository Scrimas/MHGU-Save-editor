#!/usr/bin/env python3
"""Build the save editor's game-asset pack from a RomFS dump. Pure stdlib.

    build_assets.py [ROMFS] [OUT]
    ROMFS  plain base-game RomFS image (default scratch/base_romfs.bin)
    OUT    output folder (default app/assets/gen; gitignored, never commit it:
           it holds Capcom's icons and text)

Writes:
  names.json       item, equipment, skill, monster and Palico support move names; Guild
                   Card title words, scenes and poses; item icon / colour / rarity and
                   pouch carry limit; palettes; equipment type icons; icon cell positions
                   on items.png
  names.<code>.json  the same names in French, German, Italian and Spanish (fr de it es),
                   from the game's own text
  items.png        the grayscale item icon sheet (HD_cmn_icon_GSM); the editor tints it
  monsters/<i>.png 72x72 icon per save monster index 1-137
  awards/<b>.png   48x48 Guild Card award icon per award bit

Where things are (all CONFIRMED visually or against a real save unless noted):
  textures   eng/GUI/99_texture/*.tex, TEX v0xA0, Tegra block-linear; RGBA8 / BC1 / BC3 / BC4
  tables     loc/arc/resident.arc table\\* (f32 version, u32 count, packed records)
  text       eng/arc/resident_eng.arc eng\\table\\*_eng (GMD, names at even indices);
             fre ger ita spa the same; monster names eng\\GUI\\06_msg\\monsterName_eng
             (variants named like their base: see HUB_VARIANTS, FATALIS_VARIANTS);
             Guild Card texts loose in <lang>/table/ (guild_card)
Tables the game builds in code (EXE, v1.4 main) are copied below with their addresses.
"""
import json, os, struct, sys, zlib

# ---------------------------------------------------------------- containers
class RomFS:
    def __init__(s, path):
        s.f = open(path, 'rb')
        h = struct.unpack('<10Q', s._rd(0, 80))
        s.dm = s._rd(h[3], h[4]); s.fm = s._rd(h[7], h[8]); s.data = h[9]
        s.files = {}
        s._walk(0, '')

    def _rd(s, o, n):
        s.f.seek(o); return s.f.read(n)

    def _walk(s, doff, path):
        _, _, child, fil, _, nl = struct.unpack_from('<6I', s.dm, doff)
        p = path + '/' + s.dm[doff + 24:doff + 24 + nl].decode() if doff else ''
        f = fil
        while f != 0xFFFFFFFF:
            _, fs, off, sz, _, nl2 = struct.unpack_from('<IIQQII', s.fm, f)
            s.files[p + '/' + s.fm[f + 32:f + 32 + nl2].decode()] = (s.data + off, sz)
            f = fs
        c = child
        while c != 0xFFFFFFFF:
            s._walk(c, p)
            c = struct.unpack_from('<I', s.dm, c + 4)[0]

    def read(s, name):
        o, n = s.files[name]; return s._rd(o, n)


def arc(d):
    """{entry name (backslash path, no extension): bytes}"""
    assert d[:4] == b'ARC\0'
    n = struct.unpack_from('<H', d, 6)[0]; out = {}
    for i in range(n):
        e = d[0xC + i * 0x50:0xC + (i + 1) * 0x50]
        name = e[:64].split(b'\0')[0].decode()
        _, cs, ds, off = struct.unpack_from('<4I', e, 64)
        raw = d[off:off + cs]
        out[name] = zlib.decompress(raw) if (cs != ds & 0x1FFFFFFF or raw[:1] == b'\x78') else raw
    return out


def gmd(d):
    assert d[:4] == b'GMD\0'
    sc, ss = struct.unpack_from('<I', d, 0x18)[0], struct.unpack_from('<I', d, 0x20)[0]
    return [x.decode('utf-8', 'replace') for x in d[len(d) - ss:].split(b'\0')[:sc]]

# ---------------------------------------------------------------- textures
def _c565(c):
    return ((c >> 11) * 255 // 31, ((c >> 5) & 63) * 255 // 63, (c & 31) * 255 // 31)

def _cols(b, four):
    c0, c1 = struct.unpack_from('<HH', b, 0); p0, p1 = _c565(c0), _c565(c1)
    if c0 > c1 or four:
        return [p0 + (255,), p1 + (255,), tuple((2 * x + y) // 3 for x, y in zip(p0, p1)) + (255,),
                tuple((x + 2 * y) // 3 for x, y in zip(p0, p1)) + (255,)]
    return [p0 + (255,), p1 + (255,), tuple((x + y) // 2 for x, y in zip(p0, p1)) + (255,), (0, 0, 0, 0)]

def _ramp(a0, a1):
    if a0 > a1: return [a0, a1] + [(a0 * (7 - i) + a1 * i) // 7 for i in range(1, 7)]
    return [a0, a1] + [(a0 * (5 - i) + a1 * i) // 5 for i in range(1, 5)] + [0, 255]

def _bc1(b):
    c = _cols(b, False); ci = struct.unpack_from('<I', b, 4)[0]
    return [c[(ci >> 2 * i) & 3] for i in range(16)]

def _bc3(b):
    al = _ramp(b[0], b[1]); ai = int.from_bytes(b[2:8], 'little')
    c = _cols(b[8:16], True); ci = struct.unpack_from('<I', b, 12)[0]
    return [c[(ci >> 2 * i) & 3][:3] + (al[(ai >> 3 * i) & 7],) for i in range(16)]

def _bc4(b):
    al = _ramp(b[0], b[1]); ai = int.from_bytes(b[2:8], 'little')
    return [(255, 255, 255, al[(ai >> 3 * i) & 7]) for i in range(16)]

def _bh(units):
    bh = 1
    while bh < 16 and bh * 8 < units: bh *= 2
    return bh

def _bl(x, y, wbytes, bpe, bh):
    """Tegra X1 block-linear address of element (x, y)."""
    gw = (wbytes + 63) // 64
    gob = (y // (8 * bh)) * 512 * bh * gw + (x * bpe // 64) * 512 * bh + (y % (8 * bh) // 8) * 512
    x *= bpe
    return gob + ((x % 64) // 32) * 256 + ((y % 8) // 2) * 64 + ((x % 32) // 16) * 32 + (y % 2) * 16 + (x % 16)

def tex_decode(d):
    """TEX v0xA0 -> Img of mip 0. d1 = u32 @8: width (bits 6-18), height (19-31); format = byte 1 of u32 @12."""
    assert d[:4] == b'TEX\0'
    d1, d2 = struct.unpack_from('<II', d, 8)
    w, h, fmt = (d1 >> 6) & 0x1FFF, d1 >> 19, (d2 >> 8) & 0xFF
    data = d[0x18:]; out = bytearray(w * h * 4)
    if fmt == 0x07:
        BH = _bh(h)
        for y in range(h):
            for x in range(w):
                o = _bl(x, y, w * 4, 4, BH); out[(y * w + x) * 4:(y * w + x) * 4 + 4] = data[o:o + 4]
    else:
        bpe, fn = {0x13: (8, _bc1), 0x17: (16, _bc3), 0x19: (8, _bc4)}[fmt]
        bw, bhb = (w + 3) // 4, (h + 3) // 4; BH = _bh(bhb)
        for by in range(bhb):
            for bx in range(bw):
                o = _bl(bx, by, bw * bpe, bpe, BH)
                for i, p in enumerate(fn(data[o:o + bpe])):
                    x, y = bx * 4 + i % 4, by * 4 + i // 4
                    if x < w and y < h: out[(y * w + x) * 4:(y * w + x) * 4 + 4] = bytes(p)
    im = Img(w, h); im.px = out; return im


class Img:
    def __init__(s, w, h):
        s.w, s.h = w, h; s.px = bytearray(w * h * 4)

    def crop(s, x, y, w, h):
        o = Img(w, h)
        for j in range(h):
            if 0 <= y + j < s.h:
                x0, x1 = max(x, 0), min(x + w, s.w)
                if x1 > x0:
                    o.px[(j * w + x0 - x) * 4:(j * w + x1 - x) * 4] = s.px[((y + j) * s.w + x0) * 4:((y + j) * s.w + x1) * 4]
        return o

    def save(s, path):
        raw = b''.join(b'\0' + bytes(s.px[y * s.w * 4:(y + 1) * s.w * 4]) for y in range(s.h))
        ch = lambda t, d: struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d))
        with open(path, 'wb') as f:
            f.write(b'\x89PNG\r\n\x1a\n' + ch(b'IHDR', struct.pack('>IIBBBBB', s.w, s.h, 8, 6, 0, 0, 0))
                    + ch(b'IDAT', zlib.compress(raw, 9)) + ch(b'IEND', b''))

# ---------------------------------------------------------------- EXE tables
# monster index - 1 -> icon id; icon // 98 = sheet (micon00 / micon01), cell icon % 98, 7 columns
# of 72 px. u32[137] at 0x162e7f8 (getter 0x560ca4, cell rect 0x560ab8).
MONSTER_ICON = list(range(105)) + [109] * 7 + [119, 120, 121, 132, 122, 41, 41, 41, 136, 123, 124, 125, 126,
                127, 128, 129, 137, 133, 135, 134, 140, 138, 139, 131, 130]
# large (48 px) copy of the item sheet: icon < 95 on a grid of 10 from (0, 416); 95-106 listed
# (param table 0x162e610, list 0x162e1a0, 3DS units doubled for HD at 0x560ba8).
ITEM_ICON_EXTRA = [(240, 848), (288, 848), (336, 848), (384, 848), (0, 896), (48, 896),
                   (288, 608), (288, 608), (288, 608), (288, 608), (432, 848), (96, 896)]
# item colour palette, mIconCol 0-16, 0xAABBGGRR (built by 0x562060, read by 0x560d3c)
PALETTE = [0xfff5f5f5, 0xff5c50e1, 0xff83bf69, 0xffffbb9f, 0xff65cff7, 0xffd1a3c3, 0xfff0df9b, 0xff659af7,
           0xff9d88f8, 0xff6dd5ad, 0xffaaaaaa, 0xff2e97c1, 0xffc7eb57, 0xff3b8955, 0xff000099, 0xff993300,
           0xff660066]
# rarity colours, index mRare = rarity - 1 (0x18994b4, read by 0x560ef0)
RARITY = [0xffe6e6e6, 0xfff292b0, 0xff5ed5de, 0xff9e8ee8, 0xff73c76f, 0xfff88e70, 0xff5a56da, 0xffac9e4c,
          0xff5d8cee, 0xffbd45ff, 0xffff007d]
# equipment box type -> item icon id (u8[25] at 0x162e7dc, 0x560c5c)
EQUIP_ICON = [46, 70, 71, 73, 72, 74, 75, 55, 57, 59, 61, 65, 63, 63, 56, 67, 62, 66, 58, 60, 68, 69, 76, 78, 79]

rgb = lambda c: [c & 255, (c >> 8) & 255, (c >> 16) & 255]


def item_rect(icon):
    if icon < 95: return (icon % 10) * 48, 416 + (icon // 10) * 48
    return ITEM_ICON_EXTRA[icon - 95] if icon - 95 < len(ITEM_ICON_EXTRA) else (288, 608)


def main(romfs, out):
    R = RomFS(romfs)
    for sub in ('monsters', 'awards'):
        os.makedirs(os.path.join(out, sub), exist_ok=True)
    P = lambda *a: os.path.join(out, *a)
    res = arc(R.read('/nativeNX/loc/arc/resident.arc'))
    reng = arc(R.read('/nativeNX/eng/arc/resident_eng.arc'))
    T = '/nativeNX/eng/GUI/99_texture/'
    tex = lambda n: tex_decode(R.read(T + n + '.tex'))
    names = {}

    # monsters
    mic = [tex('HD_cmn_micon00_BM_MQ_NOMIP'), tex('HD_cmn_micon01_BM_MQ_NOMIP')]
    for i, ic in enumerate(MONSTER_ICON):
        sh, c = divmod(ic, 98)
        mic[sh].crop((c % 7) * 72, (c // 7) * 72, 72, 72).save(P('monsters', '%d.png' % (i + 1)))

    # items: itemData 44-B records (+4 type, +5 rarity - 1, +6 pouch carry limit, +0x10 icon,
    # +0x11 colour). Carry: Potion 10, Max Potion 2, Ancient Potion 1, Pierce S Lv1 60, Crag S 9
    tex('HD_cmn_icon_GSM_NOMIP').save(P('items.png'))
    inames = gmd(reng['eng\\table\\itemData_eng'])[0::2]
    d = res['table\\itemData']; n = struct.unpack_from('<I', d, 4)[0]
    names['items'] = [inames[i] if i < len(inames) else '' for i in range(n)]
    names['item_icons'] = {}
    names['item_carry'] = []
    for i in range(n):
        r = d[8 + i * 44:8 + i * 44 + 44]
        names['item_icons'][str(i)] = [r[0x10], r[0x11] % 17, r[5] + 1]
        names['item_carry'].append(r[6])
    names['palette'] = [rgb(c) for c in PALETTE]
    names['rarity_colors'] = [[200, 200, 200]] + [rgb(c) for c in RARITY]     # index = rarity
    names['icon_rects'] = [list(item_rect(k)) for k in range(107)]
    names['icon_cell'] = 48
    names['equip_icons'] = {str(t): ic for t, ic in enumerate(EQUIP_ICON) if t}

    # weapons: box type 7 + NN uses weaponNNBaseData (NN 0-14, no 05), box ID = record index;
    # byte 9 maxLv, 10 maxLvForLimit, 11 mRare. Three names per tree (base / final / ultimate
    # form, 0x126dd8). Keyed by NN.
    names['weapons'] = {}
    for w in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14]:
        cls = w
        b = res['table\\weapon%02dBaseData' % w]; cnt = struct.unpack_from('<I', b, 4)[0]; st = (len(b) - 8) // cnt
        nm = gmd(reng['eng\\table\\weapon%02dMsgData_eng' % w])[0::2]
        # decoration slots per level: weaponNNLevelData records (stride per class), +4 weapon
        # ID, +5 level, last byte slots. DERIVED: every decorated weapon of 79 saves fits,
        # Dual Blades and Gunlance ones exactly
        lv = res['table\\weapon%02dLevelData' % w]; lc = struct.unpack_from('<I', lv, 4)[0]; ls = (len(lv) - 8) // lc
        slots = {}
        for r in (lv[8 + ls * j:8 + ls * (j + 1)] for j in range(lc)):
            slots.setdefault(r[4], {})[r[5]] = r[ls - 1]
        names['weapons'][str(cls)] = [
            {'id': k, 'name': nm[3 * k], 'names': nm[3 * k:3 * k + 3], 'rarity': b[8 + k * st + 11] + 1,
             'max_lv': b[8 + k * st + 9], 'lim': b[8 + k * st + 10],
             'level_slots': [slots.get(k, {}).get(n, 0) for n in range(1, max(slots.get(k, {0: 0})) + 1)]}
            for k in range(cnt)]
    # armor: armorSeriesData 127-B records, armor ID = series index; bytes 6-10 parts present,
    # 13/14 male/female, 103 mRare; names entry 10k + part - 1. DERIVED: 15/16 Blademaster /
    # Gunner (Hunter's Helm 1/0, Hunter's Cap 0/1, Leather 1/1); 108 + part - 1 decoration
    # slots (every decorated armor piece of 79 saves fills them exactly)
    a = res['table\\armorSeriesData']; cnt = struct.unpack_from('<I', a, 4)[0]
    an = gmd(reng['eng\\table\\armorSeriesData_eng'])
    names['armor'] = {}
    for p in range(5):
        names['armor'][str(p + 1)] = [
            {'id': k, 'name': an[10 * k + p], 'rarity': a[8 + k * 127 + 103] + 1,
             'male': a[8 + k * 127 + 13], 'female': a[8 + k * 127 + 14],
             'blade': a[8 + k * 127 + 15], 'gunner': a[8 + k * 127 + 16], 'slots': a[8 + k * 127 + 108 + p]}
            for k in range(cnt) if a[8 + k * 127 + 6 + p]]
    # decorations: decoData 5-B records [size, skill, points, skill, points], record k =
    # item 2638 + k (sizes match every "Jwl N" name). DERIVED
    dd = res['table\\decoData']
    names['decos'] = [[2638 + k, dd[8 + 5 * k]] for k in range(struct.unpack_from('<I', dd, 4)[0])
                      if names['items'][2638 + k] not in ('', 'DUMMY')]
    # Palico gear (box types 22 weapon, 23 head, 24 body): box ID = otWeaponData /
    # otArmorData record; names 2k (weapons), 4k head and 4k + 1 body (armor). DERIVED
    # (the starter box of a new save reads Bone Wedge, Acorn Helm / Mail, Bherna Staff)
    wn = gmd(reng['eng\\table\\otWeaponData_eng'])
    names['palico_weapons'] = [{'id': k, 'name': wn[2 * k]} for k in range(1, len(wn) // 2)]
    on = gmd(reng['eng\\table\\otArmorData_eng'])
    names['palico_armor'] = {str(23 + p): [{'id': k, 'name': on[4 * k + p]} for k in range(1, len(on) // 4)] for p in range(2)}
    am = res['table\\amuletData']; amn = gmd(reng['eng\\table\\amuletData_eng'])[0::2]
    names['talismans'] = [{'id': k, 'name': amn[k], 'rarity': am[8 + k * 9 + 8] + 1}
                          for k in range(struct.unpack_from('<I', am, 4)[0])]
    names['skills'] = gmd(reng['eng\\table\\skillTypeData_eng'])[0::2]
    # Palico support moves: name and description pairs, move ID k -> entry 2k
    # (0 "(No Move)"; the Palico record's learned slots use 57 for none). DERIVED
    names['support_moves'] = gmd(reng['eng\\otomo\\support\\spt_act_base_eng'])[0::2]
    names.update(guild_card(R, 'eng'))

    # Guild Card awards: bit i -> cell i, 10 x 48 px; 0-99 lby_deco, 100+ lby_deco2
    aw = [tex('HD_lby_deco_BM_MQ_NOMIP'), tex('HD_lby_deco2_BM_MQ_NOMIP')]
    for i in range(132):
        s, c = divmod(i, 100)
        aw[s].crop((c % 10) * 48, (c // 10) * 48, 48, 48).save(P('awards', '%d.png' % i))

    names['monsters'] = text(R, reng, 'eng')['monsters']
    json.dump(names, open(P('names.json'), 'w'), ensure_ascii=False, separators=(',', ':'))
    # the same names in the game's other languages, in the same order
    for code, lang in LANGS.items():
        rl = arc(R.read('/nativeNX/%s/arc/resident_%s.arc' % (lang, lang)))
        json.dump(text(R, rl, lang, names), open(P('names.%s.json' % code), 'w'), ensure_ascii=False, separators=(',', ':'))
    print('asset pack ->', out)


# Interface language code (app/gui/src/i18n.rs) -> the game's language folder.
LANGS = {'fr': 'fre', 'de': 'ger', 'it': 'ita', 'es': 'spa'}

# Variants monsterName names like their base monster. Four have their own name in the
# Hunters Hub objective list (<lang>/GUI/06_msg/NetworkVillage_<lang>.gmd), right after
# their base: save index -> (list entry, base save index). DERIVED
HUB_VARIANTS = {23: (159, 22), 36: (90, 35), 126: (151, 53), 128: (113, 60)}
# Crimson and Old Fatalis (save index 119, 120) are only named in sentences (Guild Card
# titles GC_Title_1 2411 / 2415, quest titles questData_0011457, 0090018-19); these are
# those names in the nominative singular. DERIVED
FATALIS_VARIANTS = {'eng': ('Crimson Fatalis', 'Old Fatalis'), 'fre': ('Fatalis rouge', 'Fatalis ancien'),
                    'ger': ('Karmesinroter Fatalis', 'Alter Fatalis'), 'ita': ('Fatalis cremisi', 'Fatalis antico'),
                    'spa': ('Fatalis Carmesí', 'Fatalis Ancestral')}


def guild_card(R, lang):
    """Guild Card texts of one language, by ID: gc_words (GC_Title_1, 1309 words, then
    their descriptions), gc_links (GC_Title_2, 121 words for the title's middle field),
    gc_scenes (GC_background, 136, then descriptions), gc_poses (GuildCardMsg 235-256:
    Stand … Beam Fire, 22). The counts match the unlock maps of S +0x9d0, +0xbbc, +0xbec,
    +0xc64 (docs/11-save-map.md). DERIVED"""
    g = lambda n: gmd(R.read('/nativeNX/%s/table/%s_%s.gmd' % (lang, n, lang)))
    return {'gc_words': g('GC_Title_1')[:1309], 'gc_links': g('GC_Title_2')[:121],
            'gc_scenes': g('GC_background')[:136], 'gc_poses': g('GuildCardMsg')[235:257]}


def text(R, r, lang, eng=None):
    """The names of one language from its resident_<lang>.arc `r` (and the variant monster
    names from RomFS `R`), entry for entry like the English names.json `eng` (the piece
    lists keep only the real IDs, so they follow it):
      items, skills, support_moves, monsters   lists by ID (monsters: save index - 1)
      weapons     class -> [[base, final, ultimate] per English piece]
      armor, palico_armor   part -> [name per English piece]
      palico_weapons, talismans   [name per English piece]
      gc_words, gc_links, gc_scenes, gc_poses   lists by ID (guild_card)"""
    g = lambda n: gmd(r['%s\\%s_%s' % (lang, n, lang)])
    out = {'items': g('table\\itemData')[0::2], 'skills': g('table\\skillTypeData')[0::2],
           'support_moves': g('otomo\\support\\spt_act_base')[0::2],
           'monsters': g('GUI\\06_msg\\monsterName')[:137]}
    out.update(guild_card(R, lang))
    m = out['monsters']
    hub = gmd(R.read('/nativeNX/%s/GUI/06_msg/NetworkVillage_%s.gmd' % (lang, lang)))
    for i, (e, b) in HUB_VARIANTS.items():
        assert hub[e - 1] == m[b - 1], (lang, i, hub[e - 1], m[b - 1])
        m[i - 1] = hub[e]
    m[118], m[119] = FATALIS_VARIANTS[lang]
    if eng is None:
        return out
    out['items'] = [out['items'][i] if i < len(out['items']) else '' for i in range(len(eng['items']))]
    out['weapons'] = {}
    for cls, pieces in eng['weapons'].items():
        nm = g('table\\weapon%02dMsgData' % int(cls))[0::2]
        out['weapons'][cls] = [nm[3 * p['id']:3 * p['id'] + 3] for p in pieces]
    an = g('table\\armorSeriesData')
    out['armor'] = {part: [an[10 * p['id'] + int(part) - 1] for p in pieces] for part, pieces in eng['armor'].items()}
    wn, on = g('table\\otWeaponData'), g('table\\otArmorData')
    out['palico_weapons'] = [wn[2 * p['id']] for p in eng['palico_weapons']]
    out['palico_armor'] = {part: [on[4 * p['id'] + int(part) - 23] for p in pieces] for part, pieces in eng['palico_armor'].items()}
    amn = g('table\\amuletData')[0::2]
    out['talismans'] = [amn[p['id']] for p in eng['talismans']]
    return out


if __name__ == '__main__':
    here = os.path.dirname(os.path.abspath(__file__))
    a = sys.argv[1:]
    if len(a) > 2 or a[:1] in (['-h'], ['--help']): sys.exit(__doc__)
    main(a[0] if a else os.path.join(here, '..', 'scratch', 'base_romfs.bin'),
         a[1] if len(a) > 1 else os.path.join(here, '..', 'app', 'assets', 'gen'))
