//! Editor state: the open save, the edit list and undo.
//!
//! Every edit is recorded as the bytes it wrote and the values it changed. Undoing edit k
//! rebuilds the buffer from the file as read and replays every other edit, so edits that
//! touch the same bytes stay consistent. One value of a bulk edit can be put back on its
//! own: its bytes go back to the file's inside that edit.

use crate::targets::Target;
use mhgu_save::store::{self, CopyState, Location};
use mhgu_save::Save;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conf {
    Confirmed,
    Derived,
    Unresolved,
}

/// What an edit is, before it runs.
pub struct Edit {
    /// Edits with the same key merge (e.g. clicking a spin box ten times); "" never merges.
    pub key: String,
    pub title: String,
    /// Second line in Review ("Quick goal", "Item box").
    pub detail: String,
    /// What else it does, in game terms ("Also rebuilds the Guild Card monster log").
    pub note: String,
    pub conf: Conf,
    /// The values it changes; the edit function can add more.
    pub targets: Vec<Target>,
}

impl Edit {
    /// An edit of one value, titled with its name.
    pub fn one(t: Target, title: String, conf: Conf) -> Edit {
        Edit { key: t.key(), title, detail: String::new(), note: String::new(), conf, targets: vec![t] }
    }

    pub fn note(mut self, n: &str) -> Edit {
        self.note = n.into();
        self
    }
}

#[derive(Clone, Debug)]
pub struct Op {
    pub id: i32,
    pub key: String,
    pub title: String,
    pub detail: String,
    pub note: String,
    pub conf: Conf,
    pub slot: usize,
    /// The values it changed, each with its value right after the edit.
    pub values: Vec<(Target, String)>,
    pub bytes: Vec<(usize, u8)>,
}

pub struct Doc {
    pub save: Save,
    /// The save as read (or as last written), for the "was" values.
    pub orig: Save,
    pub loc: Location,
    pub copies: Vec<(PathBuf, CopyState)>,
}

#[derive(Default)]
pub struct State {
    pub doc: Option<Doc>,
    pub slot: usize,
    pub ops: Vec<Op>,
    /// Edits removed by Undo all, until the next edit (Redo).
    pub redo: Vec<Op>,
    next_id: i32,
}

