//! Palico records, 324 B (docs/11-save-map.md "Palico records", from the loader; every
//! field the editor writes CONFIRMED in game, the lists and looks on 2026-10-09). A
//! Palico copied or imported into another character (`export`, `import`) is CONFIRMED in
//! game (2026-10-10).
//!
//! A Palico is named by its place: 0-83 the character's Palicoes, then 24 Palicoes for
//! hire (the same record; an edited one is hired as edited: CONFIRMED in game, 2026-10-10).
//!
//!   +0x00 char[32] name
//!   +0x20 224 B parameter block: exp u32 +0, level - 1 u8 +4, support bias +5, +6 UNRESOLVED,
//!         target +7, 8 equipped support moves +8 (0 = none), 8 equipped skills +0x10
//!         (0 = none), 16 move list slots +0x18 (57 = none), 12 skill list slots +0x28
//!         (96 = none), the random moves' and skills' point patterns +0x34 / +0x36 and
//!         the two lists' lengths +0x35 / +0x37
//!   +0x60 char[60] greeting
//!   +0x9C char[32] original owner
//!
//! Lists and equip limits (tools/palico_tables.py, DERIVED from the parameter class
//! 0xe8000-0xea000 and the analysed save): a move list is the forte's innate moves (one
//! for Charisma, else two), Mini Barrel Bombay, Herb Horn, random moves, then the slots
//! that taught moves fill (3 for Charisma, else 2); a skill list is the two innate
//! skills, random skills, then 2 taught slots. Equipped moves: at most the level's free
//! slots + 2 (+1 with skill 21); equipped skills: their slot costs at most the level's
//! skill slots. The equip limits and the innate moves are CONFIRMED in game (2026-10-16:
//! Lv 99 with Support Move +1, 8 moves and 8 skill slots; Lv 6 Charisma, 4 and 3).
//!
//! When a quest starts the game checks every Palico of it (0x25d714, on the quest's copy;
//! `Palico::check`): a move list that breaks its rules loses every equipped support move,
//! a skill list every equipped skill, while the save keeps the Palico as it was. DERIVED
//! from the code; every Palico the game made in the analysed save passes.

use crate::data::{PalicoForte, tables};
use crate::equipment::{self, Owner, PALICO_BOX_N};
use crate::save::Save;

pub const LIST: usize = 0x23BB6;
pub const LIST_N: usize = 84;
pub const HIRE_LIST: usize = 0x2A606;
pub const HIRE_N: usize = 24;
/// Places: the character's Palicoes, then those for hire.
pub const PLACES: usize = LIST_N + HIRE_N;
pub const RECORD: usize = 324;
pub const NAME: usize = 32;
/// 7 x u16 equipment references: Palico equipment box indices, 0xFFFF = none.
pub const GEAR: usize = 0x100;
pub const GEAR_N: usize = 7;
pub const NO_GEAR: u16 = 0xFFFF;
/// Start of an exported Palico file, then the record and, per equipment reference, its
/// box entry (zeros for none).
pub const MAGIC: &[u8; 8] = b"MHGUCAT1";
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
pub const SKILLS_ON: usize = 0x30;
pub const LEARNED: usize = 0x38;
pub const SKILLS: usize = 0x48;
pub const MOVE_LEN: usize = 0x55;
pub const SKILL_LEN: usize = 0x57;
/// The point patterns of the lists' random slots (data/palico-patterns.csv rows).
pub const MOVE_PATTERN: usize = 0x54;
pub const SKILL_PATTERN: usize = 0x56;
/// u32 flags; `SPECIAL_SKILL_FLAGS` (the DLC bits 7 and 8) lets one 2-point random skill
/// slot hold a special skill (0x25d714).
pub const FLAGS: usize = 0xE0;
pub const SPECIAL_SKILL_FLAGS: u32 = 0x180;
/// Mini Barrel Bombay and Herb Horn: every move list has them after the innate moves.
pub const UNIVERSAL_MOVES: [u8; 2] = [9, 1];
pub const NO_MOVE: u8 = 57;
pub const NO_SKILL: u8 = 96;
/// Support Move +1: one more support move slot (0xe8478).
pub const SKILL_MOVE_SLOT: u8 = 21;
pub const INNATE_SKILLS: usize = 2;
pub const TAUGHT_SKILLS: usize = 2;
/// The look block (the hunter's character-creation layout, +0 = 15, Prowler) and its 9
/// RGBA colours (data/palico-looks.csv says which byte and colour is which look).
pub const LOOKS: usize = 0x10E;
pub const COLOURS: usize = 0x11A;

/// Target byte -> the game's menu text (OtMenuMsg 56-61, in this order), CONFIRMED in game
/// for 1-5 (2026-10-16). 0 is not offered.
// i18n: shown through tr() in the GUI
pub const TARGETS: [&str; 6] = ["None", "Small Only", "Small First", "Balanced", "Large First", "Large Only"];

