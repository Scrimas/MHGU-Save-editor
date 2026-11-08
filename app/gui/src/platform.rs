//! Asking for files to read and write: rfd's dialogs on the desktop (the answer comes
//! before the call returns), Android's document picker there (the answer comes later, on
//! the UI thread: android/picker.rs). Callers hand over what to do with the answer; a
//! cancelled dialog calls nothing.

use std::path::{Path, PathBuf};

/// A kind of file: its name in the dialog and its extensions ("*": any file).
pub type Kind<'a> = (&'a str, &'a [&'a str]);

/// Asks for a file to open; `done` gets its path (Android: the name the picker shows) and
/// its bytes, or why they could not be read.
pub fn open_file(title: &str, kinds: &[Kind], dir: Option<&Path>, done: impl FnOnce(Result<(PathBuf, Vec<u8>), String>) + 'static) {
    #[cfg(not(target_os = "android"))]
    {
        let mut d = rfd::FileDialog::new().set_title(title);
        for (name, exts) in kinds {
            d = d.add_filter(*name, exts);
        }
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        let Some(p) = d.pick_file() else { return };
        let r = std::fs::read(&p).map_err(|e| crate::i18n::trf("Could not read {}: {}", &[&p.display(), &e]));
        done(r.map(|b| (p, b)));
    }
    #[cfg(target_os = "android")]
    {
        let _ = (title, dir);
        crate::android::picker::open_file(kinds, Box::new(done));
    }
}

/// Asks where to save `bytes` as a file of `kind`, offering `name`; `done` gets Ok or why
/// the file could not be written.
pub fn save_file(title: &str, kind: Kind, name: &str, dir: Option<&Path>, bytes: Vec<u8>, done: impl FnOnce(Result<(), String>) + 'static) {
    #[cfg(not(target_os = "android"))]
    {
        let mut d = rfd::FileDialog::new().set_title(title).add_filter(kind.0, kind.1).set_file_name(name);
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        let Some(p) = d.save_file() else { return };
        done(std::fs::write(&p, bytes).map_err(|e| crate::i18n::trf("Could not write {}: {}", &[&p.display(), &e])));
    }
    #[cfg(target_os = "android")]
    {
        let _ = (title, dir);
        crate::android::picker::save_file(kind, name, bytes, Box::new(done));
    }
}
