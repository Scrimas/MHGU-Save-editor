//! The Database page: the game's own data next to the save, read-only. Its entries link
//! to the pages that edit them, and those pages link back.

use super::*;
use crate::{CrownOdds, DbItem, DbLine, DbMonster, DbQuest, DbQuestMonster, DbRow, DbSkill, DbSource, DropItem, DropTable};
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
        _ => monsters_tab(ui, st),
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
            if !f.is_empty() && !name.to_lowercase().contains(&f) && !sub.to_lowercase().contains(&f) {
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
        DbLine { name: name.into(), sub: sub.into(), value: value.into(), icon: img, has_icon: has }
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
        m
    })
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
            };
            let how = if count > 1 { format!("{} · ×{count}", src.how) } else { src.how.to_string() };
            (chance, DbSource { how: how.into(), chance: format!("{chance} %").into(), ..src })
        })
        .collect();
    all.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.r#where.cmp(&b.1.r#where)));
    let more = all.len().saturating_sub(SOURCES);
    DbItem {
        index: id as i32,
        name: assets::item_name(id).into(),
        sub: format!("#{id} · {}", in_the_box(have, id)).into(),
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
        _ => &mut v.db_monster,
    }
}

pub(super) fn wire_database(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_select_db_rank, |ui, s, r: SharedString| {
        view(|v| v.db_rank = r.to_string());
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
                _ => 0,
            };
            view(|v| {
                *shown(v, tab) = i.max(0) as usize;
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
