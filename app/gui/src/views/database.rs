//! The Database page: the game's own data next to the save, read-only. Its entries link
//! to the pages that edit them, and those pages link back.

use super::*;
use crate::{CrownOdds, DbMonster, DbRow};

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

pub(super) fn wire_database(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_filter_db, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.db_search = api.get_db_search().to_string();
            v.db_missing = api.get_db_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_select_db, |ui, s, i: i32| {
        view(|v| v.db_monster = i.max(0) as usize);
        let _ = (&ui, &s);
    });
    // another page's entry: the Database opens on it, with no filter hiding it
    {
        let w = ui.as_weak();
        ui.global::<Api>().on_open_db(move |_kind, i| {
            let Some(ui) = w.upgrade() else { return };
            let api = ui.global::<Api>();
            view(|v| {
                v.db_monster = i.max(0) as usize;
                v.db_search.clear();
                v.db_missing = false;
            });
            api.set_db_search("".into());
            api.set_db_missing(false);
            api.set_db_tab(0);
            api.set_page("database".into());
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        ui.global::<Api>().on_show_in_editor(move |_kind, i| {
            let Some(ui) = w.upgrade() else { return };
            let i = i.max(1) as usize;
            goto_target(&ui, &st.borrow(), Target::Monster(i, Mon::Max), &format!("mon:{i}"));
        });
    }
}
