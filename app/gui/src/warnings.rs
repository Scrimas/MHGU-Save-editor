//! Values the game cannot produce. They are never refused: any value can still be
//! written. Each page shows the warning next to the value, and Review and Write list
//! the staged ones, with why it matters (`why`).

use crate::assets;
use crate::fmt::num;
use crate::i18n::{tr, trf};
use crate::targets::{Mon, Pal, Target};
use crate::views::{deco_slots, deco_used, piece};
use mhgu_save::equipment::{self, Entry, Kind, Owner};
use mhgu_save::items::{self, Store};
use mhgu_save::data::{tables, TalismanRow};
use mhgu_save::guildcard as gc;
use mhgu_save::{arena, character, monsters, palico, Save};

/// Why a warning matters, shown once next to the list of warnings.
pub fn why() -> &'static str {
    tr("The game never makes these values. They work offline; online, other hunters see your Guild Card, gear and Palicoes and may take them for a hacked save.")
}

/// Whether the asset pack names item `id`: false for the tables' gaps and placeholders.
/// Without the pack every ID passes.
fn real_item(id: u16) -> bool {
    let n = &assets::names().items;
    n.is_empty() || n.get(id as usize).is_some_and(|x| !x.is_empty() && x != "(None)" && !x.starts_with('-'))
}

/// A stack of `count` items `id` in `store`.
pub fn item(id: u16, count: u16, store: Store) -> Option<String> {
    if id == 0 {
        return None;
    }
    if !real_item(id) {
        return Some(tr("not an item of the game").into());
    }
    let cap = assets::item_max(id, store) as u16;
    (count > cap).then(|| trf("over the limit of {}", &[&cap]))
}

/// Pouch positions of a loadout, which fill the pouch: its carry limits apply.
pub fn loadout(l: &items::Loadout) -> Option<String> {
    l.items.iter().find_map(|&(id, n)| item(id, n, Store::Pouch).map(|w| format!("{} · {w}", assets::item_name(id))))
}

/// The talisman rules the game keeps whatever its tables: two different skills, the first
/// filled first, points only with a skill and none of 0.
fn talisman(t: &equipment::Talisman) -> Option<String> {
    let [a, b] = t.skills;
    let [pa, pb] = t.points;
    if a == 0 && b == 0 {
        return Some(tr("a talisman without a skill").into());
    }
    if a != 0 && a == b {
        return Some(tr("the same skill twice").into());
    }
    if a == 0 && b != 0 {
        return Some(tr("a second skill without a first").into());
    }
    if (a != 0 && pa == 0) || (b != 0 && pb == 0) {
        return Some(tr("a skill with 0 points").into());
    }
    if (a == 0 && pa != 0) || (b == 0 && pb != 0) {
        return Some(tr("points without a skill").into());
    }
    None
}

/// The charm tables of the talisman's tier (data/talisman-tables.csv): each skill must be
/// one the tier rolls for that position, with points in its range, and the slots no
/// more than the tier's slot table gives.
fn talisman_tables(t: &equipment::Talisman) -> Option<String> {
    let rows: Vec<&TalismanRow> = tables().talisman.iter().filter(|r| r.tier == t.tier).collect();
    if rows.is_empty() {
        return Some(tr("not a talisman type of the game").into());
    }
    let name = |s: u8| assets::names().skills.get(s as usize).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| trf("Skill #{}", &[&s]));
    let check = |kind: &str, skill: u8, pts: i8| -> Option<String> {
        if skill == 0 {
            return None;
        }
        match rows.iter().find(|r| r.kind == kind && r.skill == skill) {
            None if kind == "skill1" => Some(trf("{} is never the first skill of this talisman type", &[&name(skill)])),
            None => Some(trf("{} is never the second skill of this talisman type", &[&name(skill)])),
            Some(r) if pts < r.min || pts > r.max => Some(trf("{} takes {} to {} points on this talisman type", &[&name(skill), &r.min, &r.max])),
            _ => None,
        }
    };
    let top = rows.iter().find(|r| r.kind == "slots").map_or(3, |r| r.max as u8);
    check("skill1", t.skills[0], t.points[0])
        .or_else(|| check("skill2", t.skills[1], t.points[1]))
        .or_else(|| (t.slots > top).then(|| trf("at most {} slots on this talisman type", &[&top])))
}

