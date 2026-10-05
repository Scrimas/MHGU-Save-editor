#!/usr/bin/env python3
"""Build the save editor's game-asset pack from a RomFS dump. Pure stdlib.

    build_assets.py [ROMFS] [OUT]
    ROMFS  plain base-game RomFS image (default scratch/base_romfs.bin)
    OUT    output folder (default app/assets/gen; gitignored, never commit it:
           it holds Capcom's icons and text)

Writes:
  names.json       item, equipment, skill and Palico support move names; item icon /
                   colour / rarity; palettes; equipment type icons; icon cell positions
                   on items.png
  items.png        the grayscale item icon sheet (HD_cmn_icon_GSM); the editor tints it
  monsters/<i>.png 72x72 icon per save monster index 1-137
  awards/<b>.png   48x48 Guild Card award icon per award bit

Where things are (all CONFIRMED visually or against a real save unless noted):
  textures   eng/GUI/99_texture/*.tex, TEX v0xA0, Tegra block-linear; RGBA8 / BC1 / BC3 / BC4
  tables     loc/arc/resident.arc table\\* (f32 version, u32 count, packed records)
  text       eng/arc/resident_eng.arc eng\\table\\*_eng (GMD, names at even indices)
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

    # items: itemData 44-B records (+4 type, +5 rarity - 1, +0x10 icon, +0x11 colour)
    tex('HD_cmn_icon_GSM_NOMIP').save(P('items.png'))
    inames = gmd(reng['eng\\table\\itemData_eng'])[0::2]
    d = res['table\\itemData']; n = struct.unpack_from('<I', d, 4)[0]
    names['items'] = [inames[i] if i < len(inames) else '' for i in range(n)]
    names['item_icons'] = {}
    for i in range(n):
        r = d[8 + i * 44:8 + i * 44 + 44]
        names['item_icons'][str(i)] = [r[0x10], r[0x11] % 17, r[5] + 1]
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
        names['weapons'][str(cls)] = [
            {'id': k, 'name': nm[3 * k], 'names': nm[3 * k:3 * k + 3], 'rarity': b[8 + k * st + 11] + 1,
             'max_lv': b[8 + k * st + 9], 'lim': b[8 + k * st + 10]} for k in range(cnt)]
    # armor: armorSeriesData 127-B records, armor ID = series index; bytes 6-10 parts present,
    # 13/14 male/female, 103 mRare; names entry 10k + part - 1
    a = res['table\\armorSeriesData']; cnt = struct.unpack_from('<I', a, 4)[0]
    an = gmd(reng['eng\\table\\armorSeriesData_eng'])
    names['armor'] = {}
    for p in range(5):
        names['armor'][str(p + 1)] = [
            {'id': k, 'name': an[10 * k + p], 'rarity': a[8 + k * 127 + 103] + 1,
             'male': a[8 + k * 127 + 13], 'female': a[8 + k * 127 + 14]}
            for k in range(cnt) if a[8 + k * 127 + 6 + p]]
    am = res['table\\amuletData']; amn = gmd(reng['eng\\table\\amuletData_eng'])[0::2]
    names['talismans'] = [{'id': k, 'name': amn[k], 'rarity': am[8 + k * 9 + 8] + 1}
                          for k in range(struct.unpack_from('<I', am, 4)[0])]
    names['skills'] = gmd(reng['eng\\table\\skillTypeData_eng'])[0::2]
    # Palico support moves: name and description pairs, move ID k -> entry 2k
    # (0 "(No Move)"; the Palico record's learned slots use 57 for none). DERIVED
    names['support_moves'] = gmd(reng['eng\\otomo\\support\\spt_act_base_eng'])[0::2]

    # Guild Card awards: bit i -> cell i, 10 x 48 px; 0-99 lby_deco, 100+ lby_deco2
    aw = [tex('HD_lby_deco_BM_MQ_NOMIP'), tex('HD_lby_deco2_BM_MQ_NOMIP')]
    for i in range(132):
        s, c = divmod(i, 100)
        aw[s].crop((c % 10) * 48, (c // 10) * 48, 48, 48).save(P('awards', '%d.png' % i))

    json.dump(names, open(P('names.json'), 'w'), ensure_ascii=False, separators=(',', ':'))
    print('asset pack ->', out)


if __name__ == '__main__':
    here = os.path.dirname(os.path.abspath(__file__))
    a = sys.argv[1:]
    if len(a) > 2 or a[:1] in (['-h'], ['--help']): sys.exit(__doc__)
    main(a[0] if a else os.path.join(here, '..', 'scratch', 'base_romfs.bin'),
         a[1] if len(a) > 1 else os.path.join(here, '..', 'app', 'assets', 'gen'))
