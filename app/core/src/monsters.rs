//! Monster records (docs/02-monster-records.md). Index 1-137; index 0 is not a slot.
//!
//!   hunts     u16 at base + 0x5EA4 + 2i   (slain)
//!   captures  u16 at base + 0x5FB6 + 2i
//!   size      (u16 min %, u16 max %) at base + 0x60C6 + 4i; (0, 0) = no record
//!   Hunter's Notes  bit k of base + 0x32B7 (16 B), NEW copy at base + 0x32C7
//!   Guild Card monster log  own card + 0xF6C, 87 x 8 B, rebuilt by the game from the
//!   above (0x161ac8); the crown awards read it, so the editor rebuilds it too.
//!
//! Per-monster thresholds, families and positions: data/monster-sizes.csv.

use crate::data::{tables, MonsterMeta};
use crate::save::Save;

pub const HUNTS: usize = 0x5EA4;
pub const CAPTURES: usize = 0x5FB6;
pub const SIZES: usize = 0x60C6;
pub const NOTES: usize = 0x32B7;
pub const NOTES_NEW: usize = 0x32C7;
pub const CARD_LOG: usize = 0xC71BD + 0xF6C;
pub const N: usize = 137;
/// The game's increments cap at 9999 (0x526d94).
pub const MAX_COUNT: u16 = 9999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Record {
    pub hunts: u16,
    pub captures: u16,
    pub min: u16,
    pub max: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Crowns {
    pub mini: bool,
    /// 0 none, 1 silver, 2 gold
    pub large: u8,
}

impl Crowns {
    pub fn label(self) -> String {
        let l = match self.large {
            2 => "gold crown",
            1 => "silver crown",
            _ => "",
        };
        match (self.mini, l.is_empty()) {
            (true, true) => "mini crown".into(),
            (true, false) => format!("mini + {l}"),
            (false, _) => l.into(),
        }
    }
}

fn check(i: usize) {
    assert!((1..=N).contains(&i), "monster index {i} out of 1-{N}");
}

pub fn meta(i: usize) -> &'static MonsterMeta {
    check(i);
    &tables().monster_meta[i - 1]
}

pub fn get(s: &Save, base: usize, i: usize) -> Record {
    check(i);
    Record {
        hunts: s.u16(base + HUNTS + 2 * i),
        captures: s.u16(base + CAPTURES + 2 * i),
        min: s.u16(base + SIZES + 4 * i),
        max: s.u16(base + SIZES + 4 * i + 2),
    }
}

/// Writes the record; size words only for monsters the game keeps a size record for
/// (folded variants and small monsters have none). Rebuilds the Guild Card log.
pub fn set(s: &mut Save, base: usize, i: usize, r: Record) {
    check(i);
    s.set_u16(base + HUNTS + 2 * i, r.hunts.min(MAX_COUNT));
    s.set_u16(base + CAPTURES + 2 * i, r.captures.min(MAX_COUNT));
    if meta(i).size_record {
        // the game fills both on the first hunt: one size alone stands for both
        let (mut lo, mut hi) = match (r.min, r.max) {
            (0, x) | (x, 0) => (x, x),
            p => p,
        };
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        s.set_u16(base + SIZES + 4 * i, lo);
        s.set_u16(base + SIZES + 4 * i + 2, hi);
    }
    sync_card(s, base, i);
}

/// The game's crown rule (0x67310): mini from the smallest size, silver/gold from the
/// largest. Fixed-size monsters have all thresholds 0, so any record shows gold.
pub fn crowns(i: usize, r: Record) -> Crowns {
    let m = meta(i);
    if !m.size_record || (r.min == 0 && r.max == 0) {
        return Crowns::default();
    }
    if m.fixed_size {
        return Crowns { mini: false, large: if r.max > 0 { 2 } else { 0 } };
    }
    Crowns {
        mini: r.min > 0 && r.min <= m.mini_le,
        large: if r.max >= m.gold_ge { 2 } else if r.max >= m.silver_ge { 1 } else { 0 },
    }
}

/// Size records that give the crown: (min, max) to write for a mini / gold crown.
pub fn crown_sizes(i: usize) -> Option<(u16, u16)> {
    let m = meta(i);
    if !m.size_record {
        return None;
    }
    if m.fixed_size { Some((100, 100)) } else { Some((m.mini_le, m.gold_ge)) }
}

pub fn notes(s: &Save, base: usize, i: usize) -> Option<bool> {
    meta(i).notes_bit.map(|b| s.bit(base + NOTES, b))
}

