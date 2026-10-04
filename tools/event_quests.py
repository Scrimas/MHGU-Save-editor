#!/usr/bin/env python3
"""List the downloaded event and challenge quests stored in an MHGU save. Read only.

Block B of the save (loader 0x36bdc, sPrivilege) holds the quest downloads, shared by
all characters (docs/11-save-map.md):
  0x0F899   160 x 0x1C00  event quests:     u32 quest ID, u32 size, MT ARC archive
  0x127899   45 x 0x1C00  challenge quests: u32 quest ID, u32 size, MT ARC archive
  0x176499   45 x 0x800   challenge records: u32 quest ID, u32 size, raw record
ID 0 = empty slot. The ARC (zlib entries) carries the quest files: setEmMain, emSetList,
rem, supp, questPlus, questLink, and questData_<ID>_<lang> (GMD; string 0 = title).

Usage:  event_quests.py [--csv] system
"""
import sys, struct, zlib, pathlib

STORES = [('event', 0xF899, 160, 0x1C00), ('challenge', 0x127899, 45, 0x1C00), ('record', 0x176499, 45, 0x800)]


def arc_files(d):
    assert d[:4] == b'ARC\0'
    n = struct.unpack_from('<H', d, 6)[0]
    for i in range(n):
        e = d[0xC + i * 0x50:0xC + (i + 1) * 0x50]
        name = e[:64].split(b'\0')[0].decode()
        _, cs, ds, off = struct.unpack('<4I', e[64:80])
        raw = d[off:off + cs]
        yield name, zlib.decompress(raw) if cs != ds & 0x1FFFFFFF or raw[:1] == b'\x78' else raw


def gmd_first(d):
    ss = struct.unpack_from('<I', d, 0x20)[0]
    return d[len(d) - ss:].split(b'\0')[0].decode('utf-8', 'replace')


def quests(buf):
    for store, off, n, stride in STORES:
        for slot in range(n):
            o = off + slot * stride
            qid, size = struct.unpack_from('<II', buf, o)
            if not qid: continue
            title, files = '', 0
            if store != 'record':
                for name, data in arc_files(buf[o + 8:o + 8 + size]):
                    files += 1
                    if name.startswith('eng\\quest\\questData\\'): title = gmd_first(data)
            yield store, slot, o, qid, size, files, title


def main(argv):
    if not argv: sys.exit(__doc__)
    buf = pathlib.Path(argv[-1]).read_bytes()
    if '--csv' in argv:
        print('store,quest_id,title')
        seen = set()
        for store, slot, o, qid, size, files, title in sorted(quests(buf), key=lambda q: (q[0], q[3])):
            if store == 'record' or (store, qid) in seen: continue
            seen.add((store, qid))
            print(f'{store},{qid},"{title}"')
        return
    for store, slot, o, qid, size, files, title in quests(buf):
        print(f'{store:9s} slot {slot:3d}  0x{o:06X}  {qid:7d}  {size:5d} B  {files:2d} files  {title}')


if __name__ == '__main__':
    main(sys.argv[1:])
