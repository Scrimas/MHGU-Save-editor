//! Opening saves, the Open screen, Write, snapshots and Restore.

use super::*;
use crate::{diff, DiffRow};
use mhgu_save::Save;

/// A running emulator overwrites the save when it exits: Write and Restore wait for it.
fn emulator_blocks(ui: &AppWindow) -> bool {
    let running = system::running_emulators();
    if !running.is_empty() {
        toast(ui, trf("Close {} first", &[&running.join(", ")]), true);
        ui.global::<Api>().set_emulator_running(true);
    }
    !running.is_empty()
}

/// A save on the Open screen: its characters (R12).
pub(super) fn save_row(p: &Path) -> DetectedSave {
    let info = system::save_info(p);
    let (title, sub) = match &info {
        Some(i) if !i.names.is_empty() => {
            let (n, hr, t) = &i.names[0];
            let (hr, time) = (num(*hr), fmt::playtime(*t));
            let sub = if i.names.len() > 1 {
                let others = i.names[1..].iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(", ");
                trf("HR {} · {} · also {}", &[&hr, &time, &others])
            } else {
                trf("HR {} · {}", &[&hr, &time])
            };
            (n.clone(), sub)
        }
        Some(_) => (tr("No characters yet").to_string(), String::new()),
        None => (tr("Unreadable save").to_string(), String::new()),
    };
    DetectedSave {
        path: p.display().to_string().into(),
        title: title.into(),
        sub: sub.into(),
        when: info.and_then(|i| i.modified).map(fmt::when).unwrap_or_default().into(),
        short: fmt::elide_path(&p.display().to_string(), 70).into(),
    }
}

/// The Open screen's lists: recent saves still on disk, then the saves found on this
/// computer that are not among them.
pub fn detected(ui: &AppWindow) {
    let recent: Vec<PathBuf> = settings::get().recent.into_iter().filter(|p| p.is_file()).collect();
    let found: Vec<PathBuf> = system::detect_saves().into_iter().filter(|p| !recent.contains(p)).collect();
    let api = ui.global::<Api>();
    api.set_recent(model(recent.iter().map(|p| save_row(p)).collect()));
    api.set_detected(model(found.iter().map(|p| save_row(p)).collect()));
}

/// Snapshot folder of the open save.
pub(super) fn snapshot_dir(st: &State) -> Option<PathBuf> {
    Some(system::snapshot_dir(&st.doc.as_ref()?.loc))
}

/// Open a save; with staged changes it asks first (they would be lost).
pub fn open(ui: &AppWindow, st: &Shared, p: &Path) {
    if !st.borrow().ops.is_empty() {
        ui.global::<Api>().set_open_ask(p.display().to_string().into());
        return;
    }
    open_now(ui, st, p);
}

pub(super) fn open_now(ui: &AppWindow, st: &Shared, p: &Path) {
    let r = st.borrow_mut().open(p);
    match r {
        Ok(()) => {
            let api = ui.global::<Api>();
            api.set_page("overview".into());
            api.set_toast("".into());
            view(|v| {
                v.equip_sel = -1;
                v.palico_sel = -1;
                v.field_sel = -1;
                // a toast's button acts on the save it was shown for
                v.toast_act = ToastAct::None;
            });
            let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
            settings::update(|s| s.add_recent(abs));
            detected(ui);
            settings_ui(ui);
            refresh(ui, &st.borrow());
            let n = st.borrow().doc.as_ref().map(|d| (0..3).filter(|&k| d.save.slot_used(k)).count()).unwrap_or(0);
            toast(ui, trn("Save opened: {n} character", "Save opened: {n} characters", n as i64, &[]), false);
        }
        Err(e) => {
            ui.global::<Api>().set_warning(trf("Could not open {}: {}", &[&p.display(), &e]).into());
            toast(ui, e, true);
        }
    }
}

pub(super) fn list_snapshots(ui: &AppWindow, st: &State) {
    let Some(doc) = st.doc.as_ref() else { return };
    let root = system::snapshot_dir(&doc.loc);
    // with the ones earlier versions kept in the save id's folder, if of this save
    let mut all = system::snapshots(&root);
    if let Some(old) = system::legacy_snapshot_dir(&doc.loc) {
        all.extend(system::snapshots(&old).into_iter().filter(|x| system::snapshot_of(x, &doc.loc)));
        all.sort_by_key(|s| std::cmp::Reverse(s.time));
    }
    let rows: Vec<SnapRow> = all
        .into_iter()
        .map(|x| SnapRow {
            dir: x.dir.display().to_string().into(),
            when: fmt::when(x.time).into(),
            before: if x.before.is_empty() { tr("Before a write").to_string() } else { x.before }.into(),
        })
        .collect();
    let api = ui.global::<Api>();
    api.set_snapshots(model(rows));
    api.set_snapshot_dir(fmt::elide_path(&root.display().to_string(), 56).into());
}

