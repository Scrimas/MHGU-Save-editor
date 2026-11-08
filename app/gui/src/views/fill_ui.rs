//! The Items page's Fill box dialog (issue #5): the items the box can hold, by category
//! (data/item-fill.csv), picked whole or one by one and put into the box's empty slots
//! as one Derived edit.

use super::*;
use std::collections::HashSet;

/// The dialog's tabs and their categories (items::FILL keys), in order.
const TABS: [&[&str]; 5] = [
    &["low", "high", "g", "hyper", "deviant"],
    &["ore", "bone", "plant", "bug", "fish"],
    &["heal", "tool", "food", "ammo", "coating", "extract"],
    &["deco1", "deco2", "deco3"],
    &["scrap", "ticket", "trade", "other"],
];

fn tab_name(t: usize) -> &'static str {
    [tr("Monster materials"), tr("Gathered"), tr("Hunting supplies"), tr("Decorations"), tr("Other")][t]
}

/// A category's name and what it holds.
fn cat_text(k: &str) -> (&'static str, &'static str) {
    match k {
        "low" => (tr("Low rank"), tr("Village ★1–6, Hub ★1–3")),
        "high" => (tr("High rank"), tr("Village ★7–10, Hub ★4–7")),
        "g" => (tr("G rank"), tr("Hub G★1–4")),
        "hyper" => (tr("Hyper materials"), tr("From Hyper monsters")),
        "deviant" => (tr("Deviant materials"), tr("From the 18 Deviants")),
        "ore" => (tr("Ore and crystals"), tr("Mining points")),
        "bone" => (tr("Bones"), tr("Bone piles")),
        "plant" => (tr("Plants, mushrooms and berries"), tr("Gathering points")),
        "bug" => (tr("Bugs"), tr("Bug nets")),
        "fish" => (tr("Fish and bait"), tr("Fishing spots")),
        "heal" => (tr("Healing and buffs"), tr("Potions, drinks, pills, oils")),
        "tool" => (tr("Bombs, traps and tools"), tr("Bombs, traps, whetstones, pickaxes, nets")),
        "food" => (tr("Meat and food"), tr("Steaks and baited meats")),
        "ammo" => (tr("Ammo"), tr("Bowgun shots")),
        "coating" => (tr("Coatings"), tr("Bow coatings")),
        "extract" => (tr("Kinsect extracts"), tr("Jellies and ambrosias")),
        "deco1" => (tr("1-slot decorations"), tr("Take one decoration slot")),
        "deco2" => (tr("2-slot decorations"), tr("Take two decoration slots")),
        "deco3" => (tr("3-slot decorations"), tr("Take three decoration slots")),
        "scrap" => (tr("Palico scraps"), tr("For the Palico Smithy")),
        "ticket" => (tr("Tickets and vouchers"), tr("Quest, event and collaboration tickets")),
        "trade" => (tr("Coins, spheres and books"), tr("Armor spheres, coins, Books of Combos")),
        _ => (tr("Everything else"), tr("Fragments, garbage and one-offs")),
    }
}

/// The items of category `k`, by item ID.
fn cat_items(k: &str) -> Vec<u16> {
    tables().item_fill.iter().filter(|x| x.1 == k).map(|x| x.0).collect()
}

/// What the dialog works from: the items the box holds and its empty slots.
struct BoxNow {
    held: HashSet<u16>,
    used: usize,
}

impl BoxNow {
    fn of(st: &State) -> BoxNow {
        let v = items::all(st.save(), st.base(), Store::Box);
        BoxNow { held: v.iter().filter(|x| !x.is_empty()).map(|x| x.id).collect(), used: v.iter().filter(|x| !x.is_empty()).count() }
    }
    fn free(&self) -> usize {
        items::BOX_N - self.used
    }
    /// Items of `k` the box does not hold yet.
    fn to_add(&self, k: &str) -> Vec<u16> {
        cat_items(k).into_iter().filter(|i| !self.held.contains(i)).collect()
    }
}

/// The picks still missing from the box (an item put in since no longer counts).
fn picks(b: &BoxNow) -> HashSet<u16> {
    view(|v| v.fill_picks.iter().copied().filter(|i| !b.held.contains(i)).collect())
}

