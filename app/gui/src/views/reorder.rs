//! The fixed-slot lists a drag reorders (slots.slint): My Sets, Palico sets, item
//! loadouts. Each list row keeps the record it shows when the record moves (`perm`), so a
//! move slides rows instead of swapping their text. Rows are placed here: runs of unused
//! slots fold into one row; while a row is dragged every slot shows (the empty ones thin)
//! in the order the drop would make. A drop or Alt+Up/Down stages one edit that moves the
//! record (`Save::move_record`), the ones between shifting one slot.

use super::*;
use crate::SlotRow;
use crate::targets::SET_PARTS;
use mhgu_save::sets;
use std::cell::RefCell;
use std::collections::HashMap;

/// Row heights: ListRow's two densities, and an empty slot while a row is dragged.
const TWO_LINE: f32 = 52.0;
const ONE_LINE: f32 = 40.0;
const THIN: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum List {
    MySets,
    PalicoSets,
    Loadouts,
}

impl List {
    fn parse(s: &str) -> Option<List> {
        match s {
            "myset" => Some(List::MySets),
            "palset" => Some(List::PalicoSets),
            "loadout" => Some(List::Loadouts),
            _ => None,
        }
    }

    fn n(self) -> usize {
        match self {
            List::MySets => sets::MY_SETS_N,
            List::PalicoSets => sets::PALICO_SETS_N,
            List::Loadouts => items::LOADOUT_N,
        }
    }

    /// A folded run of unused slots `a`..=`b`.
    fn run(self, a: usize, b: usize) -> String {
        match (self, a == b) {
            (List::Loadouts, true) => trf("Loadout {} · empty", &[&(a + 1)]),
            (List::Loadouts, false) => trf("Loadouts {}–{} · empty", &[&(a + 1), &(b + 1)]),
            (_, true) => trf("Set {} · unused", &[&(a + 1)]),
            (_, false) => trf("Sets {}–{} · unused", &[&(a + 1), &(b + 1)]),
        }
    }

    /// An empty slot while a row is dragged.
    fn empty(self) -> String {
        match self {
            List::Loadouts => tr("Empty").into(),
            _ => tr("Unused").into(),
        }
    }

    fn sel(self) -> i32 {
        view(|v| match self {
            List::MySets => v.set_sel,
            List::PalicoSets => v.palset_sel,
            List::Loadouts => v.loadout_sel,
        })
    }

    fn set_sel(self, k: i32) {
        view(|v| match self {
            List::MySets => v.set_sel = k,
            List::PalicoSets => v.palset_sel = k,
            List::Loadouts => v.loadout_sel = k,
        });
    }
}

/// What a slot holds, as its row shows it.
pub(super) struct Slot {
    pub name: String,
    pub sub: String,
    pub used: bool,
    pub changed: bool,
}

#[derive(Default)]
struct Kept {
    /// row (model index) -> the slot it shows
    perm: Vec<usize>,
    slots: Vec<Slot>,
    /// a drag: the slot dragged and where it would go
    drag: Option<(usize, usize)>,
}

thread_local! {
    static LISTS: RefCell<HashMap<List, Kept>> = RefCell::new(HashMap::new());
}

/// Where slot `q` is after slot `from` moves to `to`.
fn shift(q: usize, from: usize, to: usize) -> usize {
    match q {
        _ if q == from => to,
        _ if from < to && q > from && q <= to => q - 1,
        _ if to < from && q >= to && q < from => q + 1,
        _ => q,
    }
}

/// Positions -> slots after slot `from` moves to `to`.
fn order(n: usize, drag: Option<(usize, usize)>) -> Vec<usize> {
    let mut o: Vec<usize> = (0..n).collect();
    if let Some((from, to)) = drag {
        let q = o.remove(from);
        o.insert(to, q);
    }
    o
}

fn height(s: &Slot, dragging: bool) -> f32 {
    match () {
        _ if dragging && !s.used => THIN,
        _ if s.used && !s.sub.is_empty() => TWO_LINE,
        _ => ONE_LINE,
    }
}

