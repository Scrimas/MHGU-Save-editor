//! My Sets, Palico equipment sets, and what the hunter has on besides the gear: armor
//! pigment, hunting style and Hunter Arts (docs/07-equipment.md, docs/11-save-map.md).
//!
//! My Set, 136 B from `MY_SETS` (the loader's record boundary, 6 bytes after the one
//! `equipment::MY_SETS` counts from):
//!   +0x00 char[42] name ("---" unused)       +0x2A 7 × u16 hunter box index (0xFFFF none)
//!   +0x38 7 × 3 × u16 the pieces' decorations +0x64 5 × RGBA pigment (chest … head)
//!   +0x78 5 × u8 colour mode                  +0x7D 5 × u8 1 = the armor's own colour
//!   +0x82 u8 hunting style (6 unused)         +0x83 3 × u8 Hunter Arts
//!   +0x86 u8 bits 0-2 art slot i is an SP Art +0x87 padding
//! In every set of the analysed saves the decorations equal those of the box entries.
//! Loading a set copies style, arts and SP bits to the player record (`PLAYER_ARTS`,
//! the slot header's `HDR_ARTS`); the player's pigment is the loaded set's.
//!
//! Palico set, 68 B from `PALICO_SETS`: +0x00 char[42] name, +0x2A 3 × u16 Palico box
//! index (weapon, head, body), +0x30 3 × u8 (1 in every unused set), then zeros.
//!
//! Hunter Arts by ID (data/hunter-arts.csv): 1-10 for any weapon, then 12 IDs (four arts
//! of three tiers) per weapon class in the box's order, 71-82 (class 5) unused. Checked
//! against the analysed saves: Round Force I (26) on a new character's Sword and Shield,
//! Wolf's Maw (150, 151) on Dual Blades, Energy Blade I (179) on a Charge Blade.

use crate::character::{LOOKS, LOOK_COLOURS};
use crate::equipment::{self, Owner};
use crate::save::Save;

pub const MY_SETS: usize = 0x208CE;
pub const MY_SETS_N: usize = 40;
pub const MY_SET: usize = 136;
pub const PALICO_SETS: usize = 0x21E0E;
pub const PALICO_SETS_N: usize = 24;
pub const PALICO_SET: usize = 68;
pub const NAME_LEN: usize = 42;
/// As the game's name entry for hunters ("Names must be 10 characters or less").
pub const NAME_CHARS: usize = 10;
pub const UNUSED_NAME: &str = "---";
pub const NO_BOX: u16 = 0xFFFF;
/// Field offsets in a set record (`GEAR` also in a Palico set's).
pub const GEAR: usize = 0x2A;
pub const DECOS: usize = 0x38;
pub const PIGMENT: usize = 0x64;
pub const MODES: usize = 0x78;
pub const OWN: usize = 0x7D;
pub const STYLE: usize = 0x82;
pub const ARTS: usize = 0x83;
pub const SP: usize = 0x86;
const PALICO_FLAGS: usize = 0x30;
/// Style byte of an unused My Set.
pub const NO_STYLE: u8 = 6;
/// Art slots per hunting style (Guild, Striker, Aerial, Adept, Alchemy, Valor).
pub const ART_SLOTS: [usize; 6] = [2, 3, 1, 1, 3, 1];
/// Pigment parts in the order of the five colours.
// i18n: shown through tr() in the GUI
pub const PIGMENT_PARTS: [&str; 5] = ["Chest", "Arms", "Waist", "Legs", "Head"];
/// Box type of each My Set piece: weapon (any class), head … legs, talisman.
pub const PIECES: usize = 7;

/// The equipped arts: 3 × u16 art ID, then u16 SP Art bits; the player record's copy (the
/// one the game loads) and the slot header's.
pub const PLAYER_ARTS: usize = 0x23A59;
pub const HDR_ARTS: usize = 0x2C;
/// Default-colour bits (0-4 the pigment parts) and the five 5-bit colour modes: player
/// record and slot header.
pub const PLAYER_OWN: usize = 0x23B7B;
pub const HDR_OWN: usize = 0x270;
pub const PLAYER_MODES: usize = 0x23B77;
pub const HDR_MODES: usize = 0x274;
/// Hunting style, byte 5 of the creation block.
pub const LOOK_STYLE: usize = 5;
/// Weapon class, byte 0 of the creation block (equipped weapon box type - 7).
pub const LOOK_WEAPON: usize = 0;

/// What takes a Hunter Art: any weapon, or one weapon class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtFor {
    Any,
    Weapon(u8),
}

