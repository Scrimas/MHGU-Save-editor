//! The character's other unlock maps (docs/11-save-map.md, S fields and sItem): Soaratorium
//! Lab upgrades, Jukebox songs, Provision Division supply drop sets, Cross coin trades,
//! Poogie and Moofy costumes, the Housekeeper's Gallery, the Trader's wares, the Hunter's
//! Notes second list and tips, the Combination List, one-time event scenes, the
//! milestones the award checks read, and the Market, Guild Store and Armory lists. Their
//! entries come from data/unlock-lists.csv (tools/unlock_lists.py); the village pets
//! (names, costumes worn, adoption) and the Moofahs' affection are at the end.
//!
//! Most maps are "U + NEW" like the Smithy and Guild Card maps: the game sets the bit and
//! its NEW copy when it unlocks an entry, and the cursor clears NEW. All DERIVED from code
//! (subagent RE of 2026-10-09) and checked against the save timeline: no map holds a bit
//! past its entries, fresh slots hold the defaults named below. CONFIRMED in game
//! (2026-10-10): the maps lab, song, costume, gallery, combos and tips; the pets' names,
//! costumes and adoption bits; the Moofahs' affection and gifts; the Housekeeper and the
//! start village. The game fills the Trader's items, the coin trades, the supply sets and
//! the delivery requests itself (`Kind::Marks`, read only). The Trader's for-sale lists
//! (words, scenes, costumes), the notes, events and milestones stay DERIVED.

use crate::data::{tables, UnlockEntry};
use crate::guildcard as gc;
use crate::monsters;
use crate::progress::{Char, QuestBit, PROGRESS};
use crate::save::Save;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Unlocked bit plus a NEW copy.
    Unlock,
    /// One bit per entry, no NEW copy: done, seen or read.
    Flag,
    /// Lists the game fills itself: it shows an entry while its condition holds (star
    /// level, flag, Lab upgrade), whatever the bit, which only records that it was listed
    /// (and so got its NEW mark once). Read only. The shops (Market / Guild Store
    /// 0x72ef60, Armory 0x6eb9fc); the Trader's items, the coin trades, the supply drop
    /// sets and the delivery requests, whose bits the game set back with NEW after an edit
    /// cleared them (in game, 2026-10-10).
    Marks,
}

#[derive(Debug, Clone)]
pub struct Map {
    /// The `map` column of data/unlock-lists.csv.
    pub id: String,
    /// Bitmap and its NEW copy, relative to the character base.
    pub on: usize,
    pub new: Option<usize>,
    pub bytes: usize,
    pub kind: Kind,
}

/// Lab upgrades installed (offered at `LAB_OFFERED`); bits 0-2 are the Item Box
/// expansions, counted for the box size (0x525878).
pub const LAB: usize = 0x507B;
pub const LAB_NEW: usize = 0x5087;
pub const LAB_OFFERED: usize = 0x5063;
pub const LAB_OFFERED_NEW: usize = 0x506F;
pub const LAB_BOX: usize = 3;
pub const SUPPLY: usize = 0x5093;
pub const SUPPLY_NEW: usize = 0x509B;
/// The set the Provision Division unlocks when it first opens (0x191374).
pub const SUPPLY_OPEN: usize = 2;
pub const COSTUMES: usize = 0x2F9F;
pub const COSTUMES_NEW: usize = 0x2FA7;
/// The Jukebox song chosen, 0-13; 0 plays the area's own music (`0x523940`).
pub const SONG: usize = 0x2C03;