/// The rows (in model order) and their total height.
fn layout(list: List, k: &Kept) -> (Vec<SlotRow>, f32) {
    let n = k.slots.len();
    let dragging = k.drag.is_some();
    let order = order(n, k.drag);
    let mut row_of = vec![0; n];
    for (e, &q) in k.perm.iter().enumerate() {
        row_of[q] = e;
    }
    let folds = |q: usize| !dragging && !k.slots[q].used && !k.slots[q].changed;
    let mut rows = vec![SlotRow::default(); n];
    let (mut y, mut p) = (0.0, 0);
    while p < n {
        let q = order[p];
        if folds(q) {
            // not dragging: positions are slots
            let mut b = p;
            while b + 1 < n && folds(b + 1) {
                b += 1;
            }
            let name: SharedString = list.run(p, b).into();
            for j in p..=b {
                let h = if j == p { ONE_LINE } else { 0.0 };
                rows[row_of[j]] = SlotRow { first: p as i32, last: b as i32, name: name.clone(), run: true, y, h, ..Default::default() };
            }
            y += ONE_LINE;
            p = b + 1;
            continue;
        }
        let s = &k.slots[q];
        let thin = dragging && !s.used;
        let h = height(s, dragging);
        rows[row_of[q]] = SlotRow {
            first: q as i32,
            last: q as i32,
            num: (if dragging { p } else { q } + 1).to_string().into(),
            moved: dragging && p != q,
            name: if thin { list.empty() } else { s.name.clone() }.into(),
            sub: if thin { "" } else { &s.sub }.into(),
            used: s.used,
            changed: s.changed,
            run: false,
            thin,
            y,
            h,
        };
        y += h;
        p += 1;
    }
    (rows, y)
}

fn push(ui: &AppWindow, list: List, k: &Kept) {
    let api = ui.global::<Api>();
    let (rows, total) = layout(list, k);
    match list {
        List::Loadouts => {
            api.set_loadouts(keep(api.get_loadouts(), rows));
            api.set_loadouts_h(total);
        }
        _ => {
            api.set_set_rows(keep(api.get_set_rows(), rows));
            api.set_set_rows_h(total);
        }
    }
}

/// The list's rows from what its slots hold (a page refresh).
pub(super) fn show(ui: &AppWindow, list: List, slots: Vec<Slot>) {
    LISTS.with_borrow_mut(|m| {
        let k = m.entry(list).or_default();
        if k.perm.len() != slots.len() {
            k.perm = (0..slots.len()).collect();
            k.drag = None;
        }
        k.slots = slots;
        push(ui, list, k);
    });
}

/// The slot a drag would drop on, from the place `to` it has: a row next to the gap
/// changes side when the dragged row's leading edge (`top`, or its bottom going down)
/// passes the row's middle, as the rows are shown (the gap included).
fn target(k: &Kept, from: usize, to: usize, top: f32) -> usize {
    let rest: Vec<f32> = (0..k.slots.len()).filter(|&q| q != from).map(|q| height(&k.slots[q], true)).collect();
    let mut ys = Vec::with_capacity(rest.len());
    let mut y = 0.0;
    for h in &rest {
        ys.push(y);
        y += h;
    }
    let mut t = to.min(rest.len());
    loop {
        // the row above, at its place: goes below once the top passes its middle
        if t > 0 && top < ys[t - 1] + rest[t - 1] / 2.0 {
            t -= 1;
        // the row below, shown after the gap (+ the dragged row's height): goes above once
        // the bottom passes its middle
        } else if t < rest.len() && top > ys[t] + rest[t] / 2.0 {
            t += 1;
        } else {
            return t;
        }
    }
}

/// Stages slot `from` of `list` moving to `to`; the rows keep their records and the
/// selection follows.
fn apply(ui: &AppWindow, s: &mut State, list: List, from: usize, to: usize) {
    let n = list.n();
    if from == to || from >= n || to >= n || refused(ui, Conf::Derived) {
        return;
    }
    let (a, b) = (from.min(to), from.max(to));
    let (title, cands): (String, Vec<Target>) = match list {
        List::MySets => (trf("Move My Set {} to {}", &[&(from + 1), &(to + 1)]), (a..=b).flat_map(|k| SET_PARTS.map(|p| Target::MySet(k, p))).collect()),
        List::PalicoSets => (trf("Move Palico set {} to {}", &[&(from + 1), &(to + 1)]), (a..=b).map(Target::PalicoSet).collect()),
        List::Loadouts => (trf("Move item loadout {} to {}", &[&(from + 1), &(to + 1)]), (a..=b).map(Target::Loadout).collect()),
    };
    let slot = s.slot;
    let e = Edit { key: String::new(), title, detail: String::new(), note: String::new(), conf: Conf::Derived, targets: vec![] };
    let done = s.edit(e, |sv, base| {
        let pre = sv.clone();
        match list {
            List::MySets => sets::move_my_set(sv, base, from, to),
            List::PalicoSets => sets::move_palico_set(sv, base, from, to),
            List::Loadouts => items::move_loadout(sv, base, from, to),
        }
        cands.into_iter().filter(|t| t.read(&pre, slot) != t.read(sv, slot)).collect()
    });
    if !done {
        return;
    }
    LISTS.with_borrow_mut(|m| {
        if let Some(k) = m.get_mut(&list) {
            for q in k.perm.iter_mut() {
                *q = shift(*q, from, to);
            }
        }
    });
    let sel = list.sel();
    if (0..n as i32).contains(&sel) {
        list.set_sel(shift(sel as usize, from, to) as i32);
    }
}

