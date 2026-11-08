//! Where the open save comes from on Android: a folder picked with the system's folder
//! picker (MHGU Recomp's save folder, kept between runs), or files picked with the document
//! picker (an exported save, an emulator's, a JKSV or Checkpoint backup).
//!
//! The editor works on a copy in the app's own folder, so opening, Write (temporary file,
//! rename, read back) and snapshots run as on the desktop (store.rs). After a write, the
//! copy goes back into the picked files and is read back from them. Before writing, the
//! picked files are read again: if one changed since it was copied in (the game saved
//! meanwhile), nothing is written.

use crate::i18n::{tr, trf};
use crate::{Api, AppWindow, DetectedSave, Shared, model};
use serde::{Deserialize, Serialize};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::path::PathBuf;

/// A picked file and the name of its copy in the work folder (`system`, `system_backup`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct File {
    pub uri: String,
    pub name: String,
    pub local: String,
}

/// A save picked once: a folder (`tree`, its files found again each time it opens) or files.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub tree: Option<String>,
    #[serde(default)]
    pub files: Vec<File>,
    /// when it was last opened, seconds since 1970
    #[serde(default)]
    pub opened: i64,
}

/// The open save: where it came from, its work folder, a fingerprint of each picked file
/// as last read or written.
struct Open {
    src: Source,
    dir: PathBuf,
    seen: Vec<u64>,
}

thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
    /// a save waiting for "Open another save?" (staged changes would be lost)
    static ASKED: RefCell<Option<Source>> = const { RefCell::new(None) };
}

fn print(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

fn id_of(key: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(key.as_bytes())[..6].iter().map(|b| format!("{b:02x}")).collect()
}

fn root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("mhgu-save-editor")
}

fn work_dir(id: &str) -> PathBuf {
    root().join("work").join(id)
}

fn list_path() -> PathBuf {
    root().join("sources.json")
}

