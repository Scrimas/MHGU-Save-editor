//! Names of equipment, skills and talismans from the asset pack.

use super::*;

/// Box type 7 + NN, NN = the game's weaponNN tables (no 05).
pub(super) const WEAPON_CLASSES: [&str; 15] = [
    "Great Sword", "Sword and Shield", "Hammer", "Lance", "Heavy Bowgun", "Weapon", "Light Bowgun", "Long Sword",
    "Switch Axe", "Gunlance", "Bow", "Dual Blades", "Hunting Horn", "Insect Glaive", "Charge Blade",
];
pub(super) const ARMOR_PARTS: [&str; 5] = ["Head", "Chest", "Arms", "Waist", "Legs"];

pub(super) fn kind_label(k: Kind) -> String {
    match k {
        Kind::Empty => "Empty".into(),
        Kind::Head | Kind::Chest | Kind::Arms | Kind::Waist | Kind::Legs => ARMOR_PARTS[k.code() as usize - 1].into(),
        Kind::Talisman => "Talisman".into(),
        Kind::Weapon(w) => WEAPON_CLASSES.get(w as usize).copied().unwrap_or("Weapon").into(),
        // the Palico box holds types 22-24 only (docs/11)
        Kind::Other(22) => "Palico weapon".into(),
        Kind::Other(23) => "Palico head".into(),
        Kind::Other(24) => "Palico body".into(),
        Kind::Other(c) => format!("Type {c}"),
    }
}

/// The game's table of `k` pieces for `owner` (empty without the asset pack).
pub(super) fn pieces(owner: Owner, k: Kind) -> &'static [assets::Piece] {
    let n = assets::names();
    match (owner, k) {
        (Owner::Hunter, Kind::Weapon(w)) => n.weapons.get(&w.to_string()),
        (Owner::Hunter, Kind::Talisman) => Some(&n.talismans),
        (Owner::Hunter, k) if k.is_armor() => n.armor.get(&k.code().to_string()),
        // the Palico box's types 22-24 (docs/11)
        (Owner::Palico, Kind::Other(22)) => Some(&n.palico_weapons),
        (Owner::Palico, Kind::Other(c @ (23 | 24))) => n.palico_armor.get(&c.to_string()),
        _ => None,
    }
    .map_or(&[], Vec::as_slice)
}

pub(super) fn piece(owner: Owner, k: Kind, id: u16) -> Option<&'static assets::Piece> {
    pieces(owner, k).iter().find(|p| p.id == id as u32)
}

/// What the equipment picker offers: for the hunter box the weapon classes, the armor
/// parts and talismans; for the Palico box its weapons, heads and bodies. Only kinds the
/// asset pack names.
pub(super) fn equip_categories(owner: Owner) -> Vec<Kind> {
    let mut v: Vec<Kind> = match owner {
        Owner::Hunter => {
            let mut v: Vec<Kind> = (0..WEAPON_CLASSES.len() as u8).map(Kind::Weapon).collect();
            v.extend([Kind::Head, Kind::Chest, Kind::Arms, Kind::Waist, Kind::Legs, Kind::Talisman]);
            v
        }
        Owner::Palico => vec![Kind::Other(22), Kind::Other(23), Kind::Other(24)],
    };
    v.retain(|&k| !pieces(owner, k).is_empty());
    v
}

/// Decoration slots of a box entry: a talisman's are in the save, armor and weapons'
/// in the asset pack (weapons' by level). None when unknown.
pub fn deco_slots(owner: Owner, e: &equipment::Entry) -> Option<u8> {
    if let Some(t) = e.talisman() {
        return Some(t.slots.min(3));
    }
    let p = piece(owner, e.kind(), e.id())?;
    match e.kind() {
        Kind::Weapon(_) => p.level_slots.get(e.level() as usize - 1).or(p.level_slots.last()).copied(),
        k if k.is_armor() => p.slots,
        _ => None,
    }
}

/// Slots the decorations of an entry take (an unknown item counts as one).
pub fn deco_used(e: &equipment::Entry) -> u8 {
    e.decos().iter().filter(|&&d| d != 0).map(|&d| assets::deco_size(d).unwrap_or(1)).sum()
}

/// Whether armor `p` can give its look to `src` worn by a hunter of body `gender` (0
/// type 1): same part, a class the piece's own allows, a body that can wear it. DERIVED.
pub(super) fn transmog_fits(src: &assets::Piece, p: &assets::Piece, gender: u8) -> bool {
    let class = (src.blade == Some(1) && p.blade == Some(1)) || (src.gunner == Some(1) && p.gunner == Some(1));
    let body = if gender == 0 { p.male != Some(0) } else { p.female != Some(0) };
    class && body && p.is_real()
}

pub(super) fn equip_keep(filter: &str, k: Kind) -> bool {
    match filter {
        "weapon" => matches!(k, Kind::Weapon(_) | Kind::Other(22)),
        "armor" => k.is_armor() || matches!(k, Kind::Other(23) | Kind::Other(24)),
        "talisman" => k == Kind::Talisman,
        "empty" => k == Kind::Empty,
        _ => k != Kind::Empty,
    }
}

pub(super) fn equip_name(owner: Owner, e: &equipment::Entry) -> String {
    let k = e.kind();
    match k {
        Kind::Empty => "Empty".into(),
        Kind::Talisman => assets::names()
            .talismans
            .get(e.id() as usize)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| tier_name(e.talisman().unwrap().tier).to_string()),
        _ => piece(owner, k, e.id())
            .map(|p| p.name_at(e.level() as u32).to_string())
            .unwrap_or_else(|| format!("{} #{}", kind_label(k), e.id())),
    }
}

/// "Fire Res +1 · Attack +3 · 2 slots": what tells talismans apart (10.2).
pub(super) fn talisman_skills(e: &equipment::Entry) -> String {
    let Some(t) = e.talisman() else { return String::new() };
    let mut d: Vec<String> = (0..2).filter(|&j| t.skills[j] != 0).map(|j| format!("{} {:+}", skill_name(t.skills[j]), t.points[j])).collect();
    if d.is_empty() {
        d.push("No skills".into());
    }
    d.push(count(t.slots as usize, "slot", "slots"));
    d.join(" · ")
}

/// A box entry as a value in Review: "Elder Rod Lv 3", "Fire Res +1 · 0 slots", "Empty".
pub fn equip_value(owner: Owner, e: &equipment::Entry) -> String {
    match e.kind() {
        Kind::Empty => "Empty".into(),
        Kind::Talisman => talisman_skills(e),
        _ => format!("{} Lv {}", equip_name(owner, e), e.level()),
    }
}

pub(super) fn piece_rarity(owner: Owner, e: &equipment::Entry) -> u32 {
    match e.kind() {
        Kind::Talisman => assets::names().talismans.get(e.id() as usize).map(|p| p.rarity).unwrap_or(0),
        k => piece(owner, k, e.id()).map(|p| p.rarity).unwrap_or(0),
    }
}

pub(super) fn tier_name(t: u8) -> &'static str {
    match t {
        97 => "Mystery Talisman",
        98 => "Shining Talisman",
        99 => "Timeworn Talisman",
        100 => "Enduring Talisman",
        _ => "Talisman",
    }
}

pub(super) fn skill_name(id: u8) -> String {
    if id == 0 {
        return "—".into();
    }
    assets::names().skills.get(id as usize).cloned().unwrap_or_else(|| format!("Skill #{id}"))
}
