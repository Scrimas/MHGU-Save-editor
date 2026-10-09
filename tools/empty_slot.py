#!/usr/bin/env python3
"""Write data/empty-slot.zlib: a character slot no character was ever made in, as the
game writes it. Pure stdlib.

    empty_slot.py SAVE SLOT     SAVE = a `system` file, SLOT = 1-3, a slot not in use

The game writes all three slots when it makes the save file; a slot without a character
holds the managers' initial state (the analysed save's slots 2 and 3 before their
characters: 108,319 non-zero bytes each, equal but for 3 bytes of Guild Card list
padding, which is uninitialised heap; docs/11-save-map.md). Making a character in game
writes only its own slot and the slot-use byte. The editor's Delete writes this image
back, so a deleted slot is what a slot never used is.
"""

import os, sys, zlib

SLOT_USED = 0x28
SLOT_PTR, SLOT_PTR_BASE = 0x34, 0x24
# the stride's last byte is alignment; slot 3 ends there
LEN = 0x11F8C4 - 1


def main(path, slot):
    b = open(path, 'rb').read()
    k = int(slot) - 1
    if b[SLOT_USED + k]:
        sys.exit(f'slot {slot} holds a character')
    base = SLOT_PTR_BASE + int.from_bytes(b[SLOT_PTR + 4 * k:SLOT_PTR + 4 * k + 4], 'little')
    img = b[base:base + LEN]
    out = os.path.join(os.path.dirname(__file__), '..', 'data', 'empty-slot.zlib')
    z = zlib.compress(img, 9)
    open(out, 'wb').write(z)
    print(f'{out}: {len(z)} bytes, {sum(1 for x in img if x)} non-zero of {LEN}')


if __name__ == '__main__':
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(*sys.argv[1:])