/// Unlock or lock the Hunter's Notes page. Unlocking also sets the NEW mark, as the
/// game's talk action does; locking clears both.
pub fn set_notes(s: &mut Save, base: usize, i: usize, v: bool) {
    if let Some(b) = meta(i).notes_bit {
        s.set_bit(base + NOTES, b, v);
        s.set_bit(base + NOTES_NEW, b, v);
        sync_card(s, base, i);
    }
}

/// The card entry of a monster's family, as the game's rebuild 0x161ac8 writes it:
/// u16 max, u16 min (own record), u32 family hunts (14 bits) | family captures << 14
/// | Hunter's Notes << 28 | silver 1 / gold 2 << 29 | mini << 31.
pub fn card_entry(s: &Save, base: usize, head: usize) -> [u8; 8] {
    let r = get(s, base, head);
    let fam: Vec<usize> = std::iter::once(head).chain(tables().monster_meta.iter().filter(|m| m.family_of == Some(head)).map(|m| m.index)).collect();
    let sum = |f: fn(&Record) -> u16| fam.iter().map(|&k| f(&get(s, base, k)) as u32).sum::<u32>().min(MAX_COUNT as u32);
    let c = crowns(head, r);
    let w = sum(|r| r.hunts)
        | sum(|r| r.captures) << 14
        | (notes(s, base, head).unwrap_or(false) as u32) << 28
        | (c.large as u32) << 29
        | (c.mini as u32) << 31;
    let mut e = [0u8; 8];
    e[0..2].copy_from_slice(&r.max.to_le_bytes());
    e[2..4].copy_from_slice(&r.min.to_le_bytes());
    e[4..8].copy_from_slice(&w.to_le_bytes());
    e
}

/// Rewrite the card entry that monster `i` contributes to (its family head's).
pub fn sync_card(s: &mut Save, base: usize, i: usize) {
    let head = meta(i).family_of.unwrap_or(i);
    if let Some(pos) = meta(head).card_pos {
        let e = card_entry(s, base, head);
        s.put(base + CARD_LOG + 8 * pos, &e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

    #[test]
    fn size_record_is_whole_and_ordered() {
        let mut s = blank();
        set(&mut s, B, 1, Record { min: 0, max: 110, ..Default::default() });
        assert_eq!((get(&s, B, 1).min, get(&s, B, 1).max), (110, 110));
        set(&mut s, B, 1, Record { min: 120, max: 95, ..Default::default() });
        assert_eq!((get(&s, B, 1).min, get(&s, B, 1).max), (95, 120));
        set(&mut s, B, 1, Record::default());
        assert_eq!((get(&s, B, 1).min, get(&s, B, 1).max), (0, 0));
        set(&mut s, B, 1, Record { hunts: 60000, ..Default::default() });
        assert_eq!(get(&s, B, 1).hunts, MAX_COUNT);
    }

    #[test]
    fn crowns_follow_the_thresholds() {
        let m = meta(1);
        let r = |min, max| Record { min, max, ..Default::default() };
        assert_eq!(crowns(1, r(m.mini_le, m.gold_ge)), Crowns { mini: true, large: 2 });
        assert_eq!(crowns(1, r(m.mini_le + 1, m.silver_ge)), Crowns { mini: false, large: 1 });
        assert_eq!(crowns(1, r(100, m.silver_ge - 1)), Crowns::default());
        let (lo, hi) = crown_sizes(1).unwrap();
        assert_eq!(crowns(1, r(lo, hi)), Crowns { mini: true, large: 2 });
    }

    #[test]
    fn card_entry_sums_the_family() {
        let head = tables().monster_meta.iter().find(|m| m.family_of.is_some()).and_then(|m| m.family_of).unwrap();
        let member = tables().monster_meta.iter().find(|m| m.family_of == Some(head)).unwrap().index;
        let mut s = blank();
        set(&mut s, B, head, Record { hunts: 5, captures: 1, ..Default::default() });
        set(&mut s, B, member, Record { hunts: 7, captures: 2, ..Default::default() });
        let w = u32::from_le_bytes(card_entry(&s, B, head)[4..8].try_into().unwrap());
        assert_eq!((w & 0x3FFF, w >> 14 & 0x3FFF), (12, 3));
        if let Some(pos) = meta(head).card_pos {
            assert_eq!(s.get(B + CARD_LOG + 8 * pos, 8), card_entry(&s, B, head));
        }
    }
}