// i18n: shown through tr() in the GUI
pub const BIASES: [&str; 8] = ["Charisma", "Fighting", "Protection", "Assisting", "Healing", "Bombing", "Gathering", "Beast"];

#[derive(Debug, Clone, PartialEq)]
pub struct Palico {
    pub name: String,
    pub exp: u32,
    pub level: u8,
    pub bias: u8,
    pub target: u8,
    /// Equipped support moves, then 0s.
    pub moves: [u8; 8],
    /// Equipped skills, then 0s.
    pub skills_on: [u8; 8],
    /// The move list: `move_len` slots, then 57s.
    pub learned: [u8; 16],
    /// The skill list: `skill_len` slots, then 96s.
    pub skills: [u8; 12],
    pub move_len: u8,
    pub skill_len: u8,
    /// Read only (`set` leaves them): the lists' point patterns and the flags.
    pub move_pattern: u8,
    pub skill_pattern: u8,
    pub flags: u32,
    pub greeting: String,
    pub owner: String,
    pub looks: [u8; 12],
    /// RGBA
    pub colours: [[u8; 4]; 9],
}

impl Palico {
    /// The move list's slots in use.
    pub fn move_list(&self) -> &[u8] {
        &self.learned[..(self.move_len as usize).min(16)]
    }

    pub fn skill_list(&self) -> &[u8] {
        &self.skills[..(self.skill_len as usize).min(12)]
    }

    /// Innate moves at the head of the list: one for Charisma, else two.
    pub fn innate_moves(&self) -> usize {
        if self.bias == 0 { 1 } else { 2 }
    }

    /// The slots at the end of the list that taught moves fill (0x262838).
    pub fn taught_moves(&self) -> usize {
        if self.bias == 0 { 3 } else { 2 }
    }

    fn level_row(&self) -> (u8, u8) {
        let l = &tables().palico_levels;
        l[(self.level.max(1) as usize - 1).min(l.len() - 1)]
    }

    /// How many support moves it can equip (0xe8478).
    pub fn move_slots(&self) -> usize {
        self.level_row().0 as usize + 2 + self.skills_on.contains(&SKILL_MOVE_SLOT) as usize
    }

    /// The skill slots its level gives.
    pub fn skill_slots(&self) -> usize {
        self.level_row().1 as usize
    }

    /// The skill slots its equipped skills take (0xe9338).
    pub fn skills_cost(&self) -> usize {
        self.skills_on.iter().filter(|&&k| k != 0).map(|&k| skill_cost(k) as usize).sum()
    }

