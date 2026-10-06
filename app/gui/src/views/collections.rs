//! The Collections page: its models and the callbacks that edit it.

use super::*;

/// Base name of a Hunter Art level ("Ground Slash III" -> "Ground Slash").
pub(super) fn art_base(name: &str) -> &str {
    for suffix in [" III", " II", " I"] {
        if let Some(b) = name.strip_suffix(suffix) {
            return b;
        }
    }
    name
}

pub(super) fn collections_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let c = Char::new(st.save(), st.slot);
    let tab = view(|v| v.collection);
    let (q, missing) = view(|v| (v.check_search.to_lowercase(), v.check_missing));
    let t = tables();
    let keep = |name: &str, on: bool| !(missing && on) && (q.is_empty() || name.to_lowercase().contains(&q));
    let changed_of = |tg: Target| !st.was(tg).is_empty();
    // counts in each tab (H5.1)
    let arts_on = t.arts.iter().filter(|a| c.art(a.0)).count();
    let dishes_on = (0..99).filter(|&b| c.dish(b)).count();
    let ingr_on = (0..45).filter(|&b| c.ingredient(b)).count();
    let awards_on = t.awards.iter().filter(|a| c.award(a.0)).count();
    let dev_levels = |d: usize| {
        let (q0, n) = Char::deviant_levels(d);
        ((0..n).filter(|&k| c.quest(QuestBit::Cleared, q0 + k)).count(), n)
    };
    let devs_done = (0..DEVIANTS.len()).filter(|&d| dev_levels(d).0 == dev_levels(d).1).count();
    api.set_collection_tabs(strings([
        trf("Hunter Arts {} / {}", &[&arts_on, &t.arts.len()]),
        trf("Canteen dishes {} / {}", &[&dishes_on, &99]),
        trf("Canteen ingredients {} / {}", &[&ingr_on, &45]),
        trf("Awards {} / {}", &[&awards_on, &t.awards.len()]),
        trf("Deviants {} / {}", &[&devs_done, &DEVIANTS.len()]),
    ]));
    let rows: Vec<CheckRow> = match tab {
        1 | 2 => t
            .canteen
            .iter()
            .filter(|(k, ..)| k == if tab == 1 { "dish" } else { "ingredient" })
            .map(|(_, b, n, _)| {
                let on = if tab == 1 { c.dish(*b) } else { c.ingredient(*b) };
                let tg = if tab == 1 { Target::Dish(*b) } else { Target::Ingredient(*b) };
                CheckRow { key: *b as i32, name: n.clone().into(), on, changed: changed_of(tg), ..Default::default() }
            })
            .filter(|r| keep(&r.name, r.on))
            .collect(),
        3 => t
            .awards
            .iter()
            .filter(|(b, _, n)| keep(n, c.award(*b)))
            .map(|(b, _, n)| {
                let (img, has) = icon(assets::award_icon(*b));
                CheckRow { key: *b as i32, name: n.clone().into(), on: c.award(*b), changed: changed_of(Target::Award(*b)), icon: img, has_icon: has }
            })
            .collect(),
        _ => vec![],
    };
    // Hunter Arts grouped by art: I, II and III on one row (H5.3)
    let mut arts: Vec<(String, Vec<u32>)> = vec![];
    for (id, n) in &t.arts {
        let b = art_base(n);
        match arts.last_mut() {
            Some(last) if last.0 == b && n != b => last.1.push(*id),
            _ => arts.push((b.to_string(), vec![*id])),
        }
    }
    let art_rows: Vec<ArtRow> = arts
        .into_iter()
        .filter(|(n, ids)| keep(n, ids.iter().all(|&i| c.art(i))))
        .map(|(n, ids)| ArtRow {
            name: n.into(),
            on: model(ids.iter().map(|&i| c.art(i)).collect()),
            changed: ids.iter().any(|&i| changed_of(Target::Art(i))),
            ids: model(ids.iter().map(|&i| i as i32).collect()),
        })
        .collect();
    let summary = match tab {
        0 => trf("Hunter Arts: {} / {}", &[&arts_on, &t.arts.len()]),
        1 => trf("Canteen dishes: {} / {}", &[&dishes_on, &99]),
        2 => trf("Canteen ingredients: {} / {}", &[&ingr_on, &45]),
        3 => trf("Awards: {} / {}", &[&awards_on, &t.awards.len()]),
        _ => trf("Deviants with every level cleared: {} / {}", &[&devs_done, &DEVIANTS.len()]),
    };
    api.set_checks_summary(trf("{} · confirmed in game except where marked", &[&summary]).into());
    api.set_checks(model(rows));
    api.set_arts(model(art_rows));
    let devs: Vec<DeviantRow> = DEVIANTS
        .iter()
        .enumerate()
        .filter(|&(d, name)| keep(name, dev_levels(d).0 == dev_levels(d).1))
        .map(|(d, name)| {
            let (lv, n) = dev_levels(d);
            let mi = t.monsters.iter().find(|m| m.name == *name).map(|m| m.index);
            let (img, has) = icon(mi.and_then(assets::monster_icon));
            // G-rank levels cleared by an edit stay off the board until the gate opens
            let gate = if lv > Char::deviant_g1(d) && !c.deviant_gate_open(d) {
                if d == 17 {
                    tr("G-rank levels appear in game only once the game releases them (event flag 1226).").to_string()
                } else {
                    let ids: Vec<String> = mhgu_save::progress::DEVIANT_GATE[d].iter().map(|i| i.to_string()).collect();
                    let base = name.split_once(' ').map_or(*name, |x| x.1);
                    trf("G-rank levels appear in game once a G-rank {} quest is cleared ({}).", &[&base, &fmt::list(&ids, 3)])
                }
            } else {
                String::new()
            };
            DeviantRow {
                index: d as i32,
                name: (*name).into(),
                permits: c.permits(d) as i32,
                levels: lv as i32,
                total: n as i32,
                icon: img,
                has_icon: has,
                was_permits: st.was(Target::Permits(d)).into(),
                was_levels: st.was(Target::Levels(d)).into(),
                gate: gate.into(),
            }
        })
        .collect();
    api.set_deviants(model(devs));
}