/// The weapons art `id` is for; `None` for an ID that names no art.
pub fn art_class(id: u8) -> Option<ArtFor> {
    match id {
        1..=10 => Some(ArtFor::Any),
        71..=82 => None,
        11..=190 => Some(ArtFor::Weapon((id - 11) / 12)),
        _ => None,
    }
}

/// Tiers of one art share it: I, II and III can't be on together.
pub fn art_family(id: u8) -> u8 {
    if id <= 10 { id } else { 11 + (id - 11) / 3 * 3 }
}

/// Style, arts and SP Art bits, as a My Set keeps them and as the hunter has them on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arts {
    pub style: u8,
    pub ids: [u8; 3],
    /// Bit i: art slot i is an SP Art.
    pub sp: u8,
}

impl Arts {
    /// Art slots of its style (0 for an unused set).
    pub fn slots(&self) -> usize {
        ART_SLOTS.get(self.style as usize).copied().unwrap_or(0)
    }

    /// A new style: the arts past its slots come off, as the game's style menu leaves them.
    pub fn set_style(&mut self, style: u8) {
        self.style = style.min(5);
        for k in self.slots()..3 {
            self.ids[k] = 0;
            self.sp &= !(1 << k);
        }
    }

    /// Art `id` (0 none) into slot `k`; the same art in another slot moves here.
    pub fn set_art(&mut self, k: usize, id: u8) {
        assert!(k < 3, "art slot {k}");
        if id != 0 {
            for j in 0..3 {
                if j != k && self.ids[j] != 0 && art_family(self.ids[j]) == art_family(id) {
                    self.ids[j] = 0;
                    self.sp &= !(1 << j);
                }
            }
        }
        self.ids[k] = id;
        if id == 0 {
            self.sp &= !(1 << k);
        }
    }

    pub fn set_sp(&mut self, k: usize, on: bool) {
        assert!(k < 3, "art slot {k}");
        if on && self.ids[k] != 0 {
            self.sp |= 1 << k;
        } else {
            self.sp &= !(1 << k);
        }
    }
}

/// Five colours and, per part, whether the armor's own colour shows instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pigment {
    pub rgba: [[u8; 4]; 5],
    pub own: [bool; 5],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MySet {
    pub name: String,
    pub gear: [u16; PIECES],
    pub pigment: Pigment,
    pub arts: Arts,
}

impl MySet {
    pub fn used(&self) -> bool {
        self.arts.style != NO_STYLE || self.gear.iter().any(|&g| g != NO_BOX)
    }
}

fn my_set_at(base: usize, k: usize) -> usize {
    assert!(k < MY_SETS_N, "My Set {k}");
    base + MY_SETS + MY_SET * k
}

fn palico_set_at(base: usize, k: usize) -> usize {
    assert!(k < PALICO_SETS_N, "Palico set {k}");
    base + PALICO_SETS + PALICO_SET * k
}

fn rgba(s: &Save, at: usize) -> [[u8; 4]; 5] {
    std::array::from_fn(|p| s.get(at + 4 * p, 4).try_into().unwrap())
}

pub fn my_set(s: &Save, base: usize, k: usize) -> MySet {
    let o = my_set_at(base, k);
    MySet {
        name: s.str(o, NAME_LEN),
        gear: std::array::from_fn(|p| s.u16(o + GEAR + 2 * p)),
        pigment: Pigment { rgba: rgba(s, o + PIGMENT), own: std::array::from_fn(|p| s.u8(o + OWN + p) != 0) },
        arts: Arts { style: s.u8(o + STYLE), ids: std::array::from_fn(|j| s.u8(o + ARTS + j)), sp: s.u8(o + SP) & 7 },
    }
}

/// Name cut to `NAME_CHARS` characters (and the field, with its NUL); set `k`'s default
/// name when blank.
fn fit_name(name: &str, k: usize) -> String {
    let mut n: String = name.trim().chars().take(NAME_CHARS).collect();
    while n.len() > NAME_LEN - 1 {
        n.pop();
    }
    if n.is_empty() { default_name(k) } else { n }
}

/// The default name of set `k`, as the game names a new one.
pub fn default_name(k: usize) -> String {
    format!("Set {:02}", k + 1)
}

pub fn set_my_set_name(s: &mut Save, base: usize, k: usize, name: &str) {
    s.set_str(my_set_at(base, k), NAME_LEN, &fit_name(name, k));
}

