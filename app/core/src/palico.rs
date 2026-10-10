//! Palico records, 324 B (docs/11-save-map.md "Palico records", from the loader; every
//! field the editor writes CONFIRMED in game, the lists and looks on 2026-10-09). A
//! Palico copied or imported into another character (`export`, `import`) is DERIVED.
//!
//! A Palico is named by its place: 0-83 the character's Palicoes, then 24 Palicoes for
//! hire (the same record; an edited one is hired as edited: DERIVED).
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
//! skill slots.

use crate::data::tables;
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

/// Target byte -> the game's menu text (OtMenuMsg 56-61, in this order). 4 = Large First
/// was read off in game; the rest follow the menu order (DERIVED). 0 is not offered.
// i18n: shown through tr() in the GUI
pub const TARGETS: [&str; 6] = ["None", "Small Only", "Small First", "Balanced", "Large First", "Large Only"];

// i18n: shown through tr() in the GUI
pub const BIASES: [&str; 8] =["Charisma", "Fighting", "Protection", "Assisting", "Healing", "Bombing", "Gathering", "Beast"];

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
    for l in &tables().palico_looks {
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
/// game does when it clears a slot (0xe8a48). Slots past the list's length are refused.
pub fn set_list_move(p: &mut Palico, k: usize, id: u8) {
    if k < p.move_list().len() {
        let old = p.learned[k];
        if old != id && !p.learned[..p.move_len as usize].iter().enumerate().any(|(j, &x)| j != k && x == old) {
            equip(&mut p.moves, old, false);
        }
        p.learned[k] = id;
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
    let worn: Vec<(usize, &[u8])> = entries.chunks(equipment::ENTRY).enumerate().filter(|(k, e)| u16::from_le_bytes([rec[GEAR + 2 * k], rec[GEAR + 2 * k + 1]]) != NO_GEAR && e.iter().any(|&x| x != 0)).collect();
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
    use crate::save::{blank, SLOT1_BASE};

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
        get(&s, SLOT1_BASE, 0)
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
        set_list_move(&mut p, 1, 26);
        assert_eq!((p.learned[1], p.moves), (26, [37, 0, 0, 0, 0, 0, 0, 0]));
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