pub(super) fn wire_reorder(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    // start and drag only re-place the rows: no page refresh while the pointer moves
    {
        let w = ui.as_weak();
        api.on_reorder_start(move |list, from| {
            let (Some(ui), Some(list)) = (w.upgrade(), List::parse(&list)) else { return };
            let Ok(from) = usize::try_from(from) else { return };
            LISTS.with_borrow_mut(|m| {
                let Some(k) = m.get_mut(&list).filter(|k| from < k.slots.len()) else { return };
                k.drag = Some((from, from));
                push(&ui, list, k);
            });
        });
    }
    {
        let w = ui.as_weak();
        api.on_reorder_drag(move |list, top| {
            let (Some(ui), Some(list)) = (w.upgrade(), List::parse(&list)) else { return };
            LISTS.with_borrow_mut(|m| {
                let Some(k) = m.get_mut(&list) else { return };
                let Some((from, to)) = k.drag else { return };
                let t = target(k, from, to, top);
                if t != to {
                    k.drag = Some((from, t));
                    push(&ui, list, k);
                }
            });
        });
    }
    on!(ui, st, on_reorder_drop, |ui, s, list: SharedString| {
        let Some(list) = List::parse(&list) else { return };
        let drag = LISTS.with_borrow_mut(|m| m.get_mut(&list).and_then(|k| k.drag.take()));
        if let Some((from, to)) = drag {
            apply(&ui, &mut s, list, from, to);
        }
    });
    on!(ui, st, on_reorder_key, |ui, s, list: SharedString, from: i32, d: i32| {
        let Some(list) = List::parse(&list) else { return };
        let to = from + d.signum();
        if from < 0 || to < 0 {
            return;
        }
        apply(&ui, &mut s, list, from as usize, to as usize);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(used: bool) -> Slot {
        Slot { name: if used { "x".into() } else { "---".into() }, sub: if used { "s".into() } else { String::new() }, used, changed: false }
    }

    #[test]
    fn shift_follows_a_move() {
        assert_eq!((0..5).map(|q| shift(q, 1, 3)).collect::<Vec<_>>(), [0, 3, 1, 2, 4]);
        assert_eq!((0..5).map(|q| shift(q, 3, 1)).collect::<Vec<_>>(), [0, 2, 3, 1, 4]);
        assert_eq!(order(5, Some((1, 3))), [0, 2, 3, 1, 4]);
    }

    #[test]
    fn unused_runs_fold_and_open_thin_while_dragging() {
        let k = Kept { perm: (0..5).collect(), slots: vec![slot(true), slot(false), slot(false), slot(false), slot(true)], drag: None };
        let (rows, total) = layout(List::MySets, &k);
        assert_eq!(total, TWO_LINE * 2.0 + ONE_LINE);
        assert!(rows[1].run && rows[1].h == ONE_LINE && (rows[1].first, rows[1].last) == (1, 3));
        assert_eq!((rows[2].h, rows[3].h), (0.0, 0.0));
        assert_eq!(rows[4].y, TWO_LINE + ONE_LINE);
        let k = Kept { drag: Some((0, 3)), ..k };
        let (rows, total) = layout(List::MySets, &k);
        assert_eq!(total, TWO_LINE * 2.0 + THIN * 3.0);
        // slot 0 shows at position 3, the empties move up one place each
        assert_eq!((rows[0].y, rows[0].num.as_str(), rows[0].moved), (THIN * 3.0, "4", true));
        assert!(rows[1].thin && rows[1].y == 0.0);
        assert_eq!(target(&k, 0, 3, THIN * 3.0), 3, "at its place: stays");
        assert_eq!(target(&k, 0, 3, 0.0), 0);
        // going down from the top: each thin row passes once the bottom is past its middle
        assert_eq!(target(&k, 0, 0, THIN / 2.0 - 1.0), 0);
        assert_eq!(target(&k, 0, 0, THIN / 2.0 + 1.0), 1);
        assert_eq!(target(&k, 0, 0, THIN * 1.5 + 1.0), 2);
    }

    #[test]
    fn same_threshold_up_and_down() {
        let k = Kept { perm: (0..3).collect(), slots: vec![slot(true), slot(true), slot(true)], drag: None };
        // the second set: up past the first once its top is above the first's middle
        assert_eq!(target(&k, 1, 1, TWO_LINE / 2.0 + 1.0), 1);
        assert_eq!(target(&k, 1, 1, TWO_LINE / 2.0 - 1.0), 0);
        // down past the third once its bottom is below the third's middle: as far
        assert_eq!(target(&k, 1, 1, TWO_LINE * 1.5 - 1.0), 1);
        assert_eq!(target(&k, 1, 1, TWO_LINE * 1.5 + 1.0), 2);
    }
}