    fn forte(&self) -> Option<&'static PalicoForte> {
        tables().palico_fortes.get(self.bias as usize)
    }

    /// The slots the move list (`moves`) or the skill list must have, in order (0x25d714).
    /// Without a roll pattern of the game (special Palicoes) there are no random slots.
    pub fn layout(&self, moves: bool) -> Vec<Slot> {
        let pat = &tables().palico_patterns;
        let innate = if self.forte().is_none() {
            0
        } else if moves {
            self.innate_moves()
        } else {
            INNATE_SKILLS
        };
        let (points, taught) = if moves {
            let table = match innate {
                2 => Some(&pat.moves),
                1 => Some(&pat.moves_charisma),
                _ => None,
            };
            (table.and_then(|t| t.get(self.move_pattern as usize)), self.taught_moves())
        } else {
            (pat.skills.get(self.skill_pattern as usize), TAUGHT_SKILLS)
        };
        let mut v = vec![Slot::Innate; innate];
        if moves {
            v.extend(UNIVERSAL_MOVES.map(Slot::Universal));
        }
        v.extend(points.into_iter().flatten().map(|&p| Slot::Random(p)));
        v.extend(std::iter::repeat_n(Slot::Taught, taught));
        v
    }

    /// Whether `id` may stand in slot `k` of a list. Past the slots, anything; the special
    /// skills a 2-point slot may hold (`special_skill`) are left out: the game draws none.
    pub fn fits(&self, moves: bool, k: usize, id: u8) -> bool {
        match self.layout(moves).get(k) {
            Some(Slot::Innate) => self.forte().is_some_and(|f| match (moves, k) {
                (true, 0) => f.moves.contains(&id),
                (true, _) => f.moves2.contains(&id),
                (false, _) => f.skills[k] == id,
            }),
            Some(&Slot::Universal(x)) => id == x,
            Some(&Slot::Random(p)) => {
                let t = tables();
                let a = if moves { &t.palico_moves } else { &t.palico_skills };
                a.get(id as usize).is_some_and(|a| a.points == p)
            }
            Some(Slot::Taught) | None => true,
        }
    }

    /// The entries slot `k` can take, in ID order: those that fit it, but for the ones the
    /// list holds in other slots (an empty taught slot, 0, is always offered).
    pub fn choices(&self, moves: bool, k: usize) -> Vec<u8> {
        let (list, end) = if moves { (self.move_list(), NO_MOVE) } else { (self.skill_list(), NO_SKILL) };
        let elsewhere = |id: u8| list.iter().enumerate().any(|(j, &x)| j != k && x == id);
        (0..end).filter(|&id| self.fits(moves, k, id) && (id == 0 || !elsewhere(id))).collect()
    }

    /// The game's check of a list (0x25d714): its own rules, then, when both lists pass
    /// theirs, what it equips. A Palico failing it loses every equipped entry of that list
    /// on the quest's copy when a quest starts.
    pub fn check(&self, moves: bool) -> Option<Problem> {
        self.list_problem(moves).or_else(|| if self.list_problem(!moves).is_some() { None } else { self.equip_problem(moves) })
    }

    fn list_problem(&self, moves: bool) -> Option<Problem> {
        // the longest list, and the length below which the game looks past the end
        let (list, len, none, max, tail_below): (&[u8], usize, u8, usize, usize) =
            if moves { (&self.learned, self.move_len as usize, NO_MOVE, 15, 15) } else { (&self.skills, self.skill_len as usize, NO_SKILL, 12, 11) };
        if !(2..=max).contains(&len) {
            return Some(Problem::Length);
        }
        let layout = self.layout(moves);
        let mut special = 0;
        for (k, s) in layout.iter().enumerate() {
            let id = list.get(k).copied().unwrap_or(none);
            if *s == Slot::Taught || self.fits(moves, k, id) {
                continue;
            }
            if !moves && *s == Slot::Random(2) && special_skill(id) {
                special += (k + 1 < len) as u32;
            } else {
                return Some(Problem::Slot(k));
            }
        }
        if special > (self.flags & SPECIAL_SKILL_FLAGS != 0) as u32 {
            return Some(Problem::Special);
        }
        let tail = len < tail_below && list[len..].iter().any(|&x| x != 0 && x != none);
        (layout.len() != len || tail).then_some(Problem::Length)
    }

    fn equip_problem(&self, moves: bool) -> Option<Problem> {
        if moves {
            let innate = &self.learned[..self.innate_moves()];
            if innate.iter().map(|x| self.moves.iter().filter(|&m| m == x).count()).sum::<usize>() != innate.len() {
                return Some(Problem::InnateOff);
            }
            if let Some(&x) = self.moves.iter().find(|&&x| x != 0 && !self.learned.contains(&x)) {
                return Some(Problem::NotInList(x));
            }
            (self.moves.iter().filter(|&&x| x != 0).count() > self.move_slots()).then_some(Problem::TooMany)
        } else {
            if let Some(&x) = self.skills_on.iter().find(|&&x| x != 0 && !self.skills.contains(&x)) {
                return Some(Problem::NotInList(x));
            }
            (self.skills_cost() > self.skill_slots()).then_some(Problem::TooMany)
        }
    }

    /// Whether equipping `id` too keeps within the level's move slots or skill slots.
    pub fn can_equip(&self, moves: bool, id: u8) -> bool {
        let mut p = self.clone();
        let on = if moves { &mut p.moves } else { &mut p.skills_on };
        equip(on, id, true);
        on.contains(&id) && (if moves { p.moves.iter().filter(|&&x| x != 0).count() <= p.move_slots() } else { p.skills_cost() <= p.skill_slots() })
    }

    /// Unequip from the end until the equipped entries fit the level, skills first (their
    /// Support Move +1 gives a move slot); the innate moves stay, as when the game takes
    /// Support Move +1 off (0xe8f90).
    pub fn fit_equipped(&mut self) {
        while self.skills_cost() > self.skill_slots() {
            let Some(&x) = self.skills_on.iter().rfind(|&&x| x != 0) else { break };
            equip(&mut self.skills_on, x, false);
        }
        let innate = self.learned[..self.innate_moves()].to_vec();
        while self.moves.iter().filter(|&&x| x != 0).count() > self.move_slots() {
            let Some(&x) = self.moves.iter().rfind(|&&x| x != 0 && !innate.contains(&x)) else { break };
            equip(&mut self.moves, x, false);
        }
    }

    /// Change the forte: the innate move, the second one (kept when the new forte has it)
    /// and the innate skills become the new forte's, equipped where the old ones were (the
    /// innate moves always), then the equipped entries are fit to the level. Charisma's
    /// lists have another shape (one innate move, three taught slots, another roll table):
    /// to or from it, and from a forte the game does not have, false and nothing changes.
    pub fn set_forte(&mut self, bias: u8) -> bool {
        let Some(f) = tables().palico_fortes.get(bias as usize) else { return false };
        if self.forte().is_none() || (self.bias == 0) != (bias == 0) {
            return false;
        }
        if self.bias == bias {
            return true;
        }
        // old -> new in an equipped list; one equipped already only loses the old
        fn swap(on: &mut [u8; 8], old: u8, new: u8, equip_new: bool) {
            if old != new && on.contains(&new) {
                equip(on, old, false);
            } else if let Some(j) = on.iter().position(|&x| x == old) {
                on[j] = new;
            } else if equip_new {
                equip(on, new, true);
            }
        }
        let second = if f.moves2.contains(&self.learned[1]) { self.learned[1] } else { f.moves2[0] };
        for (k, new) in [f.moves[0], second].into_iter().enumerate() {
            swap(&mut self.moves, self.learned[k], new, true);
            self.learned[k] = new;
        }
        for (k, &new) in f.skills.iter().enumerate() {
            swap(&mut self.skills_on, self.skills[k], new, false);
            self.skills[k] = new;
        }
        self.bias = bias;
        self.fit_equipped();
        true
    }
}

