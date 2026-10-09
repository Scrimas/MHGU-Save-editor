//! The Characters dialog: whole slots copied, swapped, deleted, exported and imported;
//! and the files a character or a Palico is exported to.

use super::*;
use mhgu_save::slots;

/// File extensions of an exported character and Palico.
pub(super) const CHARACTER_EXT: &str = "mhguchar";
pub(super) const PALICO_EXT: &str = "mhgucat";

/// A file name from a name the game allows (no path separators or reserved characters).
pub(super) fn file_name(name: &str, ext: &str) -> String {
    let n: String = name.chars().map(|c| if c.is_control() || r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect();
    let n = n.trim();
    format!("{}.{ext}", if n.is_empty() { "export" } else { n })
}

/// Ask where to save `bytes` (filter name and extension of the file) and write them
/// there; `said` is the toast when it worked.
pub(super) fn save_file(ui: &AppWindow, title: &str, (filter, ext): (&str, &str), name: &str, bytes: &[u8], said: (String, &str)) {
    let Some(p) = rfd::FileDialog::new().set_title(title).add_filter(filter, &[ext]).set_file_name(file_name(name, ext)).save_file() else { return };
    match std::fs::write(&p, bytes) {
        Ok(()) => toast_full(ui, said.0, said.1, "", ToastAct::None, false),
        Err(e) => toast(ui, trf("Could not write {}: {}", &[&p.display(), &e]), true),
    }
}

/// Ask for a file to read; None when cancelled or unreadable (said in a toast).
pub(super) fn open_file(ui: &AppWindow, title: &str, filter: &str, ext: &str) -> Option<Vec<u8>> {
    let p = rfd::FileDialog::new().set_title(title).add_filter(filter, &[ext]).pick_file()?;
    match std::fs::read(&p) {
        Ok(b) => Some(b),
        Err(e) => {
            toast(ui, trf("Could not read {}: {}", &[&p.display(), &e]), true);
            None
        }
    }
}

/// "Scrimas (slot 1)", or "slot 3" for an empty one.
fn who(s: &State, k: usize) -> String {
    let sv = s.save();
    if sv.slot_used(k) {
        trf("{} (slot {})", &[&character::get(sv, sv.base(k)).name, &(k + 1)])
    } else {
        trf("slot {}", &[&(k + 1)])
    }
}

/// Stage a whole-slot edit listed under slot `at`; says it in a toast.
fn stage(ui: &AppWindow, s: &mut State, at: usize, need_char: bool, title: String, targets: Vec<Target>, f: impl FnOnce(&mut mhgu_save::Save)) {
    let e = Edit { key: String::new(), title: title.clone(), detail: tr("Characters").into(), note: String::new(), conf: Conf::Derived, targets };
    if refused(ui, e.conf) {
        return;
    }
    if s.edit_at(at, need_char, e, |sv, _| {
        f(sv);
        vec![]
    }) {
        toast_full(ui, tr("Added to Review"), &title, "", ToastAct::None, false);
    }
}

pub(super) fn wire_characters(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    on!(ui, st, on_slot_copy, |ui, s, from: i32, to: i32| {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else { return };
        if from > 2 || to > 2 || from == to || s.doc.is_none() || !s.save().slot_used(from) {
            return;
        }
        let title = trf("Copy {} to slot {}", &[&who(&s, from), &(to + 1)]);
        stage(&ui, &mut s, to, false, title, vec![Target::Slot(to)], |sv| slots::copy(sv, from, to));
    });
    on!(ui, st, on_slot_swap, |ui, s, a: i32, b: i32| {
        let (Ok(a), Ok(b)) = (usize::try_from(a), usize::try_from(b)) else { return };
        if a > 2 || b > 2 || a == b || s.doc.is_none() {
            return;
        }
        let title = trf("Swap {} and {}", &[&who(&s, a), &who(&s, b)]);
        stage(&ui, &mut s, b, false, title, vec![Target::Slot(a), Target::Slot(b)], |sv| slots::swap(sv, a, b));
        // the character shown moves with its slot
        if s.slot == a || s.slot == b {
            s.slot = a + b - s.slot;
        }
    });
    on!(ui, st, on_slot_delete, |ui, s, k: i32| {
        let Ok(k) = usize::try_from(k) else { return };
        if k > 2 || s.doc.is_none() {
            return;
        }
        let title = trf("Delete {}", &[&who(&s, k)]);
        stage(&ui, &mut s, k, true, title, vec![Target::Slot(k)], |sv| slots::delete(sv, k));
    });
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_slot_export(move |k| {
            let Some(ui) = w.upgrade() else { return };
            let Ok(k) = usize::try_from(k) else { return };
            let (name, bytes, staged) = {
                let s = st.borrow();
                if k > 2 || s.doc.is_none() || !s.save().slot_used(k) {
                    return;
                }
                let sv = s.save();
                (character::get(sv, sv.base(k)).name, slots::export(sv, k), s.changed(sv.base(k), slots::LEN))
            };
            let sub = if staged { tr("With its changes not written yet") } else { "" };
            save_file(&ui, tr("Export character"), (tr("MHGU character"), CHARACTER_EXT), &name, &bytes, (trf("Exported {}", &[&name]), sub));
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_slot_import(move |k| {
            let Some(ui) = w.upgrade() else { return };
            let Ok(k) = usize::try_from(k) else { return };
            if k > 2 || st.borrow().doc.is_none() {
                return;
            }
            let Some(f) = open_file(&ui, tr("Import character"), tr("MHGU character"), CHARACTER_EXT) else { return };
            let Ok(name) = slots::name_of(&f) else {
                return toast(&ui, tr("Not imported: the file is not a character exported by this editor"), true);
            };
            {
                let mut s = st.borrow_mut();
                let title = trf("Import {} into slot {}", &[&name, &(k + 1)]);
                stage(&ui, &mut s, k, false, title, vec![Target::Slot(k)], |sv| {
                    let _ = slots::import(sv, k, &f);
                });
            }
            refresh(&ui, &st.borrow());
        });
    }
}