fn count() -> u8 {
    view(|v| if v.fill_count == 0 { items::MAX_COUNT } else { v.fill_count })
}

fn tab_keys() -> &'static [&'static str] {
    TABS[view(|v| v.fill_tab).min(TABS.len() - 1)]
}

/// The category whose items the expanded view shows: the one picked, or the tab's first.
fn shown_cat() -> &'static str {
    let keys = tab_keys();
    view(|v| keys.iter().copied().find(|k| *k == v.fill_cat).unwrap_or(keys[0]))
}

/// The dialog's models.
pub(super) fn fill_refresh(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    if st.doc.is_none() || !api.get_fill_open() {
        return;
    }
    let b = BoxNow::of(st);
    let p = picks(&b);
    let left = b.free().saturating_sub(p.len());
    let n = count();
    let tabs: Vec<FillTab> = TABS
        .iter()
        .enumerate()
        .map(|(t, keys)| {
            let add: usize = keys.iter().map(|k| b.to_add(k).len()).sum();
            let on: usize = keys.iter().map(|k| b.to_add(k).iter().filter(|i| p.contains(i)).count()).sum();
            FillTab { name: tab_name(t).into(), badge: if on > 0 { format!("+{}", num(on as i64)) } else { num(add as i64) }.into(), picked: on > 0 }
        })
        .collect();
    let item = |id: u16| {
        let (have, on) = (b.held.contains(&id), p.contains(&id));
        FillItem { id: id as i32, name: assets::item_name(id).into(), have, on, enabled: !have && (on || left > 0) }
    };
    let compact = api.get_fill_compact();
    let open = view(|v| v.fill_open.clone());
    let shown = shown_cat();
    let cats: Vec<FillCat> = tab_keys()
        .iter()
        .map(|&k| {
            let (name, hint) = cat_text(k);
            let all = cat_items(k);
            let add = b.to_add(k);
            let (a, on) = (add.len(), add.iter().filter(|i| p.contains(i)).count());
            let have = all.len() - a;
            let (status, tone) = if a == 0 {
                (tr("✓ all in the box").to_string(), 1)
            } else if on == a {
                (trn("+{} slot", "+{} slots", a as i64, &[&num(a as i64)]), 0)
            } else if on > 0 {
                (trf("{} of {} picked", &[&num(on as i64), &num(a as i64)]), 0)
            } else if a > left {
                if left > 0 { (trf("needs {}, {} left", &[&num(a as i64), &num(left as i64)]), 2) } else { (trf("needs {}, box full", &[&num(a as i64)]), 2) }
            } else {
                (trn("+{} slot", "+{} slots", a as i64, &[&num(a as i64)]), 0)
            };
            let is_open = compact && open.contains(k);
            FillCat {
                key: k.into(),
                name: name.into(),
                sub: format!(
                    "{} · {} · {hint}",
                    trn("{} item", "{} items", all.len() as i64, &[&num(all.len() as i64)]),
                    trf("{} in the box", &[&num(have as i64)])
                )
                .into(),
                status: status.into(),
                tone,
                done: a == 0,
                checked: a == 0 || on == a,
                partial: on > 0 && on < a,
                enabled: a > 0 && (on == a || a - on <= left),
                selected: k == shown,
                open: is_open,
                items: model(if is_open { all.iter().map(|&i| item(i)).collect() } else { vec![] }),
            }
        })
        .collect();
    let all = cat_items(shown);
    let add = b.to_add(shown).len();
    let (name, hint) = cat_text(shown);
    api.set_fill_tabs(model(tabs));
    api.set_fill_tab(view(|v| v.fill_tab) as i32);
    api.set_fill_cats(model(cats));
    api.set_fill_cat_name(name.into());
    api.set_fill_cat_sub(
        format!("{} · {} · {hint}", trf("{} to add", &[&num(add as i64)]), trf("{} already in the box", &[&num((all.len() - add) as i64)])).into(),
    );
    api.set_fill_items(model(if compact { vec![] } else { all.iter().map(|&i| item(i)).collect() }));
    api.set_fill_used(b.used as i32);
    api.set_fill_picked(p.len() as i32);
    api.set_fill_count(n as i32);
    api.set_fill_legend(strings([
        trf("{} used", &[&num(b.used as i64)]),
        trf("{} picked", &[&num(p.len() as i64)]),
        trf("{} stay free", &[&num(left as i64)]),
        trf("of {} slots", &[&num(items::BOX_N as i64)]),
    ]));
    api.set_fill_action(
        if p.is_empty() { tr("Fill").to_string() } else { trn("Fill {} slot ×{}", "Fill {} slots ×{}", p.len() as i64, &[&num(p.len() as i64), &n]) }.into(),
    );
}

