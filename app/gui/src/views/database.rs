//! The Database page: the game's own data next to the save, read-only. Its entries link
//! to the pages that edit them, and those pages link back.

use super::*;
use crate::{CrownOdds, DbEquip, DbItem, DbLine, DbMonster, DbQuest, DbQuestMonster, DbRow, DbSkill, DbSource, DbTables, DropItem, DropTable};
use std::collections::HashMap;

/// Quests a crown list shows before "and N more".
const SHOWN: usize = 8;

/// A chance as the page shows it.
fn pct(p: f32) -> String {
    match p {
        p if p >= 0.995 => "100 %".into(),
        p if p < 0.01 => "<1 %".into(),
        p => format!("{:.0} %", p * 100.0),
    }
}

/// "Hub 6★ · cleared": where a quest is listed and whether the save has it open.
fn quest_sub(c: &Char<&mhgu_save::save::Save>, q: &mhgu_save::data::Quest) -> String {
    let mut sub = vec![if q.rank.is_empty() { q.category.clone() } else { format!("{} {}★", q.category, q.rank) }];
    if c.quest(QuestBit::Cleared, q.index) {
        sub.push(tr("cleared").into());
    } else {
        match c.lock(q.id) {
            Lock::Locked(_) => sub.push(tr("locked").into()),
            Lock::Event => sub.push(tr("event quest").into()),
            Lock::Rotated => sub.push(tr("in rotation").into()),
            Lock::Unlocked => {}
        }
    }
    sub.join(" · ")
}

/// The quests with a chance of one crown, best first, and the line about the rest.
fn odds(c: &Char<&mhgu_save::save::Save>, all: &[monsters::CrownOdds], chance: fn(&monsters::CrownOdds) -> f32, none: &str) -> (Vec<CrownOdds>, String) {
    let t = tables();
    let mut v: Vec<&monsters::CrownOdds> = all.iter().filter(|o| chance(o) > 0.0).collect();
    v.sort_by(|a, b| chance(b).total_cmp(&chance(a)).then(a.quest_id.cmp(&b.quest_id)));
    let rows = v
        .iter()
        .take(SHOWN)
        .map(|o| {
            let q = t.quests.iter().find(|q| q.id == o.quest_id);
            CrownOdds {
                quest: q.map_or(format!("#{}", o.quest_id), |q| q.name.clone()).into(),
                sub: q.map(|q| quest_sub(c, q)).unwrap_or_default().into(),
                chance: chance(o),
                label: pct(chance(o)).into(),
            }
        })
        .collect();
    let more = match v.len() {
        0 => none.to_string(),
        n if n <= SHOWN => String::new(),
        n => trn("and {n} more quest at {} or less", "and {n} more quests at {} or less", (n - SHOWN) as i64, &[&pct(chance(v[SHOWN]))]),
    };
    (rows, more)
}

/// Crowns earned and missing, for the list: "Mini · Silver · gold missing".
fn crown_line(i: usize, r: monsters::Record) -> String {
    let m = monsters::meta(i);
    let c = monsters::crowns(i, r);
    let mut have = vec![];
    if c.mini {
        have.push(tr("Mini"));
    }
    match c.large {
        2 => have.push(tr("Gold")),
        1 => have.push(tr("Silver")),
        _ => {}
    }
    let missing = match (c.mini || m.fixed_size, c.large == 2) {
        (true, true) => "",
        (true, false) => tr("gold missing"),
        (false, true) => tr("mini missing"),
        (false, false) => tr("mini and gold missing"),
    };
    have.into_iter().chain((!missing.is_empty()).then_some(missing)).collect::<Vec<_>>().join(" · ")
}

pub(super) fn database_page(ui: &AppWindow, st: &State) {
    match ui.global::<Api>().get_db_tab() {
        1 => quests_tab(ui, st),
        2 => items_tab(ui, st),
        3 => skills_tab(ui, st),
        4 => equip_tab(ui, st),
        _ => monsters_tab(ui, st),
    }
}

/// The Equipment tab's kinds: the weapon classes in the Smithy's order (the game's
/// weaponNN numbers), the armor parts, decorations.
const SMITHY_CLASSES: [u8; 14] = [0, 7, 1, 11, 2, 12, 3, 9, 8, 14, 13, 6, 4, 10];

#[derive(Clone, Copy)]
enum EquipKind {
    Weapon(u8),
    /// part 1-5
    Armor(u8),
    Deco,
}

fn equip_kind(k: usize) -> EquipKind {
    match k {
        k if k < 14 => EquipKind::Weapon(SMITHY_CLASSES[k]),
        k if k < 19 => EquipKind::Armor((k - 13) as u8),
        _ => EquipKind::Deco,
    }
}

/// open-db's index for piece `id` of kind `k`.
const EQUIP_LINK: usize = 10000;

/// The pieces of kind `k`: (ID, name, rarity), decorations by item.
fn equip_pieces(k: EquipKind) -> Vec<(u16, String, u32)> {
    let n = assets::names();
    let real = |v: Option<&'static Vec<assets::Piece>>| v.map_or(&[][..], Vec::as_slice).iter().filter(|p| p.is_real()).map(|p| (p.id as u16, p.name.clone(), p.rarity)).collect();
    match k {
        EquipKind::Weapon(c) => real(n.weapons.get(&c.to_string())),
        EquipKind::Armor(p) => real(n.armor.get(&p.to_string())),
        EquipKind::Deco => n.decos.iter().map(|d| (d[0], assets::item_name(d[0]), 0)).collect(),
    }
}

fn equip_icon(k: EquipKind, id: u16, rarity: u32) -> Option<Image> {
    match k {
        EquipKind::Weapon(c) => assets::equip_icon(Kind::Weapon(c).code(), rarity),
        EquipKind::Armor(p) => assets::equip_icon(p, rarity),
        EquipKind::Deco => assets::item_icon(id),
    }
}

