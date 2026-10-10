//! MHXX saves (the 3DS game, and its Japanese Switch release): characters in and out.
//!
//! MHGU grew out of MHXX and keeps its save layout: the `system` file of MHXX is the MHGU
//! body without the 0x24-byte container header (the Switch release has one), and the
//! first `SHARED` bytes of a character slot are laid out alike in both games. Only the
//! chat phrases after them differ: 99 phrases of 60 bytes in MHXX, of 104 in MHGU
//! (`options::SHOUTOUTS`), each followed by the chat group in use. DERIVED from
//! SilverJolteon's MHXX/MHGU Save Manager (v1.5.1, `main.js`), whose conversion copies
//! the slot and the phrases this way; not checked in game with this editor yet.
//!
//! A 3DS save is read from a JKSM or Checkpoint backup of the game's extra data, which
//! those tools write decrypted.

use crate::character;
use crate::slots;
use std::fmt;

pub const SIZE_3DS: usize = 0x481D88;
pub const SIZE_SWITCH: usize = 0x481DAC;
/// Bytes of a character both games lay out alike, from the slot base.
pub const SHARED: usize = 0x11D088;
/// 3 chat groups of 24 phrases and 9 auto-shoutouts.
const PHRASES: usize = 99;
const XX_PHRASE: usize = 60;
const GU_PHRASE: usize = 104;
/// Chat bytes after `SHARED` that belong to the slot: a flag byte, the phrases, the u16
/// chat group. MHXX pads them to 0x1738 bytes, MHGU's slot ends after them.
const XX_CHAT: usize = 1 + PHRASES * XX_PHRASE + 2;
const GU_CHAT: usize = 1 + PHRASES * GU_PHRASE + 2;
/// Body offsets (after the Switch release's header): slots in use, slot pointers.
const SLOT_USED: usize = 0x04;
const SLOT_PTR: usize = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ds,
    Switch,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Not the size of an MHXX save.
    Size(usize),
    SlotPointer(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Size(n) => write!(f, "not an MHXX save: {n} bytes, expected {SIZE_3DS} (3DS) or {SIZE_SWITCH} (Switch)"),
            Error::SlotPointer(s) => write!(f, "character slot {} has an invalid pointer", s + 1),
        }
    }
}

impl std::error::Error for Error {}

/// An MHXX `system` file.
#[derive(Clone)]
pub struct Mhxx {
    bytes: Vec<u8>,
    /// The container header's size: 0 on 3DS, 0x24 on Switch.
    off: usize,
    bases: [usize; 3],
}

/// Copies the NUL-terminated string at the start of `from` into `to`, cut on a UTF-8
/// boundary so that a NUL still fits.
fn copy_str(from: &[u8], to: &mut [u8]) {
    let end = from.iter().position(|&c| c == 0).unwrap_or(from.len());
    let s = String::from_utf8_lossy(&from[..end]);
    let mut n = s.len().min(to.len() - 1);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    to.fill(0);
    to[..n].copy_from_slice(&s.as_bytes()[..n]);
}

/// Chat bytes of one game rewritten for the other (`from_xx`: MHXX to MHGU).
fn chat(from: &[u8], from_xx: bool) -> Vec<u8> {
    let (fp, tp, len) = if from_xx { (XX_PHRASE, GU_PHRASE, GU_CHAT) } else { (GU_PHRASE, XX_PHRASE, XX_CHAT) };
    let mut v = vec![0u8; len];
    v[0] = from[0];
    for i in 0..PHRASES {
        copy_str(&from[1 + fp * i..1 + fp * (i + 1)], &mut v[1 + tp * i..1 + tp * (i + 1)]);
    }
    v[len - 2..].copy_from_slice(&from[1 + fp * PHRASES..1 + fp * PHRASES + 2]);
    v
}

