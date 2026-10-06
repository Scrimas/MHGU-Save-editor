//! The Items page: its models and the callbacks that edit it.

use super::*;

pub(super) fn store_of(ui: &AppWindow) -> Store {
    if ui.global::<Api>().get_item_store() == 1 { Store::Pouch } else { Store::Box }
}

pub(super) fn items_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let store = store_of(ui);
    let all = items::all(s, base, store);
    let f = view(|v| v.item_filter.to_lowercase());
    let off = if store == Store::Box { items::BOX } else { items::POUCH };
    let rows: Vec<ItemSlot> = all
        .iter()
        .enumerate()
        .filter(|(_, x)| f.is_empty() || (!x.is_empty() && assets::item_name(x.id).to_lowercase().contains(&f)))
        .map(|(i, x)| {
            let (img, has) = if x.is_empty() { (Image::default(), false) } else { icon(assets::item_icon(x.id)) };
            let bit = 19 * i;
            let changed = st.changed(base + off + bit / 8, 3) && !st.was(Target::Item(store, i)).is_empty();
            ItemSlot {
                slot: i as i32,
                id: x.id as i32,
                name: if x.is_empty() { "".into() } else { assets::item_name(x.id).into() },
                count: x.count as i32,
                max: if x.is_empty() { 0 } else { assets::item_max(x.id, store) as i32 },
                icon: img,
                has_icon: has,
                changed,
                was: if changed { st.was(Target::Item(store, i)) } else { String::new() }.into(),
            }
        })
        .collect();
    api.set_item_used(all.iter().filter(|x| !x.is_empty()).count() as i32);
    api.set_item_total(all.len() as i32);
    api.set_item_free(all.iter().position(|x| x.is_empty()).map_or(-1, |i| i as i32));
    api.set_item_slots(model(rows));
    // runs of empty loadouts collapse into one row (C8)
    let mut lrows: Vec<LoadoutRow> = vec![];
    let mut run: Option<(usize, usize)> = None;
    let flush = |run: &mut Option<(usize, usize)>, lrows: &mut Vec<LoadoutRow>| {
        if let Some((a, b)) = run.take() {
            let name = if a == b { trf("Loadout {} · empty", &[&(a + 1)]) } else { trf("Loadouts {}–{} · empty", &[&(a + 1), &(b + 1)]) };
            lrows.push(LoadoutRow { index: -1, first: a as i32, last: b as i32, name: name.into(), summary: "".into(), used: false });
        }
    };
    for k in 0..items::LOADOUT_N {
        let l = items::loadout(s, base, k);
        let used: Vec<String> = l.items.iter().filter(|x| x.0 != 0).map(|x| format!("{} ×{}", assets::item_name(x.0), x.1)).collect();
        if used.is_empty() {
            run = Some(run.map_or((k, k), |(a, _)| (a, k)));
            continue;
        }
        flush(&mut run, &mut lrows);
        lrows.push(LoadoutRow { index: k as i32, first: k as i32, last: k as i32, name: l.name.into(), used: true, summary: used.join(", ").into() });
    }
    flush(&mut run, &mut lrows);
    api.set_loadouts(model(lrows));
    // the loadout being edited: its 32 pouch positions
    let sel = view(|v| v.loadout_sel);
    api.set_loadout_sel(sel);
    if (0..items::LOADOUT_N as i32).contains(&sel) {
        let k = sel as usize;
        let l = items::loadout(s, base, k);
        let lo = base + items::LOADOUTS + items::LOADOUT_SZ * k;
        api.set_loadout_name(l.name.clone().into());
        api.set_loadout_was(st.was(Target::Loadout(k)).into());
        let slots: Vec<ItemSlot> = l
            .items
            .iter()
            .enumerate()
            .map(|(j, &(id, n))| {
                let (img, has) = if id == 0 { (Image::default(), false) } else { icon(assets::item_icon(id)) };
                let o = lo + items::LOADOUT_NAME + 4 * j;
                let changed = st.changed(o, 4);
                let was = if changed {
                    let (oid, on) = items::loadout(st.orig(), base, k).items[j];
                    if oid == 0 { tr("Empty").to_string() } else { format!("{} ×{on}", assets::item_name(oid)) }
                } else {
                    String::new()
                };
                ItemSlot {
                    slot: j as i32,
                    id: id as i32,
                    name: if id == 0 { "".into() } else { assets::item_name(id).into() },
                    count: n as i32,
                    max: if id == 0 { 0 } else { assets::item_max(id, Store::Pouch) as i32 },
                    icon: img,
                    has_icon: has,
                    changed,
                    was: was.into(),
                }
            })
            .collect();
        api.set_loadout_slots(model(slots));
    }
    picker(ui);
}