pub fn maps() -> &'static [Map] {
    static M: OnceLock<Vec<Map>> = OnceLock::new();
    M.get_or_init(|| {
        let m = |id: &str, on: usize, new: Option<usize>, bytes: usize, kind: Kind| Map { id: id.into(), on, new, bytes, kind };
        let mut v = vec![
            m("lab", LAB, Some(LAB_NEW), 12, Kind::Unlock),
            m("song", 0x505B, Some(0x505F), 4, Kind::Unlock),
            m("supply", SUPPLY, Some(SUPPLY_NEW), 8, Kind::Marks),
            m("coin", 0x50A3, Some(0x50AB), 8, Kind::Marks),
            m("costume", COSTUMES, Some(COSTUMES_NEW), 8, Kind::Unlock),
            m("gallery", 0x2F87, Some(0x2F8B), 4, Kind::Unlock),
            m("trader:items0", 0x31EF, Some(0x31F3), 4, Kind::Marks),
            m("trader:items1", 0x31F7, Some(0x31FB), 4, Kind::Marks),
            m("trader:words", 0x31FF, Some(0x3237), 56, Kind::Unlock),
            m("trader:scenes", 0x326F, Some(0x3283), 20, Kind::Unlock),
            m("trader:costumes", 0x3297, Some(0x329B), 4, Kind::Unlock),
            m("trader:delivery", 0x32A7, Some(0x32AB), 4, Kind::Marks),
            // talk action 7 sets the bit and the large list's NEW bit (0x55686c)
            m("notes2", 0x32D7, None, 4, Kind::Flag),
            m("tips", 0x32B3, None, 4, Kind::Flag),
            m("combos", 0x2383B, None, 25, Kind::Flag),
            m("events", 0x2C6B9, None, 4, Kind::Flag),
            m("milestones", 0x4FEF, None, 12, Kind::Flag),
            m("shop:0", 0x32E7, Some(0x3307), 32, Kind::Marks),
            m("shop:1", 0x3327, Some(0x3347), 32, Kind::Marks),
        ];
        // Armory: equipment type T at 0x3367 + 40 (T - 1), 20 B listed then 20 B NEW
        let mut types: Vec<usize> = tables().unlock_lists.iter().filter_map(|e| e.map.strip_prefix("armory:")?.parse().ok()).collect();
        types.dedup();
        for t in types {
            let o = 0x3367 + 40 * (t - 1);
            v.push(m(&format!("armory:{t}"), o, Some(o + 20), 20, Kind::Marks));
        }
        v
    })
}

pub fn find(id: &str) -> Option<usize> {
    maps().iter().position(|m| m.id == id)
}

/// The entries of map `m` the game can set, in bit order.
pub fn entries(m: usize) -> &'static [UnlockEntry] {
    static E: OnceLock<HashMap<String, Vec<UnlockEntry>>> = OnceLock::new();
    let all = E.get_or_init(|| {
        let mut h: HashMap<String, Vec<UnlockEntry>> = HashMap::new();
        for e in tables().unlock_lists.iter().filter(|e| e.need != "never") {
            h.entry(e.map.clone()).or_default().push(e.clone());
        }
        h
    });
    all.get(&maps()[m].id).map_or(&[], Vec::as_slice)
}

pub fn entry(m: usize, bit: usize) -> Option<&'static UnlockEntry> {
    entries(m).iter().find(|e| e.bit == bit)
}

pub fn on(s: &Save, base: usize, m: usize, bit: usize) -> bool {
    s.bit(base + maps()[m].on, bit)
}

/// The entry shows NEW in game: its NEW bit, or for a tip, not read yet. The Lab's list
/// reads the offered map's NEW copy (the cursor clears it, `0x66cfc4`); nothing reads or
/// clears the installed one's.
pub fn is_new(s: &Save, base: usize, m: usize, bit: usize) -> bool {
    let mp = &maps()[m];
    match mp.new {
        _ if mp.id == "lab" => s.bit(base + LAB_OFFERED_NEW, bit),
        Some(n) => s.bit(base + n, bit),
        None => mp.id == "tips" && !on(s, base, m, bit),
    }
}

/// (entries set, entries).
pub fn count(s: &Save, base: usize, m: usize) -> (usize, usize) {
    let e = entries(m);
    (e.iter().filter(|e| on(s, base, m, e.bit)).count(), e.len())
}

/// Why entry `bit` can't be set to `v` by an edit, if it can't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A list the game fills itself (`Kind::Marks`): the bit does not change what it lists.
    Marks,
    /// An Item Box expansion: a smaller box could lose what is stored past it.
    BoxExpansion,
}

pub fn refusal(_s: &Save, _base: usize, m: usize, bit: usize, v: bool) -> Option<Refusal> {
    let mp = &maps()[m];
    if mp.kind == Kind::Marks {
        return Some(Refusal::Marks);
    }
    match mp.id.as_str() {
        "lab" if !v && bit < LAB_BOX => Some(Refusal::BoxExpansion),
        _ => None,
    }
}