/// Recipes by (kind, ID), in file order.
fn recipes() -> &'static HashMap<(&'static str, u16), Vec<&'static mhgu_save::data::Recipe>> {
    static R: std::sync::OnceLock<HashMap<(&str, u16), Vec<&mhgu_save::data::Recipe>>> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        let mut m: HashMap<(&str, u16), Vec<_>> = HashMap::new();
        for r in &tables().recipes {
            m.entry((r.kind.as_str(), r.id)).or_default().push(r);
        }
        m
    })
}

/// Provision groups: (item, value), cheapest first.
fn provision_groups() -> &'static HashMap<u16, Vec<(u16, u8)>> {
    static G: std::sync::OnceLock<HashMap<u16, Vec<(u16, u8)>>> = std::sync::OnceLock::new();
    G.get_or_init(|| {
        let mut m: HashMap<u16, Vec<(u16, u8)>> = HashMap::new();
        for (&item, &(value, groups)) in &tables().provisions {
            for g in groups.into_iter().filter(|&g| g != 0) {
                m.entry(g).or_default().push((item, value));
            }
        }
        for v in m.values_mut() {
            v.sort_by_key(|&(item, value)| (value, item));
        }
        m
    })
}

/// A recipe's materials against the item box, and whether the box holds them all (the
/// provision counted apart from the materials).
fn recipe_items(r: &mhgu_save::data::Recipe, have: &HashMap<u16, u32>) -> (Vec<DropItem>, bool) {
    let mut ok = true;
    let mut v: Vec<DropItem> = r
        .items
        .iter()
        .map(|&(item, count)| {
            let n = have.get(&item).copied().unwrap_or(0);
            ok &= n >= count as u32;
            let (img, has) = icon(assets::item_icon(item));
            let have = if n >= count as u32 { in_the_box(have, item) } else { trf("{} in the box, {} short", &[&n, &(count as u32 - n)]) };
            DropItem { name: assets::item_name(item).into(), icon: img, has_icon: has, chance: format!("×{count}").into(), have: have.into() }
        })
        .collect();
    if r.group != 0 {
        let members = provision_groups().get(&r.group).map_or(&[][..], Vec::as_slice);
        let worth: u32 = members.iter().map(|&(item, value)| have.get(&item).copied().unwrap_or(0) * value as u32).sum();
        ok &= worth >= r.value as u32;
        // the game names no group: shown by its first materials in item order (ores and
        // parts before scraps)
        let mut ids: Vec<u16> = members.iter().map(|m| m.0).collect();
        ids.sort_unstable();
        let mut shown: Vec<String> = ids.iter().take(3).map(|&i| assets::item_name(i)).collect();
        if ids.len() > 3 {
            shown.push("…".into());
        }
        let (img, has) = icon(assets::item_icon(ids.first().copied().unwrap_or(0)));
        v.push(DropItem {
            name: tr("Provisions").into(),
            icon: img,
            has_icon: has,
            chance: trf("value {}", &[&r.value]).into(),
            have: format!("{} · {}", trf("worth {} in the box", &[&worth]), shown.join(", ").replace(", …", "…")).into(),
        });
    }
    (v, ok)
}

/// "Forge · ready": a table title and whether the box holds what it takes.
fn recipe_table(title: String, r: &mhgu_save::data::Recipe, have: &HashMap<u16, u32>) -> (DropTable, bool) {
    let (items, ok) = recipe_items(r, have);
    let state = if ok { tr("ready") } else { tr("missing materials") };
    (DropTable { title: format!("{title} · {state}").into(), items: model(items) }, ok)
}

/// What piece `id` of kind `k` takes to make (forge, or upgrade from its parent), as
/// tables, and whether the box holds all of one of them.
fn equip_make(k: EquipKind, id: u16, have: &HashMap<u16, u32>) -> (Vec<DropTable>, bool) {
    let t = tables();
    let r = recipes();
    let mut v = vec![];
    let forge_kind = match k {
        EquipKind::Weapon(c) => format!("weapon:{c}"),
        EquipKind::Armor(p) => format!("armor:{p}"),
        EquipKind::Deco => "deco".into(),
    };
    let rows = r.get(&(forge_kind.as_str(), id)).map_or(&[][..], Vec::as_slice);
    for (j, f) in rows.iter().filter(|f| f.level == 0).enumerate() {
        v.push(recipe_table(if j == 0 { tr("Forge").to_string() } else { tr("Forge (other recipe)").to_string() }, f, have));
    }
    if let EquipKind::Weapon(c) = k
        && let Some(&(_, _, parent, lv)) = t.weapon_tree.iter().find(|w| w.0 == c && w.1 == id)
        && let Some(u) = rows.iter().find(|f| f.level == 1)
    {
        let name = piece(Owner::Hunter, Kind::Weapon(c), parent).map_or_else(|| format!("#{parent}"), |p| p.name.clone());
        v.push(recipe_table(trf("Upgrade from {} Lv {}", &[&name, &lv]), u, have));
    }
    let ok = v.iter().any(|x| x.1);
    (v.into_iter().map(|x| x.0).collect(), ok)
}

/// Pieces of equipment in the box, by (box type, ID).
fn equip_counts(st: &State) -> HashMap<(u8, u16), u32> {
    let mut m: HashMap<(u8, u16), u32> = HashMap::new();
    for i in 0..Owner::Hunter.len() {
        let e = equipment::get(st.save(), st.base(), Owner::Hunter, i);
        if !e.is_empty() {
            *m.entry((e.kind().code(), e.id())).or_default() += 1;
        }
    }
    m
}

