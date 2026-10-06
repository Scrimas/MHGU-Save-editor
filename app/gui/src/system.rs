//! Platform bits: finding saves, the running-emulator check, snapshot folder.

use mhgu_save::save::FILE_SIZE;
use mhgu_save::store::Location;
use std::path::PathBuf;

/// Ryujinx keeps its data in `<config>/Ryujinx` (Linux ~/.config, Windows %APPDATA%),
/// or in a `portable` folder next to the executable.
fn ryujinx_roots() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(c) = dirs::config_dir() {
        v.push(c.join("Ryujinx"));
    }
    if let Some(h) = dirs::home_dir() {
        v.push(h.join(".var/app/io.github.ryubing.Ryujinx/config/Ryujinx"));
        v.push(h.join(".var/app/org.ryujinx.Ryujinx/config/Ryujinx"));
    }
    v
}

fn is_save(p: &std::path::Path) -> bool {
    p.metadata().is_ok_and(|m| m.is_file() && m.len() == FILE_SIZE as u64)
}

/// `bis/user/save/<id>/0/system` files of the right size, then the `system` files found
/// in the folders added in Settings.
pub fn detect_saves() -> Vec<PathBuf> {
    let mut out = vec![];
    for r in ryujinx_roots() {
        let Ok(rd) = std::fs::read_dir(r.join("bis/user/save")) else { continue };
        let mut ids: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        ids.sort();
        for id in ids {
            let p = id.join("0").join("system");
            if is_save(&p) {
                out.push(p);
            }
        }
    }
    for f in crate::settings::get().save_folders {
        // a folder is walked once a session (up to 50,000 entries); adding it again in
        // Settings walks it anew
        let found = WALKED.with_borrow_mut(|m| m.entry(f.clone()).or_insert_with(|| find_saves(&f)).clone());
        for p in found {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Forget a walked folder (removed in Settings), so adding it again finds new saves.
pub fn forget_folder(f: &std::path::Path) {
    WALKED.with_borrow_mut(|m| m.remove(f));
}

thread_local! {
    static WALKED: std::cell::RefCell<std::collections::HashMap<PathBuf, Vec<PathBuf>>> = Default::default();
    /// Characters of a save file by (path, size, modified): read again only when it changed.
    static INFOS: std::cell::RefCell<std::collections::HashMap<PathBuf, (Option<std::time::SystemTime>, std::rc::Rc<SaveInfo>)>> = Default::default();
}

/// `system` files of the right size under `root`, at most 8 folders down (the yuzu
/// family keeps `nand/user/save/<0…0>/<user>/<title>/system`). A save in a `1/` commit
/// folder is left out when its `0/` sibling is there: both are the same save.
fn find_saves(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![(root.to_path_buf(), 0)];
    let mut seen = 0;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut subs = vec![];
        for e in rd.flatten() {
            seen += 1;
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() && depth < 8 {
                subs.push(e.path());
            } else if t.is_file() && e.file_name() == "system" && is_save(&e.path()) {
                out.push(e.path());
            }
        }
        // a folder like the home folder is not walked whole
        if seen > 50_000 {
            break;
        }
        subs.sort();
        stack.extend(subs.into_iter().rev().map(|p| (p, depth + 1)));
    }
    out.retain(|p| {
        let commit = p.parent().and_then(|d| d.file_name()).is_some_and(|n| n == "1");
        !(commit && p.parent().and_then(|d| d.parent()).is_some_and(|d| is_save(&d.join("0").join("system"))))
    });
    out.sort();
    out
}

pub const EMULATORS: [&str; 8] = ["ryujinx", "ryubing", "yuzu", "suyu", "sudachi", "citron", "eden", "torzu"];

/// Names of running Switch emulators: the built-in ones and those added in Settings.
pub fn running_emulators() -> Vec<String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
    let extra: Vec<String> = crate::settings::get().emulators.iter().map(|e| e.to_lowercase()).collect();
    let sys = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()));
    let mut found: Vec<String> = sys
        .processes()
        .values()
        .filter_map(|p| {
            let n = p.name().to_string_lossy().to_lowercase();
            let known = EMULATORS.iter().any(|e| n.starts_with(e)) || extra.iter().any(|e| n.starts_with(e.as_str()));
            known.then(|| p.name().to_string_lossy().into_owned())
        })
        .collect();
    let _ = ProcessesToUpdate::All;
    found.sort();
    found.dedup();
    found
}

pub fn default_snapshot_root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("mhgu-save-editor").join("snapshots")
}