impl Mhxx {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let off = match bytes.len() {
            SIZE_3DS => 0,
            SIZE_SWITCH => 0x24,
            n => return Err(Error::Size(n)),
        };
        let mut bases = [0; 3];
        for (k, b) in bases.iter_mut().enumerate() {
            let at = off + SLOT_PTR + 4 * k;
            *b = off + u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            if *b + SHARED + XX_CHAT > bytes.len() {
                return Err(Error::SlotPointer(k));
            }
        }
        Ok(Mhxx { bytes, off, bases })
    }

    pub fn kind(&self) -> Kind {
        if self.off == 0 { Kind::Ds } else { Kind::Switch }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn slot_used(&self, k: usize) -> bool {
        self.bytes[self.off + SLOT_USED + k] != 0
    }

    fn slot(&self, k: usize) -> &[u8] {
        &self.bytes[self.bases[k]..self.bases[k] + SHARED]
    }

    /// The character's name, HR and play time in seconds (the slot header).
    pub fn summary(&self, k: usize) -> (String, u16, u32) {
        let h = self.slot(k);
        let n = &h[character::HDR_NAME..character::HDR_NAME + character::NAME_LEN];
        let end = n.iter().position(|&c| c == 0).unwrap_or(n.len());
        let at = |o: usize, w: usize| h[o..o + w].iter().rev().fold(0u32, |a, &b| a << 8 | b as u32);
        (String::from_utf8_lossy(&n[..end]).into_owned(), at(character::HDR_HR, 2) as u16, at(character::HDR_PLAYTIME, 4))
    }

    /// Character `k` as the bytes of an MHGU slot (`slots::LEN`).
    pub fn character(&self, k: usize) -> Vec<u8> {
        let b = self.bases[k];
        let mut v = self.bytes[b..b + SHARED].to_vec();
        v.extend(chat(&self.bytes[b + SHARED..b + SHARED + XX_CHAT], true));
        debug_assert_eq!(v.len(), slots::LEN);
        v
    }

    /// The MHGU slot `slot` (`slots::LEN` bytes) becomes character `k`, over what it held.
    pub fn put_character(&mut self, k: usize, slot: &[u8]) {
        assert_eq!(slot.len(), slots::LEN, "one MHGU slot");
        let b = self.bases[k];
        self.bytes[b..b + SHARED].copy_from_slice(&slot[..SHARED]);
        let c = chat(&slot[SHARED..SHARED + GU_CHAT], false);
        self.bytes[b + SHARED..b + SHARED + XX_CHAT].copy_from_slice(&c);
        self.bytes[self.off + SLOT_USED + k] = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options;
    use crate::save::{blank, Save};

    /// A 3DS save with the slot pointers of a clean one and character 1 in use.
    fn ds() -> Mhxx {
        let mut v = vec![0u8; SIZE_3DS];
        for (k, p) in [0x126474u32, 0x244C34, 0x3633F4].iter().enumerate() {
            v[SLOT_PTR + 4 * k..SLOT_PTR + 4 * k + 4].copy_from_slice(&p.to_le_bytes());
        }
        let b = 0x126474;
        v[SLOT_USED] = 1;
        v[b..b + 6].copy_from_slice(b"Hunter");
        v[b + character::HDR_HR..b + character::HDR_HR + 2].copy_from_slice(&999u16.to_le_bytes());
        v[b + SHARED - 1] = 0x5A;
        let c = b + SHARED;
        v[c] = 3;
        v[c + 1..c + 7].copy_from_slice(b"Hello!");
        // the last auto-shoutout and the chat group
        v[c + 1 + 98 * XX_PHRASE..c + 1 + 98 * XX_PHRASE + 3].copy_from_slice(b"Bye");
        v[c + 1 + 99 * XX_PHRASE] = 2;
        Mhxx::from_bytes(v).unwrap()
    }

    #[test]
    fn mhxx_character_becomes_an_mhgu_slot() {
        let x = ds();
        assert_eq!(x.kind(), Kind::Ds);
        assert_eq!(x.summary(0), ("Hunter".to_string(), 999, 0));
        assert!(x.slot_used(0) && !x.slot_used(1));
        let slot = x.character(0);
        assert_eq!(slot.len(), slots::LEN);
        let mut s: Save = blank();
        slots::put(&mut s, 1, &slot);
        let base = s.base(1);
        assert!(s.slot_used(1));
        assert_eq!(s.u8(base + SHARED - 1), 0x5A);
        assert_eq!(options::phrase(&s, base, false, 0, 0), "Hello!");
        assert_eq!(options::phrase(&s, base, true, 2, 8), "Bye");
        assert_eq!(options::chat_group(&s, base), 2);
    }

    #[test]
    fn mhgu_slot_goes_back_into_mhxx() {
        let mut x = ds();
        let mut s: Save = blank();
        slots::put(&mut s, 0, &x.character(0));
        let base = s.base(0);
        // a phrase longer than MHXX's 60 bytes is cut on a character boundary
        options::set_phrase(&mut s, base, false, 1, 3, &"𝄞".repeat(26));
        options::set_chat_group(&mut s, base, 1);
        x.put_character(2, s.get(base, slots::LEN));
        assert!(x.slot_used(2));
        let back = x.character(2);
        assert_eq!(&back[..SHARED], s.get(base, SHARED));
        let p = &back[SHARED + 1 + GU_PHRASE * 27..SHARED + 1 + GU_PHRASE * 28];
        let end = p.iter().position(|&c| c == 0).unwrap();
        assert_eq!(end, 56, "14 four-byte characters fit in 59 bytes");
        assert_eq!(u16::from_le_bytes([back[slots::LEN - 2], back[slots::LEN - 1]]), 1);
        assert_eq!(Mhxx::from_bytes(vec![0; 10]).err(), Some(Error::Size(10)));
    }
}