/// "2 in your box", "none in your box".
fn owned(k: EquipKind, id: u16, eq: &HashMap<(u8, u16), u32>, have: &HashMap<u16, u32>) -> String {
    let n = match k {
        EquipKind::Weapon(c) => eq.get(&(Kind::Weapon(c).code(), id)).copied().unwrap_or(0),
        EquipKind::Armor(p) => eq.get(&(p, id)).copied().unwrap_or(0),
        EquipKind::Deco => return in_the_box(have, id),
    };
    match n {
        0 => tr("none in your box").to_string(),
        n => trf("{} in your box", &[&n]),
    }
}

fn equip_tab(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let (f, can, kind, mut sel) = view(|v| (v.db_search.to_lowercase(), v.db_missing, v.db_equip_kind, v.db_equip));
    let k = equip_kind(kind);
    let have = box_counts(st);
    let eq = equip_counts(st);
    let rows: Vec<DbRow> = equip_pieces(k)
        .into_iter()
        .filter(|(_, name, _)| f.is_empty() || name.to_lowercase().contains(&f))
        .filter(|(id, _, _)| !can || equip_make(k, *id, &have).1)
        .map(|(id, name, rarity)| {
            let (img, has) = icon(equip_icon(k, id, rarity));
            DbRow { index: id as i32, name: name.into(), sub: owned(k, id, &eq, &have).into(), icon: img, has_icon: has }
        })
        .collect();
    if sel == 0 || !rows.iter().any(|r| r.index as usize == sel) {
        sel = rows.first().map_or(0, |r| r.index as usize);
        view(|v| v.db_equip = sel);
    }
    let mut kinds: Vec<String> = SMITHY_CLASSES.iter().map(|&c| weapon_classes()[c as usize].to_string()).collect();
    kinds.extend(armor_parts().iter().map(|p| p.to_string()));
    kinds.push(tr("Decorations").into());
    api.set_db_equip_kinds(strings(kinds));
    api.set_db_equip_kind(kind as i32);
    api.set_db_row(rows.iter().position(|r| r.index as usize == sel).unwrap_or(0) as i32);
    api.set_db_equips(model(rows));
    api.set_db_equip(if sel == 0 { DbEquip { index: -1, ..Default::default() } } else { equip(kind, sel as u16, &have, &eq) });
}

fn equip(kind: usize, id: u16, have: &HashMap<u16, u32>, eq: &HashMap<(u8, u16), u32>) -> DbEquip {
    let t = tables();
    let k = equip_kind(kind);
    let (name, rarity) = equip_pieces(k).into_iter().find(|p| p.0 == id).map_or((format!("#{id}"), 0), |p| (p.1, p.2));
    let (img, has) = icon(equip_icon(k, id, rarity));
    let (make, _) = equip_make(k, id, have);
    let mut sub = vec![];
    if rarity > 0 {
        sub.push(trf("Rarity {}", &[&rarity]));
    }
    sub.push(owned(k, id, eq, have));
    // the pieces before and after it, and its levels past the forge
    let mut tree = vec![];
    let mut steps: Vec<&mhgu_save::data::Recipe> = vec![];
    let line = |c: u8, w: u16, sub: String| {
        let p = piece(Owner::Hunter, Kind::Weapon(c), w);
        let (img, has) = icon(assets::equip_icon(Kind::Weapon(c).code(), p.map_or(1, |p| p.rarity)));
        DbLine { name: p.map_or_else(|| format!("#{w}"), |p| p.name.clone()).into(), sub: sub.into(), icon: img, has_icon: has, link: (kind * EQUIP_LINK + w as usize) as i32, ..Default::default() }
    };
    match k {
        EquipKind::Weapon(c) => {
            if let Some(&(_, _, parent, lv)) = t.weapon_tree.iter().find(|w| w.0 == c && w.1 == id) {
                tree.push(line(c, parent, trf("Upgrades into this one from Lv {}", &[&lv])));
            }
            let mut kids: Vec<_> = t.weapon_tree.iter().filter(|w| w.0 == c && w.2 == id).collect();
            kids.sort_by_key(|w| (w.3, w.1));
            for &&(_, child, _, lv) in &kids {
                tree.push(line(c, child, trf("This one upgrades into it from Lv {}", &[&lv])));
            }
            steps.extend(recipes().get(&(format!("weapon:{c}").as_str(), id)).map_or(&[][..], Vec::as_slice).iter().filter(|r| r.level >= 2));
        }
        EquipKind::Armor(_) => steps.extend(recipes().get(&("armor", id)).map_or(&[][..], Vec::as_slice).iter().filter(|r| r.level >= 2)),
        EquipKind::Deco => {}
    }
    let tables: Vec<DropTable> = steps.iter().map(|r| recipe_table(trf("Level {}", &[&r.level]), r, have).0).collect();
    let levels = tables.chunks(4).map(|c| DbTables { tables: model(c.to_vec()) }).collect();
    // a provision's line gives a value, a material's a count
    let provisions = make.iter().chain(&tables).any(|m| m.items.iter().any(|i| !i.chance.starts_with('×')));
    DbEquip {
        index: id as i32,
        name: name.into(),
        sub: sub.join(" · ").into(),
        icon: img,
        has_icon: has,
        make: model(make),
        tree: model(tree),
        levels: model(levels),
        note: if provisions {
            tr("Provisions: any materials of the kind listed, as long as their values add up to the value asked (a material of value 2 counts twice). Zenny costs are not in these tables.")
        } else {
            tr("Zenny costs are not in these tables.")
        }
        .into(),
    }
}

/// Armor pieces a skill lists before "and N more".
const PIECES: usize = 40;

/// Points as the game shows them: "+3", "-2".
fn points(p: i8) -> String {
    format!("{p:+}")
}

/// A skill a tree activates (skillData record).
fn activated(id: u16) -> String {
    assets::names().skill_names.get(id as usize).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| trf("Skill #{}", &[&id]))
}