/// An equipment box entry. Without the asset pack only the talisman rules apply.
pub fn equip(owner: Owner, e: &Entry) -> Option<String> {
    let k = e.kind();
    if k == Kind::Empty {
        return None;
    }
    if let Some(t) = e.talisman() {
        if let Some(w) = talisman(&t).or_else(|| talisman_tables(&t)) {
            return Some(w);
        }
    } else if assets::available() && !piece(owner, k, e.id()).is_some_and(|p| p.is_real()) {
        return Some(tr("not a piece of the game").into());
    }
    if let (Kind::Weapon(_), Some(p)) = (k, piece(owner, k, e.id()))
        && p.max_lv > 0
        && e.level() as u32 > p.max_lv + p.lim
    {
        return Some(trf("above the top level, {}", &[&(p.max_lv + p.lim)]));
    }
    if let Some(&d) = e.decos().iter().find(|&&d| d != 0 && !assets::names().decos.is_empty() && assets::deco_size(d).is_none()) {
        return Some(trf("{} is not a decoration", &[&assets::item_name(d)]));
    }
    let used = deco_used(e);
    match deco_slots(owner, e) {
        Some(n) if used > n => Some(trf("decorations take {} slots; it has {}", &[&used, &n])),
        _ => None,
    }
}

/// A Palico's level, forte and target.
pub fn palico(p: &palico::Palico, f: Pal) -> Option<String> {
    match f {
        Pal::Level if p.level > palico::MAX_LEVEL => Some(trf("above the top level, {}", &[&palico::MAX_LEVEL])),
        Pal::Bias if p.bias as usize >= palico::BIASES.len() => Some(tr("not a forte of the game").into()),
        // target 0 is not offered by the menu
        Pal::Target if p.target == 0 || p.target as usize >= palico::TARGETS.len() => Some(tr("not a target the game offers").into()),
        Pal::Moves => palico_list(p, true),
        Pal::Skills => palico_list(p, false),
        Pal::Looks => palico::look_fields().into_iter().find(|f| !f.fits(p)).map(|f| {
            let what = crate::views::look_label(f.key);
            if f.colour.is_some() { trf("{}: a colour new Palicoes never get", &[&what]) } else { trf("{}: not a choice of the game", &[&what]) }
        }),
        _ => None,
    }
}

/// A Palico's move or skill list and what it equips (data/palico-*.csv): the forte's
/// innate entries first, innate entries nowhere else (they cannot be taught), each entry
/// once, and the equipped ones from the list within the level's limit.
fn palico_list(p: &palico::Palico, moves: bool) -> Option<String> {
    let t = tables();
    let n = assets::names();
    let name = |id: u8| crate::targets::palico_name(if moves { &n.support_moves } else { &n.palico_skills }, id);
    let (list, on) = if moves { (p.move_list(), &p.moves[..]) } else { (p.skill_list(), &p.skills_on[..]) };
    let count = on.iter().filter(|&&x| x != 0).count();
    if moves && count > p.move_slots() {
        return Some(trf("{} moves equipped, its level allows {}", &[&count, &p.move_slots()]));
    }
    if !moves && p.skills_cost() > p.skill_slots() {
        return Some(trf("skills taking {} slots, its level gives {}", &[&p.skills_cost(), &p.skill_slots()]));
    }
    if let Some(&x) = on.iter().find(|&&x| x != 0 && !list.contains(&x)) {
        return Some(trf("{} equipped but not in its list", &[&name(x)]));
    }
    let f = t.palico_fortes.get(p.bias as usize)?;
    let innate: Vec<u8> = t.palico_fortes.iter().flat_map(|f| if moves { [f.moves.clone(), f.moves2.clone()].concat() } else { f.skills.to_vec() }).collect();
    let head = if moves { p.innate_moves() } else { palico::INNATE_SKILLS };
    let fits = |k: usize, x: u8| match (moves, k) {
        (true, 0) => f.moves.contains(&x),
        (true, _) => f.moves2.contains(&x),
        (false, _) => f.skills[k] == x,
    };
    if list.iter().take(head).enumerate().any(|(k, &x)| !fits(k, x)) {
        return Some(if moves { tr("innate moves that are not its forte's") } else { tr("innate skills that are not its forte's") }.into());
    }
    if let Some(&x) = list.iter().skip(head).find(|&&x| innate.contains(&x)) {
        return Some(trf("{} is a forte's innate entry", &[&name(x)]));
    }
    let none = if moves { palico::NO_MOVE } else { palico::NO_SKILL };
    let twice = list.iter().enumerate().find(|&(k, &x)| x != 0 && x != none && list[..k].contains(&x));
    twice.map(|(_, &x)| trf("{} twice", &[&name(x)]))
}

