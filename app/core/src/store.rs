//! Finding the save on disk, snapshots, and writing every copy.
//!
//! Ryujinx keeps the title's savedata as `<save-id>/0/` and `<save-id>/1/` (a
//! double-commit scheme), each with `system` and `system_backup`. All four files are
//! written with the same body; each keeps its own header (the nonce at 0x14 differs
//! between them and is left alone). See docs/01-container.md.

use crate::save::{Error, Save, FILE_SIZE};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Bytes before this offset are the container header and are never written.
pub const HEADER: usize = 0x24;
pub const FILES: [&str; 2] = ["system", "system_backup"];

#[derive(Debug, Clone)]
pub struct Location {
    /// The file the user opened.
    pub opened: PathBuf,
    /// Every copy that will be written: `0/system`, `0/system_backup`, `1/system`, ...
    pub copies: Vec<PathBuf>,
    /// Folder holding `0/` and `1/`, when the opened file sits in that layout.
    pub save_dir: Option<PathBuf>,
}

impl Location {
    pub fn of(opened: &Path) -> Location {
        let opened = opened.to_path_buf();
        let commit = opened.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str());
        let save_dir = match commit {
            Some("0") | Some("1") => opened.parent().and_then(Path::parent).map(Path::to_path_buf),
            _ => None,
        };
        let mut copies = vec![];
        match &save_dir {
            Some(d) => {
                for c in ["0", "1"] {
                    for f in FILES {
                        let p = d.join(c).join(f);
                        if p.is_file() {
                            copies.push(p);
                        }
                    }
                }
            }
            None => {
                copies.push(opened.clone());
                if let Some(dir) = opened.parent() {
                    let other = if opened.file_name().is_some_and(|n| n == "system_backup") { "system" } else { "system_backup" };
                    let p = dir.join(other);
                    if p.is_file() {
                        copies.push(p);
                    }
                }
            }
        }
        if !copies.contains(&opened) {
            copies.insert(0, opened.clone());
        }
        Location { opened, copies, save_dir }
    }
}

/// How the copies on disk relate to the opened one.
#[derive(Debug, Clone, PartialEq)]
pub enum CopyState {
    Same,
    /// Body differs from the opened file (an older or newer commit).
    Differs,
    Missing,
    WrongSize(usize),
}

pub fn open(path: &Path) -> Result<(Save, Location, Vec<(PathBuf, CopyState)>), Error> {
    let save = Save::from_bytes(fs::read(path)?)?;
    let loc = Location::of(path);
    let states = loc.copies.iter().map(|p| (p.clone(), copy_state(&save, p))).collect();
    Ok((save, loc, states))
}

fn copy_state(save: &Save, p: &Path) -> CopyState {
    match fs::read(p) {
        Err(_) => CopyState::Missing,
        Ok(b) if b.len() != FILE_SIZE => CopyState::WrongSize(b.len()),
        Ok(b) if b[HEADER..] == save.original()[HEADER..] => CopyState::Same,
        Ok(_) => CopyState::Differs,
    }
}

/// Copies every file of the save (both commit folders) into `dest/<timestamp>/`.
pub fn snapshot(loc: &Location, dest_root: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    let dest = dest_root.join(stamp);
    for p in &loc.copies {
        let rel = match &loc.save_dir {
            Some(d) => p.strip_prefix(d).unwrap_or(p).to_path_buf(),
            None => PathBuf::from(p.file_name().unwrap()),
        };
        let to = dest.join(rel);
        fs::create_dir_all(to.parent().unwrap())?;
        fs::copy(p, &to)?;
    }
    Ok(dest)
}