/// Compare snapshot `dir` with the save as open (`other` < 0) or with snapshot `other` of
/// the list; the older one is the left side.
fn compare(ui: &AppWindow, st: &State, dir: &str, other: i32) {
    let Some(doc) = st.doc.as_ref() else { return };
    let api = ui.global::<Api>();
    let snaps: Vec<SnapRow> = api.get_snapshots().iter().collect();
    let Some(me) = snaps.iter().position(|r| r.dir == dir) else { return };
    let read = |d: &str| -> Result<Save, String> {
        let p = store::snapshot_file(&doc.loc, Path::new(d));
        let b = std::fs::read(&p).map_err(|e| trf("Could not read {}: {}", &[&p.display(), &e]))?;
        Save::from_bytes(b).map_err(|e| e.to_string())
    };
    let other = usize::try_from(other).ok().filter(|&o| o < snaps.len() && o != me);
    let snap = |k: usize| trf("the snapshot from {}", &[&snaps[k].when]);
    let sides = match other {
        None => read(dir).map(|a| (a, doc.save.clone(), snap(me), tr("the save as open").to_string())),
        // the list is newest first
        Some(o) => read(dir).and_then(|a| read(&snaps[o].dir).map(|b| if o < me { (a, b, snap(me), snap(o)) } else { (b, a, snap(o), snap(me)) })),
    };
    let (old, new, from, to) = match sides {
        Ok(x) => x,
        Err(e) => return toast(ui, e, true),
    };
    let lines = diff::diff(&old, &new);
    let n = lines.iter().filter(|l| !l.label.is_empty()).count();
    let mut sub = trf("From {} to {}", &[&from, &to]);
    if other.is_none() && st.save().is_dirty() {
        sub.push_str(" · ");
        sub.push_str(tr("the open save includes the changes not written yet"));
    }
    // a whole box changed is thousands of lines: the dialog shows the first ones
    const SHOWN: usize = 1500;
    let more = lines.len().saturating_sub(SHOWN);
    let mut rows: Vec<DiffRow> = lines.into_iter().take(SHOWN).map(|l| DiffRow { group: l.group.into(), label: l.label.into(), old: l.old.into(), new: l.new.into() }).collect();
    if more > 0 {
        let label = trn("{n} more line not shown", "{n} more lines not shown", more as i64, &[]);
        rows.push(DiffRow { group: "".into(), label: label.into(), old: "".into(), new: "".into() });
    }
    let others: Vec<String> = std::iter::once(tr("The save as open").to_string()).chain(snaps.iter().map(|r| trf("Snapshot from {}", &[&r.when]))).collect();
    api.set_diff_rows(model(rows));
    api.set_diff_sub(sub.into());
    api.set_diff_dir(dir.into());
    api.set_diff_others(strings(others));
    api.set_diff_other(other.map_or(0, |o| o as i32 + 1));
    api.set_diff_title(trn("{n} difference", "{n} differences", n as i64, &[]).into());
}

