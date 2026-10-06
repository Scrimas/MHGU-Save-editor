//! Opening saves, the Open screen, Write, snapshots and Restore.

use super::*;

/// A running emulator overwrites the save when it exits: Write and Restore wait for it.
fn emulator_blocks(ui: &AppWindow) -> bool {
    let running = system::running_emulators();
    if !running.is_empty() {
        toast(ui, format!("Close {} first", running.join(", ")), true);
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
            let more = if i.names.len() > 1 {
                format!(" · also {}", i.names[1..].iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(", "))
            } else {
                String::new()
            };
            (n.clone(), format!("HR {} · {}{more}", num(*hr), fmt::playtime(*t)))
        }
        Some(_) => ("No characters yet".to_string(), String::new()),
        None => ("Unreadable save".to_string(), String::new()),
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
            toast(ui, format!("Save opened: {}", count(n, "character", "characters")), false);
        }
        Err(e) => {
            ui.global::<Api>().set_warning(format!("Could not open {}: {e}", p.display()).into());
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
            before: if x.before.is_empty() { "Before a write".to_string() } else { x.before }.into(),
        })
        .collect();
    let api = ui.global::<Api>();
    api.set_snapshots(model(rows));
    api.set_snapshot_dir(fmt::elide_path(&root.display().to_string(), 56).into());
}

pub(super) fn wire_file(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_dialog(move || {
            let Some(ui) = w.upgrade() else { return };
            let mut d = rfd::FileDialog::new().set_title("Open MHGU save (system)");
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
                toast(&ui, "Path copied", false);
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
                    kept = Some((p, fmt::when(now)));
                }
                Err(e) => {
                    toast(&ui, format!("Snapshot failed, nothing written: {e}"), true);
                    return false;
                }
            }
        }
        match store::write_all(&mut doc.save, &mut doc.loc) {
            Ok(_) => {
                s.written();
                if let Some((p, _)) = &kept {
                    system::prune_snapshots(p.parent().unwrap(), p);
                }
                // after writing: a toast with Restore (02.5, S8)
                match kept {
                    Some((p, when)) => {
                        let sub = format!("Snapshot from {} kept", when.trim_start_matches("Today, "));
                        toast_full(&ui, format!("Wrote {}", count(n, "change", "changes")), &sub, "Restore…", ToastAct::Restore(p, when), false)
                    }
                    None => toast(&ui, format!("Wrote {} (no snapshot)", count(n, "change", "changes")), false),
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
                    Some((p, when)) => toast_full(&ui, format!("Write failed: {e}"), "The snapshot taken first restores the save", "Restore…", ToastAct::Restore(p, when), true),
                    None => toast(&ui, format!("Write failed: {e}"), true),
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
    // restoring snapshots the current save first, so a restore can be undone too (A3)
    on!(ui, st, on_restore, |ui, s, dir: SharedString| {
        if emulator_blocks(&ui) {
            return;
        }
        let dir = PathBuf::from(dir.as_str());
        let Some(doc) = s.doc.as_ref() else { return };
        let loc = doc.loc.clone();
        let Some(snap) = system::snapshots(dir.parent().unwrap_or(&dir)).into_iter().find(|x| x.dir == dir) else {
            return toast(&ui, format!("No snapshot at {}", dir.display()), true);
        };
        // a snapshot of another save never goes over this one
        if !system::snapshot_of(&snap, &loc) {
            let sub = snap.source.map(|p| format!("It was taken of {}", p.display())).unwrap_or_default();
            return toast_full(&ui, "Not restored: this snapshot is of another save", &sub, "", ToastAct::None, true);
        }
        let when = fmt::when(snap.time);
        let stamp = chrono::Local::now().format(system::STAMP).to_string();
        let kept = match store::snapshot(&loc, &system::snapshot_dir(&loc), &stamp) {
            Ok(p) => {
                system::write_note(&p, &format!("Before restoring the snapshot from {when}"), &loc);
                p
            }
            Err(e) => return toast(&ui, format!("Snapshot failed, nothing restored: {e}"), true),
        };
        match store::restore(&loc, &dir) {
            Ok(_) => {
                // after the restore: the snapshot it came from may be among the oldest
                system::prune_snapshots(kept.parent().unwrap(), &kept);
                let opened = loc.opened.clone();
                let slot = s.slot;
                if let Err(e) = s.open(&opened) {
                    return toast(&ui, format!("Restored, but reading it back failed: {e}"), true);
                }
                if s.save().slot_used(slot) {
                    s.slot = slot;
                }
                ui.global::<Api>().set_snapshots_open(false);
                toast_full(&ui, format!("Restored the save from {when}"), "The save before restoring is kept as a snapshot", "", ToastAct::None, false);
            }
            Err(e) => toast(&ui, format!("Restore failed: {e}"), true),
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