/// Monster `i`'s hunt counts and size records.
pub fn monster(i: usize, r: &monsters::Record, f: Mon) -> Option<String> {
    let m = monsters::meta(i);
    match f {
        Mon::Hunts | Mon::Captures => {
            let v = if f == Mon::Hunts { r.hunts } else { r.captures };
            (v > monsters::MAX_COUNT).then(|| trf("over the limit of {}", &[&num(monsters::MAX_COUNT)]))
        }
        Mon::Min | Mon::Max if m.size_record => {
            let v = if f == Mon::Min { r.min } else { r.max };
            if m.fixed_size && v != 0 && v != 100 {
                Some(tr("this monster is always size 100").into())
            } else if r.min > r.max {
                Some(tr("the smallest is above the largest").into())
            } else if let Some((lo, hi)) = monsters::quest_range(i).filter(|&(lo, hi)| v != 0 && !(lo..=hi).contains(&v)) {
                Some(trf("its quests give {}–{}", &[&lo, &hi]))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Whether entry `i` of a Guild Card name list is real: named, not a reserve cell.
/// Without the pack every entry within the map passes.
pub fn card_entry(names: &[String], m: gc::Map, i: usize) -> bool {
    let (_, _, n) = m.at();
    i < n && (names.is_empty() || names.get(i).is_some_and(|x| !x.is_empty() && x != "Reserve cell"))
}

/// The card's title, scene or pose: each entry must exist and be unlocked, as the card
/// editor offers only unlocked ones. Linking word 0 (none) is always allowed.
pub fn card(t: Target, s: &Save, base: usize) -> Option<String> {
    let n = assets::names();
    let picks: Vec<(gc::Map, usize, &[String])> = match t {
        Target::Title => {
            let [a, b, c] = gc::title(s, base);
            let mut v = vec![(gc::Map::Words, a as usize, n.gc_words.as_slice()), (gc::Map::Words, c as usize, n.gc_words.as_slice())];
            if b != 0 {
                v.push((gc::Map::Links, b as usize, n.gc_links.as_slice()));
            }
            v
        }
        Target::Scene => vec![(gc::Map::Scenes, gc::scene(s, base) as usize, n.gc_scenes.as_slice())],
        Target::Pose => vec![(gc::Map::Poses, gc::pose(s, base) as usize, n.gc_poses.as_slice())],
        _ => return None,
    };
    if picks.iter().any(|&(m, i, names)| !card_entry(names, m, i)) {
        return Some(tr("not an entry of the game").into());
    }
    picks.iter().any(|&(m, i, _)| !gc::unlocked(s, base, m, i)).then(|| tr("not unlocked in this save").into())
}

/// The warning of a value of the editor, if the game cannot produce it.
/// The card's best time on Arena quest `q`: the clear caps it at 30 minutes, grades it
/// by the quest's times and writes the weapon of the set used.
pub fn arena_best(q: usize, e: &arena::Entry) -> Option<String> {
    let set_weapon = arena::quests()[q].sets.get(e.set as usize).copied();
    if e.time > arena::MAX_TIME {
        Some(tr("over 30 minutes").into())
    } else if e.grade != arena::grade(q, e.time) {
        Some(tr("a grade its time does not earn").into())
    } else if set_weapon != Some(e.weapon) {
        Some(tr("a weapon the set does not have").into())
    } else {
        None
    }
}

/// The hunter's creation choices: faces, hairstyles and voices the game has models for.
fn appearance(s: &Save, base: usize) -> Option<String> {
    use character as ch;
    let l = |k| ch::look(s, base, k);
    if l(ch::LOOK_FACE) >= ch::FACES {
        Some(trf("a face past the game's {}", &[&ch::FACES]))
    } else if l(ch::LOOK_HAIR) >= ch::HAIRSTYLES {
        Some(trf("a hairstyle past the game's {}", &[&ch::HAIRSTYLES]))
    } else if !(1..=ch::VOICES).contains(&l(ch::LOOK_VOICE)) {
        Some(trf("a voice outside the game's 1–{}", &[&ch::VOICES]))
    } else {
        None
    }
}

pub fn of(t: Target, s: &Save, slot: usize) -> Option<String> {
    let base = s.base(slot);
    match t {
        Target::Item(st, i) => {
            let x = items::get(s, base, st, i);
            item(x.id, x.count as u16, st)
        }
        Target::Loadout(k) => loadout(&items::loadout(s, base, k)),
        Target::Equip(o, i) => equip(o, &equipment::get(s, base, o, i)),
        Target::Palico(i, f) => palico(&palico::get(s, base, i), f),
        Target::Monster(i, f) => monster(i, &monsters::get(s, base, i), f),
        Target::Title | Target::Scene | Target::Pose => card(t, s, base),
        Target::Arena(q) => arena::best(s, base, q).and_then(|e| arena_best(q, &e)),
        Target::Appearance => appearance(s, base),
        Target::Gender => (character::look(s, base, character::LOOK_GENDER) > 1).then(|| tr("not a body type of the game").into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mhgu_save::equipment::Talisman;

    #[test]
    fn talisman_rules() {
        let t = |skills: [u8; 2], points: [i8; 2]| talisman(&Talisman { skills, points, slots: 0, tier: 97 });
        assert_eq!(t([5, 9], [4, -2]), None);
        assert_eq!(t([5, 0], [4, 0]), None);
        assert!(t([0, 0], [0, 0]).is_some(), "no skill");
        assert!(t([5, 5], [4, 2]).is_some(), "same skill twice");
        assert!(t([0, 5], [0, 2]).is_some(), "second without first");
        assert!(t([5, 9], [4, 0]).is_some(), "0 points");
        assert!(t([5, 0], [4, 3]).is_some(), "points without a skill");
    }

    #[test]
    fn charm_tables() {
        let t = |tier, skills: [u8; 2], points: [i8; 2], slots| talisman_tables(&Talisman { skills, points, slots, tier });
        // Mystery (97): skill 1 at 1-5 points, no second skill, one slot at most
        assert_eq!(t(97, [1, 0], [5, 0], 1), None);
        assert!(t(97, [1, 0], [6, 0], 0).is_some(), "points above the table");
        assert!(t(97, [1, 2], [3, 1], 0).is_some(), "Mystery has no second skill");
        assert!(t(97, [1, 0], [3, 0], 2).is_some(), "slots");
        assert!(t(42, [1, 0], [3, 0], 0).is_some(), "unknown tier");
    }

    #[test]
    fn palico_list_rules() {
        // Musashi of the analysed save: Gathering, Lv 7, as the game made it
        let mut p = palico::Palico {
            name: "Musashi".into(),
            exp: 0,
            level: 7,
            bias: 6,
            target: 3,
            moves: [37, 27, 0, 0, 0, 0, 0, 0],
            skills_on: [0; 8],
            learned: [37, 27, 9, 1, 39, 44, 50, 34, 32, 14, 0, 0, 57, 57, 57, 57],
            skills: [44, 22, 34, 15, 29, 26, 35, 1, 30, 37, 0, 0],
            move_len: 12,
            skill_len: 12,
            greeting: String::new(),
            owner: String::new(),
            looks: [15, 1, 4, 1, 0, 0, 1, 4, 3, 0, 0, 0],
            colours: [[0xf0, 0xf0, 0xf0, 0xff], [0xed, 0x87, 0x40, 0xff], [0xed, 0x87, 0x40, 0xff], [0xa1, 0x76, 0x4f, 0xff], [0xff; 4], [0xff; 4], [0, 0, 0, 0xff], [0, 0, 0, 0xff], [0, 0, 0, 0xff]],
        };
        assert_eq!(palico(&p, Pal::Looks), None);
        assert_eq!((palico(&p, Pal::Moves), palico(&p, Pal::Skills)), (None, None));
        p.moves = [37, 27, 9, 1, 39, 0, 0, 0];
        assert!(palico(&p, Pal::Moves).is_some(), "5 equipped, 4 slots");
        p.skills_on[0] = palico::SKILL_MOVE_SLOT;
        assert!(palico(&p, Pal::Skills).is_some(), "Support Move +1 is not in its list");
        p.skills[10] = palico::SKILL_MOVE_SLOT;
        assert_eq!((palico(&p, Pal::Moves), palico(&p, Pal::Skills)), (None, None));
        p.learned[10] = 31; // Charisma's innate move in a taught slot
        assert!(palico(&p, Pal::Moves).is_some());
        p.learned[10] = 0;
        p.bias = 0;
        assert!(palico(&p, Pal::Moves).is_some(), "Gathering's innate moves on a Charisma Palico");
    }

    #[test]
    fn arena_rules() {
        // Grudge Match: Malfestio, set 3 = Gunlance (8), A within 300 s
        let e = |time, weapon, set, grade| arena::Entry { time, weapon, partner: 14, set, grade };
        assert_eq!(arena_best(0, &e(8485, 8, 3, 0)), None);
        assert!(arena_best(0, &e(8485, 8, 3, 1)).is_some(), "grade");
        assert!(arena_best(0, &e(8485, 0, 3, 0)).is_some(), "weapon");
        assert!(arena_best(0, &e(200_000, 8, 3, 3)).is_some(), "time");
    }

    #[test]
    fn monster_rules() {
        // index 1 has a size record and crowns (data/monster-sizes.csv)
        let r = |min, max| monsters::Record { hunts: 1, captures: 0, min, max };
        assert_eq!(monster(1, &r(95, 110), Mon::Min), None);
        assert!(monster(1, &r(120, 110), Mon::Max).is_some());
        // Rathian's quests give 88-125 (data/quest-sizes.csv)
        assert_eq!(monster(1, &r(88, 125), Mon::Max), None);
        assert!(monster(1, &r(87, 110), Mon::Min).is_some());
        assert!(monster(1, &r(95, 126), Mon::Max).is_some());
        assert_eq!(monster(1, &r(87, 110), Mon::Max), None);
        assert!(monster(1, &monsters::Record { hunts: 10_000, ..r(0, 0) }, Mon::Hunts).is_some());
        let fixed = (1..=monsters::N).find(|&i| monsters::meta(i).size_record && monsters::meta(i).fixed_size);
        if let Some(i) = fixed {
            assert_eq!(monster(i, &r(100, 100), Mon::Max), None);
            assert!(monster(i, &r(100, 120), Mon::Max).is_some());
        }
    }
}