impl State {
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let (save, loc, copies) = store::open(path).map_err(|e| e.to_string())?;
        self.slot = (0..3).find(|&s| save.slot_used(s)).unwrap_or(0);
        let orig = save.clone();
        self.doc = Some(Doc { save, orig, loc, copies });
        self.ops.clear();
        self.redo.clear();
        Ok(())
    }

    pub fn save(&self) -> &Save {
        &self.doc.as_ref().unwrap().save
    }

    pub fn orig(&self) -> &Save {
        &self.doc.as_ref().unwrap().orig
    }

    pub fn base(&self) -> usize {
        self.save().base(self.slot)
    }

    /// Run `f` on the save and record what it changed as one edit; `f` returns the values
    /// it changed beyond `e.targets`. Returns whether an edit holds it afterwards. With
    /// Confirmed only on (Settings), edits not confirmed in game are refused.
    pub fn edit(&mut self, e: Edit, f: impl FnOnce(&mut Save, usize) -> Vec<Target>) -> bool {
        if e.conf != Conf::Confirmed && crate::settings::get().confirmed_only {
            return false;
        }
        let slot = self.slot;
        let Some(doc) = self.doc.as_mut() else { return false };
        let before = doc.save.bytes().to_vec();
        let base = doc.save.base(slot);
        let mut targets = e.targets;
        for t in f(&mut doc.save, base) {
            if !targets.contains(&t) {
                targets.push(t);
            }
        }
        let after = doc.save.bytes();
        let bytes: Vec<(usize, u8)> = before.iter().zip(after).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (_, b))| (i, *b)).collect();
        let merge = !e.key.is_empty() && self.ops.last().is_some_and(|o| o.key == e.key && o.slot == slot);
        if bytes.is_empty() && !merge {
            return false;
        }
        self.redo.clear();
        let values: Vec<(Target, String)> = targets.iter().map(|t| (*t, t.read(&doc.save, slot))).collect();
        if merge {
            let last = self.ops.last_mut().unwrap();
            for (a, v) in bytes {
                match last.bytes.iter_mut().find(|(x, _)| *x == a) {
                    Some(b) => b.1 = v,
                    None => last.bytes.push((a, v)),
                }
            }
            // an edit that went back to the original value disappears
            let orig = doc.save.original();
            last.bytes.retain(|&(a, v)| orig[a] != v);
            last.title = e.title;
            last.detail = e.detail;
            last.note = e.note;
            for (t, v) in values {
                match last.values.iter_mut().find(|(x, _)| *x == t) {
                    Some(x) => x.1 = v,
                    None => last.values.push((t, v)),
                }
            }
            if last.bytes.is_empty() {
                self.ops.pop();
                self.replay();
                return false;
            }
            return true;
        }
        self.next_id += 1;
        self.ops.push(Op { id: self.next_id, key: e.key, title: e.title, detail: e.detail, note: e.note, conf: e.conf, slot, values, bytes });
        true
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

    pub fn undo_all(&mut self) -> usize {
        let n = self.ops.len();
        if n > 0 {
            self.redo = std::mem::take(&mut self.ops);
            self.replay();
        }
        n
    }

    /// Bring back what Undo all removed, if nothing was edited since.
    pub fn redo_all(&mut self) -> usize {
        if !self.ops.is_empty() {
            return 0;
        }
        self.ops = std::mem::take(&mut self.redo);
        self.replay();
        self.ops.len()
    }

    /// The value with this key goes back to the file's value: its latest edit is undone
    /// when that edit changed only it, otherwise its bytes are restored inside that edit.
    pub fn undo_value(&mut self, key: &str) {
        let slot = self.slot;
        let Some(k) = self.ops.iter().rposition(|o| o.slot == slot && o.values.iter().any(|(t, _)| t.key() == key)) else { return };
        if self.ops[k].values.len() == 1 {
            let id = self.ops[k].id;
            return self.undo(id);
        }
        let Some(doc) = self.doc.as_mut() else { return };
        let t = self.ops[k].values.iter().find(|(t, _)| t.key() == key).unwrap().0;
        let before = doc.save.bytes().to_vec();
        t.restore(&mut doc.save, &doc.orig, slot);
        let op = &mut self.ops[k];
        for (a, (b, n)) in before.iter().zip(doc.save.bytes()).enumerate() {
            if b != n {
                match op.bytes.iter_mut().find(|(x, _)| *x == a) {
                    Some(e) => e.1 = *n,
                    None => op.bytes.push((a, *n)),
                }
            }
        }
        let orig = doc.save.original();
        op.bytes.retain(|&(a, v)| orig[a] != v);
        op.values.retain(|(x, _)| x.key() != key);
        if op.bytes.is_empty() || op.values.is_empty() {
            self.ops.remove(k);
        }
        self.replay();
    }

    /// After a write the written bytes are the new original.
    pub fn written(&mut self) {
        self.ops.clear();
        self.redo.clear();
        if let Some(doc) = self.doc.as_mut() {
            doc.orig = doc.save.clone();
            doc.copies = doc.loc.copies.iter().map(|p| (p.clone(), CopyState::Same)).collect();
        }
    }

    /// Absolute offsets touched by pending edits (to mark changed rows).
    pub fn changed(&self, abs: usize, len: usize) -> bool {
        let Some(doc) = self.doc.as_ref() else { return false };
        doc.save.bytes()[abs..abs + len] != doc.save.original()[abs..abs + len]
    }

    /// The file's value of `t` in the current character when it differs from the staged
    /// one ("was 830"), else "".
    pub fn was(&self, t: Target) -> String {
        let (Some(doc), slot) = (self.doc.as_ref(), self.slot) else { return String::new() };
        let o = t.read(&doc.orig, slot);
        if o == t.read(&doc.save, slot) { String::new() } else { o }
    }

    /// Values not written yet: each staged (character, value) once, if it still differs
    /// from the file.
    pub fn staged(&self) -> Vec<(usize, Target)> {
        let Some(doc) = self.doc.as_ref() else { return vec![] };
        let mut out: Vec<(usize, Target)> = vec![];
        for op in &self.ops {
            for (t, _) in &op.values {
                if !out.contains(&(op.slot, *t)) && t.read(&doc.orig, op.slot) != t.read(&doc.save, op.slot) {
                    out.push((op.slot, *t));
                }
            }
        }
        out
    }
}
