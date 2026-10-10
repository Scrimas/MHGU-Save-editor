//! Whole character slots: copy, swap, delete, export and import.
//!
//! A slot is self-contained (docs/11-save-map.md, "File layout"): the three slots share
//! one layout at a fixed stride, and making a character in game wrote only its own slot
//! and the slot-use byte (CONFIRMED with two new characters). So a slot's bytes moved to
//! another slot, or to another save, are that character there. Copy, swap, delete (the
//! slot-use byte and `LAST_PLAYED`) and import of an editor character file (the same write
//! as a copy) are CONFIRMED in game (2026-10-10); import of an MHXX character stays
//! DERIVED.

use crate::character;
use crate::save::{Save, LAST_PLAYED, SLOT_SIZE};
use std::fmt;

/// Bytes of a slot: the stride's last byte is alignment, and slot 3 ends before it.
pub const LEN: usize = SLOT_SIZE - 1;
/// Start of an exported character file, then the slot's `LEN` bytes.
pub const MAGIC: &[u8; 8] = b"MHGUCHR1";

/// A file that is not an exported character.
#[derive(Debug, PartialEq, Eq)]
pub struct NotACharacter;

impl fmt::Display for NotACharacter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "not a character exported by this editor")
    }
}

impl std::error::Error for NotACharacter {}

/// A slot no character was made in, as the game writes it (tools/empty_slot.py).
pub fn empty() -> Vec<u8> {
    let v = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../../data/empty-slot.zlib")).expect("empty-slot.zlib is a zlib stream");
    assert_eq!(v.len(), LEN, "empty-slot.zlib holds one slot");
    v
}

fn bytes(s: &Save, slot: usize) -> Vec<u8> {
    s.get(s.base(slot), LEN).to_vec()
}

/// The character of `from` also in `to`, over what `to` held.
pub fn copy(s: &mut Save, from: usize, to: usize) {
    if from != to {
        let b = bytes(s, from);
        s.put(s.base(to), &b);
        s.set_slot_used(to, s.slot_used(from));
    }
}

/// The two slots trade places; the slot played last follows its character.
pub fn swap(s: &mut Save, a: usize, b: usize) {
    if a == b {
        return;
    }
    let (x, y) = (bytes(s, a), bytes(s, b));
    let (ua, ub) = (s.slot_used(a), s.slot_used(b));
    s.put(s.base(a), &y);
    s.put(s.base(b), &x);
    s.set_slot_used(a, ub);
    s.set_slot_used(b, ua);
    let last = s.u8(LAST_PLAYED) as usize;
    if last == a || last == b {
        s.set_u8(LAST_PLAYED, (a + b - last) as u8);
    }
}

/// The slot as one never used: no character, the game's initial bytes. The slot played
/// last moves to the first slot still in use.
pub fn delete(s: &mut Save, slot: usize) {
    s.put(s.base(slot), &empty());
    s.set_slot_used(slot, false);
    if s.u8(LAST_PLAYED) as usize == slot {
        let first = (0..3).find(|&k| s.slot_used(k)).unwrap_or(0);
        s.set_u8(LAST_PLAYED, first as u8);
    }
}

/// The character of `slot` as a file.
pub fn export(s: &Save, slot: usize) -> Vec<u8> {
    [MAGIC.as_slice(), s.get(s.base(slot), LEN)].concat()
}

/// The slot bytes of a file made by `export`.
fn body(file: &[u8]) -> Result<&[u8], NotACharacter> {
    file.strip_prefix(MAGIC.as_slice()).filter(|b| b.len() == LEN).ok_or(NotACharacter)
}

/// The name of the character in a file made by `export`.
pub fn name_of(file: &[u8]) -> Result<String, NotACharacter> {
    let n = &body(file)?[character::PLAYER_NAME..character::PLAYER_NAME + character::NAME_LEN];
    let end = n.iter().position(|&c| c == 0).unwrap_or(n.len());
    Ok(String::from_utf8_lossy(&n[..end]).into_owned())
}

/// A file made by `export` becomes the character of `slot`, over what it held.
pub fn import(s: &mut Save, slot: usize, file: &[u8]) -> Result<(), NotACharacter> {
    put(s, slot, body(file)?);
    Ok(())
}

/// The slot bytes `b` (`LEN`, as `export` or `mhxx::Mhxx::character` give them) become
/// the character of `slot`.
pub fn put(s: &mut Save, slot: usize, b: &[u8]) {
    assert_eq!(b.len(), LEN, "one slot");
    s.put(s.base(slot), b);
    s.set_slot_used(slot, true);
}

/// The bytes of `slot`, `LEN` of them.
pub fn get(s: &Save, slot: usize) -> &[u8] {
    s.get(s.base(slot), LEN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::blank;

    /// Slots 1 and 2 in use, each marked at its start and end.
    fn two() -> Save {
        let mut s = blank();
        for k in 0..2 {
            s.set_slot_used(k, true);
            s.set_u8(s.base(k), 10 + k as u8);
            s.set_u8(s.base(k) + LEN - 1, 20 + k as u8);
        }
        s.set_u8(LAST_PLAYED, 1);
        s
    }

    #[test]
    fn copy_takes_every_byte_and_the_use_byte() {
        let mut s = two();
        copy(&mut s, 0, 2);
        assert!(s.slot_used(2));
        assert_eq!(s.get(s.base(2), LEN), s.get(s.base(0), LEN));
        // slot 3's last byte is the file's last slot byte, and the zero tail stays
        assert_eq!(s.u8(s.base(2) + LEN), 0);
    }

    #[test]
    fn swap_moves_the_last_played_slot_with_its_character() {
        let mut s = two();
        swap(&mut s, 1, 2);
        assert_eq!((s.slot_used(1), s.slot_used(2)), (false, true));
        assert_eq!((s.u8(s.base(2)), s.u8(s.base(2) + LEN - 1)), (11, 21));
        assert_eq!(s.u8(s.base(1)), 0);
        assert_eq!(s.u8(LAST_PLAYED), 2);
    }

    #[test]
    fn delete_writes_the_empty_slot() {
        let mut s = two();
        delete(&mut s, 1);
        assert!(!s.slot_used(1));
        assert_eq!(s.get(s.base(1), LEN), empty().as_slice());
        assert_eq!(s.u8(LAST_PLAYED), 0);
        // slot 1 untouched
        assert_eq!(s.u8(s.base(0)), 10);
    }

    #[test]
    fn export_import_round_trip() {
        let mut s = two();
        s.set_str(s.base(1) + character::PLAYER_NAME, character::NAME_LEN, "Scrimas2");
        let f = export(&s, 1);
        assert_eq!(name_of(&f).as_deref(), Ok("Scrimas2"));
        assert_eq!(f.len(), MAGIC.len() + LEN);
        import(&mut s, 2, &f).unwrap();
        assert!(s.slot_used(2));
        assert_eq!(s.get(s.base(2), LEN), s.get(s.base(1), LEN));
        assert_eq!(import(&mut s, 2, &f[..100]), Err(NotACharacter));
        assert_eq!(import(&mut s, 2, &[b"XXXXXXXX".as_slice(), &f[8..]].concat()), Err(NotACharacter));
    }
}
