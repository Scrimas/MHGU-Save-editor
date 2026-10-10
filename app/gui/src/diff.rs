//! Two saves compared in game terms: each value the editor names that differs, by
//! character and page, then the bytes no value covers, by the save map's field.

use crate::i18n::{tr, trf, trn};
use crate::targets::{self, Target};
use mhgu_save::data::tables;
use mhgu_save::{character, save, slots, store, Save};

/// A value that differs; heading lines (an empty label) open each group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub group: String,
    pub label: String,
    pub old: String,
    pub new: String,
}

fn heading(v: &mut Vec<Line>, group: &str) {
    if v.last().is_none_or(|l| l.group != group) {
        v.push(Line { group: group.into(), label: String::new(), old: String::new(), new: String::new() });
    }
}

fn line(v: &mut Vec<Line>, group: &str, label: String, old: String, new: String) {
    heading(v, group);
    v.push(Line { group: group.into(), label, old, new });
}

/// "Scrimas (slot 1)"; the name in either save.
fn who(a: &Save, b: &Save, k: usize) -> String {
    let name = [b, a].iter().find(|s| s.slot_used(k)).map(|s| character::get(s, s.base(k)).name).unwrap_or_default();
    trf("{} (slot {})", &[&name, &(k + 1)])
}

/// What differs from `a` (old) to `b` (new). The container header is left out.
pub fn diff(a: &Save, b: &Save) -> Vec<Line> {
    let (x, y) = (a.bytes(), b.bytes());
    let mut changed: Vec<bool> = x.iter().zip(y).map(|(p, q)| p != q).collect();
    changed[..store::HEADER].fill(false);
    if !changed.contains(&true) {
        return vec![];
    }
    let mut out = vec![];
    let (per_slot, shared) = Target::all();
    let check = |t: &Target, k: usize, group: &str, out: &mut Vec<Line>, changed: &mut Vec<bool>| {
        let bytes = t.bytes(b, k);
        if !bytes.iter().any(|&i| changed[i]) {
            return;
        }
        let (old, new) = (t.read(a, k), t.read(b, k));
        if old != new {
            line(out, group, t.label(b, k), old, new);
        }
        for i in bytes {
            changed[i] = false;
        }
    };
    for k in 0..3 {
        let (base, end) = (b.base(k), b.base(k) + slots::LEN);
        // an empty slot's bytes are not a character: they go to the fields below
        if (!changed[base..end].contains(&true) && !changed[save::SLOT_USED + k]) || (!a.slot_used(k) && !b.slot_used(k)) {
            continue;
        }
        let who = who(a, b, k);
        // a character made or deleted: one line, not its every value
        if a.slot_used(k) != b.slot_used(k) {
            let name = |s: &Save| if s.slot_used(k) { character::get(s, base).name } else { tr("Empty").to_string() };
            line(&mut out, &who, trf("Slot {}", &[&(k + 1)]), name(a), name(b));
            changed[base..end].fill(false);
            changed[save::SLOT_USED + k] = false;
            continue;
        }
        for t in &per_slot {
            let group = format!("{who} · {}", targets::page_title(targets::page_index(t.page())));
            check(t, k, &group, &mut out, &mut changed);
        }
    }
    for t in &shared {
        check(t, 0, tr("Shared by the characters"), &mut out, &mut changed);
    }
    // the rest, by the save map's field (fields of a slot repeat in every slot)
    let mut rest: Vec<(String, usize)> = vec![];
    for f in &tables().fields {
        let starts: Vec<usize> = if f.block == "char1" { (0..3).map(|k| b.base(k) + f.rel).collect() } else { vec![f.abs] };
        for a0 in starts {
            let end = (a0 + f.size).min(changed.len());
            let n = changed[a0.min(end)..end].iter().filter(|&&c| c).count();
            if n > 0 {
                changed[a0..end].fill(false);
                rest.push((f.label.clone(), n));
            }
        }
    }
    let unmapped = changed.iter().filter(|&&c| c).count();
    if unmapped > 0 {
        rest.push((tr("Bytes outside the save map").into(), unmapped));
    }
    for (label, n) in rest {
        line(&mut out, tr("Other bytes"), label, String::new(), trn("{n} byte", "{n} bytes", n as i64, &[]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mhgu_save::items::{self, Stack, Store};

    fn blank() -> Save {
        let mut v = vec![0u8; save::FILE_SIZE];
        for (s, b) in [0x18CC9Cu32, 0x2AC560, 0x3CBE24].iter().enumerate() {
            v[0x34 + 4 * s..0x38 + 4 * s].copy_from_slice(&(b - 0x24).to_le_bytes());
        }
        v[save::SLOT_USED] = 1;
        Save::from_bytes(v).unwrap()
    }

    #[test]
    fn values_that_differ_by_character_and_page() {
        let a = blank();
        let mut b = a.clone();
        assert!(diff(&a, &b).is_empty());
        let base = b.base(0);
        items::set(&mut b, base, Store::Box, 4, Stack { id: 1, count: 10 });
        character::set_funds(&mut b, base, 5000);
        let d = diff(&a, &b);
        let values: Vec<&Line> = d.iter().filter(|l| !l.label.is_empty()).collect();
        assert_eq!(values.len(), 2, "{d:?}");
        assert!(d.iter().all(|l| l.group.contains("(slot 1)")), "{d:?}");
        assert!(values.iter().any(|l| l.new.contains("5,000")), "{values:?}");
        // a byte no value names falls back to its field
        b.set_u8(base + 0x11D088, 1);
        let d = diff(&a, &b);
        assert_eq!(d.last().map(|l| l.group.as_str()), Some(tr("Other bytes")));
    }
}