pub(super) fn wire_fill(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    api.set_fill_compact(settings::get().fill_compact);
    on!(ui, st, on_fill_show, |ui, s| {
        if s.doc.is_none() {
            return;
        }
        view(|v| v.fill_picks.clear());
        ui.global::<Api>().set_fill_open(true);
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_set_compact, |ui, s, on: bool| {
        settings::update(|x| x.fill_compact = on);
        // the category in view stays in view: its items open in the compact list
        view(|v| {
            if on {
                v.fill_open.insert(shown_cat_of(v).into());
            }
        });
        if !on {
            let keys = tab_keys();
            if let Some(k) = view(|v| keys.iter().rev().find(|k| v.fill_open.contains(**k)).copied()) {
                view(|v| v.fill_cat = k.into());
            }
        }
        ui.global::<Api>().set_fill_compact(on);
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_select_tab, |ui, s, t: i32| {
        view(|v| v.fill_tab = (t.max(0) as usize).min(TABS.len() - 1));
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_select_cat, |ui, s, k: SharedString| {
        view(|v| v.fill_cat = k.to_string());
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_open_cat, |ui, s, k: SharedString| {
        view(|v| {
            if !v.fill_open.remove(k.as_str()) {
                v.fill_open.insert(k.to_string());
            }
        });
        fill_refresh(&ui, &s);
    });
    // a whole category: all its missing items when they fit, none once all are picked
    on!(ui, st, on_fill_toggle_cat, |ui, s, k: SharedString| {
        let b = BoxNow::of(&s);
        let p = picks(&b);
        let add = b.to_add(&k);
        let on = add.iter().filter(|i| p.contains(i)).count();
        view(|v| {
            if !add.is_empty() && on == add.len() {
                add.iter().for_each(|i| {
                    v.fill_picks.remove(i);
                });
            } else if add.len() - on <= b.free().saturating_sub(p.len()) {
                v.fill_picks.extend(add.iter().copied());
            }
        });
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_toggle_item, |ui, s, id: i32| {
        let b = BoxNow::of(&s);
        let p = picks(&b);
        let id = id as u16;
        view(|v| {
            if !v.fill_picks.remove(&id) && !b.held.contains(&id) && p.len() < b.free() {
                v.fill_picks.insert(id);
            }
        });
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_set_count, |ui, s, n: i32| {
        view(|v| v.fill_count = n.clamp(1, items::MAX_COUNT as i32) as u8);
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_clear, |ui, s| {
        view(|v| v.fill_picks.clear());
        fill_refresh(&ui, &s);
    });
    on!(ui, st, on_fill_apply, |ui, s| {
        let b = BoxNow::of(&s);
        let mut ids: Vec<u16> = picks(&b).into_iter().collect();
        if ids.is_empty() || refused(&ui, Conf::Derived) {
            return;
        }
        ids.sort_unstable();
        let n = count();
        let title = trn("Item box: {} item added ×{}", "Item box: {} items added ×{}", ids.len() as i64, &[&num(ids.len() as i64), &n]);
        let mut all: Vec<Target> = (0..items::BOX_N).map(|i| Target::Item(Store::Box, i)).collect();
        all.push(Target::Obtained);
        stage_many(&ui, &mut s, title, tr("Item box"), Conf::Derived, all, |sv, base| {
            items::fill(sv, base, &ids, n);
        });
        view(|v| v.fill_picks.clear());
        ui.global::<Api>().set_fill_open(false);
    });
}

fn shown_cat_of(v: &View) -> &'static str {
    let keys = TABS[v.fill_tab.min(TABS.len() - 1)];
    keys.iter().copied().find(|k| *k == v.fill_cat).unwrap_or(keys[0])
}
