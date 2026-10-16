#!/usr/bin/env python3
"""sources.py : every CONFIRMED tag of the save map has a proof that can be followed.

    python3 tools/evidence/sources.py

A CONFIRMED entry of LABELS (tools/emu/savemap.py) rests either on the save timeline (its
check is in run.py, which needs the local saves) or on an earlier experiment written up in
docs/: SOURCES below names that section and the editor's tests that read the field. This
needs no save, so CI runs it. Exit status 1 when a CONFIRMED entry has neither, a SOURCES
entry is not a CONFIRMED entry, a section named here is gone or does not say CONFIRMED, or
a test named here no longer exists.
"""
import os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, HERE + '/../emu')
from savemap import LABELS
from run import LISTS

ROOT = HERE + '/../../'

# (scope, offset) -> (doc, heading text, tests as module::name of app/core/src)
SOURCES = {
    ('abs', 0x0): ('docs/01-container.md', 'Container header', ['store::writes_four_copies_and_keeps_headers']),
    ('abs', 0x24): ('docs/01-container.md', 'File layout', ['save::slot_bases_follow_pointers', 'save::slot_past_the_end_is_refused']),
    ('char', 0x278): ('docs/11-save-map.md', 'Item box, pouch and loadouts',
                      ['save::bit_stream_round_trip', 'transfer::item_box_round_trip_and_hand_written_lines']),
    ('char', 0x27BF): ('docs/11-save-map.md', 'Item box, pouch and loadouts', ['items::compact_pouch_keeps_one_stack_at_carry_limit']),
    ('char', 0x283C): ('docs/11-save-map.md', 'Counters and other fields', []),
    ('char', 0x2C13): ('docs/08-progression.md', 'Hunter Arts', []),
    ('char', 0x2C77): ('docs/05-quests.md', 'Quest bitmaps', ['character::card_counts_follow_cleared_quests']),
    ('char', 0x2D77): ('docs/05-quests.md', 'Quest bitmaps', []),
    ('char', 0x2F8F): ('docs/08-progression.md', 'Canteen ingredients', []),
    ('char', 0x3187): ('docs/05-quests.md', 'Quest sets', []),
    ('char', 0x504B): ('docs/05-quests.md', 'Rotating quests', []),
    ('char', 0x5EA6): ('docs/02-monster-records.md', 'Hunt tallies', ['monsters::card_entry_sums_the_family']),
    ('char', 0x5FB8): ('docs/02-monster-records.md', 'Capture counts', ['monsters::card_entry_sums_the_family']),
    ('char', 0x60CA): ('docs/02-monster-records.md', 'Size records', ['monsters::size_record_is_whole_and_ordered']),
    ('char', 0x62EE): ('docs/07-equipment.md', 'Equipment box',
                       ['transfer::equipment_box_round_trip_and_edits', 'equipment::level_and_transmog_bits', 'equipment::decorations_stay_packed']),
    ('char', 0x208CE): ('docs/07-equipment.md', 'My Sets', ['sets::my_set_round_trip', 'sets::current_and_saved_sets_agree']),
    ('char', 0x2C4DA): ('docs/05-quests.md', 'Star levels', []),
    ('char', 0x2C4DC): ('docs/05-quests.md', 'Star levels', []),
    ('char', 0x2C56D): ('docs/05-quests.md', 'Request flags', []),
    ('char', 0x2C62D): ('docs/10-npc-talk.md', 'Per-NPC bits', []),
    ('char', 0x2C67D): ('docs/08-progression.md', 'Canteen dishes', []),
    ('char', 0xC7A77): ('docs/04-weapon-usage.md', 'Verification', ['character::weapon_usage_at_the_documented_offsets']),
    ('char', 0xC7A95): ('docs/04-weapon-usage.md', 'Verification', ['character::weapon_usage_at_the_documented_offsets']),
    ('char', 0xC7AB3): ('docs/04-weapon-usage.md', 'Verification', ['character::weapon_usage_at_the_documented_offsets']),
    ('char', 0xC8115): ('docs/09-awards.md', 'Evidence', []),
    ('abs', 0xF899): ('docs/11-save-map.md', 'Downloaded quests', []),
    ('abs', 0x127899): ('docs/11-save-map.md', 'Downloaded quests', []),
}


def section(doc, heading):
    """The text of the first heading of `doc` that contains `heading`, up to the next
    heading of the same or a higher level (headings inside code blocks do not count)."""
    out, level, fence = None, 0, False
    for line in open(ROOT + doc, encoding='utf-8'):
        if line.startswith('```'):
            fence = not fence
        m = None if fence else re.match(r'(#+) ', line)
        if m and out is not None and len(m.group(1)) <= level:
            break
        if m and out is None and heading in line:
            out, level = [], len(m.group(1))
        if out is not None:
            out.append(line)
    return None if out is None else ''.join(out)


def test_exists(name):
    mod, fn = name.split('::')
    path = ROOT + 'app/core/src/%s.rs' % mod
    return os.path.exists(path) and re.search(r'\bfn %s\(' % fn, open(path, encoding='utf-8').read()) is not None


def main():
    checked = {k[:2] for lst in LISTS for k, f in lst}
    confirmed = {(sc, o): l for sc, o, n, l, c in LABELS if c == 'CONFIRMED'}
    bad = []
    for key, label in confirmed.items():
        if key not in checked and key not in SOURCES:
            bad.append('%s 0x%X (%s): CONFIRMED with no check in run.py and no entry here' % (*key, label))
    for key, (doc, heading, tests) in SOURCES.items():
        k = '%s 0x%X' % key
        if key not in confirmed:
            bad.append('%s: listed here but not a CONFIRMED entry of LABELS' % k)
        text = section(doc, heading)
        if text is None:
            bad.append('%s: no heading "%s" in %s' % (k, heading, doc))
        elif 'CONFIRMED' not in text:
            bad.append('%s: %s "%s" does not say CONFIRMED' % (k, doc, heading))
        bad += ['%s: no test %s' % (k, t) for t in tests if not test_exists(t)]
    print('%d CONFIRMED entries: %d checked on the save timeline, %d from docs/' % (
        len(confirmed), len(confirmed.keys() & checked), len(confirmed.keys() & SOURCES.keys() - checked)))
    for b in bad:
        print('FAIL', b)
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