pub(super) fn wire_file(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_dialog(move || {
            let Some(ui) = w.upgrade() else { return };
            let mut d = rfd::FileDialog::new().set_title(tr("Open MHGU save (system)"));
            if let Some(dir) = system::detect_saves().first().and_then(|p| p.parent()) {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                open(&ui, &st, &p);
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_path(move |p| {
            if let Some(ui) = w.upgrade() {
                open(&ui, &st, Path::new(p.as_str()));
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_confirmed(move || {
            let Some(ui) = w.upgrade() else { return };
            let p = ui.global::<Api>().get_open_ask();
            ui.global::<Api>().set_open_ask("".into());
            open_now(&ui, &st, Path::new(p.as_str()));
        });
    }
    {
        let st = st.clone();
        api.on_open_folder(move || {
            if let Some(d) = st.borrow().doc.as_ref().and_then(|d| d.loc.opened.parent().map(Path::to_path_buf)) {
                system::open_folder(&d);
            }
        });
    }
    {
        let w = ui.as_weak();
        api.on_copy_path(move || {
            if let Some(ui) = w.upgrade() {
                toast(&ui, tr("Path copied"), false);
            }
        });
    }

    on!(ui, st, on_write, |ui, s, snapshot: bool| {
        if emulator_blocks(&ui) {
            return false;
        }
        let titles: Vec<String> = s.ops.iter().map(|o| if o.values.len() == 1 { o.values[0].0.label(s.save(), o.slot) } else { o.title.clone() }).collect();
        let n = s.ops.len();
        let Some(doc) = s.doc.as_mut() else { return false };
        // a backup made on the Switch goes back there with the tool that made it
        let tool = system::console_backup(&doc.loc.opened);
        // the game saved since the save was read: writing would undo that play
        if let Some(p) = store::changed_on_disk(&doc.loc) {
            changed_on_disk(&ui, &p);
            return false;
        }
        let mut kept = None;
        if snapshot {
            let now = chrono::Local::now();
            let stamp = now.format(system::STAMP).to_string();
            match store::snapshot(&doc.loc, &system::snapshot_dir(&doc.loc), &stamp) {
                Ok(p) => {
                    system::write_note(&p, &format!("Before: {}", fmt::list(&titles, 3)), &doc.loc);
                    kept = Some((p, fmt::when(now), now.format("%H:%M").to_string()));
                }
                Err(e) => {
                    toast(&ui, trf("Snapshot failed, nothing written: {}", &[&e]), true);
                    return false;
                }
            }
        }
        match store::write_all(&mut doc.save, &mut doc.loc) {
            Ok(_) => {
                s.written();
                if let Some((p, ..)) = &kept {
                    system::prune_snapshots(p.parent().unwrap(), p);
                }
                // after writing: a toast with Restore (02.5, S8)
                match kept {
                    // taken just now: its time says enough
                    Some((p, when, time)) => {
                        let sub = match tool {
                            Some(t) => trf("Snapshot from {} kept · restore the backup on the Switch with {}", &[&time, &t]),
                            None => trf("Snapshot from {} kept", &[&time]),
                        };
                        let msg = trn("Wrote {n} change", "Wrote {n} changes", n as i64, &[]);
                        toast_full(&ui, msg, &sub, tr("Restore…"), ToastAct::Restore(p, when), false)
                    }
                    None => {
                        let msg = trn("Wrote {n} change (no snapshot)", "Wrote {n} changes (no snapshot)", n as i64, &[]);
                        match tool {
                            Some(t) => toast_full(&ui, msg, &trf("Restore the backup on the Switch with {}", &[&t]), "", ToastAct::None, false),
                            None => toast(&ui, msg, false),
                        }
                    }
                }
                true
            }
            Err(mhgu_save::Error::ChangedOnDisk(p)) => {
                changed_on_disk(&ui, &p);
                false
            }
            // the copies are checked before any is written, so a failure here is rare;
            // the snapshot just taken puts every file back
            Err(e) => {
                match kept {
                    Some((p, when, _)) => toast_full(&ui, trf("Write failed: {}", &[&e]), tr("The snapshot taken first restores the save"), tr("Restore…"), ToastAct::Restore(p, when), true),
                    None => toast(&ui, trf("Write failed: {}", &[&e]), true),
                }
                false
            }
        }
    });
    {
        let st = st.clone();
        api.on_open_snapshots(move || {
            let dir = snapshot_dir(&st.borrow()).filter(|p| p.is_dir()).unwrap_or_else(system::snapshot_root);
            system::open_folder(&dir);
        });
    }
    on!(ui, st, on_list_snapshots, |ui, s| {
        list_snapshots(&ui, &s);
    });
    on!(ui, st, on_compare, |ui, s, dir: SharedString, other: i32| {
        compare(&ui, &s, &dir, other);
    });
    // restoring snapshots the current save first, so a restore can be undone too (A3)
    on!(ui, st, on_restore, |ui, s, dir: SharedString| {
        if emulator_blocks(&ui) {
            return;
        }
        let dir = PathBuf::from(dir.as_str());
        let Some(doc) = s.doc.as_ref() else { return };
        let loc = doc.loc.clone();
        let Some(snap) = system::snapshots(dir.parent().unwrap_or(&dir)).into_iter().find(|x| x.dir == dir) else {
            return toast(&ui, trf("No snapshot at {}", &[&dir.display()]), true);
        };
        // a snapshot of another save never goes over this one
        if !system::snapshot_of(&snap, &loc) {
            let sub = snap.source.map(|p| trf("It was taken of {}", &[&p.display()])).unwrap_or_default();
            return toast_full(&ui, tr("Not restored: this snapshot is of another save"), &sub, "", ToastAct::None, true);
        }
        let when = fmt::when(snap.time);
        let stamp = chrono::Local::now().format(system::STAMP).to_string();
        let kept = match store::snapshot(&loc, &system::snapshot_dir(&loc), &stamp) {
            Ok(p) => {
                system::write_note(&p, &format!("Before restoring the snapshot from {when}"), &loc);
                p
            }
            Err(e) => return toast(&ui, trf("Snapshot failed, nothing restored: {}", &[&e]), true),
        };
        match store::restore(&loc, &dir) {
            Ok(_) => {
                // after the restore: the snapshot it came from may be among the oldest
                system::prune_snapshots(kept.parent().unwrap(), &kept);
                let opened = loc.opened.clone();
                let slot = s.slot;
                if let Err(e) = s.open(&opened) {
                    return toast(&ui, trf("Restored, but reading it back failed: {}", &[&e]), true);
                }
                if s.save().slot_used(slot) {
                    s.slot = slot;
                }
                ui.global::<Api>().set_snapshots_open(false);
                toast_full(&ui, trf("Restored the save from {}", &[&when]), tr("The save before restoring is kept as a snapshot"), "", ToastAct::None, false);
            }
            Err(e) => toast(&ui, trf("Restore failed: {}", &[&e]), true),
        }
    });
    {
        let w = ui.as_weak();
        api.on_quit(move || {
            view(|v| v.quitting = true);
            if let Some(ui) = w.upgrade() {
                let _ = ui.hide();
            }
            let _ = slint::quit_event_loop();
        });
    }
}