pub(super) fn picker(ui: &AppWindow) {
    let f = view(|v| v.picker_filter.to_lowercase());
    let n = assets::names();
    // loadouts fill the pouch: its carry limits apply
    let store = if ui.global::<Api>().get_item_store() == 2 { Store::Pouch } else { store_of(ui) };
    let mut v: Vec<PickItem> = vec![];
    let count = if n.items.is_empty() { 1900 } else { n.items.len() };
    for id in 1..count.min(items::MAX_ID as usize + 1) {
        let name = assets::item_name(id as u16);
        if n.items.get(id).is_some_and(|x| x.is_empty() || x == "(None)" || x.starts_with('-')) {
            continue;
        }
        if !f.is_empty() && !name.to_lowercase().contains(&f) && id.to_string() != f {
            continue;
        }
        let (img, has) = icon(assets::item_icon(id as u16));
        let max = assets::item_max(id as u16, store) as i32;
        v.push(PickItem { id: id as i32, name: name.into(), icon: img, has_icon: has, sub: SharedString::default(), max });
        if v.len() >= 400 {
            break;
        }
    }
    ui.global::<Api>().set_pick_items(model(v));
}

pub(super) fn wire_items(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    // items
    on!(ui, st, on_filter_items, |ui, s, t: SharedString| {
        view(|v| v.item_filter = t.to_string());
        let _ = (&ui, &s);
    });
    {
        let w = ui.as_weak();
        api.on_search_picker(move |t| {
            view(|v| v.picker_filter = t.to_string());
            if let Some(ui) = w.upgrade() {
                picker(&ui);
            }
        });
    }
    on!(ui, st, on_set_item, |ui, s, slot: i32, id: i32, count: i32| {
        let store = store_of(&ui);
        let t = Target::Item(store, slot as usize);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            let max = assets::item_max(id as u16, store) as i32;
            items::set(sv, base, store, slot as usize, Stack { id: id as u16, count: count.clamp(0, max) as u8 });
            vec![]
        });
    });
    on!(ui, st, on_select_loadout, |ui, s, k: i32| {
        view(|v| v.loadout_sel = k);
        let _ = (&ui, &s);
    });
    // a loadout is one value: its edits merge, its name follows the game's "Set NN"
    on!(ui, st, on_set_loadout_item, |ui, s, j: i32, id: i32, count: i32| {
        let k = view(|v| v.loadout_sel);
        if !(0..items::LOADOUT_N as i32).contains(&k) || !(0..items::LOADOUT_ITEMS as i32).contains(&j) {
            return;
        }
        let (k, j) = (k as usize, j as usize);
        let t = Target::Loadout(k);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            let mut l = items::loadout(sv, base, k);
            let max = assets::item_max(id as u16, Store::Pouch) as i32;
            l.items[j] = if id <= 0 || count <= 0 { (0, 0) } else { (id as u16, count.clamp(1, max) as u16) };
            if l.name.is_empty() && l.items.iter().any(|x| x.0 != 0) {
                l.name = format!("Set {:02}", k + 1);
            }
            items::set_loadout(sv, base, k, &l);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_loadout_name, |ui, s, name: SharedString| {
        let k = view(|v| v.loadout_sel);
        if !(0..items::LOADOUT_N as i32).contains(&k) {
            return;
        }
        let k = k as usize;
        let t = Target::Loadout(k);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            let mut l = items::loadout(sv, base, k);
            l.name = name.trim().to_string();
            items::set_loadout(sv, base, k, &l);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_item_bulk, |ui, s, what: SharedString| {
        let id = bulk_id(&ui, &format!("items:{what}"));
        apply_plan(&ui, &mut s, &id);
    });
}