/// The skills tree `k` activates: (points needed, skill), positive tiers then negative ones.
fn tiers(k: usize) -> Vec<(i8, u16)> {
    const AT: [i8; 6] = [-20, -15, -10, 10, 15, 20];
    let ids = tables().skill_trees[k].1;
    let mut v = vec![];
    // each tier holds the skill active from its points on: a new skill starts where it changes
    for side in [[3, 4, 5], [2, 1, 0]] {
        let mut last = 0;
        for j in side {
            if ids[j] != 0 && ids[j] != last {
                v.push((AT[j], ids[j]));
            }
            last = ids[j];
        }
    }
    v
}

/// Skill trees in ID order, as the talisman editor lists them; the search also finds the
/// skills they activate.
fn skills_tab(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let t = tables();
    let (f, decos, mut sel) = view(|v| (v.db_search.to_lowercase(), v.db_missing, v.db_skill));
    let rows: Vec<DbRow> = (1..t.skill_trees.len())
        .filter(|&k| !decos || t.decorations.iter().any(|d| d.2 as usize == k))
        .filter_map(|k| {
            let name = skill_name(k as u8);
            let sub = tiers(k).iter().filter(|x| x.0 > 0).map(|x| activated(x.1)).collect::<Vec<_>>().join(" · ");
            // found by its name, any skill it activates (down ones too) or a decoration raising it
            let found = || {
                let mut words = vec![name.clone()];
                words.extend(tiers(k).iter().map(|x| activated(x.1)));
                words.extend(t.decorations.iter().filter(|d| d.2 as usize == k && d.3 > 0).map(|d| assets::item_name(d.0)));
                words.iter().any(|w| w.to_lowercase().contains(&f))
            };
            if !f.is_empty() && !found() {
                return None;
            }
            Some(DbRow { index: k as i32, name: name.into(), sub: sub.into(), ..Default::default() })
        })
        .collect();
    if sel == 0 || !rows.iter().any(|r| r.index as usize == sel) {
        sel = rows.first().map_or(0, |r| r.index as usize);
        view(|v| v.db_skill = sel);
    }
    api.set_db_row(rows.iter().position(|r| r.index as usize == sel).unwrap_or(0) as i32);
    api.set_db_skills(model(rows));
    api.set_db_skill(if sel == 0 { DbSkill { index: -1, ..Default::default() } } else { skill(st, sel) });
}

fn skill(st: &State, k: usize) -> DbSkill {
    let t = tables();
    let n = assets::names();
    let have = box_counts(st);
    let line = |name: String, sub: String, value: String, i: Option<Image>| {
        let (img, has) = icon(i);
        DbLine { name: name.into(), sub: sub.into(), value: value.into(), icon: img, has_icon: has, link: 0 }
    };
    let tiers: Vec<DbLine> = tiers(k).into_iter().map(|(at, id)| line(activated(id), String::new(), trf("{} points", &[&points(at)]), None)).collect();
    // the other skill of a decoration or piece: "Expert +2"
    let others = |rows: &mut dyn Iterator<Item = (u8, i8)>| rows.filter(|r| r.0 as usize != k).map(|(s, p)| format!("{} {}", skill_name(s), points(p))).collect::<Vec<_>>();
    // only what raises it: the decorations that lower it are counted, armor that does is left out
    let lower = t.decorations.iter().filter(|d| d.2 as usize == k && d.3 < 0).count();
    let mut decos: Vec<_> = t.decorations.iter().filter(|d| d.2 as usize == k && d.3 > 0).collect();
    decos.sort_by_key(|d| (-d.3, d.1, d.0));
    let decos: Vec<DbLine> = decos
        .into_iter()
        .map(|&(item, slots_, _, p)| {
            let mut sub = vec![slots(slots_ as usize)];
            sub.extend(others(&mut t.decorations.iter().filter(|d| d.0 == item).map(|d| (d.2, d.3))));
            sub.push(in_the_box(&have, item));
            line(assets::item_name(item), sub.join(" · "), points(p), assets::item_icon(item))
        })
        .collect();
    let charms: Vec<DbLine> = t
        .talisman
        .iter()
        .filter(|r| r.skill as usize == k && r.kind != "slots")
        .map(|r| {
            let item = 353 + (r.tier - 97) as u16;
            let which = if r.kind == "skill1" { tr("first skill") } else { tr("second skill") };
            line(assets::item_name(item), which.into(), format!("{} … {}", points(r.min), points(r.max)), assets::item_icon(item))
        })
        .collect();
    let mut armor: Vec<_> = t.armor_skills.iter().filter(|a| a.2 as usize == k && a.3 > 0).collect();
    armor.sort_by_key(|a| (-a.3, std::cmp::Reverse(a.0), a.1));
    let more = armor.len().saturating_sub(PIECES);
    let armor: Vec<DbLine> = armor
        .into_iter()
        .take(PIECES)
        .map(|&(series, part, _, p)| {
            let piece = n.armor.get(&part.to_string()).and_then(|v| v.iter().find(|x| x.id == series as u32));
            let sub = others(&mut t.armor_skills.iter().filter(|a| a.0 == series && a.1 == part).map(|a| (a.2, a.3))).join(" · ");
            let name = piece.map_or_else(|| trf("Armor #{}", &[&series]), |x| x.name.clone());
            line(name, sub, points(p), assets::equip_icon(part, piece.map_or(1, |x| x.rarity)))
        })
        .collect();
    DbSkill {
        index: k as i32,
        name: skill_name(k as u8).into(),
        sub: format!("#{k}").into(),
        tiers: model(tiers),
        decos: model(decos),
        decos_more: if lower > 0 { trn("{n} other decoration lowers it", "{n} other decorations lower it", lower as i64, &[]) } else { String::new() }.into(),
        charms: model(charms),
        armor: model(armor),
        armor_more: if more > 0 { trn("and {n} more piece", "and {n} more pieces", more as i64, &[]) } else { String::new() }.into(),
    }
}

/// Sources of an item listed before "and N more".
const SOURCES: usize = 40;