pub(super) fn wire_collections(ui: &AppWindow, st: &Shared) {
    // collections
    on!(ui, st, on_select_collection, |ui, s, i: i32| {
        view(|v| v.collection = i as usize);
        ui.global::<Api>().set_collection_tab(i);
        let _ = &s;
    });
    on!(ui, st, on_filter_checks, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.check_search = api.get_check_search().to_string();
            v.check_missing = api.get_check_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_check, |ui, s, key: i32, on: bool| {
        let tab = view(|v| v.collection);
        let slot = s.slot;
        let t = match tab {
            0 => Target::Art(key as u32),
            1 => Target::Dish(key as usize),
            2 => Target::Ingredient(key as usize),
            _ => Target::Award(key as usize),
        };
        let title = t.label(s.save(), slot);
        let mut e = Edit::one(t, title, Conf::Confirmed);
        if tab == 3 {
            e.note = tr("Written to both of the game's award lists").into();
        }
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            match tab {
                0 => c.set_art(key as u32, on),
                1 => c.set_dish(key as usize, on),
                2 => c.set_ingredient(key as usize, on),
                _ => c.set_award(key as usize, on),
            }
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_deviant, |ui, s, d: i32, what: SharedString, v: i32| {
        let d = d as usize;
        let slot = s.slot;
        let t = if what == "permits" { Target::Permits(d) } else { Target::Levels(d) };
        let title = t.label(s.save(), slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, _| {
            let mut c = Char::new(sv, slot);
            if what == "permits" {
                c.set_permits(d, v.clamp(0, 99) as u8);
            } else {
                let (q0, n) = Char::deviant_levels(d);
                for k in 0..n {
                    let on = (k as i32) < v;
                    c.set_quest(QuestBit::Cleared, q0 + k, on);
                    if on {
                        c.set_quest(QuestBit::Seen, q0 + k, true);
                    }
                }
            }
            vec![]
        });
        let _ = &ui;
    });
}