/// The saves opened before, the last one first.
fn recent() -> Vec<Source> {
    std::fs::read_to_string(list_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(src: &Source) {
    let mut v = recent();
    v.retain(|s| s.id != src.id);
    let mut s = src.clone();
    s.opened = chrono::Local::now().timestamp();
    v.insert(0, s);
    v.truncate(6);
    let _ = std::fs::create_dir_all(root());
    if let Ok(t) = serde_json::to_string_pretty(&v) {
        let _ = std::fs::write(list_path(), t);
    }
}

/// The Open screen's "Recent saves": who is in each (from its last copy) and where it is.
pub fn show_recent(ui: &AppWindow) {
    let rows: Vec<DetectedSave> = recent()
        .into_iter()
        .map(|s| {
            let copy = work_dir(&s.id).join("system");
            let mut row = if copy.is_file() { crate::views::save_row(&copy) } else { DetectedSave { title: s.label.clone().into(), ..Default::default() } };
            row.path = s.id.clone().into();
            row.short = s.label.clone().into();
            row.when = chrono::DateTime::from_timestamp(s.opened, 0).map(|t| crate::fmt::when(t.with_timezone(&chrono::Local))).unwrap_or_default().into();
            row
        })
        .collect();
    ui.global::<Api>().set_recent(model(rows));
}

/// The picked files Write puts the save into.
pub fn targets() -> Vec<String> {
    OPEN.with_borrow(|o| o.as_ref().map(|o| o.src.files.iter().map(|f| f.name.clone()).collect())).unwrap_or_default()
}

/// The save in a picked folder: its system and system_backup.
fn resolve(tree: &str) -> Result<Source, String> {
    let (label, sys, bak) = super::find_save(tree)
        .map_err(|e| if e == "none" { tr("No MHGU save in that folder: it has no system file, in it or in save_0 inside it.").to_string() } else { e })?;
    let mut files = vec![File { uri: sys, name: format!("{label}/system"), local: "system".into() }];
    if !bak.is_empty() {
        files.push(File { uri: bak, name: format!("{label}/system_backup"), local: "system_backup".into() });
    }
    Ok(Source { id: id_of(tree), label, tree: Some(tree.to_string()), files, opened: 0 })
}

/// Files picked with the document picker: the one whose name says backup is the backup,
/// the first other one is the save.
fn from_files(uris: &[String]) -> Result<Source, String> {
    let named: Vec<(String, String)> = uris.iter().map(|u| (u.clone(), super::display_name(u))).collect();
    let main = named.iter().find(|(_, n)| !n.to_lowercase().contains("backup")).or(named.first()).cloned().ok_or_else(|| tr("No file picked").to_string())?;
    let mut files = vec![File { uri: main.0.clone(), name: main.1.clone(), local: "system".into() }];
    if let Some(b) = named.iter().find(|(u, n)| *u != main.0 && n.to_lowercase().contains("backup")) {
        files.push(File { uri: b.0.clone(), name: b.1.clone(), local: "system_backup".into() });
    }
    let label = if files.len() > 1 { trf("{} and its backup", &[&main.1]) } else { main.1.clone() };
    let key: Vec<&str> = files.iter().map(|f| f.uri.as_str()).collect();
    Ok(Source { id: id_of(&key.join("\n")), label, tree: None, files, opened: 0 })
}

/// Copies the picked files into the save's work folder.
fn pull(src: &Source) -> Result<(PathBuf, Vec<u64>), String> {
    let dir = work_dir(&src.id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // a copy an earlier pick left that this one does not have (a backup no longer there)
    for n in mhgu_save::store::FILES {
        if !src.files.iter().any(|f| f.local == n) {
            let _ = std::fs::remove_file(dir.join(n));
        }
    }
    let mut seen = vec![];
    for f in &src.files {
        let b = super::read(&f.uri).map_err(|e| trf("Could not read {}: {}", &[&f.name, &e]))?;
        std::fs::write(dir.join(&f.local), &b).map_err(|e| e.to_string())?;
        seen.push(print(&b));
    }
    Ok((dir, seen))
}

/// Opens a picked save, asking first when changes are staged.
fn open(ui: &AppWindow, st: &Shared, src: Source) {
    if !st.borrow().ops.is_empty() {
        ui.global::<Api>().set_open_ask(src.label.clone().into());
        ASKED.set(Some(src));
        return;
    }
    open_now(ui, st, src);
}

fn open_now(ui: &AppWindow, st: &Shared, src: Source) {
    let api = ui.global::<Api>();
    match pull(&src) {
        Ok((dir, seen)) => {
            remember(&src);
            api.set_source_label(src.label.clone().into());
            api.set_warning("".into());
            OPEN.set(Some(Open { src, dir: dir.clone(), seen }));
            crate::views::open_now(ui, st, &dir.join("system"));
            show_recent(ui);
        }
        Err(e) => {
            api.set_warning(e.clone().into());
            crate::views::say(ui, &e, "", true);
        }
    }
}

/// Whether the app may still read every place the source names.
fn reachable(src: &Source) -> bool {
    match &src.tree {
        Some(t) => super::can_access(t),
        None => src.files.iter().all(|f| super::can_access(&f.uri)),
    }
}

/// Opens a save of the recent list (the folder's files found again).
fn reopen_source(ui: &AppWindow, st: &Shared, src: Source) {
    if !reachable(&src) {
        let e = trf(
            "The editor can no longer open {}. Pick it again: Android drops the editor's access when the editor is reinstalled or the folder is moved.",
            &[&src.label],
        );
        ui.global::<Api>().set_warning(e.into());
        return;
    }
    let src = match &src.tree {
        Some(t) => match resolve(t) {
            Ok(s) => s,
            Err(e) => return crate::views::say(ui, &e, "", true),
        },
        None => src,
    };
    open(ui, st, src);
}

/// At start: the save opened last.
pub fn reopen(ui: &AppWindow, st: &Shared) {
    show_recent(ui);
    if let Some(src) = recent().into_iter().next() {
        reopen_source(ui, st, src);
    }
}

/// What a write finds when it reads the picked files again.
pub enum Check {
    Same,
    /// a file changed since it was read (its name)
    Changed(String),
    /// a file could not be read (its name, why)
    Unreadable(String, String),
}

pub fn check() -> Check {
    OPEN.with_borrow(|o| {
        let Some(o) = o.as_ref() else { return Check::Same };
        for (f, seen) in o.src.files.iter().zip(&o.seen) {
            match super::read(&f.uri) {
                Ok(b) if print(&b) == *seen => {}
                Ok(_) => return Check::Changed(f.name.clone()),
                Err(e) => return Check::Unreadable(f.name.clone(), e),
            }
        }
        Check::Same
    })
}

/// After the copy was written (or restored): each copy file back into its picked file,
/// read back and compared. The first failure stops and is returned.
pub fn push() -> Result<(), String> {
    OPEN.with_borrow_mut(|o| {
        let Some(o) = o.as_mut() else { return Ok(()) };
        for (k, f) in o.src.files.iter().enumerate() {
            let b = std::fs::read(o.dir.join(&f.local)).map_err(|e| trf("Could not read {}: {}", &[&f.local, &e]))?;
            super::write(&f.uri, &b).map_err(|e| trf("Could not write {}: {}", &[&f.name, &e]))?;
            match super::read(&f.uri) {
                Ok(back) if back == b => o.seen[k] = print(&b),
                Ok(_) => return Err(trf("{} does not read back as written", &[&f.name])),
                Err(e) => return Err(trf("Could not read {} back: {}", &[&f.name, &e])),
            }
        }
        Ok(())
    })
}

/// Reload after the game saved: the picked files copied in again.
pub fn pull_again() -> Result<(), String> {
    let src = OPEN.with_borrow(|o| o.as_ref().map(|o| o.src.clone()));
    let Some(src) = src else { return Ok(()) };
    let (dir, seen) = pull(&src)?;
    OPEN.set(Some(Open { src, dir, seen }));
    Ok(())
}

pub fn wire(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    {
        let (w, st) = (ui.as_weak(), st.clone());
        api.on_open_tree(move || {
            let (w, st) = (w.clone(), st.clone());
            super::picker::pick(
                super::picker::FOLDER,
                "",
                "",
                Box::new(move |r| {
                    let Some(ui) = w.upgrade() else { return };
                    match r.and_then(|uris| resolve(&uris[0])) {
                        Ok(src) => open(&ui, &st, src),
                        Err(e) => crate::views::say(&ui, &e, "", true),
                    }
                }),
            );
        });
    }
    {
        let (w, st) = (ui.as_weak(), st.clone());
        api.on_open_dialog(move || {
            let (w, st) = (w.clone(), st.clone());
            super::picker::pick(
                super::picker::FILES,
                "*/*",
                "",
                Box::new(move |r| {
                    let Some(ui) = w.upgrade() else { return };
                    match r.and_then(|uris| from_files(&uris)) {
                        Ok(src) => open(&ui, &st, src),
                        Err(e) => crate::views::say(&ui, &e, "", true),
                    }
                }),
            );
        });
    }
    {
        let (w, st) = (ui.as_weak(), st.clone());
        api.on_open_path(move |id| {
            let Some(ui) = w.upgrade() else { return };
            if let Some(src) = recent().into_iter().find(|s| s.id == id.as_str()) {
                reopen_source(&ui, &st, src);
            }
        });
    }
    {
        let (w, st) = (ui.as_weak(), st.clone());
        api.on_open_confirmed(move || {
            let Some(ui) = w.upgrade() else { return };
            ui.global::<Api>().set_open_ask("".into());
            if let Some(src) = ASKED.take() {
                open_now(&ui, &st, src);
            }
        });
    }
    show_recent(ui);
}
