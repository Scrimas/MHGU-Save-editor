//! The item box and equipment boxes to and from CSV files, and talismans as text
//! (core `transfer`).

use super::*;
use mhgu_save::equipment::Talisman;
use mhgu_save::transfer::{self, LineError, Problem};
use mhgu_save::Save;

/// What is wrong with an imported file, for a toast.
fn problem(e: &LineError) -> String {
    let what = match &e.problem {
        Problem::Format => tr("not a file of this kind").to_string(),
        Problem::Slot(s) => trf("no slot {}", &[s]),
        Problem::Twice(n) => trf("slot {} is given twice", &[n]),
        Problem::Id(s) if s.is_empty() => tr("neither an ID nor a name").to_string(),
        Problem::Id(s) => trf("nothing is called {}", &[s]),
        Problem::Count(s) => trf("{} is not a count of 1 to 99", &[s]),
        Problem::Level(s) => trf("{} is not a level of 1 to 32", &[s]),
        Problem::Raw => tr("the raw entry is not 72 hex digits").to_string(),
        Problem::Full => tr("more lines than the box has slots").to_string(),
        Problem::Skill(s) if s.is_empty() => tr("a talisman needs a first skill").to_string(),
        Problem::Skill(s) => trf("no skill is called {}", &[s]),
        Problem::Points(s) => trf("{} is not a number of points", &[s]),
    };
    trf("Line {}: {}", &[&e.line, &what])
}

/// A name in the language in use or in English, whatever its case: its index in `list`.
fn find(list: &[String], english: &[String], name: &str) -> Option<usize> {
    let n = name.trim().to_lowercase();
    list.iter().position(|x| x.to_lowercase() == n).or_else(|| english.iter().position(|x| x.to_lowercase() == n))
}

/// "Scrimas item box.csv"
fn file_name(st: &State, what: &str) -> String {
    let sv = st.save();
    format!("{} {what}", character::get(sv, st.base()).name)
}

/// Write `text` where the user picks; says it in a toast.
fn save_csv(ui: &AppWindow, title: &str, name: &str, text: &str, said: String) {
    save_file(ui, title, (tr("CSV file"), "csv"), name, text.as_bytes(), (said, ""));
}

fn open_csv(ui: &AppWindow, title: &str) -> Option<String> {
    let b = open_file(ui, title, tr("CSV file"), "csv")?;
    match String::from_utf8(b) {
        Ok(t) => Some(t.trim_start_matches('\u{feff}').to_string()),
        Err(_) => {
            toast(ui, tr("Not imported: the file is not UTF-8 text"), true);
            None
        }
    }
}

/// Stage `f` as one edit over many values of the character shown; lists the targets of
/// `all` whose value changed.
fn stage_many(ui: &AppWindow, s: &mut State, title: String, detail: &str, conf: Conf, all: Vec<Target>, f: impl FnOnce(&mut Save, usize)) {
    let e = Edit { key: String::new(), title: title.clone(), detail: detail.into(), note: String::new(), conf, targets: vec![] };
    if refused(ui, e.conf) {
        return;
    }
    let slot = s.slot;
    if s.edit(e, |sv, base| {
        let pre = sv.clone();
        f(sv, base);
        all.into_iter().filter(|t| t.read(&pre, slot) != t.read(sv, slot)).collect()
    }) {
        toast_full(ui, tr("Added to Review"), &title, "", ToastAct::None, false);
    }
}

/// The talisman text: every talisman of the hunter box, or the one selected.
fn talisman_text(st: &State, selected: bool) -> String {
    let (sv, base) = (st.save(), st.base());
    let en = assets::english();
    let skill = |k: u8| en.skills.get(k as usize).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| k.to_string());
    let sel = view(|v| v.equip_sel);
    let lines: Vec<String> = (0..Owner::Hunter.len())
        .filter(|&i| !selected || i as i32 == sel)
        .filter_map(|i| equipment::get(sv, base, Owner::Hunter, i).talisman())
        .map(|t| transfer::talisman_line(&t, skill))
        .collect();
    format!("{}\n{}\n", transfer::TALISMAN_HEADER, lines.join("\n"))
}

/// The lowest talisman type whose charm tables can make `t`, else the highest: its
/// rarity (talisman ID) and tier.
fn talisman_type(t: &Talisman) -> (u16, u8) {
    let rows = &tables().talisman;
    let fits = |tier: u8| {
        let ok = |kind: &str, skill: u8, pts: i8| skill == 0 || rows.iter().any(|r| r.tier == tier && r.kind == kind && r.skill == skill && (r.min..=r.max).contains(&pts));
        let top = rows.iter().find(|r| r.tier == tier && r.kind == "slots").map_or(3, |r| r.max as u8);
        ok("skill1", t.skills[0], t.points[0]) && ok("skill2", t.skills[1], t.points[1]) && t.slots <= top
    };
    // the highest rarity of each tier (equipment::talisman_tier)
    [(2, 97), (4, 98), (7, 99), (10, 100)].into_iter().find(|&(_, tier)| fits(tier)).unwrap_or((10, 100))
}

