//! The `system` file: character slots, typed access and the edit buffer.
//!
//! Layout notes are in ../../../docs (01 container, 11 whole-file map). Offsets in this
//! crate are relative to a character base unless named `abs`.

use std::fmt;
use std::path::PathBuf;

pub const FILE_SIZE: usize = 5_159_100;
/// u32 changed on every in-game save; not a checksum. Never written by the editor.
pub const HEADER_NONCE: usize = 0x14;
/// 3 x u8: character slot in use.
const SLOT_USED: usize = 0x28;
/// 3 x u32 slot pointers, relative to 0x24.
const SLOT_PTR: usize = 0x34;
const SLOT_PTR_BASE: usize = 0x24;
/// Slot 1 base; the absolute offsets quoted in the docs are slot 1's.
pub const SLOT1_BASE: usize = 0x18CC9C;
/// Bytes of one character slot (the stride between slot bases, docs/11).
pub const SLOT_SIZE: usize = 0x11F8C4;

#[derive(Debug)]
pub enum Error {
    Size(usize),
    SlotPointer(usize),
    Io(std::io::Error),
    /// An I/O error on one file of the save.
    File(PathBuf, std::io::Error),
    /// A copy of the save that is not save-sized.
    CopySize(PathBuf, usize),
    /// A copy changed on disk since the save was opened (the game saved meanwhile).
    ChangedOnDisk(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Size(n) => write!(f, "not an MHGU Switch save: {n} bytes, expected {FILE_SIZE}"),
            Error::SlotPointer(s) => write!(f, "character slot {} has an invalid pointer", s + 1),
            Error::Io(e) => write!(f, "{e}"),
            Error::File(p, e) => write!(f, "{}: {e}", p.display()),
            Error::CopySize(p, n) => write!(f, "{} is {n} bytes, not a save copy", p.display()),
            Error::ChangedOnDisk(p) => write!(f, "{} changed on disk since it was opened", p.display()),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// The whole file. `orig` is what was read; `buf` carries the edits.
#[derive(Clone)]
pub struct Save {
    orig: Vec<u8>,
    buf: Vec<u8>,
    bases: [usize; 3],
}

impl Save {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() != FILE_SIZE {
            return Err(Error::Size(bytes.len()));
        }
        let mut bases = [0; 3];
        for (s, b) in bases.iter_mut().enumerate() {
            let p = u32::from_le_bytes(bytes[SLOT_PTR + 4 * s..SLOT_PTR + 4 * s + 4].try_into().unwrap());
            *b = SLOT_PTR_BASE + p as usize;
            // the whole slot must fit, or reading it would run past the end
            if *b + SLOT_SIZE > FILE_SIZE {
                return Err(Error::SlotPointer(s));
            }
        }
        Ok(Save { orig: bytes.clone(), buf: bytes, bases })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn original(&self) -> &[u8] {
        &self.orig
    }

    pub fn slot_used(&self, slot: usize) -> bool {
        self.buf[SLOT_USED + slot] != 0
    }

    pub fn base(&self, slot: usize) -> usize {
        self.bases[slot]
    }

    /// Byte ranges that differ from the file as read, as (abs offset, old, new).
    pub fn diff(&self) -> Vec<(usize, u8, u8)> {
        self.orig.iter().zip(&self.buf).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (a, b))| (i, *a, *b)).collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.orig != self.buf
    }

    /// Forget the edits (after a successful write the written bytes become the original).
    pub fn commit(&mut self) {
        self.orig.clone_from(&self.buf);
    }

    pub fn revert(&mut self) {
        self.buf.clone_from(&self.orig);
    }

    // --- absolute access -------------------------------------------------------------

    pub fn get(&self, abs: usize, len: usize) -> &[u8] {
        &self.buf[abs..abs + len]
    }

    pub fn get_orig(&self, abs: usize, len: usize) -> &[u8] {
        &self.orig[abs..abs + len]
    }

    pub fn put(&mut self, abs: usize, data: &[u8]) {
        assert!(abs >= 0x24, "the container header is never written");
        self.buf[abs..abs + data.len()].copy_from_slice(data);
    }

    pub fn u8(&self, abs: usize) -> u8 {
        self.buf[abs]
    }

    pub fn u16(&self, abs: usize) -> u16 {
        u16::from_le_bytes(self.get(abs, 2).try_into().unwrap())
    }

    pub fn u32(&self, abs: usize) -> u32 {
        u32::from_le_bytes(self.get(abs, 4).try_into().unwrap())
    }