/// A list slot, as the game's check sees it (0x25d714).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The forte's innate entry.
    Innate,
    /// Mini Barrel Bombay or Herb Horn, which every Palico has.
    Universal(u8),
    /// Drawn when the Palico was born: an entry worth these points (data/palico-actions.csv).
    Random(u8),
    /// The Palico Dojo's: anything, the check does not look.
    Taught,
}

/// Why the game's check fails a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The list's length is not that of its slots (forte, roll pattern), or entries follow
    /// its end.
    Length,
    /// Slot `k` holds an entry that does not fit it.
    Slot(usize),
    /// More special skills in 2-point slots than its flags allow.
    Special,
    /// Equipped but not in the list.
    NotInList(u8),
    /// More moves equipped, or skills costing more, than its level allows.
    TooMany,
    /// An innate move not equipped, or equipped twice.
    InnateOff,
}

/// The skills a 2-point random slot may hold instead of a 2-point skill (0x25d714).
fn special_skill(id: u8) -> bool {
    (48..=76).contains(&id) || (82..=95).contains(&id)
}

/// One look as the editor shows it: a byte of the look block or a colour slot, and the
/// choices new Palicoes get (data/palico-looks.csv; the eye colour is two fields).
#[derive(Debug, Clone, Copy)]
pub struct LookField {
    /// coat, coat_colour, clothing, clothing_colour, eyes, eye_left, eye_right, ears, tail, voice
    pub key: &'static str,
    pub byte: Option<usize>,
    pub colour: Option<usize>,
    /// Values `first .. first + choices` (voice starts at 1).
    pub first: u8,
    pub choices: u8,
    pub palette: &'static [[u8; 3]],
}

impl LookField {
    pub fn get(&self, p: &Palico) -> u8 {
        self.byte.map_or(0, |b| p.looks[b])
    }

    /// The colour slot's RGB.
    pub fn rgb(&self, p: &Palico) -> [u8; 3] {
        let [r, g, b, _] = self.colour.map_or([0; 4], |c| p.colours[c]);
        [r, g, b]
    }

    /// Whether the game can make this value: a choice it offers, a colour of its palette.
    pub fn fits(&self, p: &Palico) -> bool {
        match self.colour {
            Some(_) => self.palette.contains(&self.rgb(p)),
            None => (self.first..self.first + self.choices).contains(&self.get(p)),
        }
    }

    /// Set the byte to `v`, or the colour to palette entry `v` (alpha 0xFF).
    pub fn set(&self, p: &mut Palico, v: u8) {
        if let Some(b) = self.byte {
            p.looks[b] = v;
        }
        if let (Some(c), Some(&[r, g, b])) = (self.colour, self.palette.get(v as usize)) {
            p.colours[c] = [r, g, b, 0xFF];
        }
    }
}

pub fn look_fields() -> Vec<LookField> {
    let mut v = vec![];
    for l in tables().palico_looks.iter() {
        let f = |key: &'static str, colour: Option<usize>| LookField {
            key,
            byte: l.byte,
            colour,
            first: (l.look == "voice") as u8,
            choices: l.choices,
            palette: &l.palette,
        };
        match l.look.as_str() {
            "eye_colour" => v.extend([f("eye_left", Some(1)), f("eye_right", Some(2))]),
            k => {
                let key = ["coat", "coat_colour", "clothing", "clothing_colour", "eyes", "ears", "tail", "voice"].into_iter().find(|&x| x == k).unwrap_or("?");
                v.push(f(key, l.colour));
            }
        }
    }
    v
}

/// The skill slots skill `k` takes.
pub fn skill_cost(k: u8) -> u8 {
    tables().palico_skills.get(k as usize).map_or(0, |a| a.cost)
}

/// Equip (`on`) or unequip `id` in an equipped list kept packed at the front, 0 = none.
/// Equipping into a full list does nothing.
pub fn equip(list: &mut [u8], id: u8, on: bool) {
    let had = list.iter().position(|&x| x == id);
    match (on, had) {
        (true, None) => {
            if let Some(free) = list.iter().position(|&x| x == 0) {
                list[free] = id;
            }
        }
        (false, Some(k)) => {
            list[k..].rotate_left(1);
            *list.last_mut().unwrap() = 0;
        }
        _ => {}
    }
}