/// The Lab upgrade a supply set follows (`0x1914a0`: set T[i] = upgrade 3 + i, T = 1, 3,
/// 4 … 24).
pub fn supply_lab(set: usize) -> Option<usize> {
    match set {
        1 => Some(3),
        3..=24 => Some(set + 1),
        _ => None,
    }
}

/// Set entry `bit` of map `m` as the game does; false when refused (`refusal`) or not an
/// entry. Unlocking sets the NEW copy, locking clears both. Installing a Lab upgrade also
/// offers it (the game installs only offered ones) and, once the Provision Division is
/// open, unlocks the supply set it gives; removing one locks that set. A Hunter's Notes
/// entry of the second list gets the NEW mark of the monster's Notes page.
pub fn set(s: &mut Save, base: usize, m: usize, bit: usize, v: bool) -> bool {
    if refusal(s, base, m, bit, v).is_some() || entry(m, bit).is_none() {
        return false;
    }
    let mp = &maps()[m];
    if on(s, base, m, bit) == v {
        return true;
    }
    s.set_bit(base + mp.on, bit, v);
    if let Some(n) = mp.new {
        s.set_bit(base + n, bit, v);
    }
    match mp.id.as_str() {
        "lab" => {
            if v && !s.bit(base + LAB_OFFERED, bit) {
                s.set_bit(base + LAB_OFFERED, bit, true);
                s.set_bit(base + LAB_OFFERED_NEW, bit, true);
            }
            if let Some(k) = (1..=24).find(|&k| supply_lab(k) == Some(bit))
                && (!v || s.bit(base + SUPPLY, SUPPLY_OPEN))
            {
                s.set_bit(base + SUPPLY, k, v);
                s.set_bit(base + SUPPLY_NEW, k, v);
            }
        }
        "notes2" if v => {
            if let Some(e) = entry(m, bit) {
                s.set_bit(base + monsters::NOTES_NEW, e.id as usize, true);
            }
        }
        _ => {}
    }
    true
}

/// Every entry of map `m` that can be set to `v`; returns how many changed.
pub fn set_all(s: &mut Save, base: usize, m: usize, v: bool) -> usize {
    let bits: Vec<usize> = entries(m).iter().map(|e| e.bit).filter(|&b| on(s, base, m, b) != v).collect();
    bits.into_iter().filter(|&b| set(s, base, m, b, v)).count()
}

/// Whether the game's condition `need` (data/unlock-lists.csv) holds for this character;
/// None when the editor can't tell (downloads, talks, story progress).
pub fn need_holds(s: &Save, slot: usize, need: &str) -> Option<bool> {
    let c = Char::new(s, slot);
    let base = s.base(slot);
    let one = |t: &str| -> Option<bool> {
        let n = |p: &str| t.strip_prefix(p).and_then(|x| x.parse::<usize>().ok());
        Some(match t {
            "start" => true,
            "never" => false,
            "hr" => s.bit(base + PROGRESS, 20),
            "open" => s.bit(base + SUPPLY, SUPPLY_OPEN),
            _ if t.starts_with('v') => c.village_star() as usize >= n("v")?,
            _ if t.starts_with('h') => c.hub_star() as usize >= n("h")?,
            _ if t.starts_with("q") => c.quest(QuestBit::Cleared, tables().quest_index(n("q")? as u32)?),
            _ if t.starts_with("f") => c.flag(n("f")?),
            _ if t.starts_with("lab") => s.bit(base + LAB, n("lab")?),
            _ if t.starts_with("item") => crate::items::obtained(s, base, n("item")? as u16),
            _ => return None,
        })
    };
    let mut any = Some(false);
    for alt in need.split('|') {
        let mut all = Some(true);
        for t in alt.split('&') {
            all = match (all, one(t)) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            };
        }
        any = match (any, all) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        };
    }
    any
}

// --- village pets --------------------------------------------------------------------