    pub fn set_u8(&mut self, abs: usize, v: u8) {
        self.put(abs, &[v]);
    }

    pub fn set_u16(&mut self, abs: usize, v: u16) {
        self.put(abs, &v.to_le_bytes());
    }

    pub fn set_u32(&mut self, abs: usize, v: u32) {
        self.put(abs, &v.to_le_bytes());
    }

    /// Bit `i` of the LSB-first bitfield starting at byte `abs`.
    pub fn bit(&self, abs: usize, i: usize) -> bool {
        self.buf[abs + (i >> 3)] >> (i & 7) & 1 != 0
    }

    pub fn set_bit(&mut self, abs: usize, i: usize, v: bool) {
        let o = abs + (i >> 3);
        let m = 1u8 << (i & 7);
        let b = if v { self.buf[o] | m } else { self.buf[o] & !m };
        self.set_u8(o, b);
    }

    /// `width`-bit unsigned field at bit `pos` of the LSB-first bit stream at `abs`.
    pub fn bits(&self, abs: usize, pos: usize, width: usize) -> u32 {
        let mut v = 0u32;
        for k in 0..width {
            v |= (self.bit(abs, pos + k) as u32) << k;
        }
        v
    }

    pub fn set_bits(&mut self, abs: usize, pos: usize, width: usize, v: u32) {
        for k in 0..width {
            self.set_bit(abs, pos + k, v >> k & 1 != 0);
        }
    }

    /// NUL-terminated UTF-8 string of at most `len` bytes.
    pub fn str(&self, abs: usize, len: usize) -> String {
        let b = self.get(abs, len);
        let end = b.iter().position(|&c| c == 0).unwrap_or(len);
        String::from_utf8_lossy(&b[..end]).into_owned()
    }

    /// Writes `s` truncated to `len - 1` bytes on a char boundary, NUL-padded.
    pub fn set_str(&mut self, abs: usize, len: usize, s: &str) {
        let mut end = s.len().min(len - 1);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut v = vec![0u8; len];
        v[..end].copy_from_slice(&s.as_bytes()[..end]);
        self.put(abs, &v);
    }
}

/// An all-zero save with the three slot pointers of a real one, for tests.
#[cfg(test)]
pub(crate) fn blank() -> Save {
    let mut v = vec![0u8; FILE_SIZE];
    for (s, b) in [0x18CC9C, 0x2AC560, 0x3CBE24].iter().enumerate() {
        v[SLOT_PTR + 4 * s..SLOT_PTR + 4 * s + 4].copy_from_slice(&((b - SLOT_PTR_BASE) as u32).to_le_bytes());
    }
    Save::from_bytes(v).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_bases_follow_pointers() {
        let s = blank();
        assert_eq!(s.base(0), SLOT1_BASE);
        assert_eq!(s.base(1) - s.base(0), 0x11F8C4);
    }

    #[test]
    fn slot_past_the_end_is_refused() {
        let mut v = blank().bytes().to_vec();
        let p = (FILE_SIZE - SLOT_SIZE + 1 - SLOT_PTR_BASE) as u32;
        v[SLOT_PTR + 8..SLOT_PTR + 12].copy_from_slice(&p.to_le_bytes());
        assert!(matches!(Save::from_bytes(v), Err(Error::SlotPointer(2))));
        assert!(matches!(Save::from_bytes(vec![0; 10]), Err(Error::Size(10))));
    }

    #[test]
    fn bit_stream_round_trip() {
        let mut s = blank();
        let a = SLOT1_BASE + 0x278;
        s.set_bits(a, 19 * 5, 12, 0xABC);
        s.set_bits(a, 19 * 5 + 12, 7, 99);
        assert_eq!(s.bits(a, 19 * 5, 12), 0xABC);
        assert_eq!(s.bits(a, 19 * 5 + 12, 7), 99);
        assert_eq!(s.bits(a, 19 * 4, 19), 0);
        assert_eq!(s.bits(a, 19 * 6, 19), 0);
    }

    #[test]
    fn strings_truncate_on_char_boundary() {
        let mut s = blank();
        s.set_str(SLOT1_BASE, 4, "aé€");
        assert_eq!(s.str(SLOT1_BASE, 4), "aé");
    }

    #[test]
    fn diff_and_revert() {
        let mut s = blank();
        s.set_u16(SLOT1_BASE + 0x28, 999);
        assert_eq!(s.diff().len(), 2);
        s.revert();
        assert!(!s.is_dirty());
    }
}
