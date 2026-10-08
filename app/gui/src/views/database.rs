//! The Database page: the game's own data next to the save, read-only. Its entries link
//! to the pages that edit them, and those pages link back.

use super::*;
use crate::{CrownOdds, DbMonster, DbQuest, DbQuestMonster, DbRow, DropItem, DropTable};
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
        _ => monsters_tab(ui, st),
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
    let n = have.get(&item).copied().unwrap_or(0);
    let name = assets::item_name(item);
    DropItem {
        name: if count > 1 { format!("{name} ×{count}") } else { name }.into(),
        icon: img,
        has_icon: has,
        chance: format!("{chance} %").into(),
        have: if n == 0 { tr("none in the box").to_string() } else { trf("{} in the box", &[&num(n)]) }.into(),
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
        let quest = ui.global::<Api>().get_db_tab() == 1;
        view(|v| *if quest { &mut v.db_quest } else { &mut v.db_monster } = i.max(0) as usize);
        let _ = &s;
    });
    // an entry of another page or tab: the Database opens on it, with no filter hiding it
    {
        let w = ui.as_weak();
        let st = st.clone();
        ui.global::<Api>().on_open_db(move |kind, i| {
            let Some(ui) = w.upgrade() else { return };
            let api = ui.global::<Api>();
            let quest = kind == "quest";
            view(|v| {
                *if quest { &mut v.db_quest } else { &mut v.db_monster } = i.max(0) as usize;
                v.db_search.clear();
                v.db_missing = false;
            });
            api.set_db_search("".into());
            api.set_db_missing(false);
            api.set_db_tab(if quest { 1 } else { 0 });
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
