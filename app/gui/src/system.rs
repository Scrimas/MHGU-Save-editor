//! Platform bits: finding saves, the running-emulator check, snapshot folder.

use mhgu_save::save::FILE_SIZE;
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

/// `bis/user/save/<id>/0/system` files of the right size.
pub fn detect_saves() -> Vec<PathBuf> {
    let mut out = vec![];
    for r in ryujinx_roots() {
        let Ok(rd) = std::fs::read_dir(r.join("bis/user/save")) else { continue };
        let mut ids: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        ids.sort();
        for id in ids {
            let p = id.join("0").join("system");
            if p.metadata().is_ok_and(|m| m.len() == FILE_SIZE as u64) {
                out.push(p);
            }
        }
    }
    out
}

const EMULATORS: [&str; 8] = ["ryujinx", "ryubing", "yuzu", "suyu", "sudachi", "citron", "eden", "torzu"];

/// Names of running Switch emulators.
pub fn running_emulators() -> Vec<String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
    let sys = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()));
    let mut found: Vec<String> = sys
        .processes()
        .values()
        .filter_map(|p| {
            let n = p.name().to_string_lossy().to_lowercase();
            EMULATORS.iter().any(|e| n.starts_with(e)).then(|| p.name().to_string_lossy().into_owned())
        })
        .collect();
    let _ = ProcessesToUpdate::All;
    found.sort();
    found.dedup();
    found
}

pub fn snapshot_root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("mhgu-save-editor").join("snapshots")
}

/// "Ryujinx" when the save sits in a Ryujinx folder, else a generic name.
pub fn emulator_name(save: &std::path::Path) -> &'static str {
    if save.to_string_lossy().to_lowercase().contains("ryujinx") { "Ryujinx" } else { "The emulator" }
}

/// What a snapshot was taken before, kept next to its files.
const NOTE: &str = "snapshot.json";

pub fn write_note(dir: &std::path::Path, before: &str) {
    let v = serde_json::json!({ "before": before });
    let _ = std::fs::write(dir.join(NOTE), v.to_string());
}

pub struct Snapshot {
    pub dir: PathBuf,
    pub time: chrono::DateTime<chrono::Local>,
    pub before: String,
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
            let time = chrono::NaiveDateTime::parse_from_str(&name, "%Y-%m-%d_%H%M%S")
                .ok()
                .and_then(|t| t.and_local_timezone(chrono::Local).single())
                .or_else(|| e.metadata().and_then(|m| m.modified()).ok().map(chrono::DateTime::<chrono::Local>::from))
                .unwrap_or_else(chrono::Local::now);
            let before = std::fs::read(dir.join(NOTE))
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|j| j.get("before").and_then(|s| s.as_str()).map(String::from))
                .unwrap_or_default();
            Snapshot { dir, time, before }
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

pub fn save_info(p: &std::path::Path) -> Option<SaveInfo> {
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

pub fn open_folder(p: &std::path::Path) {
    let _ = std::fs::create_dir_all(p);
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}
