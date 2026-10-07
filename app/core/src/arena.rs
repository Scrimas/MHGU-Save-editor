//! Arena records (docs/11-save-map.md, "Arena log" and S fields): per Arena quest of the
//! Arena Counter's table (data/arena.csv), the own Guild Card keeps the five best times
//! with weapons, equipment set and grade; the save object keeps the counter's best time
//! with the card ID of the hunter who set it, and which sets each quest was cleared with.
//!
//!   log       own card + 0x1224, 17 x 5 u32, best first: bits 0-17 time in 1/100 s,
//!             18-25 weapons (hunter + 15 x partner, 14 = none; Prowler quests 9 and 8),
//!             26-28 set, 29-30 grade (3 = none)
//!   partners  own card + 0x16D4, 17 x 8 B: the partner's card ID of the best time
//!   best      base + 0x291F (S+0x134), 57 x (u32 time, 8-B card ID); 0-16 = the log's
//!   sets      base + 0x4FFB (S+0xca0), bit 5 x quest + set; NEW copy base + 0x5023
//!
//! DERIVED from the clear's writer (0x3b123c); the own card's one record agrees with the
//! best time and set bit of the analysed save.

use crate::character::CARD;
use crate::data::{tables, ArenaQuest};
use crate::save::Save;

pub const LOG: usize = CARD + 0x1224;
pub const PARTNERS: usize = CARD + 0x16D4;
/// The card owner's ID, which the best times name.
pub const OWN_ID: usize = CARD + 0x8B0;
pub const BEST: usize = 0x291F;
pub const SETS: usize = 0x4FFB;
pub const SETS_NEW: usize = 0x5023;
/// Times kept per quest.
pub const RANKS: usize = 5;
/// The clear caps a time at 30 minutes (0x3b1530).
pub const MAX_TIME: u32 = 180_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// 1/100 s
    pub time: u32,
    /// Guild Card weapon (storage order) or Prowler support bias; `none(q)` = none.
    pub weapon: u8,
    pub partner: u8,
    pub set: u8,
    /// 0, 1, 2; 3 = slower than every grade time
    pub grade: u8,
}

pub fn quests() -> &'static [ArenaQuest] {
    &tables().arena
}

fn quest(q: usize) -> &'static ArenaQuest {
    &quests()[q]
}

/// The weapon value that stands for none: 14 for hunters, 8 for Prowlers.
pub fn none(q: usize) -> u8 {
    if quest(q).prowler { 8 } else { 14 }
}

fn radix(q: usize) -> u32 {
    if quest(q).prowler { 9 } else { 15 }
}

/// The grade the game gives a time on quest `q`.
pub fn grade(q: usize, time: u32) -> u8 {
    quest(q).grades.iter().position(|&g| time <= 100 * g).unwrap_or(3) as u8
}

pub fn encode(q: usize, e: Entry) -> u32 {
    let w = e.weapon as u32 + radix(q) * e.partner as u32;
    e.time.min(MAX_TIME) | (w & 0xFF) << 18 | (e.set as u32 & 7) << 26 | (e.grade as u32 & 3) << 29
}

/// None for an empty rank (time 0).
pub fn decode(q: usize, v: u32) -> Option<Entry> {
    let time = v & 0x3FFFF;
    let w = v >> 18 & 0xFF;
    (time != 0).then(|| Entry {
        time,
        weapon: (w % radix(q)) as u8,
        partner: (w / radix(q) % radix(q)) as u8,
        set: (v >> 26 & 7) as u8,
        grade: (v >> 29 & 3) as u8,
    })
}

/// What the game puts in an empty rank: no time, no weapons, no grade.
pub fn empty(q: usize) -> u32 {
    encode(q, Entry { time: 0, weapon: none(q), partner: none(q), set: 0, grade: 3 })
}

pub fn best(s: &Save, base: usize, q: usize) -> Option<Entry> {
    decode(q, s.u32(base + LOG + 20 * q))
}

/// The counter's best time of quest `q` and whether the card owner set it.
pub fn counter_best(s: &Save, base: usize, q: usize) -> (u32, bool) {
    let at = base + BEST + 12 * q;
    (s.u32(at), s.get(at + 4, 8) == s.get(base + OWN_ID, 8))
}

