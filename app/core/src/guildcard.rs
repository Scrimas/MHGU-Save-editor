//! The own Guild Card's title, scene and pose, and the unlock maps they are picked from
//! (docs/11-save-map.md, "Card layout" and S fields).
//!
//! Only the card holds the choice: the card editor (`cUIOGuildCardEdit`) writes it there.
//! The title is three IDs: a word of `GC_Title_1`, a linking word of `GC_Title_2` (0 =
//! none) and a word of `GC_Title_1`. Each unlock map has a NEW copy; the game sets both
//! when it unlocks an entry. CONFIRMED in game 2026-10-09 (a written title, scene and
//! pose showed on the card; every unlocked entry was offered).

use crate::character::CARD;
use crate::save::Save;

pub const TITLE: usize = CARD + 0x854;
pub const SCENE: usize = CARD + 0x85A;
pub const POSE: usize = CARD + 0x85B;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Map {
    /// Title words (`GC_Title_1`), the title's first and last field.
    Words,
    /// Linking words (`GC_Title_2`), the title's middle field.
    Links,
    Scenes,
    Poses,
}

pub const MAPS: [Map; 4] = [Map::Words, Map::Links, Map::Scenes, Map::Poses];

impl Map {
    /// (unlocked map, its NEW copy, entries), relative to the character base.
    pub fn at(self) -> (usize, usize, usize) {
        match self {
            Map::Words => (0x2FC7, 0x306B, 1309),
            Map::Links => (0x310F, 0x311F, 121),
            Map::Scenes => (0x312F, 0x3143, 136),
            Map::Poses => (0x317F, 0x3183, 22),
        }
    }
}

pub fn unlocked(s: &Save, base: usize, m: Map, i: usize) -> bool {
    let (on, _, n) = m.at();
    i < n && s.bit(base + on, i)
}

/// Unlock entry `i` with its NEW mark, as the game does, or lock it (both cleared).
pub fn set_unlocked(s: &mut Save, base: usize, m: Map, i: usize, v: bool) {
    let (on, new, n) = m.at();
    if i < n && s.bit(base + on, i) != v {
        s.set_bit(base + on, i, v);
        s.set_bit(base + new, i, v);
    }
}

/// The title's three IDs: word, linking word, word.
pub fn title(s: &Save, base: usize) -> [u16; 3] {
    [s.u16(base + TITLE), s.u16(base + TITLE + 2), s.u16(base + TITLE + 4)]
}

pub fn set_title(s: &mut Save, base: usize, t: [u16; 3]) {
    for (k, v) in t.into_iter().enumerate() {
        s.set_u16(base + TITLE + 2 * k, v);
    }
}

pub fn scene(s: &Save, base: usize) -> u8 {
    s.u8(base + SCENE)
}

pub fn set_scene(s: &mut Save, base: usize, v: u8) {
    s.set_u8(base + SCENE, v);
}

pub fn pose(s: &Save, base: usize) -> u8 {
    s.u8(base + POSE)
}

pub fn set_pose(s: &mut Save, base: usize, v: u8) {
    s.set_u8(base + POSE, v);
}

// --- quest history log ---------------------------------------------------------------------

/// The card's history log (docs/05-quests.md, "Quest history log"): 10 records of 160
/// bytes, newest first. CONFIRMED layout (save timeline), read only.
pub const HISTORY: usize = CARD + 0x918;
pub const HISTORY_N: usize = 10;
const RECORD: usize = 0xA0;
/// Record kind the constructor leaves in a record never written.
pub const KIND_UNSET: u16 = 11;
/// Party weapon byte: a Palico; `0xFF`: nobody in that place.
pub const PARTY_PALICO: u8 = 15;

/// One history record: what happened, on which day, with whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub day: u8,
    pub month: u8,
    pub year: u16,
    /// 0 HR up, 1 MHGen transfer, 2 Welcome, 3 forged, 4 / 5 upgrade / forge counts,
    /// 6 all awards, 7 / 8 / 9 quest completed / failed / abandoned, 10 hired.
    pub kind: u16,
    /// Quest ID, or the record's value (the HR of an HR-up record).
    pub value: u16,
    /// The quest name as the game stored it (16 characters, cut with …).
    pub name: String,
    /// The party: name and weapon (`character::USE_WEAPONS` index, `PARTY_PALICO`).
    pub party: Vec<(String, u8)>,
}