/// Put move `id` in slot `k` of the move list; the move it replaces is unequipped, as the
/// game does when it clears a slot (0xe8a48), and an innate move equipped (they always
/// are). Slots past the list's length are refused.
pub fn set_list_move(p: &mut Palico, k: usize, id: u8) {
    if k < p.move_list().len() {
        let old = p.learned[k];
        if old != id && !p.learned[..p.move_len as usize].iter().enumerate().any(|(j, &x)| j != k && x == old) {
            equip(&mut p.moves, old, false);
        }
        p.learned[k] = id;
        if k < p.innate_moves() {
            equip(&mut p.moves, id, true);
        }
    }
}

pub fn set_list_skill(p: &mut Palico, k: usize, id: u8) {
    if k < p.skill_list().len() {
        let old = p.skills[k];
        if old != id && !p.skills[..p.skill_len as usize].iter().enumerate().any(|(j, &x)| j != k && x == old) {
            equip(&mut p.skills_on, old, false);
        }
        p.skills[k] = id;
    }
}

/// The record of place `i`, from the character base.
pub fn offset(i: usize) -> usize {
    assert!(i < PLACES, "Palico {i} out of range");
    if i < LIST_N { LIST + RECORD * i } else { HIRE_LIST + RECORD * (i - LIST_N) }
}

pub fn for_hire(i: usize) -> bool {
    i >= LIST_N
}

fn at(base: usize, i: usize) -> usize {
    base + offset(i)
}

pub fn is_empty(s: &Save, base: usize, i: usize) -> bool {
    s.get(at(base, i), NAME)[0] == 0
}

pub fn get(s: &Save, base: usize, i: usize) -> Palico {
    let o = at(base, i);
    Palico {
        name: s.str(o, NAME),
        exp: s.u32(o + EXP),
        level: s.u8(o + LEVEL).saturating_add(1),
        bias: s.u8(o + BIAS),
        target: s.u8(o + TARGET),
        moves: s.get(o + MOVES, 8).try_into().unwrap(),
        skills_on: s.get(o + SKILLS_ON, 8).try_into().unwrap(),
        learned: s.get(o + LEARNED, 16).try_into().unwrap(),
        skills: s.get(o + SKILLS, 12).try_into().unwrap(),
        move_len: s.u8(o + MOVE_LEN),
        skill_len: s.u8(o + SKILL_LEN),
        move_pattern: s.u8(o + MOVE_PATTERN),
        skill_pattern: s.u8(o + SKILL_PATTERN),
        flags: s.u32(o + FLAGS),
        greeting: s.str(o + GREETING.0, GREETING.1),
        owner: s.str(o + OWNER.0, OWNER.1),
        looks: s.get(o + LOOKS, 12).try_into().unwrap(),
        colours: std::array::from_fn(|k| s.get(o + COLOURS + 4 * k, 4).try_into().unwrap()),
    }
}

/// Palico equipment box indices of Palico `i`'s equipment references (none skipped).
pub fn gear(s: &Save, base: usize, i: usize) -> Vec<usize> {
    let o = at(base, i);
    (0..GEAR_N).map(|k| s.u16(o + GEAR + 2 * k)).filter(|&r| r != NO_GEAR && (r as usize) < PALICO_BOX_N).map(usize::from).collect()
}

/// Why a Palico file did not go in.
#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    NotAPalico,
    /// All 84 Palico places hold one.
    ListFull,
    /// Fewer free Palico equipment box entries than the Palico's equipment.
    BoxFull,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            ImportError::NotAPalico => "not a Palico exported by this editor",
            ImportError::ListFull => "every Palico place is taken",
            ImportError::BoxFull => "the Palico equipment box has no room for its equipment",
        })
    }
}

impl std::error::Error for ImportError {}

/// Palico `i` and the equipment it wears, as a file.
pub fn export(s: &Save, base: usize, i: usize) -> Vec<u8> {
    let o = at(base, i);
    let mut v = [MAGIC.as_slice(), s.get(o, RECORD)].concat();
    for k in 0..GEAR_N {
        let r = s.u16(o + GEAR + 2 * k);
        let e = if r != NO_GEAR && (r as usize) < PALICO_BOX_N { equipment::get(s, base, Owner::Palico, r as usize).raw } else { [0; equipment::ENTRY] };
        v.extend_from_slice(&e);
    }
    v
}

/// The record and the equipment entries of a file made by `export`.
fn parts(file: &[u8]) -> Result<(&[u8], &[u8]), ImportError> {
    let b = file.strip_prefix(MAGIC.as_slice()).filter(|b| b.len() == RECORD + GEAR_N * equipment::ENTRY).ok_or(ImportError::NotAPalico)?;
    let (rec, entries) = b.split_at(RECORD);
    // a record without a name is an empty place
    if rec[0] == 0 { Err(ImportError::NotAPalico) } else { Ok((rec, entries)) }
}