/// Piece `p` (0 weapon, 1-5 head … legs, 6 talisman) of set `k`: hunter box entry `i`,
/// with a copy of its decorations, or none.
pub fn set_my_set_piece(s: &mut Save, base: usize, k: usize, p: usize, i: Option<usize>) {
    assert!(p < PIECES, "piece {p}");
    let o = my_set_at(base, k);
    let decos = i.map_or([0; 3], |i| equipment::get(s, base, Owner::Hunter, i).decos());
    s.set_u16(o + GEAR + 2 * p, i.map_or(NO_BOX, |i| i as u16));
    for (j, d) in decos.into_iter().enumerate() {
        s.set_u16(o + DECOS + 6 * p + 2 * j, d);
    }
}

/// A custom colour for part `p` of set `k`: the RGBA, colour mode 0, not the own colour.
pub fn set_my_set_colour(s: &mut Save, base: usize, k: usize, p: usize, [r, g, b]: [u8; 3]) {
    assert!(p < 5, "pigment part {p}");
    let o = my_set_at(base, k);
    s.put(o + PIGMENT + 4 * p, &[r, g, b, 0xFF]);
    s.set_u8(o + MODES + p, 0);
    s.set_u8(o + OWN + p, 0);
}

/// Part `p` of set `k` back to the armor's own colour (the flag; the RGBA stays).
pub fn set_my_set_own_colour(s: &mut Save, base: usize, k: usize, p: usize) {
    assert!(p < 5, "pigment part {p}");
    let o = my_set_at(base, k);
    s.set_u8(o + MODES + p, 0);
    s.set_u8(o + OWN + p, 1);
}

pub fn set_my_set_arts(s: &mut Save, base: usize, k: usize, a: Arts) {
    let o = my_set_at(base, k);
    s.set_u8(o + STYLE, a.style);
    for j in 0..3 {
        s.set_u8(o + ARTS + j, a.ids[j]);
    }
    s.set_u8(o + SP, a.sp & 7);
}

/// Set `k` as the game's "register" makes it from what the hunter has on: the worn gear
/// with its decorations, the pigment with its flags and modes, style, arts and SP bits.
/// An unused set gets the game's default name.
pub fn save_current(s: &mut Save, base: usize, k: usize) {
    let o = my_set_at(base, k);
    if !my_set(s, base, k).used() {
        s.set_str(o, NAME_LEN, &default_name(k));
    }
    for p in 0..PIECES {
        let i = s.u16(base + equipment::WORN + 2 * p);
        set_my_set_piece(s, base, k, p, (i != NO_BOX && (i as usize) < equipment::BOX_N).then_some(i as usize));
    }
    let pg = pigment(s, base);
    let modes = s.u32(base + PLAYER_MODES);
    for p in 0..5 {
        s.put(o + PIGMENT + 4 * p, &pg.rgba[p]);
        s.set_u8(o + MODES + p, (modes >> (5 * p) & 0x1F) as u8);
        s.set_u8(o + OWN + p, pg.own[p] as u8);
    }
    set_my_set_arts(s, base, k, arts(s, base));
}

