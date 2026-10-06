//! User settings, kept as JSON in the config folder (`~/.config/mhgu-save-editor` on
//! Linux, `%APPDATA%\mhgu-save-editor` on Windows). Read once, written on every change.
//! Tests start from the defaults and never write the file.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::path::PathBuf;

/// Interface sizes offered (percent, 0 = the system's).
pub const SCALES: [u32; 6] = [0, 100, 125, 150, 175, 200];
/// Snapshots kept per save (0 = all).
pub const KEEPS: [u32; 5] = [0, 5, 10, 20, 50];
pub const THEMES: [&str; 3] = ["system", "light", "dark"];
/// Recent saves remembered.
const RECENT: usize = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// "system", "light" or "dark"
    pub theme: String,
    /// Interface size in percent (SLINT_SCALE_FACTOR); 0 leaves it to the system.
    pub scale: u32,
    /// Snapshot folder; None for the default in the data folder.
    pub snapshot_dir: Option<PathBuf>,
    /// Snapshots kept per save, oldest removed first; 0 keeps all.
    pub keep_snapshots: u32,
    /// The Write dialog's "Take a snapshot first" starts ticked.
    pub snapshot_first: bool,
    /// Folders searched for saves besides Ryujinx's.
    pub save_folders: Vec<PathBuf>,
    /// Process names checked besides the built-in emulators (prefix, any case).
    pub emulators: Vec<String>,
    /// Open the last save on start.
    pub reopen_last: bool,
    /// Saves opened, newest first.
    pub recent: Vec<PathBuf>,
    /// Refuse edits that are not confirmed in game.
    pub confirmed_only: bool,
    /// Ask GitHub for a newer version on start.
    pub check_updates: bool,
    /// The .exe an update renamed aside (Windows), deleted on the next start.
    pub update_leftover: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            theme: "system".into(),
            scale: 0,
            snapshot_dir: None,
            keep_snapshots: 0,
            snapshot_first: true,
            save_folders: vec![],
            emulators: vec![],
            reopen_last: false,
            recent: vec![],
            confirmed_only: false,
            check_updates: true,
            update_leftover: None,
        }
    }
}

impl Settings {
    pub fn add_recent(&mut self, p: PathBuf) {
        self.recent.retain(|x| *x != p);
        self.recent.insert(0, p);
        self.recent.truncate(RECENT);
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("mhgu-save-editor").join("settings.json"))
}

fn load() -> Settings {
    if cfg!(test) {
        return Settings::default();
    }
    path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

thread_local! {
    static SETTINGS: RefCell<Settings> = RefCell::new(load());
}

pub fn get() -> Settings {
    SETTINGS.with_borrow(Clone::clone)
}

/// Change the settings and write them; a failed write keeps them for this session.
pub fn update(f: impl FnOnce(&mut Settings)) {
    let s = SETTINGS.with_borrow_mut(|s| {
        f(s);
        s.clone()
    });
    if cfg!(test) {
        return;
    }
    let Some(p) = path() else { return };
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(&s).unwrap()) {
        eprintln!("settings: {}: {e}", p.display());
    }
}

thread_local! {
    /// The interface size this run started with; None when the environment set it.
    static STARTED: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(Some(0)) };
}

/// Before the window exists: the interface size becomes SLINT_SCALE_FACTOR, unless the
/// environment already sets it.
pub fn apply_scale() {
    if std::env::var_os("SLINT_SCALE_FACTOR").is_some() {
        return STARTED.set(None);
    }
    let s = get().scale;
    if s > 0 {
        // SAFETY: called first thing in main, before any other thread exists
        unsafe { std::env::set_var("SLINT_SCALE_FACTOR", format!("{}", s as f32 / 100.0)) };
    }
    STARTED.set(Some(s));
}

pub fn scale_started() -> Option<u32> {
    STARTED.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"theme":"dark","keep_snapshots":10}"#).unwrap();
        assert_eq!((s.theme.as_str(), s.keep_snapshots, s.snapshot_first), ("dark", 10, true));
    }

    #[test]
    fn recent_is_newest_first_without_repeats() {
        let mut s = Settings::default();
        for p in ["a", "b", "c", "d", "e", "f", "b"] {
            s.add_recent(p.into());
        }
        assert_eq!(s.recent, ["b", "f", "e", "d", "c"].map(PathBuf::from));
    }
}