/// The name of the Palico in a file made by `export`.
pub fn name_of(file: &[u8]) -> Result<String, ImportError> {
    let n = &parts(file)?.0[..NAME];
    let end = n.iter().position(|&c| c == 0).unwrap_or(NAME);
    Ok(String::from_utf8_lossy(&n[..end]).into_owned())
}

/// A file made by `export` becomes a Palico of this character, in the first free place;
/// its equipment goes into free Palico equipment box entries. Returns its index.
pub fn import(s: &mut Save, base: usize, file: &[u8]) -> Result<usize, ImportError> {
    let (rec, entries) = parts(file)?;
    let i = (0..LIST_N).find(|&i| is_empty(s, base, i)).ok_or(ImportError::ListFull)?;
    let worn: Vec<(usize, &[u8])> = entries
        .chunks(equipment::ENTRY)
        .enumerate()
        .filter(|(k, e)| u16::from_le_bytes([rec[GEAR + 2 * k], rec[GEAR + 2 * k + 1]]) != NO_GEAR && e.iter().any(|&x| x != 0))
        .collect();
    let free = (0..PALICO_BOX_N).filter(|&j| equipment::get(s, base, Owner::Palico, j).is_empty()).count();
    if free < worn.len() {
        return Err(ImportError::BoxFull);
    }
    let mut rec = rec.to_vec();
    rec[GEAR..GEAR + 2 * GEAR_N].fill(0xFF);
    for (k, e) in worn {
        let j = equipment::free_slot(s, base, Owner::Palico).expect("counted free above");
        equipment::set(s, base, Owner::Palico, j, &equipment::Entry { raw: e.try_into().unwrap() });
        rec[GEAR + 2 * k..GEAR + 2 * k + 2].copy_from_slice(&(j as u16).to_le_bytes());
    }
    s.put(at(base, i), &rec);
    Ok(i)
}