/// Make `time` with equipment set `set` the card's best time on quest `q`, as a solo
/// clear: the set's weapon, no partner, the grade the time earns. Earlier times slower
/// than it move down, faster ones go; the counter's best follows when this beats it or was the owner's; the
/// set is marked cleared (with its NEW mark the first time).
pub fn set_best(s: &mut Save, base: usize, q: usize, time: u32, set: u8) {
    let set = set.min(4);
    let time = time.clamp(1, MAX_TIME);
    let e = Entry { time, weapon: quest(q).sets[set as usize], partner: none(q), set, grade: grade(q, time) };
    let at = base + LOG + 20 * q;
    let mut list = vec![encode(q, e)];
    list.extend((0..RANKS).map(|k| s.u32(at + 4 * k)).filter(|&v| decode(q, v).is_some_and(|o| o.time >= time)));
    list.resize(RANKS, empty(q));
    for (k, v) in list.into_iter().enumerate() {
        s.set_u32(at + 4 * k, v);
    }
    s.put(base + PARTNERS + 8 * q, &[0; 8]);
    let bit = 5 * q + set as usize;
    if !s.bit(base + SETS, bit) {
        s.set_bit(base + SETS, bit, true);
        s.set_bit(base + SETS_NEW, bit, true);
    }
    let (t, own) = counter_best(s, base, q);
    if t == 0 || time < t || own {
        let id = s.get(base + OWN_ID, 8).to_vec();
        s.set_u32(base + BEST + 12 * q, time);
        s.put(base + BEST + 12 * q + 4, &id);
    }
}

/// Empty the card's times on quest `q`, and the counter's best when it is the owner's.
/// The sets stay marked cleared.
pub fn clear(s: &mut Save, base: usize, q: usize) {
    for k in 0..RANKS {
        s.set_u32(base + LOG + 20 * q + 4 * k, empty(q));
    }
    s.put(base + PARTNERS + 8 * q, &[0; 8]);
    if counter_best(s, base, q).1 {
        s.put(base + BEST + 12 * q, &[0; 12]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

    #[test]
    fn table_has_the_counter_quests() {
        assert_eq!(quests().len(), 17);
        assert_eq!((quests()[0].quest_id, quests()[11].quest_id), (20001, 120001));
        assert!(quests()[11..].iter().all(|q| q.prowler && q.sets.iter().all(|&b| b < 8)));
    }

    #[test]
    fn entries_decode_like_the_game() {
        // the analysed save's one record: 84.85 s, Gunlance (8), set 3, grade 0, no partner
        assert_eq!(decode(0, 0x0F68_2125), Some(Entry { time: 8485, weapon: 8, partner: 14, set: 3, grade: 0 }));
        assert_eq!(encode(0, decode(0, 0x0F68_2125).unwrap()), 0x0F68_2125);
        // the game's initial values (0x161184)
        assert_eq!((empty(0), empty(11)), (0x6380_0000, 0x6140_0000));
        assert_eq!(decode(0, empty(0)), None);
    }

    #[test]
    fn grades_follow_the_times() {
        // Grudge Match: Malfestio, 300 / 600 / 780 s
        assert_eq!([grade(0, 30_000), grade(0, 30_001), grade(0, 78_000), grade(0, 78_001)], [0, 1, 2, 3]);
    }

    #[test]
    fn set_best_keeps_the_ranks_ordered() {
        let mut s = blank();
        s.put(B + OWN_ID, &[1, 2, 3, 4, 5, 6, 7, 8]);
        for k in 0..RANKS {
            s.set_u32(B + LOG + 4 * k, empty(0));
        }
        set_best(&mut s, B, 0, 20_000, 3);
        set_best(&mut s, B, 0, 15_000, 1);
        let at = |s: &Save, k: usize| decode(0, s.u32(B + LOG + 4 * k));
        assert_eq!(at(&s, 0), Some(Entry { time: 15_000, weapon: 1, partner: 14, set: 1, grade: 0 }));
        assert_eq!(at(&s, 1).map(|e| e.time), Some(20_000));
        // a slower best drops the faster ranks below it
        set_best(&mut s, B, 0, 50_000, 0);
        assert_eq!((at(&s, 0).map(|e| (e.time, e.grade)), at(&s, 1)), (Some((50_000, 1)), None));
        assert!(s.bit(B + SETS, 1) && s.bit(B + SETS_NEW, 3) && !s.bit(B + SETS, 2));
        assert_eq!(counter_best(&s, B, 0), (50_000, true));
        clear(&mut s, B, 0);
        assert_eq!((best(&s, B, 0), counter_best(&s, B, 0).0), (None, 0));
    }

    #[test]
    fn another_hunters_faster_time_stays() {
        let mut s = blank();
        s.put(B + OWN_ID, &[1; 8]);
        s.set_u32(B + BEST + 12 * 5, 10_000);
        s.put(B + BEST + 12 * 5 + 4, &[9; 8]);
        set_best(&mut s, B, 5, 20_000, 0);
        assert_eq!(counter_best(&s, B, 5), (10_000, false));
        clear(&mut s, B, 5);
        assert_eq!(counter_best(&s, B, 5), (10_000, false));
        set_best(&mut s, B, 5, 9_000, 0);
        assert_eq!(counter_best(&s, B, 5), (9_000, true));
    }
}
