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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

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
