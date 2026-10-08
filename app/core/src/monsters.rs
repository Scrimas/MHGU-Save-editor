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

/// The smallest and largest size the game's quests give monster `i` and its folded
/// variants (data/quest-sizes.csv); None for a monster no quest has.
pub fn quest_range(i: usize) -> Option<(u16, u16)> {
    check(i);
    let t = tables();
    std::iter::once(i)
        .chain(t.monster_meta.iter().filter(|m| m.family_of == Some(i)).map(|m| m.index))
        .filter_map(|j| t.quest_sizes.get(&j).copied())
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
}

/// One quest's chances (0-1) to give monster `i` a crown size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrownOdds {
    pub quest_id: u32,
    /// The quest's size % (the largest if `i` comes more than once).
    pub size: u16,
    pub mini: f32,
    /// Silver or gold.
    pub silver: f32,
    pub gold: f32,
}

/// Crown chances per quest for monster `i` and its folded variants, from each boss entry's
/// size % and variation table (data/quest-monsters.csv, data/size-variation.csv). The size
/// rolled is taken as size % x a rate of the table, rounded down: the game's rounding is
/// not checked. Entries of one quest count as separate chances. Quests in quest ID order;
/// empty for a monster without a size record or with a fixed size.
pub fn crown_odds(i: usize) -> Vec<CrownOdds> {
    let m = meta(i);
    if !m.size_record || m.fixed_size {
        return Vec::new();
    }
    let t = tables();
    let fam = |j: usize| j == i || meta(j).family_of == Some(i);
    let mut out: Vec<CrownOdds> = Vec::new();
    for &(quest_id, j, size, table) in &t.quest_monsters {
        if !fam(j) {
            continue;
        }
        let Some((mini, silver, gold)) = entry_odds(j, size, table) else { continue };
        let any = |a: f32, b: f32| 1.0 - (1.0 - a) * (1.0 - b);
        match out.last_mut() {
            Some(o) if o.quest_id == quest_id => {
                *o = CrownOdds { quest_id, size: o.size.max(size), mini: any(o.mini, mini), silver: any(o.silver, silver), gold: any(o.gold, gold) }
            }
            _ => out.push(CrownOdds { quest_id, size, mini, silver, gold }),
        }
    }
    out
}

/// Chances (mini, silver or gold, gold) that a boss entry of monster `j` with size %
/// `size` and variation table `table` is crown size, by the thresholds of the monster
/// keeping its record (`j` or its family head; see `crown_odds`). None when that monster
/// has no size record or a fixed size.
pub fn entry_odds(j: usize, size: u16, table: usize) -> Option<(f32, f32, f32)> {
    let m = meta(meta(j).family_of.unwrap_or(j));
    if !m.size_record || m.fixed_size {
        return None;
    }
    let p = |hit: &dyn Fn(u32) -> bool| {
        tables().size_variation[table].iter().filter(|&&(r, _)| hit(size as u32 * r as u32 / 100)).map(|&(_, c)| c as f32).sum::<f32>() / 100.0
    };
    Some((p(&|v| v <= m.mini_le as u32), p(&|v| v >= m.silver_ge as u32), p(&|v| v >= m.gold_ge as u32)))
}

/// A boss entry of a quest: its monster and size %, the sizes its variation table can
/// roll, and the crown chances (`entry_odds`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestMonster {
    pub monster: usize,
    pub size: u16,
    pub min: u16,
    pub max: u16,
    pub odds: Option<(f32, f32, f32)>,
}

/// The boss entries with a size of quest `quest_id`, in the quest's order.
pub fn quest_monsters(quest_id: u32) -> Vec<QuestMonster> {
    let t = tables();
    t.quest_monsters
        .iter()
        .filter(|e| e.0 == quest_id)
        .map(|&(_, j, size, table)| {
            let rates = t.size_variation[table].iter().map(|&(r, _)| size as u32 * r as u32 / 100);
            QuestMonster {
                monster: j,
                size,
                min: rates.clone().min().unwrap_or(0) as u16,
                max: rates.max().unwrap_or(0) as u16,
                odds: entry_odds(j, size, table),
            }
        })
        .collect()
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
    fn crown_sizes_are_within_the_quests() {
        for i in (1..=N).filter(|&i| meta(i).size_record) {
            let (Some((lo, hi)), Some((a, b))) = (crown_sizes(i), quest_range(i)) else { continue };
            assert!(a <= lo && hi <= b, "monster {i}: crowns {lo}-{hi}, quests {a}-{b}");
        }
        // Raging Brachydios (126, always 100) folds into Brachydios (53)
        assert_eq!(quest_range(126), Some((100, 100)));
        assert_eq!(quest_range(53), Some((88, 125)));
    }

    #[test]
    fn crown_odds_cover_every_crown() {
        // Rathalos: gold in 45 quests, at best 23 % (Paint It Gold)
        let o = crown_odds(4);
        assert_eq!(o.iter().filter(|q| q.gold > 0.0).count(), 45);
        assert!((o.iter().map(|q| q.gold).fold(0.0, f32::max) - 0.23).abs() < 1e-4);
        assert!(o.iter().all(|q| q.gold <= q.silver));
        // each crown that a quest can give shows up with a chance
        for i in (1..=N).filter(|&i| meta(i).size_record && !meta(i).fixed_size) {
            let (Some((lo, hi)), Some((a, b))) = (crown_sizes(i), quest_range(i)) else { continue };
            let o = crown_odds(i);
            assert_eq!(o.iter().any(|q| q.mini > 0.0), a <= lo, "monster {i} mini");
            assert_eq!(o.iter().any(|q| q.gold > 0.0), hi <= b, "monster {i} gold");
        }
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
