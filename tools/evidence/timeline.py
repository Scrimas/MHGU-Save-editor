"""The save timeline behind the CONFIRMED tags of data/save-map.csv (run.py). Read-only.

Saves, oldest first: snapshots/ (repo, never committed), the editor's own pre-write snapshots and
the live Ryujinx save. NOTES says what happened between each save and the one before it: the
in-game actions and tool writes that the checks tie the fields to. A save added later needs a note.
"""
import os, csv, bisect, struct, time
R = os.path.dirname(os.path.abspath(__file__)) + '/../../'
A = os.path.expanduser('~/.local/share/mhgu-save-editor/snapshots/0000000000000001/')
LIVE = os.path.expanduser('~/.config/Ryujinx/bis/user/save/0000000000000001/0/system')

# What happened between the previous save (time order) and this one. backup-...-pre-X is taken
# BEFORE the tool wrote X, so X's write shows up in the NEXT save. Notes marked '?' are guesses.
NOTES = {
 'backup-20260919-113617-pre-db-builds': 'oldest; editor-tool backup (09-05 state)',
 'backup-20260919-122446-pre-fashion': 'tool wrote "db builds" (equipment box / My Sets) + game played (pt +642 s)',
 'backup-20260919-125813-pre-arts': 'tool wrote "fashion" (equipment/pigment) + game played, one quest (quest counter 382->383); funds 9999039->9999999, source unknown',
 'backup-20260919-130346-pre-ingredients': 'tool wrote Hunter Arts unlocked (all 178) + game played briefly',
 'backup-20260919-130939-pre-dish76': 'tool wrote Canteen ingredients; game played',
 'capture-A': 'tool wrote Canteen dish 76 etc; game played',
 'backup-20260919-142019-pre-capfix-awardtest': 'game: one quest with a monster capture (qc 383->384)',
 'capture-B': 'identical to previous',
 'backup-20260919-143350-pre-allawards': 'tool: capture count fix (2 bytes)',
 'backup-20260919-151405-pre-questtest': 'tool wrote all awards (Guild Card award field) + game played',
 'req-0-before-fatedfour': 'game played (talks), quest seen bit',
 'req-1-after-fatedfour': 'game: villager request quest "Fated Four" done (qc +1)',
 'req-2-after-ludroth': 'game: request quest with Ludroth done (qc +1)',
 'req-3-after-captain': 'game: Captain request reported (talk; reward incl. Poogie costume, award)',
 'req-4-pre-accept-607': 'identical',
 'unlock-0-pre-flag1059': 'game played (quest 607 accepted/seen)',
 'unlock-1-pre-cleared-619-11404': 'tool wrote event flag 1059 only',
 'unlock-2-pre-rotation-618': 'tool wrote cleared bits 619, 11404 only',
 'unlock-3-pre-star-bit22-hold': 'tool wrote rotation bit; game played',
 'complete-0-pre-full-completion': 'game: one quest cleared (qc +1), Hub star 12->13',
 'complete-1-after-talks': 'tool bulk quest completion (980 cleared/seen, 63 request flags, 66 set bits) + game: one round of NPC talks, 87 reports fired (rewards), award 39',
 'complete-2-after-quest': 'game: Harvest Tour 201 (a TOUR, not counted by the quest counter); award check granted awards 0-6,10,59,74,89,103,109-114',
 'complete-3-pre-lesson-flags': 'identical',
 'char2-0-pre-new-character': 'tool wrote lesson done flags + cleared unused-slot bits',
 'char2-1-after-scrimas2': 'game: new character Scrimas2 created in slot 2 (pre-intro)',
 'char2-2-after-scrimas3': 'game: new character Scrimas3 created in slot 3 (pre-intro)',
 'app-2026-10-04_233922': 'identical to previous',
 'app-2026-10-04_234149': 'app wrote ? (size records, own card, award bit; probably the crowns goal) - check',
 'app-2026-10-04_234914': 'app wrote 13 event-flag bytes (requests)',
 'app-2026-10-04_235015': 'app wrote Wycademy points 9999999 and play time 560908 (3 copies)',
 'app-2026-10-04_235124': 'app wrote item box',
 'app-2026-10-05_124632': 'GAME SESSION after the app writes (pt +151 s, funds -960 spent in game, awards granted)',
 'app-2026-10-05_173839': 'app wrote funds 9999999 (Zenny)',
 'app-2026-10-05_173848': 'app sorted/merged item box',
 'app-2026-10-05_180822': 'app item box every stack x99',
 'confirm-0-before-tests': 'identical to app-2026-10-05_180822 + the play-time write (snapshot before the 2026-10-05 in-game tests)',
 'confirm-1-test-write': 'editor core code wrote on Scrimas: Rathian Notes page locked, HR 500, Hunter\'s Knife in box slot 557',
 'confirm-2-after-game': 'GAME SESSION: seen in game: Rathian gone from the Notes, HR 500, the knife in the box and equippable; saved',
 'confirm-3-restore-write': 'editor core code wrote HR 999 (goal hr999) and Rathian\'s Notes page (goal notes); knife kept',
 'LIVE': 'latest live save',
}

def saves():
    """[(mtime, name, bytes)] in time order: snapshots/, the app's snapshots, the live save."""
    out = []
    for n in os.listdir(R + 'snapshots'):
        p = R + 'snapshots/%s/0/system' % n; out.append((os.stat(p).st_mtime, n, p))
    for n in os.listdir(A):
        p = A + n + '/0/system'; out.append((os.stat(p).st_mtime, 'app-' + n, p))
    out.append((os.stat(LIVE).st_mtime, 'LIVE', LIVE))
    out.sort()
    return [(t, n, open(p, 'rb').read()) for t, n, p in out]

def base(b, slot=1): return 0x24 + int.from_bytes(b[0x34 + 4 * (slot - 1):0x38 + 4 * (slot - 1)], 'little')
def u8(b, o): return b[o]
def u16(b, o): return int.from_bytes(b[o:o + 2], 'little')
def u32(b, o): return int.from_bytes(b[o:o + 4], 'little')
def f32(b, o): return struct.unpack('<f', b[o:o + 4])[0]
def bit(b, o, i): return (b[o + (i >> 3)] >> (i & 7)) & 1
def when(t): return time.strftime('%m-%d %H:%M', time.localtime(t))
rows = list(csv.DictReader(open(R + 'data/save-map.csv')))