/// A UTF-16LE string of at most `units` code units, up to its NUL: the bytes after a
/// short name can hold the tail of an earlier one.
fn utf16(b: &[u8], units: usize) -> String {
    let v: Vec<u16> = b.as_chunks::<2>().0.iter().take(units).map(|&c| u16::from_le_bytes(c)).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&v)
}

/// The records written so far, newest first.
pub fn history(s: &Save, base: usize) -> Vec<Record> {
    (0..HISTORY_N)
        .map(|k| s.get(base + HISTORY + RECORD * k, RECORD))
        .filter(|r| u16::from_le_bytes([r[4], r[5]]) != KIND_UNSET && r[..4] != [0; 4])
        .map(|r| Record {
            day: r[0],
            month: r[1],
            year: u16::from_le_bytes([r[2], r[3]]),
            kind: u16::from_le_bytes([r[4], r[5]]),
            value: u16::from_le_bytes([r[6], r[7]]),
            name: utf16(&r[0x08..0x28], 16),
            party: (0..4).filter(|&p| r[0x9C + p] != 0xFF).map(|p| (utf16(&r[0x44 + 22 * p..0x44 + 22 * (p + 1)], 11), r[0x9C + p])).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

    #[test]
    fn history_records_read_and_skip_unset() {
        let mut s = blank();
        let r = B + HISTORY;
        s.put(r, &[0x13, 0x09, 0xEA, 0x07, 7, 0, 0xC9, 0x00]);
        let name: Vec<u8> = "Harvest Tour".encode_utf16().flat_map(u16::to_le_bytes).collect();
        s.put(r + 8, &name);
        s.put(r + 0x44, &"Scrimas".encode_utf16().flat_map(u16::to_le_bytes).chain([0, 0, 1, 0]).collect::<Vec<u8>>());
        s.put(r + 0x44 + 22, &"Suds".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>());
        s.put(r + 0x9C, &[13, 15, 0xFF, 0xFF]);
        // record 2: never written
        s.put(r + RECORD + 4, &KIND_UNSET.to_le_bytes());
        let h = history(&s, B);
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].day, h[0].month, h[0].year, h[0].kind, h[0].value), (19, 9, 2026, 7, 201));
        assert_eq!(h[0].name, "Harvest Tour");
        assert_eq!(h[0].party, vec![("Scrimas".to_string(), 13), ("Suds".to_string(), 15)]);
    }

    #[test]
    fn unlocking_sets_new_and_locking_clears_both() {
        let mut s = blank();
        set_unlocked(&mut s, B, Map::Scenes, 135, true);
        let (on, new, _) = Map::Scenes.at();
        assert!(unlocked(&s, B, Map::Scenes, 135) && s.bit(B + new, 135));
        set_unlocked(&mut s, B, Map::Scenes, 136, true);
        assert!(!s.bit(B + on, 136), "past the map's entries");
        set_unlocked(&mut s, B, Map::Scenes, 135, false);
        assert!(!s.bit(B + on, 135) && !s.bit(B + new, 135));
    }

    #[test]
    fn maps_fit_before_the_next_field() {
        // each map and its NEW copy fit the bytes the save map gives them
        for (m, bytes) in [(Map::Words, 164), (Map::Links, 16), (Map::Scenes, 20), (Map::Poses, 4)] {
            let (on, new, n) = m.at();
            assert!(n <= 8 * bytes && new >= on + bytes, "{m:?}");
        }
    }

    #[test]
    fn title_scene_pose_round_trip() {
        let mut s = blank();
        set_title(&mut s, B, [140, 0, 502]);
        set_scene(&mut s, B, 35);
        set_pose(&mut s, B, 3);
        assert_eq!((title(&s, B), scene(&s, B), pose(&s, B)), ([140, 0, 502], 35, 3));
    }
}
