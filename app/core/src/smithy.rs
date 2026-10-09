//! Smithy lists (docs/11-save-map.md, S fields): the create-table entries the Smithy has
//! listed, and their NEW marks. Entries come from data/smithy-lists.csv
//! (tools/smithy_lists.py).
//!
//! The Smithy shows an entry whose listed bit is set, whatever its other conditions
//! (`0x5248c0` weapons, `0x5249f4` armor, read by `0x6fa928`); when it first lists one it
//! sets listed and NEW (`0x524980`). Bit = record index of the list's create table; armor
//! bit = 4 x record + armor ID slot. CONFIRMED in game 2026-10-09 (every list written
//! showed with its NEW marks).

use crate::data::{tables, SmithyList};
use crate::save::Save;

/// Weapon lists, classes 0-14: 15 x (20 B listed, 20 B NEW).
pub const WEAPONS: usize = 0x36AF;
pub const WEAPON_STRIDE: usize = 40;
/// Armor lists, head … legs: 5 x (288 B listed, 288 B NEW).
pub const ARMOR: usize = 0x3907;
pub const ARMOR_STRIDE: usize = 576;
/// Decorations: 36 B listed, then 36 B NEW.
pub const DECOS: usize = 0x31A7;
pub const DECOS_NEW: usize = 0x31CB;
/// Palico smithy, weapons / helms / mail: 3 x (68 B listed, 68 B NEW).
pub const PALICO: usize = 0x4447;
pub const PALICO_STRIDE: usize = 136;

/// Where a list's listed and NEW bitmaps are (relative to the character base) and their
/// size in bytes.
pub fn at(list: &str) -> Option<(usize, usize, usize)> {
    let (kind, n) = list.split_once(':').unwrap_or((list, ""));
    let k: usize = n.parse().unwrap_or(0);
    match kind {
        "weapon" if k < 15 => Some((WEAPONS + WEAPON_STRIDE * k, WEAPONS + WEAPON_STRIDE * k + 20, 20)),
        "armor" if (1..=5).contains(&k) => {
            let o = ARMOR + ARMOR_STRIDE * (k - 1);
            Some((o, o + 288, 288))
        }
        "deco" => Some((DECOS, DECOS_NEW, 36)),
        "palico" => {
            let p = ["weapon", "helm", "mail"].iter().position(|&x| x == n)?;
            let o = PALICO + PALICO_STRIDE * p;
            Some((o, o + 68, 68))
        }
        _ => None,
    }
}

pub fn lists() -> &'static [SmithyList] {
    &tables().smithy
}

/// The bits of entries the Smithy can list for a hunter of body type `gender` (0 = type
/// 1): every record, and of armor records with four IDs only the two of the hunter's
/// type (slots 0-1 type 1, 2-3 type 2), as the game does.
pub fn entries(l: &SmithyList, gender: u8) -> Vec<usize> {
    if !l.list.starts_with("armor:") {
        return (0..l.records).collect();
    }
    let mine = if gender == 0 { [0, 1] } else { [2, 3] };
    l.ids
        .iter()
        .enumerate()
        .flat_map(|(r, &n)| {
            let slots: Vec<usize> = if n == 4 { mine.to_vec() } else { (0..n as usize).collect() };
            slots.into_iter().map(move |k| 4 * r + k)
        })
        .collect()
}

/// (entries listed, entries) of a list.
pub fn count(s: &Save, base: usize, l: &SmithyList, gender: u8) -> (usize, usize) {
    let Some((listed, _, _)) = at(&l.list) else { return (0, 0) };
    let e = entries(l, gender);
    (e.iter().filter(|&&b| s.bit(base + listed, b)).count(), e.len())
}

/// List every entry the hunter can have; each one newly listed gets its NEW mark, as the
/// game's first listing does. Returns how many were added.
pub fn list_all(s: &mut Save, base: usize, l: &SmithyList, gender: u8) -> usize {
    let Some((listed, new, _)) = at(&l.list) else { return 0 };
    let mut n = 0;
    for b in entries(l, gender) {
        if !s.bit(base + listed, b) {
            s.set_bit(base + listed, b, true);
            s.set_bit(base + new, b, true);
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_list_fits_its_bitmap() {
        for l in lists() {
            let (_, _, bytes) = at(&l.list).unwrap_or_else(|| panic!("{}", l.list));
            for g in [0, 1] {
                assert!(entries(l, g).iter().all(|&b| b < 8 * bytes), "{}", l.list);
            }
        }
        // a four-ID armor record lists two slots for either type
        let head = lists().iter().find(|l| l.list == "armor:1").unwrap();
        assert_eq!(entries(head, 0).len(), entries(head, 1).len());
    }

    #[test]
    fn list_all_sets_listed_and_new() {
        use crate::save::{blank, SLOT1_BASE as B};
        let mut s = blank();
        let l = lists().iter().find(|l| l.list == "deco").unwrap();
        assert_eq!(count(&s, B, l, 0), (0, l.records));
        assert_eq!(list_all(&mut s, B, l, 0), l.records);
        assert_eq!(count(&s, B, l, 0), (l.records, l.records));
        assert!(s.bit(B + DECOS_NEW, 0) && s.bit(B + DECOS_NEW, l.records - 1) && !s.bit(B + DECOS_NEW, l.records));
        assert_eq!(list_all(&mut s, B, l, 0), 0);
    }
}
