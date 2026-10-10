//! The Characters dialog: whole slots copied, swapped, deleted, exported and imported;
//! and the files a character or a Palico is exported to.

use super::*;
use crate::MhxxRow;
use mhgu_save::mhxx::{self, Mhxx};
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

/// A whole-slot edit. Copy, swap, delete and a character file's import are Confirmed in
/// game; an MHXX character is Derived.
fn slot_edit(conf: Conf, title: String, targets: Vec<Target>) -> Edit {
    Edit { key: String::new(), title, detail: tr("Characters").into(), note: String::new(), conf, targets }
}

/// Stage a whole-slot edit listed under slot `at`; says it in a toast.
fn stage(ui: &AppWindow, s: &mut State, at: usize, need_char: bool, e: Edit, f: impl FnOnce(&mut mhgu_save::Save)) {
    if refused(ui, e.conf) {
        return;
    }
    let title = e.title.clone();
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
        stage(&ui, &mut s, to, false, slot_edit(Conf::Confirmed, title, vec![Target::Slot(to)]), |sv| slots::copy(sv, from, to));
    });
    on!(ui, st, on_slot_swap, |ui, s, a: i32, b: i32| {
        let (Ok(a), Ok(b)) = (usize::try_from(a), usize::try_from(b)) else { return };
        if a > 2 || b > 2 || a == b || s.doc.is_none() {
            return;
        }
        let title = trf("Swap {} and {}", &[&who(&s, a), &who(&s, b)]);
        stage(&ui, &mut s, b, false, slot_edit(Conf::Confirmed, title, vec![Target::Slot(a), Target::Slot(b)]), |sv| slots::swap(sv, a, b));
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
        stage(&ui, &mut s, k, true, slot_edit(Conf::Confirmed, title, vec![Target::Slot(k)]), |sv| slots::delete(sv, k));
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
            // an exported character, or an MHXX save (`system`, no extension)
            let Some(p) = rfd::FileDialog::new()
                .set_title(tr("Import character"))
                .add_filter(tr("MHGU character"), &[CHARACTER_EXT])
                .add_filter(tr("MHXX save (system)"), &["*"])
                .pick_file()
            else {
                return;
            };
            let f = match std::fs::read(&p) {
                Ok(b) => b,
                Err(e) => return toast(&ui, trf("Could not read {}: {}", &[&p.display(), &e]), true),
            };
            if f.starts_with(slots::MAGIC) {
                let Ok(name) = slots::name_of(&f) else {
                    return toast(&ui, tr("Not imported: the file is not a character exported by this editor"), true);
                };
                {
                    let mut s = st.borrow_mut();
                    let title = trf("Import {} into slot {}", &[&name, &(k + 1)]);
                    stage(&ui, &mut s, k, false, slot_edit(Conf::Confirmed, title, vec![Target::Slot(k)]), |sv| {
                        let _ = slots::import(sv, k, &f);
                    });
                }
                return refresh(&ui, &st.borrow());
            }
            let x = match Mhxx::from_bytes(f) {
                Ok(x) => x,
                Err(_) => return toast(&ui, tr("Not imported: the file is neither a character exported by this editor nor an MHXX save"), true),
            };
            let used: Vec<usize> = (0..3).filter(|&j| x.slot_used(j)).collect();
            match used[..] {
                [] => toast(&ui, tr("Not imported: that MHXX save holds no character"), true),
                [j] => {
                    import_mhxx(&ui, &mut st.borrow_mut(), &x, j, k);
                    refresh(&ui, &st.borrow());
                }
                _ => ask_mhxx(&ui, MhxxPick { file: x, path: p, export: None, into: k }),
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_mhxx_export_start(move |k| {
            let Some(ui) = w.upgrade() else { return };
            let Ok(k) = usize::try_from(k) else { return };
            if k > 2 || st.borrow().doc.is_none() || !st.borrow().save().slot_used(k) {
                return;
            }
            let name = who(&st.borrow(), k);
            let Some(p) = rfd::FileDialog::new().set_title(trf("MHXX save to put {} in", &[&name])).add_filter(tr("MHXX save (system)"), &["*"]).pick_file() else { return };
            match std::fs::read(&p).map(Mhxx::from_bytes) {
                Ok(Ok(x)) => ask_mhxx(&ui, MhxxPick { file: x, path: p, export: Some(k), into: 0 }),
                Ok(Err(e)) => toast(&ui, trf("Not an MHXX save: {}", &[&e]), true),
                Err(e) => toast(&ui, trf("Could not read {}: {}", &[&p.display(), &e]), true),
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_mhxx_pick(move |j| {
            let Some(ui) = w.upgrade() else { return };
            ui.global::<Api>().set_mhxx_title("".into());
            let (Some(pick), Ok(j)) = (view(|v| v.mhxx.take()), usize::try_from(j)) else { return };
            if j > 2 || st.borrow().doc.is_none() {
                return;
            }
            match pick.export {
                None => {
                    import_mhxx(&ui, &mut st.borrow_mut(), &pick.file, j, pick.into);
                    refresh(&ui, &st.borrow());
                }
                Some(k) => export_mhxx(&ui, &st.borrow(), pick, k, j),
            }
        });
    }
}

/// An MHXX save the Characters dialog asks about: which of its characters to import into
/// slot `into`, or (`export` Some) which of its slots gets the character of that slot.
pub(super) struct MhxxPick {
    file: Mhxx,
    path: PathBuf,
    export: Option<usize>,
    into: usize,
}

/// Show the question of `pick` in the Characters dialog.
fn ask_mhxx(ui: &AppWindow, pick: MhxxPick) {
    let rows: Vec<MhxxRow> = (0..3)
        .map(|j| {
            let used = pick.file.slot_used(j);
            let (name, hr, time) = pick.file.summary(j);
            MhxxRow {
                name: if used { format!("{} · {name}", j + 1) } else { trf("{} · empty", &[&(j + 1)]) }.into(),
                sub: if used { trf("HR {} · {}", &[&hr, &fmt::playtime(time)]) } else { tr("No character in this slot").into() }.into(),
                used,
            }
        })
        .collect();
    let api = ui.global::<Api>();
    let file = pick.path.display().to_string();
    let title = match pick.export {
        None => trf("Which character of {} goes into slot {}?", &[&fmt::elide_path(&file, 48), &(pick.into + 1)]),
        Some(_) => trf("Which slot of {} does the character go into?", &[&fmt::elide_path(&file, 48)]),
    };
    api.set_mhxx_export(pick.export.is_some());
    api.set_mhxx_rows(model(rows));
    api.set_mhxx_title(title.into());
    view(|v| v.mhxx = Some(pick));
}

/// Stage MHXX character `j` as the character of slot `k`.
fn import_mhxx(ui: &AppWindow, s: &mut State, x: &Mhxx, j: usize, k: usize) {
    let (name, ..) = x.summary(j);
    let bytes = x.character(j);
    let title = trf("Import {} from MHXX into slot {}", &[&name, &(k + 1)]);
    stage(ui, s, k, false, slot_edit(Conf::Derived, title, vec![Target::Slot(k)]), |sv| slots::put(sv, k, &bytes));
}

/// The character of slot `k` into slot `j` of the MHXX save, written where the user says
/// (a new file: the picked save stays as it was).
fn export_mhxx(ui: &AppWindow, s: &State, mut pick: MhxxPick, k: usize, j: usize) {
    let sv = s.save();
    let name = character::get(sv, sv.base(k)).name;
    pick.file.put_character(j, slots::get(sv, k));
    let dir = pick.path.parent().map(Path::to_path_buf).unwrap_or_default();
    let Some(p) = rfd::FileDialog::new().set_title(tr("Save the MHXX save")).set_directory(dir).set_file_name("system").save_file() else { return };
    let back = match pick.file.kind() {
        mhxx::Kind::Ds => tr("Restore it on the 3DS with JKSM or Checkpoint"),
        mhxx::Kind::Switch => tr("Restore it on the Switch with JKSV or Checkpoint"),
    };
    match std::fs::write(&p, pick.file.bytes()) {
        Ok(()) => toast_full(ui, trf("Exported {} to slot {} of the MHXX save", &[&name, &(j + 1)]), back, "", ToastAct::None, false),
        Err(e) => toast(ui, trf("Could not write {}: {}", &[&p.display(), &e]), true),
    }
}
