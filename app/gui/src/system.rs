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

pub fn open_folder(p: &std::path::Path) {
    let _ = std::fs::create_dir_all(p);
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}