/// Copies a snapshot made by `snapshot` back over the save: every file it holds goes back
/// to its place, whole (headers too), through a temporary file and a rename. Files of
/// the wrong size are refused before anything is written. Returns the files restored.
pub fn restore(loc: &Location, snap: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut pairs = vec![];
    for p in &loc.copies {
        let rel = match &loc.save_dir {
            Some(d) => p.strip_prefix(d).unwrap_or(p).to_path_buf(),
            None => PathBuf::from(p.file_name().unwrap()),
        };
        let from = snap.join(rel);
        if !from.is_file() {
            continue;
        }
        let n = fs::metadata(&from)?.len() as usize;
        if n != FILE_SIZE {
            return Err(std::io::Error::other(format!("{} is {n} bytes, not a save", from.display())));
        }
        pairs.push((from, p.clone()));
    }
    if pairs.is_empty() {
        return Err(std::io::Error::other(format!("no save files in {}", snap.display())));
    }
    let mut done = vec![];
    for (from, to) in pairs {
        let b = fs::read(&from)?;
        let tmp = to.with_extension("mhgu-editor-tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&b)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &to)?;
        if fs::read(&to)? != b {
            return Err(std::io::Error::other(format!("read-back mismatch in {}", to.display())));
        }
        done.push(to);
    }
    Ok(done)
}

/// Writes the edited body into every copy, keeping each file's own header, through a
/// temporary file and a rename. Returns the files written.
pub fn write_all(save: &mut Save, loc: &Location) -> Result<Vec<PathBuf>, Error> {
    let mut done = vec![];
    for p in &loc.copies {
        let mut b = fs::read(p)?;
        if b.len() != FILE_SIZE {
            return Err(Error::Size(b.len()));
        }
        b[HEADER..].copy_from_slice(&save.bytes()[HEADER..]);
        let tmp = p.with_extension("mhgu-editor-tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&b)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, p)?;
        // read back
        if fs::read(p)?[HEADER..] != save.bytes()[HEADER..] {
            return Err(Error::Io(std::io::Error::other(format!("read-back mismatch in {}", p.display()))));
        }
        done.push(p.clone());
    }
    save.commit();
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(dir: &Path, nonce: u8, body: u8) {
        for c in ["0", "1"] {
            fs::create_dir_all(dir.join(c)).unwrap();
            for (k, f) in FILES.iter().enumerate() {
                let mut v = vec![body; FILE_SIZE];
                v[..HEADER].fill(0);
                v[0x34..0x40].copy_from_slice(&[0x78, 0xCC, 0x18, 0, 0x3C, 0xC5, 0x2A, 0, 0x00, 0xBE, 0x3C, 0]);
                v[0x14] = nonce + k as u8;
                fs::write(dir.join(c).join(f), v).unwrap();
            }
        }
    }

    #[test]
    fn writes_four_copies_and_keeps_headers() {
        let dir = std::env::temp_dir().join(format!("mhgu-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fake(&dir, 7, 0);
        let (mut s, loc, states) = open(&dir.join("0/system")).unwrap();
        assert_eq!(loc.copies.len(), 4);
        assert!(states.iter().all(|(_, st)| *st == CopyState::Same));
        let snap = snapshot(&loc, &dir.join("snap"), "t").unwrap();
        assert!(snap.join("1/system_backup").is_file());
        s.set_u16(0x18CC9C + 0x28, 0x1234);
        let w = write_all(&mut s, &loc).unwrap();
        assert_eq!(w.len(), 4);
        for (k, f) in FILES.iter().enumerate() {
            let b = fs::read(dir.join("1").join(f)).unwrap();
            assert_eq!(b[0x14], 7 + k as u8, "nonce untouched");
            assert_eq!(&b[0x18CC9C + 0x28..0x18CC9C + 0x2A], &[0x34, 0x12]);
        }
        assert!(!s.is_dirty());
        // the snapshot puts every file back, headers included
        let r = restore(&loc, &snap).unwrap();
        assert_eq!(r.len(), 4);
        for (k, f) in FILES.iter().enumerate() {
            let b = fs::read(dir.join("1").join(f)).unwrap();
            assert_eq!(b[0x14], 7 + k as u8);
            assert_eq!(&b[0x18CC9C + 0x28..0x18CC9C + 0x2A], &[0, 0]);
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