/// Writes the edited fields only; appearance, equipment, the unresolved bytes and fields
/// left as read stay byte for byte. An empty name is refused: a record without a name
/// is an empty slot.
pub fn set(s: &mut Save, base: usize, i: usize, p: &Palico) {
    let o = at(base, i);
    let old = get(s, base, i);
    if old.name != p.name && !p.name.trim().is_empty() {
        s.set_str(o, NAME, &p.name);
    }
    if old.exp != p.exp {
        s.set_u32(o + EXP, p.exp);
    }
    if old.level != p.level {
        s.set_u8(o + LEVEL, p.level.clamp(1, MAX_LEVEL) - 1);
    }
    if old.bias != p.bias {
        s.set_u8(o + BIAS, p.bias.min(7));
    }
    if old.target != p.target {
        s.set_u8(o + TARGET, p.target);
    }
    if old.moves != p.moves {
        s.put(o + MOVES, &p.moves);
    }
    if old.learned != p.learned {
        s.put(o + LEARNED, &p.learned);
    }
    if old.skills_on != p.skills_on {
        s.put(o + SKILLS_ON, &p.skills_on);
    }
    if old.skills != p.skills {
        s.put(o + SKILLS, &p.skills);
    }
    if old.greeting != p.greeting {
        s.set_str(o + GREETING.0, GREETING.1, &p.greeting);
    }
    if old.owner != p.owner {
        s.set_str(o + OWNER.0, OWNER.1, &p.owner);
    }
    if old.looks != p.looks {
        s.put(o + LOOKS, &p.looks);
    }
    if old.colours != p.colours {
        s.put(o + COLOURS, p.colours.as_flattened());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{SLOT1_BASE, blank};

    #[test]
    fn set_writes_only_what_changed() {
        let mut s = blank();
        let o = SLOT1_BASE + LIST;
        s.set_str(o, NAME, "Felyne");
        s.set_u8(o + LEVEL, 200);
        s.set_u8(o + BIAS, 9);
        s.set_u8(o + 6, 0xAB);
        let mut p = get(&s, SLOT1_BASE, 0);
        assert_eq!((p.level, p.bias), (201, 9));
        p.exp = 1234;
        set(&mut s, SLOT1_BASE, 0, &p);
        // out-of-range bytes nobody edited are left alone
        assert_eq!((s.u8(o + LEVEL), s.u8(o + BIAS), s.u8(o + 6)), (200, 9, 0xAB));
        assert_eq!(get(&s, SLOT1_BASE, 0).exp, 1234);
        p.level = 120;
        set(&mut s, SLOT1_BASE, 0, &p);
        assert_eq!(get(&s, SLOT1_BASE, 0).level, MAX_LEVEL);
    }

    #[test]
    fn empty_name_keeps_the_palico() {
        let mut s = blank();
        s.set_str(SLOT1_BASE + LIST + RECORD, NAME, "Tama");
        let mut p = get(&s, SLOT1_BASE, 1);
        p.name = "  ".into();
        p.greeting = "Meow".into();
        set(&mut s, SLOT1_BASE, 1, &p);
        assert!(!is_empty(&s, SLOT1_BASE, 1));
        assert_eq!(get(&s, SLOT1_BASE, 1).name, "Tama");
        assert_eq!(get(&s, SLOT1_BASE, 1).greeting, "Meow");
    }

    #[test]
    fn import_brings_the_equipment_into_free_box_entries() {
        use crate::equipment::{Entry, Kind};
        let mut s = blank();
        let (a, b) = (SLOT1_BASE, s.base(1));
        let o = a + LIST + RECORD * 3;
        s.set_str(o, NAME, "Suds");
        s.put(o + GEAR, &[0xFF; 2 * GEAR_N]);
        // weapon in entry 13, mail in entry 15
        s.set_u16(o + GEAR, 13);
        s.set_u16(o + GEAR + 4, 15);
        equipment::set(&mut s, a, Owner::Palico, 13, &Entry::new(Kind::from_code(22), 7));
        equipment::set(&mut s, a, Owner::Palico, 15, &Entry::new(Kind::from_code(24), 9));
        // the other character: Palico 0 and box entry 0 taken
        s.set_str(b + LIST, NAME, "Tama");
        equipment::set(&mut s, b, Owner::Palico, 0, &Entry::new(Kind::from_code(22), 1));
        let f = export(&s, a, 3);
        assert_eq!(import(&mut s, b, &f), Ok(1));
        let r = b + LIST + RECORD;
        assert_eq!((s.u16(r + GEAR), s.u16(r + GEAR + 2), s.u16(r + GEAR + 4)), (1, NO_GEAR, 2));
        assert_eq!(equipment::get(&s, b, Owner::Palico, 1).id(), 7);
        assert_eq!(equipment::get(&s, b, Owner::Palico, 2).id(), 9);
        assert_eq!(get(&s, b, 1).name, "Suds");
        assert_eq!(gear(&s, b, 1), vec![1, 2]);
        assert_eq!(import(&mut s, b, &f[..50]), Err(ImportError::NotAPalico));
    }

    /// Musashi of the analysed save: Gathering, Lv 7, 12 moves (2 taught slots), 12 skills.
    fn musashi() -> Palico {
        let mut s = blank();
        let o = SLOT1_BASE + LIST;
        s.set_str(o, NAME, "Musashi");
        s.set_u8(o + LEVEL, 6);
        s.set_u8(o + BIAS, 6);
        s.put(o + MOVES, &[37, 27, 0, 0, 0, 0, 0, 0]);
        s.put(o + LEARNED, &[37, 27, 9, 1, 39, 44, 50, 34, 32, 14, 0, 0, 57, 57, 57, 57]);
        s.put(o + SKILLS, &[44, 22, 34, 15, 29, 26, 35, 1, 30, 37, 0, 0]);
        s.put(o + MOVE_LEN, &[12, 0, 12]);
        s.set_u8(o + MOVE_PATTERN, 1);
        get(&s, SLOT1_BASE, 0)
    }

    #[test]
    fn game_check_holds_the_lists_to_their_slots() {
        let p = musashi();
        assert_eq!((p.check(true), p.check(false)), (None, None));
        assert_eq!(p.layout(true)[..5], [Slot::Innate, Slot::Innate, Slot::Universal(9), Slot::Universal(1), Slot::Random(2)]);
        // issue 10: Mini Barrel Bombay replaced
        let mut q = musashi();
        q.learned[2] = 10;
        assert_eq!(q.check(true), Some(Problem::Slot(2)));
        let mut q = musashi();
        q.learned[4] = 41;
        assert_eq!(q.check(true), Some(Problem::Slot(4)), "a 1-point move in a 2-point slot");
        let c = q.choices(true, 4);
        assert!(c.iter().all(|&m| tables().palico_moves[m as usize].points == 2) && !c.contains(&44), "44 is in slot 5");
        let mut q = musashi();
        q.moves = [27, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(q.check(true), Some(Problem::InnateOff));
        let mut q = musashi();
        q.move_len = 13;
        assert_eq!(q.check(true), Some(Problem::Length));
        // taught slots take anything
        let mut q = musashi();
        q.learned[10] = 31;
        q.skills[11] = 45;
        assert_eq!((q.check(true), q.check(false)), (None, None));
    }

    #[test]
    fn forte_change_swaps_the_innate_entries() {
        let mut p = musashi();
        let before = p.clone();
        assert!(!p.set_forte(0), "Charisma's lists have another shape");
        assert_eq!(p, before);
        // Gathering -> Fighting: Camouflage (27) is not one of Fighting's second moves
        assert!(p.set_forte(1));
        assert_eq!((&p.learned[..2], &p.skills[..2], &p.moves[..3]), (&[29, 6][..], &[3, 10][..], &[29, 6, 0][..]));
        assert_eq!((p.check(true), p.check(false)), (None, None));
    }

    #[test]
    fn fitting_to_a_lower_level_keeps_the_innate_moves() {
        let mut p = musashi();
        p.level = 99;
        p.moves = [37, 27, 39, 44, 50, 34, 0, 0];
        assert!(p.can_equip(true, 32));
        p.level = 1;
        p.fit_equipped();
        assert_eq!(p.moves, [37, 27, 39, 0, 0, 0, 0, 0]);
        assert!(!p.can_equip(true, 44));
        assert_eq!(p.check(true), None);
    }

    #[test]
    fn lists_and_limits() {
        let mut p = musashi();
        assert_eq!((p.move_list().len(), p.innate_moves(), p.taught_moves()), (12, 2, 2));
        // Lv 7: 2 free slots + the 2 innate ones; skill slots 3 (data/palico-levels.csv)
        assert_eq!((p.move_slots(), p.skill_slots()), (4, 3));
        p.skills_on[0] = SKILL_MOVE_SLOT;
        assert_eq!(p.move_slots(), 5);
        assert_eq!(p.skills_cost(), skill_cost(SKILL_MOVE_SLOT) as usize);
        assert_eq!(tables().palico_fortes[6].moves, vec![37]);
    }

    #[test]
    fn looks_follow_the_tables() {
        let mut p = musashi();
        // Musashi: Melynx coat (1), coat colour f0f0f0, eye colour ed8740 (analysed save)
        p.looks = [15, 1, 4, 1, 0, 0, 1, 4, 3, 0, 0, 0];
        p.colours[0] = [0xf0, 0xf0, 0xf0, 0xff];
        p.colours[1] = [0xed, 0x87, 0x40, 0xff];
        p.colours[2] = p.colours[1];
        p.colours[3] = [0xa1, 0x76, 0x4f, 0xff];
        let f = look_fields();
        assert_eq!(f.len(), 10);
        assert!(f.iter().all(|x| x.fits(&p)), "{:?}", f.iter().find(|x| !x.fits(&p)));
        let coat = f.iter().find(|x| x.key == "coat").unwrap();
        coat.set(&mut p, 7);
        assert!(!coat.fits(&p), "7 coats");
        let voice = f.iter().find(|x| x.key == "voice").unwrap();
        voice.set(&mut p, 0);
        assert!(!voice.fits(&p), "voices are 1-3");
        let eye = f.iter().find(|x| x.key == "eye_right").unwrap();
        eye.set(&mut p, 8);
        assert_eq!(p.colours[2], [0xeb, 0xf5, 0xff, 0xff]);
        p.colours[2] = [1, 2, 3, 0xff];
        assert!(!eye.fits(&p));
    }

    #[test]
    fn equipping_keeps_the_list_packed() {
        let mut l = [5, 6, 7, 0, 0, 0, 0, 0];
        equip(&mut l, 6, false);
        assert_eq!(l, [5, 7, 0, 0, 0, 0, 0, 0]);
        equip(&mut l, 9, true);
        equip(&mut l, 9, true);
        assert_eq!(l, [5, 7, 9, 0, 0, 0, 0, 0]);
        let mut full = [1; 8];
        equip(&mut full, 2, true);
        assert_eq!(full, [1; 8]);
    }

    #[test]
    fn replacing_a_list_move_unequips_it() {
        let mut p = musashi();
        p.moves[2] = 39;
        set_list_move(&mut p, 4, 22);
        assert_eq!((p.learned[4], p.moves), (22, [37, 27, 0, 0, 0, 0, 0, 0]));
        // an innate move stays equipped
        set_list_move(&mut p, 1, 26);
        assert_eq!(p.moves, [37, 26, 0, 0, 0, 0, 0, 0]);
        set_list_move(&mut p, 12, 5);
        assert_eq!(p.learned[12], NO_MOVE, "past the list");
        set_list_skill(&mut p, 10, 21);
        assert_eq!(p.skills[10], 21);
    }

    #[test]
    fn places_past_the_list_are_for_hire() {
        let mut s = blank();
        s.set_str(SLOT1_BASE + HIRE_LIST + RECORD, NAME, "Nick");
        let i = LIST_N + 1;
        assert!(for_hire(i) && !for_hire(LIST_N - 1));
        assert_eq!(get(&s, SLOT1_BASE, i).name, "Nick");
        let mut p = get(&s, SLOT1_BASE, i);
        p.level = 40;
        set(&mut s, SLOT1_BASE, i, &p);
        assert_eq!(s.u8(SLOT1_BASE + HIRE_LIST + RECORD + LEVEL), 39);
        // an import goes into the character's own places only
        let f = export(&s, SLOT1_BASE, i);
        assert_eq!(import(&mut s, SLOT1_BASE, &f), Ok(0));
    }

    #[test]
    #[should_panic]
    fn index_past_the_list_panics() {
        get(&blank(), SLOT1_BASE, PLACES);
    }
}
