//! Item box, pouch and loadouts (docs/11-save-map.md; tools/items.py).
//!
//! A box or pouch slot is 19 bits of an LSB-first bit stream: item ID (12 bits), then
//! count (7 bits). ID 0 = empty.

use crate::save::Save;

pub const BOX: usize = 0x278;
pub const BOX_N: usize = 2300;
pub const POUCH: usize = 0x27BF;
pub const POUCH_N: usize = 32;
pub const LOADOUTS: usize = 0x17CF;
pub const LOADOUT_N: usize = 24;
pub const LOADOUT_SZ: usize = 170;
pub const LOADOUT_NAME: usize = 42;
pub const LOADOUT_ITEMS: usize = 32;
pub const MAX_ID: u16 = 0xFFF;
pub const MAX_COUNT: u8 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stack {
    pub id: u16,
    pub count: u8,
}

impl Stack {
    pub fn is_empty(&self) -> bool {
        self.id == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Box,
    Pouch,
}

impl Store {
    fn at(self) -> (usize, usize) {
        match self {
            Store::Box => (BOX, BOX_N),
            Store::Pouch => (POUCH, POUCH_N),
        }
    }
    pub fn len(self) -> usize {
        self.at().1
    }
}

pub fn get(s: &Save, base: usize, store: Store, i: usize) -> Stack {
    let (off, n) = store.at();
    assert!(i < n);
    Stack { id: s.bits(base + off, 19 * i, 12) as u16, count: s.bits(base + off, 19 * i + 12, 7) as u8 }
}

pub fn set(s: &mut Save, base: usize, store: Store, i: usize, st: Stack) {
    let (off, n) = store.at();
    assert!(i < n);
    // an ID that does not fit the 12-bit field would become another item: empty instead
    let st = if st.id == 0 || st.id > MAX_ID || st.count == 0 { Stack::default() } else { st };
    s.set_bits(base + off, 19 * i, 12, st.id as u32);
    s.set_bits(base + off, 19 * i + 12, 7, st.count.min(MAX_COUNT) as u32);
}

pub fn all(s: &Save, base: usize, store: Store) -> Vec<Stack> {
    (0..store.len()).map(|i| get(s, base, store, i)).collect()
}

pub fn set_all(s: &mut Save, base: usize, store: Store, v: &[Stack]) {
    for i in 0..store.len() {
        set(s, base, store, i, v.get(i).copied().unwrap_or_default());
    }
}

/// Merge stacks of the same item and move used slots to the front, sorted by item ID.
/// Returns the new slot list. Box: stacks of up to 99, overflow kept as extra stacks
/// while slots remain. Pouch: one stack per item of at most `max(id)` (its carry limit);
/// the excess is dropped.
pub fn compact(v: &[Stack], sort: bool, store: Store, max: impl Fn(u16) -> u8) -> Vec<Stack> {
    let mut order: Vec<u16> = vec![];
    let mut totals: std::collections::HashMap<u16, u32> = Default::default();
    for st in v.iter().filter(|s| !s.is_empty()) {
        if !totals.contains_key(&st.id) {
            order.push(st.id);
        }
        *totals.entry(st.id).or_default() += st.count as u32;
    }
    if sort {
        order.sort_unstable();
    }
    let mut out = vec![];
    for id in order {
        let m = max(id).clamp(1, MAX_COUNT) as u32;
        let mut t = if store == Store::Pouch { totals[&id].min(m) } else { totals[&id] };
        while t > 0 && out.len() < v.len() {
            let c = t.min(m);
            out.push(Stack { id, count: c as u8 });
            t -= c;
        }
    }
    out.resize(v.len(), Stack::default());
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loadout {
    pub name: String,
    pub items: Vec<(u16, u16)>,
}

fn loadout_at(base: usize, k: usize) -> usize {
    assert!(k < LOADOUT_N, "loadout {k} out of range");
    base + LOADOUTS + LOADOUT_SZ * k
}

pub fn loadout(s: &Save, base: usize, k: usize) -> Loadout {
    let o = loadout_at(base, k);
    Loadout {
        name: s.str(o, LOADOUT_NAME),
        items: (0..LOADOUT_ITEMS).map(|j| (s.u16(o + LOADOUT_NAME + 4 * j), s.u16(o + LOADOUT_NAME + 4 * j + 2))).collect(),
    }
}

pub fn set_loadout(s: &mut Save, base: usize, k: usize, l: &Loadout) {
    let o = loadout_at(base, k);
    s.set_str(o, LOADOUT_NAME, &l.name);
    for j in 0..LOADOUT_ITEMS {
        let (id, c) = l.items.get(j).copied().unwrap_or((0, 0));
        let (id, c) = if id == 0 || id > MAX_ID || c == 0 { (0, 0) } else { (id, c.min(MAX_COUNT as u16)) };
        s.set_u16(o + LOADOUT_NAME + 4 * j, id);
        s.set_u16(o + LOADOUT_NAME + 4 * j + 2, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE};

    #[test]
    fn slots_pack_at_the_field_limits() {
        let mut s = blank();
        set(&mut s, SLOT1_BASE, Store::Box, 1, Stack { id: MAX_ID, count: 127 });
        assert_eq!(get(&s, SLOT1_BASE, Store::Box, 1), Stack { id: MAX_ID, count: MAX_COUNT });
        assert_eq!(get(&s, SLOT1_BASE, Store::Box, 0), Stack::default());
        assert_eq!(get(&s, SLOT1_BASE, Store::Box, 2), Stack::default());
        // too big for 12 bits: the slot is emptied, not turned into item ID & 0xFFF
        set(&mut s, SLOT1_BASE, Store::Box, 1, Stack { id: MAX_ID + 2, count: 5 });
        assert_eq!(get(&s, SLOT1_BASE, Store::Box, 1), Stack::default());
        set(&mut s, SLOT1_BASE, Store::Pouch, POUCH_N - 1, Stack { id: 7, count: 3 });
        assert_eq!(all(&s, SLOT1_BASE, Store::Box).iter().filter(|x| !x.is_empty()).count(), 0, "pouch apart from the box");
    }

    #[test]
    fn loadout_round_trip_and_bounds() {
        let mut s = blank();
        let l = Loadout { name: "Hunt".into(), items: vec![(1, 5), (0x1000, 3), (2, 200)] };
        set_loadout(&mut s, SLOT1_BASE, LOADOUT_N - 1, &l);
        let r = loadout(&s, SLOT1_BASE, LOADOUT_N - 1);
        assert_eq!(r.name, "Hunt");
        assert_eq!(&r.items[..3], &[(1, 5), (0, 0), (2, 99)]);
        assert_eq!(get(&s, SLOT1_BASE, Store::Pouch, 0), Stack::default(), "the last loadout ends before the pouch");
        assert!(std::panic::catch_unwind(|| loadout(&blank(), SLOT1_BASE, LOADOUT_N)).is_err());
    }

    #[test]
    fn compact_merges_and_sorts() {
        let v = [
            Stack { id: 5, count: 60 },
            Stack::default(),
            Stack { id: 2, count: 1 },
            Stack { id: 5, count: 60 },
        ];
        let c = compact(&v, true, Store::Box, |_| MAX_COUNT);
        assert_eq!(c, vec![Stack { id: 2, count: 1 }, Stack { id: 5, count: 99 }, Stack { id: 5, count: 21 }, Stack::default()]);
    }

    #[test]
    fn compact_pouch_keeps_one_stack_at_carry_limit() {
        let v = [Stack { id: 29, count: 2 }, Stack { id: 10, count: 8 }, Stack { id: 10, count: 7 }, Stack::default()];
        let c = compact(&v, true, Store::Pouch, |id| if id == 29 { 1 } else { 10 });
        assert_eq!(c, vec![Stack { id: 10, count: 10 }, Stack { id: 29, count: 1 }, Stack::default(), Stack::default()]);
    }
}
