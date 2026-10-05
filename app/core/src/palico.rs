//! Palico records, 324 B (docs/11-save-map.md "Palico records", DERIVED from the loader).
//!
//!   +0x00 char[32] name
//!   +0x20 224 B parameter block: exp u32 +0, level - 1 u8 +4, support bias +5, +6 UNRESOLVED,
//!         target +7, 8 equipped support moves +8, 16 learned move slots +0x18 (57 = none)
//!   +0x60 char[60] greeting
//!   +0x9C char[32] original owner

use crate::save::Save;

pub const LIST: usize = 0x23BB6;
pub const LIST_N: usize = 84;
pub const HIRE_LIST: usize = 0x2A606;
pub const HIRE_N: usize = 24;
pub const RECORD: usize = 324;
pub const NAME: usize = 32;
pub const GREETING: (usize, usize) = (0x60, 60);
pub const OWNER: (usize, usize) = (0x9C, 32);
pub const EXP: usize = 0x20;
/// The byte holds the shown level - 1: 63 is Lv 64 in game; the level-up code (`0x262e1c`)
/// counts byte 49 as level 50 and byte 98 as the top level, 99.
pub const LEVEL: usize = 0x24;
pub const MAX_LEVEL: u8 = 99;
pub const BIAS: usize = 0x25;
pub const TARGET: usize = 0x27;
pub const MOVES: usize = 0x28;
pub const LEARNED: usize = 0x38;
pub const NO_MOVE: u8 = 57;

pub const BIASES: [&str; 8] = ["Charisma", "Fighting", "Protection", "Assisting", "Healing", "Bombing", "Gathering", "Beast"];

#[derive(Debug, Clone, PartialEq)]
pub struct Palico {
    pub name: String,
    pub exp: u32,
    pub level: u8,
    pub bias: u8,
    pub target: u8,
    pub moves: [u8; 8],
    pub learned: [u8; 16],
    pub greeting: String,
    pub owner: String,
}

fn at(base: usize, list: usize, i: usize) -> usize {
    base + list + RECORD * i
}

pub fn is_empty(s: &Save, base: usize, i: usize) -> bool {
    s.get(at(base, LIST, i), NAME)[0] == 0
}

pub fn get(s: &Save, base: usize, i: usize) -> Palico {
    let o = at(base, LIST, i);
    Palico {
        name: s.str(o, NAME),
        exp: s.u32(o + EXP),
        level: s.u8(o + LEVEL).saturating_add(1),
        bias: s.u8(o + BIAS),
        target: s.u8(o + TARGET),
        moves: s.get(o + MOVES, 8).try_into().unwrap(),
        learned: s.get(o + LEARNED, 16).try_into().unwrap(),
        greeting: s.str(o + GREETING.0, GREETING.1),
        owner: s.str(o + OWNER.0, OWNER.1),
    }
}

/// Writes the edited fields only; appearance, equipment and the unresolved bytes stay.
pub fn set(s: &mut Save, base: usize, i: usize, p: &Palico) {
    let o = at(base, LIST, i);
    let old = get(s, base, i);
    if old.name != p.name {
        s.set_str(o, NAME, &p.name);
    }
    s.set_u32(o + EXP, p.exp);
    s.set_u8(o + LEVEL, p.level.clamp(1, MAX_LEVEL) - 1);
    s.set_u8(o + BIAS, p.bias.min(7));
    s.set_u8(o + TARGET, p.target);
    s.put(o + MOVES, &p.moves);
    s.put(o + LEARNED, &p.learned);
    if old.greeting != p.greeting {
        s.set_str(o + GREETING.0, GREETING.1, &p.greeting);
    }
    if old.owner != p.owner {
        s.set_str(o + OWNER.0, OWNER.1, &p.owner);
    }
}
