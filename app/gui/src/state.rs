//! Editor state: the open save, the edit list and undo.
//!
//! Every edit is recorded as the bytes it wrote. Undoing edit k rebuilds the buffer
//! from the file as read and replays every other edit, so edits that touch the same
//! bytes stay consistent.

use mhgu_save::store::{self, CopyState, Location};
use mhgu_save::Save;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conf {
    Confirmed,
    Derived,
    Unresolved,
}

#[derive(Clone, Debug)]
pub struct Op {
    pub id: i32,
    /// Edits with the same key merge (e.g. clicking a spin box ten times).
    pub key: String,
    pub title: String,
    pub detail: String,
    pub conf: Conf,
    pub slot: usize,
    pub bytes: Vec<(usize, u8)>,
}

pub struct Doc {
    pub save: Save,
    pub loc: Location,
    pub copies: Vec<(PathBuf, CopyState)>,
}

#[derive(Default)]
pub struct State {
    pub doc: Option<Doc>,
    pub slot: usize,
    pub ops: Vec<Op>,
    next_id: i32,
}

impl State {
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let (save, loc, copies) = store::open(path).map_err(|e| e.to_string())?;
        self.slot = (0..3).find(|&s| save.slot_used(s)).unwrap_or(0);
        self.doc = Some(Doc { save, loc, copies });
        self.ops.clear();
        Ok(())
    }

    pub fn save(&self) -> &Save {
        &self.doc.as_ref().unwrap().save
    }

    pub fn base(&self) -> usize {
        self.save().base(self.slot)
    }

    /// Run `f` on the save and record what it changed as one edit.
    pub fn edit(&mut self, key: &str, title: String, detail: String, conf: Conf, f: impl FnOnce(&mut Save, usize)) {
        let slot = self.slot;
        let Some(doc) = self.doc.as_mut() else { return };
        let before = doc.save.bytes().to_vec();
        let base = doc.save.base(slot);
        f(&mut doc.save, base);
        let after = doc.save.bytes();
        let bytes: Vec<(usize, u8)> = before.iter().zip(after).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (_, b))| (i, *b)).collect();
        if bytes.is_empty() && !self.ops.last().is_some_and(|o| o.key == key) {
            return;
        }
        if let Some(last) = self.ops.last_mut().filter(|o| o.key == key && !key.is_empty()) {
            for (a, v) in bytes {
                match last.bytes.iter_mut().find(|(x, _)| *x == a) {
                    Some(e) => e.1 = v,
                    None => last.bytes.push((a, v)),
                }
            }
            // an edit that went back to the original value disappears
            let orig = doc.save.original();
            last.bytes.retain(|&(a, v)| orig[a] != v);
            last.title = title;
            last.detail = detail;
            if last.bytes.is_empty() {
                self.ops.pop();
                self.replay();
            }
            return;
        }
        self.next_id += 1;
        self.ops.push(Op { id: self.next_id, key: key.into(), title, detail, conf, slot, bytes });
    }

    fn replay(&mut self) {
        let Some(doc) = self.doc.as_mut() else { return };
        doc.save.revert();
        for op in &self.ops {
            for &(a, v) in &op.bytes {
                doc.save.put(a, &[v]);
            }
        }
    }

    pub fn undo(&mut self, id: i32) {
        self.ops.retain(|o| o.id != id);
        self.replay();
    }

    pub fn undo_all(&mut self) {
        self.ops.clear();
        self.replay();
    }

    /// Absolute offsets touched by pending edits (to highlight changed rows).
    pub fn changed(&self, abs: usize, len: usize) -> bool {
        let Some(doc) = self.doc.as_ref() else { return false };
        doc.save.bytes()[abs..abs + len] != doc.save.original()[abs..abs + len]
    }
}