pub(super) fn wire_transfer(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_item_export(move || {
            let Some(ui) = w.upgrade() else { return };
            let s = st.borrow();
            if s.doc.is_none() {
                return;
            }
            let text = transfer::items_csv(&items::all(s.save(), s.base(), Store::Box), assets::item_name);
            save_csv(&ui, tr("Export the item box"), &file_name(&s, tr("item box")), &text, tr("Item box exported").into());
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_item_import(move || {
            let Some(ui) = w.upgrade() else { return };
            if st.borrow().doc.is_none() {
                return;
            }
            let Some(text) = open_csv(&ui, tr("Import the item box")) else { return };
            let (n, en) = (assets::names(), assets::english());
            let lookup = |name: &str| find(&n.items, &en.items, name).and_then(|i| u16::try_from(i).ok()).filter(|&i| i != 0);
            let v = match transfer::read_items(&text, items::BOX_N, lookup) {
                Ok(v) => v,
                Err(e) => return toast_full(&ui, tr("Item box not imported"), &problem(&e), "", ToastAct::None, true),
            };
            {
                let mut s = st.borrow_mut();
                let all = (0..items::BOX_N).map(|i| Target::Item(Store::Box, i)).collect();
                stage_many(&ui, &mut s, tr("Import the item box").into(), tr("Item box"), Conf::Confirmed, all, |sv, base| items::set_all(sv, base, Store::Box, &v));
            }
            refresh(&ui, &st.borrow());
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_equip_export(move || {
            let Some(ui) = w.upgrade() else { return };
            let s = st.borrow();
            if s.doc.is_none() {
                return;
            }
            let owner = owner_of(&ui);
            let v: Vec<_> = (0..owner.len()).map(|i| equipment::get(s.save(), s.base(), owner, i)).collect();
            let text = transfer::equipment_csv(&v, |e| equip_name(owner, e));
            let what = if owner == Owner::Hunter { tr("equipment box") } else { tr("Palico equipment box") };
            save_csv(&ui, tr("Export the equipment box"), &file_name(&s, what), &text, tr("Equipment box exported").into());
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_equip_import(move || {
            let Some(ui) = w.upgrade() else { return };
            if st.borrow().doc.is_none() {
                return;
            }
            let owner = owner_of(&ui);
            let Some(text) = open_csv(&ui, tr("Import the equipment box")) else { return };
            let v = match transfer::read_equipment(&text, owner.len()) {
                Ok(v) => v,
                Err(e) => return toast_full(&ui, tr("Equipment box not imported"), &problem(&e), "", ToastAct::None, true),
            };
            let kept = {
                let mut s = st.borrow_mut();
                let (sv, base) = (s.save(), s.base());
                // what the worn gear and the sets point at stays where it is
                let used = transfer::referenced(sv, base, owner);
                let kept = used.iter().filter(|&&i| v[i] != equipment::get(sv, base, owner, i)).count();
                let all = (0..owner.len()).map(|i| Target::Equip(owner, i)).collect();
                let title = if owner == Owner::Hunter { tr("Import the equipment box") } else { tr("Import the Palico equipment box") };
                stage_many(&ui, &mut s, title.into(), tr("Equipment box"), Conf::Confirmed, all, |sv, base| {
                    for (i, e) in v.iter().enumerate().filter(|(i, _)| !used.contains(i)) {
                        equipment::set(sv, base, owner, i, e);
                    }
                });
                kept
            };
            refresh(&ui, &st.borrow());
            if kept > 0 {
                let sub = trn("{n} entry kept as it was: worn or in a set", "{n} entries kept as they were: worn or in a set", kept as i64, &[]);
                toast_full(&ui, tr("Added to Review"), &sub, "", ToastAct::None, false);
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_talisman_text_show(move |selected| {
            let Some(ui) = w.upgrade() else { return };
            let s = st.borrow();
            if s.doc.is_none() {
                return;
            }
            let api = ui.global::<Api>();
            api.set_talisman_text(talisman_text(&s, selected).into());
            api.set_talisman_text_open(true);
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_talisman_text_add(move |text| {
            let Some(ui) = w.upgrade() else { return };
            if st.borrow().doc.is_none() {
                return;
            }
            let (n, en) = (assets::names(), assets::english());
            let lookup = |name: &str| find(&n.skills, &en.skills, name).and_then(|i| u8::try_from(i).ok());
            let ts = match transfer::read_talismans(&text, lookup) {
                Ok(v) if v.is_empty() => return toast(&ui, tr("No talisman in the text"), true),
                Ok(v) => v,
                Err(e) => return toast_full(&ui, tr("Talismans not added"), &problem(&e), "", ToastAct::None, true),
            };
            let free: Vec<usize> = {
                let s = st.borrow();
                let (sv, base) = (s.save(), s.base());
                let used = transfer::referenced(sv, base, Owner::Hunter);
                (0..Owner::Hunter.len()).filter(|&i| equipment::get(sv, base, Owner::Hunter, i).is_empty() && !used.contains(&i)).take(ts.len()).collect()
            };
            if free.len() < ts.len() {
                return toast(&ui, trn("Not added: the box has room for {n} talisman", "Not added: the box has room for {n} talismans", free.len() as i64, &[]), true);
            }
            {
                let mut s = st.borrow_mut();
                let all = free.iter().map(|&i| Target::Equip(Owner::Hunter, i)).collect();
                let title = trn("Add {n} talisman", "Add {n} talismans", ts.len() as i64, &[]);
                stage_many(&ui, &mut s, title, tr("Equipment box"), Conf::Confirmed, all, |sv, base| {
                    for (&i, t) in free.iter().zip(&ts) {
                        let (id, tier) = talisman_type(t);
                        let mut e = equipment::Entry::new(Kind::Talisman, id);
                        e.set_talisman(Talisman { tier, ..*t });
                        equipment::set(sv, base, Owner::Hunter, i, &e);
                    }
                });
            }
            ui.global::<Api>().set_talisman_text_open(false);
            refresh(&ui, &st.borrow());
        });
    }
}