/// Pet k: 0 Moofy (Bherna), 1-3 the Poogies of Kokoto, Pokke, Yukumo (`0x50d818`).
pub const PETS: usize = 4;
pub const PET_NAMES: usize = 0x2C4E3;
pub const PET_NAME_LEN: usize = 32;
/// Costume worn, u8 per pet: Moofy 0-5, a Poogie 6-39 (the pet menu `0x77809c`).
pub const PET_COSTUMES: usize = 0x2C563;
/// Pet adopted, bit per pet (`0x50d890`); the title word check grants words 108-111 by
/// it and 107 Pet Lover for all four.
pub const PETS_ADOPTED: usize = 0x2C567;
/// The costume adopting a pet gives (`0x6bf3c8`).
pub const PET_DEFAULT: [u8; PETS] = [0, 6, 19, 23];
/// Event flags the adoption raises: 773-775 for the Poogies, 769 for every pet.
pub const ADOPT_FLAG: usize = 769;
pub const POOGIE_FLAGS: usize = 772;
/// The four Moofahs' affection (Bherna NPCs 24, 25, Palico Ranch 626, 627), at most 10;
/// petting to 6 or more unlocks title word 114 Moofah (`0x50d728`).
pub const MOOFAHS: usize = 0x2C4DE;
pub const MOOFAH_MAX: u8 = 10;
pub const MOOFAH_WORD: usize = 114;

pub fn costume_range(pet: usize) -> std::ops::RangeInclusive<u8> {
    if pet == 0 { 0..=5 } else { 6..=39 }
}

pub fn pet_name(s: &Save, base: usize, k: usize) -> String {
    s.str(base + PET_NAMES + PET_NAME_LEN * k, PET_NAME_LEN)
}

pub fn set_pet_name(s: &mut Save, base: usize, k: usize, name: &str) {
    s.set_str(base + PET_NAMES + PET_NAME_LEN * k, PET_NAME_LEN, name);
}

pub fn pet_costume(s: &Save, base: usize, k: usize) -> u8 {
    s.u8(base + PET_COSTUMES + k)
}

/// Out of the pet's range, the costume stays as it is.
pub fn set_pet_costume(s: &mut Save, base: usize, k: usize, c: u8) -> bool {
    costume_range(k).contains(&c) && {
        s.set_u8(base + PET_COSTUMES + k, c);
        true
    }
}

pub fn adopted(s: &Save, base: usize, k: usize) -> bool {
    s.bit(base + PETS_ADOPTED, k)
}

/// Adopt the pet as its menu's first use does: the default costume (without NEW) and the
/// event flags; un-adopting clears the bit only.
pub fn set_adopted(s: &mut Save, base: usize, slot: usize, k: usize, v: bool) {
    s.set_bit(base + PETS_ADOPTED, k, v);
    if v {
        let c = PET_DEFAULT[k] as usize;
        if !s.bit(base + COSTUMES, c) {
            s.set_bit(base + COSTUMES, c, true);
            s.set_bit(base + COSTUMES_NEW, c, false);
        }
        let mut ch = Char::new(&mut *s, slot);
        if k > 0 {
            ch.set_flag(POOGIE_FLAGS + k, true);
        }
        ch.set_flag(ADOPT_FLAG, true);
    }
}

pub fn moofah(s: &Save, base: usize, k: usize) -> u8 {
    s.u8(base + MOOFAHS + k)
}

pub fn set_moofah(s: &mut Save, base: usize, k: usize, v: u8) {
    let v = v.min(MOOFAH_MAX);
    s.set_u8(base + MOOFAHS + k, v);
    if v >= 6 {
        gc::set_unlocked(s, base, gc::Map::Words, MOOFAH_WORD, true);
    }
}

/// Times the Bherna Moofahs gave a Moofah Fleeceball (once per quest), at most 10
/// (`0x50d7e8`). At 10 the end of the next petting grants award 49 Ball of Moofah Wool
/// (`0x3f31b8`); nothing else reads it.
pub const MOOFAH_GIFTS: usize = 0x2C4E2;
pub const MOOFAH_AWARD: usize = 49;

pub fn moofah_gifts(s: &Save, base: usize) -> u8 {
    s.u8(base + MOOFAH_GIFTS)
}

/// The count, and the award the game would grant at 10.
pub fn set_moofah_gifts(s: &mut Save, slot: usize, v: u8) {
    let v = v.min(MOOFAH_MAX);
    let base = s.base(slot);
    s.set_u8(base + MOOFAH_GIFTS, v);
    if v == MOOFAH_MAX {
        Char::new(&mut *s, slot).set_award(MOOFAH_AWARD, true);
    }
}