/// Set `k` as a new character's unused one: "---", no gear, no colours, style 6.
pub fn clear_my_set(s: &mut Save, base: usize, k: usize) {
    let o = my_set_at(base, k);
    let mut v = [0u8; MY_SET];
    v[..UNUSED_NAME.len()].copy_from_slice(UNUSED_NAME.as_bytes());
    v[GEAR..GEAR + 2 * PIECES].fill(0xFF);
    v[OWN..OWN + 5].fill(1);
    v[STYLE] = NO_STYLE;
    s.put(o, &v);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalicoSet {
    pub name: String,
    /// Palico box index of the weapon, head and body piece.
    pub gear: [u16; 3],
}

impl PalicoSet {
    pub fn used(&self) -> bool {
        self.name != UNUSED_NAME || self.gear.iter().any(|&g| g != NO_BOX)
    }
}

pub fn palico_set(s: &Save, base: usize, k: usize) -> PalicoSet {
    let o = palico_set_at(base, k);
    PalicoSet { name: s.str(o, NAME_LEN), gear: std::array::from_fn(|p| s.u16(o + GEAR + 2 * p)) }
}

pub fn set_palico_set_name(s: &mut Save, base: usize, k: usize, name: &str) {
    s.set_str(palico_set_at(base, k), NAME_LEN, &fit_name(name, k));
}

/// Piece `p` (0 weapon, 1 head, 2 body) of Palico set `k`: Palico box entry `i` or none.
/// An unused set takes the game's default name.
pub fn set_palico_set_piece(s: &mut Save, base: usize, k: usize, p: usize, i: Option<usize>) {
    assert!(p < 3, "Palico piece {p}");
    let o = palico_set_at(base, k);
    if !palico_set(s, base, k).used() && i.is_some() {
        s.set_str(o, NAME_LEN, &default_name(k));
    }
    s.set_u16(o + GEAR + 2 * p, i.map_or(NO_BOX, |i| i as u16));
}

/// Palico set `k` unused again, as a new character's.
pub fn clear_palico_set(s: &mut Save, base: usize, k: usize) {
    let o = palico_set_at(base, k);
    let mut v = [0u8; PALICO_SET];
    v[..UNUSED_NAME.len()].copy_from_slice(UNUSED_NAME.as_bytes());
    v[GEAR..GEAR + 6].fill(0xFF);
    v[PALICO_FLAGS..PALICO_FLAGS + 3].fill(1);
    s.put(o, &v);
}

/// The style, arts and SP bits the hunter has on (player record).
pub fn arts(s: &Save, base: usize) -> Arts {
    let w = |j: usize| s.u16(base + PLAYER_ARTS + 2 * j);
    Arts { style: s.u8(base + LOOKS[0] + LOOK_STYLE), ids: std::array::from_fn(|j| w(j).min(255) as u8), sp: (w(3) & 7) as u8 }
}

/// Write style, arts and SP bits to the player record and the slot header, as loading a
/// set does. The Guild Card's copies are the game's to rebuild: they lag behind in the
/// analysed saves (card style 5 while the player's was 3).
pub fn set_arts(s: &mut Save, base: usize, a: Arts) {
    for at in [PLAYER_ARTS, HDR_ARTS] {
        for j in 0..3 {
            s.set_u16(base + at + 2 * j, a.ids[j] as u16);
        }
        let keep = s.u16(base + at + 6) & !7;
        s.set_u16(base + at + 6, keep | a.sp as u16);
    }
    for l in &LOOKS[..2] {
        s.set_u8(base + l + LOOK_STYLE, a.style);
    }
}

/// The weapon class the hunter has on (`equipment::Kind::Weapon` numbering).
pub fn weapon_class(s: &Save, base: usize) -> u8 {
    s.u8(base + LOOKS[0] + LOOK_WEAPON)
}

/// The pigment the hunter has on (player record).
pub fn pigment(s: &Save, base: usize) -> Pigment {
    let own = s.u16(base + PLAYER_OWN);
    Pigment { rgba: rgba(s, base + LOOK_COLOURS[0]), own: std::array::from_fn(|p| own >> p & 1 != 0) }
}

fn set_own_bit(s: &mut Save, base: usize, p: usize, on: bool) {
    for at in [PLAYER_OWN, HDR_OWN] {
        let v = s.u16(base + at);
        s.set_u16(base + at, if on { v | 1 << p } else { v & !(1 << p) });
    }
    for at in [PLAYER_MODES, HDR_MODES] {
        let v = s.u32(base + at);
        s.set_u32(base + at, v & !(0x1F << (5 * p)));
    }
}

/// A custom colour for part `p` of what the hunter has on: every copy of the colour, its
/// default flag and colour mode cleared.
pub fn set_colour(s: &mut Save, base: usize, p: usize, [r, g, b]: [u8; 3]) {
    assert!(p < 5, "pigment part {p}");
    for a in LOOK_COLOURS {
        s.put(base + a + 4 * p, &[r, g, b, 0xFF]);
    }
    set_own_bit(s, base, p, false);
}

/// Part `p` of what the hunter has on back to the armor's own colour (the flag).
pub fn set_own_colour(s: &mut Save, base: usize, p: usize) {
    assert!(p < 5, "pigment part {p}");
    set_own_bit(s, base, p, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE};

    #[test]
    fn art_numbering() {
        assert_eq!(art_class(1), Some(ArtFor::Any));
        assert_eq!(art_class(26), Some(ArtFor::Weapon(1)), "Round Force I, Sword and Shield");
        assert_eq!(art_class(151), Some(ArtFor::Weapon(11)), "Wolf's Maw III, Dual Blades");
        assert_eq!(art_class(179), Some(ArtFor::Weapon(14)), "Energy Blade I, Charge Blade");
        assert!(art_class(0).is_none() && art_class(75).is_none() && art_class(191).is_none());
        assert_eq!(art_family(149), art_family(151));
        assert_ne!(art_family(151), art_family(152));
    }

    #[test]
    fn style_trims_arts_and_tiers_exclude_each_other() {
        let mut a = Arts { style: 1, ids: [26, 29, 1], sp: 0b101 };
        a.set_art(1, 27);
        assert_eq!(a.ids, [0, 27, 1], "Round Force II takes I's place");
        assert_eq!(a.sp, 0b100);
        a.set_style(5);
        assert_eq!((a.ids, a.sp), ([0, 0, 0], 0));
        a.set_art(0, 179);
        a.set_sp(0, true);
        assert_eq!((a.slots(), a.sp), (1, 1));
    }

    #[test]
    fn my_set_round_trip() {
        let mut s = blank();
        let b = SLOT1_BASE;
        for k in [0, MY_SETS_N - 1] {
            clear_my_set(&mut s, b, k);
            assert!(!my_set(&s, b, k).used());
        }
        let mut e = equipment::Entry::new(equipment::Kind::Head, 5);
        e.add_deco(2700);
        equipment::set(&mut s, b, Owner::Hunter, 9, &e);
        set_my_set_piece(&mut s, b, 0, 1, Some(9));
        set_my_set_name(&mut s, b, 0, "  A very long set name ");
        set_my_set_colour(&mut s, b, 0, 2, [1, 2, 3]);
        let mut a = my_set(&s, b, 0).arts;
        a.set_style(4);
        a.set_art(2, 120);
        set_my_set_arts(&mut s, b, 0, a);
        let m = my_set(&s, b, 0);
        assert!(m.used());
        assert_eq!(m.name, "A very lon");
        assert_eq!(m.gear[1], 9);
        assert_eq!(s.u16(b + MY_SETS + DECOS + 6), 2700, "the head's first decoration");
        assert_eq!((m.pigment.rgba[2], m.pigment.own[2], m.pigment.own[1]), ([1, 2, 3, 0xFF], false, true));
        assert_eq!((m.arts.style, m.arts.ids), (4, [0, 0, 120]));
        assert_eq!(my_set(&s, b, MY_SETS_N - 1).arts.style, NO_STYLE, "the last set is untouched");
        assert_eq!(MY_SETS + MY_SET * MY_SETS_N, PALICO_SETS, "Palico sets follow the My Sets");
    }

    #[test]
    fn current_and_saved_sets_agree() {
        let mut s = blank();
        let b = SLOT1_BASE;
        s.set_u16(b + PLAYER_OWN, 0xFFFF);
        s.set_u16(b + HDR_OWN, 0xFFFF);
        s.set_u32(b + PLAYER_MODES, 0x1F << 5);
        for p in 0..PIECES {
            s.set_u16(b + equipment::WORN + 2 * p, if p == 6 { NO_BOX } else { p as u16 });
        }
        set_colour(&mut s, b, 1, [9, 8, 7]);
        set_arts(&mut s, b, Arts { style: 2, ids: [140, 0, 0], sp: 1 });
        assert_eq!((s.u16(b + PLAYER_OWN), s.u16(b + HDR_OWN), s.u32(b + PLAYER_MODES)), (0xFFFD, 0xFFFD, 0));
        assert_eq!(s.u16(b + HDR_ARTS), 140);
        assert!(LOOKS[..2].iter().all(|&l| s.u8(b + l + LOOK_STYLE) == 2));
        clear_my_set(&mut s, b, 3);
        save_current(&mut s, b, 3);
        let m = my_set(&s, b, 3);
        assert_eq!(m.name, "Set 04");
        assert_eq!(m.gear, [0, 1, 2, 3, 4, 5, NO_BOX]);
        assert_eq!(m.pigment, pigment(&s, b));
        assert_eq!(m.arts, arts(&s, b));
        set_own_colour(&mut s, b, 1);
        assert!(pigment(&s, b).own[1]);
    }

    #[test]
    fn palico_sets() {
        let mut s = blank();
        let b = SLOT1_BASE;
        clear_palico_set(&mut s, b, 0);
        assert!(!palico_set(&s, b, 0).used());
        set_palico_set_piece(&mut s, b, 0, 2, Some(4));
        assert_eq!(palico_set(&s, b, 0), PalicoSet { name: "Set 01".into(), gear: [NO_BOX, NO_BOX, 4] });
        set_palico_set_name(&mut s, b, 0, "Felyne");
        assert_eq!(palico_set(&s, b, 0).name, "Felyne");
        assert_eq!(s.get(b + PALICO_SETS + PALICO_FLAGS, 3), [1, 1, 1]);
    }
}
