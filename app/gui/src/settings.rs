//! User settings, kept as JSON in the config folder (`~/.config/mhgu-save-editor` on
//! Linux, `%APPDATA%\mhgu-save-editor` on Windows). Read once, written on every change.
//! Tests start from the defaults and never write the file.
//!
//! Before going back to an older version (Settings, Updates and versions), the file is
//! copied to `settings-<this version>.json`: the older one rewrites it without the
//! settings it does not know. The next start of that version or a newer one merges the
//! copy back and deletes it.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::path::PathBuf;

/// Interface sizes offered (percent, 0 = the system's).
pub const SCALES: [u32; 6] = [0, 100, 125, 150, 175, 200];
/// Snapshots kept per save (0 = all).
pub const KEEPS: [u32; 5] = [0, 5, 10, 20, 50];
pub const THEMES: [&str; 3] = ["system", "light", "dark"];
/// Where a save opens: the Overview, or the page used last.
pub const START_PAGES: [&str; 2] = ["overview", "last"];
/// How the window is drawn: Slint's choice (OpenGL), or the software renderer.
pub const RENDERERS: [&str; 2] = ["", "software"];
/// Recent saves remembered.
const RECENT: usize = 5;
/// Colours the picker remembers (one row of its chips).
const RECENT_COLOURS: usize = 12;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Interface language, a code of i18n::LANGS; "" for the system's.
    pub language: String,
    /// "system", "light" or "dark"
    pub theme: String,
    /// Colours from matugen when its file is there (Linux; theme.rs).
    pub matugen: bool,
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
    /// Colours applied with the colour picker, "#RRGGBB", newest first.
    pub recent_colours: Vec<String>,
    /// Layout moves, slides and fades.
    pub animations: bool,
    /// Where a save opens: one of START_PAGES.
    pub start_page: String,
    /// The page shown when the editor was last closed (for start_page "last").
    pub last_page: String,
    /// Take a snapshot when a save opens, unless it has one from that day.
    pub snapshot_on_open: bool,
    /// On the first start after an update, list what changed since the version before.
    pub show_changes: bool,
    /// The version that ran last.
    pub last_version: String,
    /// One of RENDERERS, read on start.
    pub renderer: String,
    /// The Items page's Fill box dialog opens in its compact view.
    pub fill_compact: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            language: String::new(),
            theme: "system".into(),
            matugen: true,
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
            recent_colours: vec![],
            animations: true,
            start_page: "overview".into(),
            last_page: String::new(),
            snapshot_on_open: false,
            show_changes: true,
            last_version: String::new(),
            renderer: String::new(),
            fill_compact: false,
        }
    }
}

impl Settings {
    pub fn add_recent(&mut self, p: PathBuf) {
        self.recent.retain(|x| *x != p);
        self.recent.insert(0, p);
        self.recent.truncate(RECENT);
    }

    pub fn add_colour(&mut self, hex: String) {
        self.recent_colours.retain(|x| !x.eq_ignore_ascii_case(&hex));
        self.recent_colours.insert(0, hex);
        self.recent_colours.truncate(RECENT_COLOURS);
    }
}

pub fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("mhgu-save-editor").join("settings.json"))
}

fn load() -> Settings {
    if cfg!(test) {
        return Settings::default();
    }
    let Some(p) = path() else { return Settings::default() };
    let read = |p: &std::path::Path| std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let mut now = read(&p);
    let kept = kept(&p);
    for k in &kept {
        if let Some(k) = read(k) {
            now = Some(merge_kept(k, now));
        }
    }
    if let Some(v) = now.as_ref().filter(|_| !kept.is_empty())
        && write(&p, &serde_json::to_string_pretty(v).unwrap())
    {
        for k in &kept {
            let _ = std::fs::remove_file(k);
        }
    }
    now.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

use serde_json::Value;

/// The copies kept before going back (`settings-<version>.json`) that this version takes
/// back: of this version or an older one, oldest first.
fn kept(p: &std::path::Path) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(p.parent().unwrap()) else { return vec![] };
    let mut v: Vec<_> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let ver = crate::update::version(name.strip_prefix("settings-")?.strip_suffix(".json")?)?;
            (Some(ver) <= crate::update::version(crate::update::VERSION)).then_some((ver, e.path()))
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, p)| p).collect()
}

/// The copy kept before going back, with the file the older version left: its values
/// win, the settings it did not know come back, and so does the update check it was
/// told to skip.
fn merge_kept(kept: Value, now: Option<Value>) -> Value {
    match (kept, now) {
        (Value::Object(mut k), Some(Value::Object(n))) => {
            for (key, v) in n {
                if key != "check_updates" {
                    k.insert(key, v);
                }
            }
            Value::Object(k)
        }
        (k, None) => k,
        (_, Some(n)) => n,
    }
}

/// Before going back to an older version: a copy of the file for this version to merge
/// back. False when there is a file and it could not be copied.
pub fn keep_for_rollback() -> bool {
    let Some(p) = path().filter(|p| p.exists()) else { return true };
    std::fs::copy(&p, p.with_file_name(format!("settings-{}.json", crate::update::VERSION))).is_ok()
}

/// Through a temporary file: a crash mid-write must not reset every setting.
fn write(p: &std::path::Path, text: &str) -> bool {
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    let tmp = p.with_extension("json.tmp");
    let r = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, p));
    if let Err(e) = &r {
        let _ = std::fs::remove_file(&tmp);
        eprintln!("settings: {}: {e}", p.display());
    }
    r.is_ok()
}

// Per thread on purpose: only the UI thread reads and writes them (workers hand their
// results back through the event loop), and each test thread starts from the defaults.
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
    if let Some(p) = path() {
        write(&p, &serde_json::to_string_pretty(&s).unwrap());
    }
}

/// Back to the defaults, but for what the editor keeps for itself.
pub fn reset() {
    update(|s| {
        let d = Settings { update_leftover: s.update_leftover.take(), last_version: std::mem::take(&mut s.last_version), ..Settings::default() };
        *s = d;
    });
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
    fn a_kept_copy_brings_back_what_an_older_version_dropped() {
        let kept = serde_json::json!({"theme": "dark", "animations": false, "check_updates": true, "language": "fr"});
        // the older version: no "animations", the theme changed, update checks turned off
        let now = serde_json::json!({"theme": "light", "check_updates": false, "language": "fr"});
        let m = merge_kept(kept.clone(), Some(now));
        assert_eq!(m, serde_json::json!({"theme": "light", "animations": false, "check_updates": true, "language": "fr"}));
        assert_eq!(merge_kept(kept.clone(), None), kept);
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