/// The Housekeeper of the Room Service, an index into the NPC table `0x162bde8`
/// (`0x50c988`): 0 the Chamberlyne, 1-6 the Guildmarm, Moga Sweetheart, Tanzia
/// Sweetheart, Headwhiskress, Lil Miss Forge and Funky Felyne. Room Service, Change
/// Housekeeper lists 0 and those whose progress bit 23 + i is set (`0x78b474`); the game
/// sets the bit once that villager's request is done (`0x3eebb8`). Past 6 no NPC
/// matches and the Room Service can't be reached.
pub const HOUSEKEEPER: usize = 0x2C56B;
pub const HOUSEKEEPERS: usize = 7;

pub fn housekeeper(s: &Save, base: usize) -> u8 {
    s.u8(base + HOUSEKEEPER)
}

pub fn housekeeper_available(s: &Save, base: usize, i: usize) -> bool {
    i == 0 || (i < HOUSEKEEPERS && s.bit(base + PROGRESS, 23 + i))
}

/// Only a housekeeper the change list offers.
pub fn set_housekeeper(s: &mut Save, base: usize, i: usize) -> bool {
    housekeeper_available(s, base, i) && {
        s.set_u8(base + HOUSEKEEPER, i as u8);
        true
    }
}

/// The place a load starts in, a scene number (`0x6a6e34`). Each village scene start
/// stores its own (`0x5105dc`): 1 Bherna, 2 Kokoto, 3 Pokke, 4 Yukumo, 6 the
/// Soaratorium (the Hub, Palico Ranch and Wycademy Hub store 1, or 6 from the
/// Soaratorium side). The load starts in Bherna past 7.
pub const START_VILLAGE: usize = 0x2C56C;
pub const START_PLACES: [u8; 5] = [1, 2, 3, 4, 6];
/// The airship's gates (`0x557ac4`): Kokoto, Pokke and Yukumo open with event flag 11,
/// the Soaratorium with 1068.
const KOKOTO_FLAG: usize = 11;
const SOARATORIUM_FLAG: usize = 1068;

pub fn start_village(s: &Save, base: usize) -> u8 {
    s.u8(base + START_VILLAGE)
}

/// Whether the airship takes the hunter to `scene` yet.
pub fn place_open(s: &Save, slot: usize, scene: u8) -> bool {
    let c = Char::new(s, slot);
    match scene {
        1 => true,
        2..=4 => c.flag(KOKOTO_FLAG),
        6 => c.flag(SOARATORIUM_FLAG),
        _ => false,
    }
}