/// Where an item comes from.
enum Source {
    /// monster, rank, kind, count, chance
    Carve(usize, &'static str, &'static str, u8, u8),
    /// quest, reward slot, count, chance
    Reward(&'static mhgu_save::data::Quest, u8, u8, u8),
    /// the two items combined, chance
    Combo(u16, u16, u8),
}

/// Every carve and quest reward, by item.
fn sources() -> &'static HashMap<u16, Vec<Source>> {
    static S: std::sync::OnceLock<HashMap<u16, Vec<Source>>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let t = tables();
        let mut m: HashMap<u16, Vec<Source>> = HashMap::new();
        for c in &t.carves {
            m.entry(c.item).or_default().push(Source::Carve(c.monster, &c.rank, &c.kind, c.count, c.chance));
        }
        for q in Char::real_quests(true) {
            for &(slot, rem) in t.quest_rewards.get(&q.id).map_or(&[][..], Vec::as_slice) {
                for &(item, count, chance) in &t.rewards[&rem] {
                    m.entry(item).or_default().push(Source::Reward(q, slot, count, chance));
                }
            }
        }
        for &(a, b, result, chance) in &t.combinations {
            m.entry(result).or_default().push(Source::Combo(a, b, chance));
        }
        m
    })
}

/// What takes an item: a Smithy recipe (with how many) or a combination.
enum Use {
    Recipe(&'static mhgu_save::data::Recipe, u8),
    /// the other item, the result, chance
    Combo(u16, u16, u8),
}

/// Every Smithy recipe and combination, by item taken.
fn uses() -> &'static HashMap<u16, Vec<Use>> {
    static U: std::sync::OnceLock<HashMap<u16, Vec<Use>>> = std::sync::OnceLock::new();
    U.get_or_init(|| {
        let t = tables();
        let mut m: HashMap<u16, Vec<Use>> = HashMap::new();
        for r in &t.recipes {
            for &(item, count) in &r.items {
                m.entry(item).or_default().push(Use::Recipe(r, count));
            }
        }
        for &(a, b, result, chance) in &t.combinations {
            m.entry(a).or_default().push(Use::Combo(b, result, chance));
            if b != a {
                m.entry(b).or_default().push(Use::Combo(a, result, chance));
            }
        }
        m
    })
}

/// A recipe's piece: its name, icon, and the Equipment tab entry (open-db index).
fn recipe_piece(r: &mhgu_save::data::Recipe) -> (String, Option<Image>, i32) {
    let link = |k: usize| (k * EQUIP_LINK + r.id as usize) as i32;
    let n = assets::names();
    if let Some(c) = r.kind.strip_prefix("weapon:").and_then(|c| c.parse::<u8>().ok()) {
        let p = piece(Owner::Hunter, Kind::Weapon(c), r.id);
        let k = SMITHY_CLASSES.iter().position(|&x| x == c).unwrap_or(0);
        return (p.map_or_else(|| format!("#{}", r.id), |p| p.name.clone()), assets::equip_icon(Kind::Weapon(c).code(), p.map_or(1, |p| p.rarity)), link(k));
    }
    if r.kind == "deco" {
        return (assets::item_name(r.id), assets::item_icon(r.id), link(19));
    }
    // armor: the part forged, or for levels the first part of the series
    let parts: Vec<u8> = match r.kind.strip_prefix("armor:").and_then(|p| p.parse().ok()) {
        Some(p) => vec![p],
        None => (1..=5).collect(),
    };
    for p in parts {
        if let Some(x) = n.armor.get(&p.to_string()).and_then(|v| v.iter().find(|x| x.id == r.id as u32)) {
            let name = if r.kind == "armor" { trf("{} and the rest of its set", &[&x.name]) } else { x.name.clone() };
            return (name, assets::equip_icon(p, x.rarity), link(13 + p as usize));
        }
    }
    (trf("Armor #{}", &[&r.id]), None, 0)
}

/// Uses of an item listed before "and N more".
const USES: usize = 40;