/// The folder chosen in Settings, else the default.
pub fn snapshot_root() -> PathBuf {
    crate::settings::get().snapshot_dir.unwrap_or_else(default_snapshot_root)
}

/// What identifies a save: its folder (`<id>` holding `0/` and `1/`), else the opened
/// file, made absolute.
fn save_key(loc: &Location) -> PathBuf {
    let p = loc.save_dir.as_ref().unwrap_or(&loc.opened);
    std::fs::canonicalize(p).or_else(|_| std::path::absolute(p)).unwrap_or_else(|_| p.clone())
}

/// Snapshot folder of a save: `<root>/<folder name>-<8 hex digits of its path>`, so two
/// saves with the same folder name (two Ryujinx installs, every yuzu-style save) never
/// share one.
pub fn snapshot_dir(loc: &Location) -> PathBuf {
    use sha2::{Digest, Sha256};
    let key = save_key(loc);
    let name = match &loc.save_dir {
        Some(d) => d.file_name(),
        None => loc.opened.parent().and_then(|d| d.file_name()),
    };
    let name = name.map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save".into());
    let h = Sha256::digest(key.to_string_lossy().as_bytes());
    let hex: String = h[..4].iter().map(|b| format!("{b:02x}")).collect();
    snapshot_root().join(format!("{name}-{hex}"))
}

/// The folder earlier versions used for a Ryujinx-layout save (`<root>/<save id>`);
/// its snapshots are listed too, but never pruned.
pub fn legacy_snapshot_dir(loc: &Location) -> Option<PathBuf> {
    let d = snapshot_root().join(loc.save_dir.as_ref()?.file_name()?);
    d.is_dir().then_some(d)
}

/// Whether a snapshot may be restored over this save: it was taken of it, or it is
/// from an earlier version that did not record the save.
pub fn snapshot_of(s: &Snapshot, loc: &Location) -> bool {
    s.source.as_ref().is_none_or(|p| p.to_string_lossy() == save_key(loc).to_string_lossy())
}

/// Removes the oldest snapshots of one save beyond the number kept in Settings, never
/// `keep` (the one just taken). Only folders named like a snapshot are touched.
pub fn prune_snapshots(root: &std::path::Path, keep: &std::path::Path) -> usize {
    let n = crate::settings::get().keep_snapshots as usize;
    if n == 0 {
        return 0;
    }
    let named = |s: &Snapshot| s.dir.file_name().and_then(|n| n.to_str()).is_some_and(|n| chrono::NaiveDateTime::parse_from_str(n, STAMP).is_ok());
    let old = snapshots(root).into_iter().filter(|s| s.dir != keep && named(s)).skip(n.saturating_sub(1));
    old.filter(|s| std::fs::remove_dir_all(&s.dir).is_ok()).count()
}

/// Snapshot folder names.
pub const STAMP: &str = "%Y-%m-%d_%H%M%S";

/// "Ryujinx" when the save sits in a Ryujinx folder, else a generic name.
pub fn emulator_name(save: &std::path::Path) -> &'static str {
    if save.to_string_lossy().to_lowercase().contains("ryujinx") { "Ryujinx" } else { "The emulator" }
}

/// What a snapshot was taken before, kept next to its files.
const NOTE: &str = "snapshot.json";

/// The note of a snapshot of `loc`: what it was taken before and which save it is of.
pub fn write_note(dir: &std::path::Path, before: &str, loc: &Location) {
    let v = serde_json::json!({ "before": before, "source": save_key(loc).to_string_lossy() });
    let _ = std::fs::write(dir.join(NOTE), v.to_string());
}

pub struct Snapshot {
    pub dir: PathBuf,
    pub time: chrono::DateTime<chrono::Local>,
    pub before: String,
    /// The save it was taken of (None before this was recorded).
    pub source: Option<PathBuf>,
}

/// Snapshots of one save folder (`root/<save id>/<stamp>`), newest first.
pub fn snapshots(root: &std::path::Path) -> Vec<Snapshot> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut v: Vec<Snapshot> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let dir = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let time = chrono::NaiveDateTime::parse_from_str(&name, STAMP)
                .ok()
                .and_then(|t| t.and_local_timezone(chrono::Local).single())
                .or_else(|| e.metadata().and_then(|m| m.modified()).ok().map(chrono::DateTime::<chrono::Local>::from))
                .unwrap_or_else(chrono::Local::now);
            let note = std::fs::read(dir.join(NOTE)).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
            let field = |k: &str| note.as_ref().and_then(|j| j.get(k)).and_then(|s| s.as_str()).map(String::from);
            let before = field("before").unwrap_or_default();
            let source = field("source").map(PathBuf::from);
            Snapshot { dir, time, before, source }
        })
        .collect();
    v.sort_by(|a, b| b.time.cmp(&a.time));
    v
}