/// Only a place the game stores and the airship has opened.
pub fn set_start_village(s: &mut Save, slot: usize, scene: u8) -> bool {
    START_PLACES.contains(&scene) && place_open(s, slot, scene) && {
        let base = s.base(slot);
        s.set_u8(base + START_VILLAGE, scene);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

    #[test]
    fn maps_hold_their_entries() {
        for (m, mp) in maps().iter().enumerate() {
            assert!(!entries(m).is_empty(), "{}", mp.id);
            assert!(entries(m).iter().all(|e| e.bit < 8 * mp.bytes), "{}", mp.id);
            if let Some(n) = mp.new {
                assert!(n >= mp.on + mp.bytes, "{}", mp.id);
            }
        }
        let n = |id| entries(find(id).unwrap()).len();
        assert_eq!((n("lab"), n("song"), n("supply"), n("costume"), n("gallery"), n("combos")), (69, 14, 24, 40, 14, 183));
        // DUMMY title words of the Trader are left out
        assert_eq!(n("trader:words"), 396);
    }

    #[test]
    fn village_counters_keep_to_what_the_game_offers() {
        let mut s = blank();
        let slot = 0;
        // the airship's gates
        assert!(set_start_village(&mut s, slot, 1) && !set_start_village(&mut s, slot, 2) && !set_start_village(&mut s, slot, 5));
        Char::new(&mut s, slot).set_flag(KOKOTO_FLAG, true);
        assert!(set_start_village(&mut s, slot, 3) && start_village(&s, B) == 3);
        assert!(!set_start_village(&mut s, slot, 6));
        // Housekeeper 2 once progress bit 25 is set; none past 6
        assert!(set_housekeeper(&mut s, B, 0) && !set_housekeeper(&mut s, B, 2) && !set_housekeeper(&mut s, B, 7));
        s.set_bit(B + PROGRESS, 25, true);
        assert!(set_housekeeper(&mut s, B, 2) && housekeeper(&s, B) == 2);
        // gifts capped at 10, the award with them
        set_moofah_gifts(&mut s, slot, 9);
        assert!(!Char::new(&s, slot).award(MOOFAH_AWARD));
        set_moofah_gifts(&mut s, slot, 12);
        assert!(moofah_gifts(&s, B) == 10 && Char::new(&s, slot).award(MOOFAH_AWARD));
        // the quest counter takes the Courier's copy along
        let mut c = Char::new(&mut s, slot);
        c.set_quest_counter(500);
        c.set_courier_points(20_000);
        c.set_permit_points(17, true, 60_000);
        assert_eq!((c.quest_counter(), c.courier_points(), c.permit_points(17, true)), (500, 10_000, 9999));
        assert_eq!(s.u32(B + crate::progress::COURIER_TALK), 500);
    }

    #[test]
    fn lab_offers_and_gives_its_supply_set() {
        let mut s = blank();
        let lab = find("lab").unwrap();
        // the Division is not open: the supply set waits for the game
        assert!(set(&mut s, B, lab, 4, true));
        assert!(s.bit(B + LAB_OFFERED, 4) && s.bit(B + LAB_NEW, 4) && !s.bit(B + SUPPLY, 3));
        s.set_bit(B + SUPPLY, SUPPLY_OPEN, true);
        assert!(set(&mut s, B, lab, 5, true) && s.bit(B + SUPPLY, 4) && s.bit(B + SUPPLY_NEW, 4));
        let supply = find("supply").unwrap();
        assert_eq!(refusal(&s, B, supply, 4, false), Some(Refusal::Marks));
        assert!(set(&mut s, B, lab, 5, false) && !s.bit(B + SUPPLY, 4) && s.bit(B + LAB_OFFERED, 5));
        // box expansions stay
        assert!(set(&mut s, B, lab, 0, true) && !set(&mut s, B, lab, 0, false) && on(&s, B, lab, 0));
    }

    #[test]
    fn marks_are_read_only_and_flags_have_no_new_copy() {
        let mut s = blank();
        let shop = find("shop:0").unwrap();
        assert!(!set(&mut s, B, shop, 0, true) && !on(&s, B, shop, 0));
        let notes = find("notes2").unwrap();
        assert!(set(&mut s, B, notes, 12, true));
        assert!(s.bit(B + monsters::NOTES_NEW, 95), "Kirin's Notes page gets NEW");
        let tips = find("tips").unwrap();
        assert!(is_new(&s, B, tips, 3) && set(&mut s, B, tips, 3, true) && !is_new(&s, B, tips, 3));
    }

    #[test]
    fn needs() {
        let mut s = blank();
        let slot = 0;
        assert_eq!(need_holds(&s, slot, "start"), Some(true));
        assert_eq!(need_holds(&s, slot, "v5|h3"), Some(false));
        assert_eq!(need_holds(&s, slot, "dlc"), None);
        assert_eq!(need_holds(&s, slot, "dlc|start"), Some(true));
        Char::new(&mut s, slot).set_hub_star(3);
        assert_eq!(need_holds(&s, slot, "v5|h3"), Some(true));
        assert_eq!(need_holds(&s, slot, "h3&f1303"), Some(false));
    }

    #[test]
    fn pets() {
        let mut s = blank();
        assert!(!set_pet_costume(&mut s, B, 0, 6) && set_pet_costume(&mut s, B, 1, 39));
        set_adopted(&mut s, B, 0, 2, true);
        assert!(adopted(&s, B, 2) && s.bit(B + COSTUMES, 19) && !s.bit(B + COSTUMES_NEW, 19));
        let c = Char::new(&s, 0);
        assert!(c.flag(774) && c.flag(769));
        set_moofah(&mut s, B, 1, 12);
        assert_eq!(moofah(&s, B, 1), 10);
        assert!(gc::unlocked(&s, B, gc::Map::Words, MOOFAH_WORD));
        set_pet_name(&mut s, B, 3, "Bacon");
        assert_eq!(pet_name(&s, B, 3), "Bacon");
    }
}