/// What takes item `id`: combinations first, then the Smithy's recipes, forges first.
fn item_uses(id: u16) -> (Vec<DbSource>, String) {
    let all = uses().get(&id).map_or(&[][..], Vec::as_slice);
    let mut v: Vec<(u8, DbSource)> = all
        .iter()
        .map(|u| match *u {
            Use::Combo(other, result, chance) => {
                let (img, has) = icon(assets::item_icon(result));
                let src = DbSource {
                    kind: "item".into(),
                    index: result as i32,
                    r#where: assets::item_name(result).into(),
                    how: trf("Combined with {}", &[&assets::item_name(other)]).into(),
                    chance: format!("{chance} %").into(),
                    icon: img,
                    has_icon: has,
                };
                (0, src)
            }
            Use::Recipe(r, count) => {
                let (name, i, link) = recipe_piece(r);
                let (img, has) = icon(i);
                let how = match r.level {
                    0 => tr("Forge").to_string(),
                    1 if r.kind.starts_with("weapon") => tr("Upgrade").to_string(),
                    l => trf("Level {}", &[&l]),
                };
                let src = DbSource { kind: if link > 0 { "equip" } else { "" }.into(), index: link, r#where: name.into(), how: how.into(), chance: format!("×{count}").into(), icon: img, has_icon: has };
                (1 + r.level.min(1), src)
            }
        })
        .collect();
    v.sort_by_key(|x| x.0);
    let more = v.len().saturating_sub(USES);
    let more = if more > 0 { trn("and {n} more use", "and {n} more uses", more as i64, &[]) } else { String::new() };
    (v.into_iter().take(USES).map(|x| x.1).collect(), more)
}

/// Items in ID order, as the item picker lists them.
fn items_tab(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let (f, in_box, mut sel) = view(|v| (v.db_search.to_lowercase(), v.db_missing, v.db_item));
    let n = assets::names();
    let have = box_counts(st);
    let rows: Vec<DbRow> = (1..n.items.len().min(items::MAX_ID as usize + 1))
        .filter(|&id| !n.items[id].is_empty() && n.items[id] != "(None)" && n.items[id] != "DUMMY" && !n.items[id].starts_with('-'))
        .filter(|id| !in_box || have.contains_key(&(*id as u16)))
        .filter_map(|id| {
            let name = assets::item_name(id as u16);
            if !f.is_empty() && !name.to_lowercase().contains(&f) && id.to_string() != f {
                return None;
            }
            let (img, has) = icon(assets::item_icon(id as u16));
            Some(DbRow { index: id as i32, name: name.into(), sub: in_the_box(&have, id as u16).into(), icon: img, has_icon: has })
        })
        .collect();
    if sel == 0 || !rows.iter().any(|r| r.index as usize == sel) {
        sel = rows.first().map_or(0, |r| r.index as usize);
        view(|v| v.db_item = sel);
    }
    api.set_db_row(rows.iter().position(|r| r.index as usize == sel).unwrap_or(0) as i32);
    api.set_db_items(model(rows));
    api.set_db_item(if sel == 0 { DbItem { index: -1, ..Default::default() } } else { item(sel as u16, &have) });
}

/// "3 in the box", "none in the box".
fn in_the_box(have: &HashMap<u16, u32>, id: u16) -> String {
    match have.get(&id).copied().unwrap_or(0) {
        0 => tr("none in the box").to_string(),
        n => trf("{} in the box", &[&num(n)]),
    }
}

fn item(id: u16, have: &HashMap<u16, u32>) -> DbItem {
    let t = tables();
    let (img, has) = icon(assets::item_icon(id));
    let rank = |r: &str| match r {
        "low" => tr("Low rank"),
        "high" => tr("High rank"),
        _ => tr("G rank"),
    };
    let mut all: Vec<(u8, DbSource)> = sources()
        .get(&id)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|s| {
            let (count, chance, src) = match *s {
                Source::Carve(m, r, kind, count, chance) => {
                    let (img, has) = icon(assets::monster_icon(m));
                    let how = match kind {
                        "body" => tr("Body carve"),
                        "tail" => tr("Tail carve"),
                        "shiny" => tr("Shiny drop"),
                        _ => tr("Other carve or drop"),
                    };
                    let src = DbSource {
                        kind: "monster".into(),
                        index: m as i32,
                        r#where: assets::monster_name(m).unwrap_or(&t.monsters[m - 1].name).into(),
                        how: format!("{} · {how}", rank(r)).into(),
                        icon: img,
                        has_icon: has,
                        ..Default::default()
                    };
                    (count, chance, src)
                }
                Source::Reward(q, slot, count, chance) => {
                    let how = match slot {
                        0 => tr("Main rewards"),
                        1 => tr("More main rewards"),
                        4 => tr("Subquest rewards"),
                        _ => tr("Extra rewards"),
                    };
                    let src = DbSource { kind: "quest".into(), index: q.index as i32, r#where: q.name.clone().into(), how: how.into(), ..Default::default() };
                    (count, chance, src)
                }
                Source::Combo(a, b, chance) => {
                    let (img, has) = icon(assets::item_icon(a));
                    let src = DbSource {
                        r#where: format!("{} + {}", assets::item_name(a), assets::item_name(b)).into(),
                        how: tr("Combination").into(),
                        icon: img,
                        has_icon: has,
                        ..Default::default()
                    };
                    (1, chance, src)
                }
            };
            let how = if count > 1 { format!("{} · ×{count}", src.how) } else { src.how.to_string() };
            (chance, DbSource { how: how.into(), chance: format!("{chance} %").into(), ..src })
        })
        .collect();
    all.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.r#where.cmp(&b.1.r#where)));
    let more = all.len().saturating_sub(SOURCES);
    let (used, uses_more) = item_uses(id);
    let mut sub = vec![format!("#{id}"), in_the_box(have, id)];
    if let Some(&(value, _)) = t.provisions.get(&id) {
        sub.push(trf("value {} as Smithy provisions", &[&value]));
    }
    DbItem {
        index: id as i32,
        name: assets::item_name(id).into(),
        sub: sub.join(" · ").into(),
        uses: model(used),
        uses_more: uses_more.into(),
        icon: img,
        has_icon: has,
        more: if more > 0 { trn("and {n} more source", "and {n} more sources", more as i64, &[]) } else { String::new() }.into(),
        sources: model(all.into_iter().take(SOURCES).map(|s| s.1).collect()),
    }
}

/// What the item box holds, by item.
fn box_counts(st: &State) -> HashMap<u16, u32> {
    let mut have: HashMap<u16, u32> = HashMap::new();
    for s in items::all(st.save(), st.base(), Store::Box) {
        *have.entry(s.id).or_default() += s.count as u32;
    }
    have
}

/// An item of a carve or reward table.
fn drop_item(item: u16, count: u8, chance: u8, have: &HashMap<u16, u32>) -> DropItem {
    let (img, has) = icon(assets::item_icon(item));
    let name = assets::item_name(item);
    DropItem {
        name: if count > 1 { format!("{name} ×{count}") } else { name }.into(),
        icon: img,
        has_icon: has,
        chance: format!("{chance} %").into(),
        have: in_the_box(have, item).into(),
    }
}

/// Quests in the Quests page's order: by tab, deviant, rank, ID.
fn quests_tab(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let c = Char::new(st.save(), st.slot);
    let (f, missing, mut sel) = view(|v| (v.db_search.to_lowercase(), v.db_missing, v.db_quest));
    let tabs = quest_tabs();
    let mut qs = Char::real_quests(true);
    qs.sort_by_key(|q| (tabs.iter().position(|t| *t == q.category), deviant_of(q.index).unwrap_or(0), q.rank.parse::<u32>().unwrap_or(99), q.id));
    let rows: Vec<DbRow> = qs
        .iter()
        .filter(|q| !missing || !c.quest(QuestBit::Cleared, q.index))
        .filter(|q| f.is_empty() || q.name.to_lowercase().contains(&f) || q.id.to_string().contains(&f))
        .map(|q| DbRow { index: q.index as i32, name: q.name.clone().into(), sub: quest_sub(&c, q).into(), ..Default::default() })
        .collect();
    if sel == 0 || !rows.iter().any(|r| r.index as usize == sel) {
        sel = rows.first().map_or(0, |r| r.index as usize);
        view(|v| v.db_quest = sel);
    }
    api.set_db_row(rows.iter().position(|r| r.index as usize == sel).unwrap_or(0) as i32);
    api.set_db_quests(model(rows));
    api.set_db_quest(match tables().quests.iter().find(|q| q.index == sel && sel > 0) {
        Some(q) => quest(st, &c, q),
        None => DbQuest { index: -1, ..Default::default() },
    });
}

/// A chance in a table cell; "–" for none.
fn cell(p: Option<f32>) -> String {
    match p {
        Some(p) if p > 0.0 => pct(p),
        _ => "–".into(),
    }
}

fn quest(st: &State, c: &Char<&mhgu_save::save::Save>, q: &mhgu_save::data::Quest) -> DbQuest {
    let t = tables();
    let monsters = monsters::quest_monsters(q.id)
        .into_iter()
        .map(|e| {
            let (img, has) = icon(assets::monster_icon(e.monster));
            DbQuestMonster {
                index: e.monster as i32,
                name: assets::monster_name(e.monster).unwrap_or(&t.monsters[e.monster - 1].name).into(),
                icon: img,
                has_icon: has,
                size: format!("{} %", e.size).into(),
                range: if e.min == e.max { format!("{} %", e.min) } else { format!("{}–{} %", e.min, e.max) }.into(),
                gold: cell(e.odds.map(|o| o.2)).into(),
                mini: cell(e.odds.map(|o| o.0)).into(),
            }
        })
        .collect();
    let have = box_counts(st);
    let slots = t.quest_rewards.get(&q.id).map_or(&[][..], Vec::as_slice);
    let rewards: Vec<DropTable> = slots
        .iter()
        .map(|&(slot, rem)| DropTable {
            title: match slot {
                0 => tr("Main rewards"),
                1 => tr("More main rewards"),
                4 => tr("Subquest rewards"),
                _ => tr("Extra rewards"),
            }
            .into(),
            items: model(t.rewards[&rem].iter().map(|&(item, count, chance)| drop_item(item, count, chance, &have)).collect()),
        })
        .collect();
    DbQuest {
        index: q.index as i32,
        name: q.name.clone().into(),
        sub: format!("#{} · {}", q.id, quest_sub(c, q)).into(),
        monsters: model(monsters),
        note: if rewards.is_empty() {
            tr("This quest is not in the game's files: it comes as a download.")
        } else {
            tr("The chance of each item per reward drawn; how many are drawn is not in these files.")
        }
        .into(),
        rewards: model(rewards),
    }
}

fn monsters_tab(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let t = tables();
    let (f, missing, mut sel) = view(|v| (v.db_search.to_lowercase(), v.db_missing, v.db_monster));
    let name = |i: usize| assets::monster_name(i).unwrap_or(&t.monsters[i - 1].name);
    let rows: Vec<DbRow> = t
        .monsters
        .iter()
        .filter(|m| m.large && !m.name.is_empty() && !m.name.starts_with("dummy") && !(106..=112).contains(&m.index))
        .filter(|m| !missing || misses_crown(m.index, monsters::get(s, base, m.index)))
        .filter(|m| f.is_empty() || name(m.index).to_lowercase().contains(&f))
        .map(|m| {
            let meta = monsters::meta(m.index);
            let (img, has) = icon(assets::monster_icon(m.index));
            let sub = if meta.size_record && meta.family_of.is_none() {
                crown_line(m.index, monsters::get(s, base, m.index))
            } else {
                class_name(&meta.class).to_string()
            };
            DbRow { index: m.index as i32, name: name(m.index).into(), sub: sub.into(), icon: img, has_icon: has }
        })
        .collect();
    if sel == 0 || !rows.iter().any(|r| r.index as usize == sel) {
        sel = rows.first().map_or(0, |r| r.index as usize);
        view(|v| v.db_monster = sel);
    }
    api.set_db_row(rows.iter().position(|r| r.index as usize == sel).unwrap_or(0) as i32);
    api.set_db_monsters(model(rows));
    api.set_db_monster(if sel == 0 { DbMonster { index: -1, ..Default::default() } } else { monster(st, sel) });
}

fn monster(st: &State, i: usize) -> DbMonster {
    let s = st.save();
    let c = Char::new(s, st.slot);
    let t = tables();
    let meta = monsters::meta(i);
    let r = monsters::get(s, st.base(), i);
    let crown = monsters::crowns(i, r);
    let name = |i: usize| assets::monster_name(i).unwrap_or(&t.monsters[i - 1].name);
    let (img, has) = icon(assets::monster_icon(i));
    let sub = [class_name(&meta.class).to_string(), format!("#{i}")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
    let mut d = DbMonster {
        index: i as i32,
        name: name(i).into(),
        sub: sub.into(),
        icon: img,
        has_icon: has,
        has_size: meta.size_record,
        fixed: meta.fixed_size,
        min: r.min as i32,
        max: r.max as i32,
        mini_le: meta.mini_le as i32,
        silver_ge: meta.silver_ge as i32,
        gold_ge: meta.gold_ge as i32,
        mini: crown.mini,
        crown: crown.large as i32,
        ..Default::default()
    };
    carves(st, i, &mut d);
    if let Some(h) = meta.family_of {
        d.size_note = trf("Its size record is kept by {}.", &[&name(h)]).into();
        return d;
    }
    if !meta.size_record {
        d.size_note = tr("The game keeps no size record for it.").into();
        return d;
    }
    if meta.fixed_size {
        d.size_note = tr("Always the same size: any record shows the gold crown.").into();
        return d;
    }
    let range = monsters::quest_range(i);
    let (a, b) = range.unwrap_or((meta.mini_le, meta.gold_ge));
    let lo = [a, meta.mini_le, if r.min > 0 { r.min } else { a }].into_iter().min().unwrap_or(a);
    let hi = [b, meta.gold_ge, r.max].into_iter().max().unwrap_or(b);
    d.lo = lo.saturating_sub(2) as i32;
    d.hi = (hi + 2) as i32;
    let mut note = vec![if r.max > 0 { trf("Your record: {}–{} %", &[&r.min, &r.max]) } else { tr("No record yet").into() }];
    if let Some((a, b)) = range {
        note.push(trf("its quests give {}–{} %", &[&a, &b]));
    }
    if let Some(cm) = meta.base_cm {
        note.push(trf("100 % is {} cm", &[&num(cm.round() as i64)]));
    }
    d.size_note = note.join(" · ").into();
    let all = monsters::crown_odds(i);
    let (g, gm) = odds(&c, &all, |o| o.gold, tr("No quest gives it a gold size."));
    let (m, mm) = odds(&c, &all, |o| o.mini, tr("No quest gives it a mini size."));
    d.gold_odds = model(g);
    d.gold_more = gm.into();
    d.mini_odds = model(m);
    d.mini_more = mm.into();
    d
}

/// The carve tables of monster `i` in the rank picked (the highest it has by default),
/// each item with what the item box holds of it.
fn carves(st: &State, i: usize, d: &mut DbMonster) {
    let t = tables();
    let ranks: Vec<(&str, &str)> = [("low", tr("Low rank")), ("high", tr("High rank")), ("g", tr("G rank"))]
        .into_iter()
        .filter(|(r, _)| t.carves.iter().any(|c| c.monster == i && c.rank == *r))
        .collect();
    let Some(last) = ranks.len().checked_sub(1) else { return };
    let want = view(|v| v.db_rank.clone());
    let k = ranks.iter().position(|(r, _)| *r == want).unwrap_or(last);
    let rank = ranks[k].0;
    let have = box_counts(st);
    let rows: Vec<_> = t.carves.iter().filter(|c| c.monster == i && c.rank == rank).collect();
    let tables = [("body", tr("Body")), ("tail", tr("Tail")), ("shiny", tr("Shiny drop")), ("other", tr("Other"))]
        .into_iter()
        .filter_map(|(kind, title)| {
            let items: Vec<DropItem> = rows.iter().filter(|c| c.kind == kind).map(|c| drop_item(c.item, c.count, c.chance, &have)).collect();
            (!items.is_empty()).then(|| DropTable { title: title.into(), items: model(items) })
        })
        .collect();
    let sets = rows.first().map_or(1, |c| c.sets);
    d.carve_ranks = strings(ranks.iter().map(|r| r.1.to_string()));
    d.carve_ids = strings(ranks.iter().map(|r| r.0.to_string()));
    d.carve_rank = k as i32;
    d.carves = model(tables);
    d.carve_note = if sets > 1 {
        trf("{} sets of tables in this rank: the quest picks one (the stronger quests the later ones). The first is shown.", &[&sets]).into()
    } else {
        SharedString::default()
    };
}

/// The entry shown in Database tab `tab`.
fn shown(v: &mut View, tab: i32) -> &mut usize {
    match tab {
        1 => &mut v.db_quest,
        2 => &mut v.db_item,
        3 => &mut v.db_skill,
        4 => &mut v.db_equip,
        _ => &mut v.db_monster,
    }
}

pub(super) fn wire_database(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_select_db_rank, |ui, s, r: SharedString| {
        view(|v| v.db_rank = r.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_db_kind, |ui, s, k: i32| {
        view(|v| {
            v.db_equip_kind = k.max(0) as usize;
            v.db_equip = 0;
        });
        let _ = (&ui, &s);
    });
    on!(ui, st, on_filter_db, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.db_search = api.get_db_search().to_string();
            v.db_missing = api.get_db_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_select_db, |ui, s, i: i32| {
        let tab = ui.global::<Api>().get_db_tab();
        view(|v| *shown(v, tab) = i.max(0) as usize);
        let _ = &s;
    });
    // an entry of another page or tab: the Database opens on it, with no filter hiding it
    {
        let w = ui.as_weak();
        let st = st.clone();
        ui.global::<Api>().on_open_db(move |kind, i| {
            let Some(ui) = w.upgrade() else { return };
            let api = ui.global::<Api>();
            let tab = match kind.as_str() {
                "quest" => 1,
                "item" => 2,
                "skill" => 3,
                "equip" => 4,
                _ => 0,
            };
            view(|v| {
                *shown(v, tab) = i.max(0) as usize;
                if tab == 4 {
                    v.db_equip_kind = i.max(0) as usize / EQUIP_LINK;
                    v.db_equip = i.max(0) as usize % EQUIP_LINK;
                }
                v.db_search.clear();
                v.db_missing = false;
            });
            api.set_db_search("".into());
            api.set_db_missing(false);
            api.set_db_tab(tab);
            if api.get_page() == "database" {
                refresh(&ui, &st.borrow());
            } else {
                api.set_page("database".into());
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        ui.global::<Api>().on_show_in_editor(move |kind, i| {
            let Some(ui) = w.upgrade() else { return };
            let i = i.max(1) as usize;
            let (t, key) = if kind == "quest" { (Target::Quest(i), format!("quest:{i}")) } else { (Target::Monster(i, Mon::Max), format!("mon:{i}")) };
            goto_target(&ui, &st.borrow(), t, &key);
        });
    }
}