/// Characters and last change of a detected save, for the Open screen.
pub struct SaveInfo {
    pub names: Vec<(String, u16, u32)>,
    pub modified: Option<chrono::DateTime<chrono::Local>>,
}

/// `read_info`, kept while the file's modification time stays the same.
pub fn save_info(p: &std::path::Path) -> Option<std::rc::Rc<SaveInfo>> {
    let modified = p.metadata().and_then(|m| m.modified()).ok();
    if let Some(i) = INFOS.with_borrow(|m| m.get(p).filter(|(t, _)| *t == modified && t.is_some()).map(|(_, i)| i.clone())) {
        return Some(i);
    }
    let i = std::rc::Rc::new(read_info(p)?);
    INFOS.with_borrow_mut(|m| m.insert(p.to_path_buf(), (modified, i.clone())));
    Some(i)
}

fn read_info(p: &std::path::Path) -> Option<SaveInfo> {
    let s = mhgu_save::Save::from_bytes(std::fs::read(p).ok()?).ok()?;
    let names = (0..3)
        .filter(|&k| s.slot_used(k))
        .map(|k| {
            let c = mhgu_save::character::get(&s, s.base(k));
            (c.name, c.hr, c.playtime)
        })
        .collect();
    let modified = p.metadata().and_then(|m| m.modified()).ok().map(chrono::DateTime::<chrono::Local>::from);
    Some(SaveInfo { names, modified })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_at(p: &std::path::Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![0u8; FILE_SIZE]).unwrap();
    }

    #[test]
    fn finds_saves_in_added_folders() {
        let d = std::env::temp_dir().join(format!("mhgu-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let yuzu = d.join("nand/user/save/0000000000000000/AB/0100770008DD8000/system");
        save_at(&yuzu);
        save_at(&d.join("ryu/0/system"));
        save_at(&d.join("ryu/1/system"));
        save_at(&d.join("other/1/system"));
        std::fs::create_dir_all(d.join("small")).unwrap();
        std::fs::write(d.join("small/system"), b"x").unwrap();
        let found = find_saves(&d);
        assert_eq!(found, vec![yuzu, d.join("other/1/system"), d.join("ryu/0/system")]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn snapshots_are_kept_per_save() {
        let d = std::env::temp_dir().join(format!("mhgu-snapid-{}", std::process::id()));
        let (a, b) = (Location::of(&d.join("a/0001/0/system")), Location::of(&d.join("b/0001/0/system")));
        let (da, db) = (snapshot_dir(&a), snapshot_dir(&b));
        assert_ne!(da, db);
        assert!(da.file_name().unwrap().to_string_lossy().starts_with("0001-"));
        assert_eq!(da, snapshot_dir(&Location::of(&d.join("a/0001/1/system_backup"))), "one save, one folder");
        // yuzu-style saves (no 0/ and 1/) differ by their own path
        assert_ne!(snapshot_dir(&Location::of(&d.join("y/A/system"))), snapshot_dir(&Location::of(&d.join("y/B/system"))));
        let snap = |source: Option<&Location>| Snapshot { dir: d.clone(), time: chrono::Local::now(), before: String::new(), source: source.map(save_key) };
        assert!(snapshot_of(&snap(Some(&a)), &a));
        assert!(!snapshot_of(&snap(Some(&a)), &b));
        assert!(snapshot_of(&snap(None), &b), "older snapshots did not record their save");
    }

    #[test]
    fn prune_keeps_the_newest_and_foreign_folders() {
        let d = std::env::temp_dir().join(format!("mhgu-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let names = ["2026-01-01_000000", "2026-01-02_000000", "2026-01-03_000000", "2026-01-04_000000"];
        for n in names {
            std::fs::create_dir_all(d.join(n)).unwrap();
        }
        std::fs::create_dir_all(d.join("mine")).unwrap();
        crate::settings::update(|s| s.keep_snapshots = 2);
        assert_eq!(prune_snapshots(&d, &d.join(names[3])), 2);
        crate::settings::update(|s| s.keep_snapshots = 0);
        let mut left: Vec<String> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["2026-01-03_000000", "2026-01-04_000000", "mine"]);
        std::fs::remove_dir_all(&d).unwrap();
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

pub fn open_folder(p: &std::path::Path) {
    let _ = std::fs::create_dir_all(p);
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}
