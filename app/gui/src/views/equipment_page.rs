//! The Equipment page: its models and the callbacks that edit it.

use super::*;

pub(super) fn owner_of(ui: &AppWindow) -> Owner {
    if ui.global::<Api>().get_equip_owner() == 1 { Owner::Palico } else { Owner::Hunter }
}

/// "your current gear, My Set 2 “Fire DB”" for the references to hunter box entry `i`.
pub(super) fn uses_label(s: &mhgu_save::Save, base: usize, i: usize) -> String {
    equipment::uses(s, base, i)
        .into_iter()
        .map(|u| match u {
            equipment::Use::Worn => "your current gear".to_string(),
            equipment::Use::MySet(n, name) => format!("My Set {n} “{name}”"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The pieces of the selected category matching the picker search.
/// The equipment picker's list: pieces of the category picked, or with `pick-mode`
/// "transmog" the looks the selected armor piece can take.
pub(super) fn equip_picker(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let owner = owner_of(ui);
    let transmog = api.get_pick_mode() == "transmog";
    let sel = view(|v| v.equip_sel);
    let src = (st.doc.is_some() && sel >= 0).then(|| equipment::get(st.save(), st.base(), owner, sel as usize));
    let k = match &src {
        Some(e) if transmog => e.kind(),
        _ => match equip_categories(owner).get(api.get_equip_category().max(0) as usize) {
            Some(&k) => k,
            None => return api.set_pick_equip(model(vec![])),
        },
    };
    let gender = if st.doc.is_some() { character::get(st.save(), st.base()).gender } else { 0 };
    let own = src.as_ref().and_then(|e| piece(owner, k, e.id()));
    let f = view(|v| v.equip_search.to_lowercase());
    let v: Vec<PickItem> = pieces(owner, k)
        .iter()
        .filter(|p| p.is_real())
        .filter(|p| !transmog || own.is_some_and(|o| transmog_fits(o, p, gender) && o.id != p.id))
        .filter(|p| f.is_empty() || p.id.to_string() == f || [&p.name].into_iter().chain(&p.names).any(|n| n.to_lowercase().contains(&f)))
        .map(|p| {
            // Palico gear has no rarity in the pack
            let mut sub: Vec<String> = if p.rarity > 0 { vec![format!("Rare {}", p.rarity)] } else { vec![] };
            match (p.blade, p.gunner) {
                (Some(1), Some(0)) => sub.push("Blademaster".into()),
                (Some(0), Some(1)) => sub.push("Gunner".into()),
                _ => {}
            }
            match k {
                Kind::Talisman => sub.push(tier_name(equipment::talisman_tier(p.id as u16)).into()),
                // upgrade names past the max level and the limit break
                Kind::Weapon(_) if p.names.len() == 3 && p.names[1] != p.name => sub.push(format!("then {}", p.names[1..].join(", "))),
                _ => {}
            }
            match (p.male, p.female) {
                (Some(1), Some(0)) => sub.push("type 1 (male) only".into()),
                (Some(0), Some(1)) => sub.push("type 2 (female) only".into()),
                _ => {}
            }
            let (img, has) = icon(assets::equip_icon(k.code(), p.rarity));
            PickItem { id: p.id as i32, name: p.name.clone().into(), icon: img, has_icon: has, sub: sub.join(" · ").into(), max: 0 }
        })
        .collect();
    api.set_pick_equip(model(v));
}

/// Decorations that fit the free slots of the selected hunter box entry, by name.
pub(super) fn deco_picker(ui: &AppWindow, st: &State, search: &str) {
    let api = ui.global::<Api>();
    let sel = view(|v| v.equip_sel);
    let free = match (st.doc.is_some(), sel) {
        (true, 0..) => {
            let e = equipment::get(st.save(), st.base(), Owner::Hunter, sel as usize);
            deco_slots(Owner::Hunter, &e).unwrap_or(0).saturating_sub(deco_used(&e))
        }
        _ => 0,
    };
    let f = search.to_lowercase();
    let v: Vec<PickItem> = assets::names()
        .decos
        .iter()
        .filter(|d| d[1] as u8 <= free)
        .map(|d| (d[0], d[1], assets::item_name(d[0])))
        .filter(|(_, _, n)| f.is_empty() || n.to_lowercase().contains(&f))
        .map(|(id, size, name)| {
            let (img, has) = icon(assets::item_icon(id));
            PickItem { id: id as i32, name: name.into(), icon: img, has_icon: has, sub: count(size as usize, "slot", "slots").into(), max: size as i32 }
        })
        .collect();
    api.set_pick_decos(model(v));
}

pub(super) fn equipment_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let orig = st.orig();
    let base = st.base();
    let owner = owner_of(ui);
    let off = if owner == Owner::Hunter { equipment::BOX } else { equipment::PALICO_BOX };
    let (f, q) = view(|v| (v.equip_filter.clone(), v.equip_list_search.to_lowercase()));
    let mut used = 0;
    let rows: Vec<EquipRow> = (0..owner.len())
        .filter_map(|i| {
            let e = equipment::get(s, base, owner, i);
            let k = e.kind();
            used += !e.is_empty() as usize;
            if !equip_keep(&f, k) {
                return None;
            }
            let (title, sub) = match k {
                Kind::Empty => ("Empty".to_string(), String::new()),
                Kind::Talisman => (talisman_skills(&e), format!("{} · Talisman", equip_name(owner, &e))),
                // Palico gear has no level or decorations; its type only when the name doesn't say it (K3)
                _ if owner == Owner::Palico => {
                    let (name, kind) = (equip_name(owner, &e), kind_label(k));
                    let sub = if name.starts_with(&kind) { String::new() } else { kind };
                    (name, sub)
                }
                _ => {
                    let decos = e.decos().iter().filter(|&&d| d != 0).count();
                    let mut sub = vec![kind_label(k), format!("Lv {}", e.level())];
                    if decos > 0 {
                        sub.push(count(decos, "decoration", "decorations"));
                    }
                    if e.transmog() != 0 {
                        sub.push("transmog".into());
                    }
                    (equip_name(owner, &e), sub.join(" · "))
                }
            };
            // search matches the name, the weapon type and talisman skills (08.2)
            if !q.is_empty() && !title.to_lowercase().contains(&q) && !sub.to_lowercase().contains(&q) && (i + 1).to_string() != q {
                return None;
            }
            let rarity = piece_rarity(owner, &e);
            let (img, has) = icon(assets::equip_icon(k.code(), rarity));
            let changed = st.changed(base + off + equipment::ENTRY * i, equipment::ENTRY);
            let status = if !changed {
                ""
            } else {
                let was = equipment::get(orig, base, owner, i);
                if was.is_empty() && !e.is_empty() {
                    "New · in Review"
                } else if e.is_empty() {
                    "Removed · in Review"
                } else {
                    "Changed · in Review"
                }
            };
            Some(EquipRow { slot: i as i32, kind_code: k.code() as i32, title: title.into(), sub: sub.into(), icon: img, has_icon: has, changed, status: status.into() })
        })
        .collect();
    api.set_equip_summary(
        if owner == Owner::Hunter {
            format!("Hunter box: {} / {} slots used · confirmed in game except where marked", num(used as i64), num(owner.len() as i64))
        } else {
            format!("Palico box: {} / {} slots used · gear names from the game's tables (Derived)", num(used as i64), num(owner.len() as i64))
        }
        .into(),
    );
    // after selecting or adding, the row is scrolled into view (10.1)
    if let Some(slot) = view(|v| v.equip_jump.take()) {
        if let Some(row) = rows.iter().position(|r| r.slot == slot) {
            jump(ui, Some(row), "");
        }
    }
    api.set_equip_rows(model(rows));
    let sel = view(|v| v.equip_sel);
    let mut d = EquipDetail { slot: -1, category: -1, ..Default::default() };
    if sel >= 0 && (sel as usize) < owner.len() {
        let i = sel as usize;
        let e = equipment::get(s, base, owner, i);
        let k = e.kind();
        let ds = e.decos();
        let t = e.talisman();
        // the decorations in it, packed from the first field, and the slots they take
        // of the piece's (10.3)
        let decos: Vec<DecoRow> = (0..3)
            .filter(|&j| ds[j] != 0)
            .map(|j| DecoRow { index: j as i32, name: assets::item_name(ds[j]).into(), size: assets::deco_size(ds[j]).unwrap_or(1) as i32 })
            .collect();
        let slots = deco_slots(owner, &e);
        let field = tables()
            .fields
            .iter()
            .find(|fl| fl.block == "char1" && fl.rel <= off && off < fl.rel + fl.size)
            .map(|fl| fl.label.clone())
            .unwrap_or_default();
        let was_e = equipment::get(orig, base, owner, i);
        let staged = st.changed(base + off + equipment::ENTRY * i, equipment::ENTRY);
        d = EquipDetail {
            slot: sel,
            kind: kind_label(k).into(),
            kind_code: k.code() as i32,
            name: if k == Kind::Talisman { talisman_skills(&e) } else { equip_name(owner, &e) }.into(),
            id: e.id() as i32,
            level: e.level() as i32,
            was_level: if staged && was_e.kind() == k && was_e.id() == e.id() && was_e.level() != e.level() { num(was_e.level()) } else { String::new() }.into(),
            decos: model(decos),
            transmog: if e.transmog() == 0 {
                "".into()
            } else {
                piece(owner, k, e.transmog()).map(|p| p.name.clone()).unwrap_or_else(|| format!("#{}", e.transmog())).into()
            },
            transmog_id: e.transmog() as i32,
            is_talisman: t.is_some(),
            skill1: t.map(|t| t.skills[0] as i32).unwrap_or(0),
            points1: t.map(|t| t.points[0] as i32).unwrap_or(0),
            skill2: t.map(|t| t.skills[1] as i32).unwrap_or(0),
            points2: t.map(|t| t.points[1] as i32).unwrap_or(0),
            slots: t.map(|t| t.slots as i32).unwrap_or(0),
            tier: if k == Kind::Talisman { format!("{} · {}", equip_name(owner, &e), tier_name(t.unwrap().tier)) } else { String::new() }.into(),
            raw: e.raw.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ").into(),
            field: field.into(),
            uses: if owner == Owner::Hunter { uses_label(s, base, i) } else { String::new() }.into(),
            category: equip_categories(owner).iter().position(|&c| c == k).map_or(-1, |p| p as i32),
            was: if staged { equip_value(owner, &was_e) } else { String::new() }.into(),
            deco_slots: slots.map_or(-1, |n| n as i32),
            deco_used: deco_used(&e) as i32,
            // the looks this piece can take (none without the pack's armor classes)
            can_transmog: k.is_armor() && piece(owner, k, e.id()).is_some_and(|p| p.blade.is_some()),
        };
    }
    api.set_equip_detail(d);
    let cats = equip_categories(owner);
    api.set_equip_categories(strings(cats.iter().map(|&k| kind_label(k))));
    api.set_equip_free(match equipment::free_slot(s, base, owner) {
        Some(i) if !cats.is_empty() => i as i32,
        _ => -1,
    });
    let skills: Vec<String> = if assets::names().skills.is_empty() {
        (0..160).map(|i| if i == 0 { "—".to_string() } else { format!("Skill #{i}") }).collect()
    } else {
        assets::names().skills.iter().enumerate().map(|(i, n)| if i == 0 { "—".into() } else { n.clone() }).collect()
    };
    api.set_skill_names(strings(skills));
}

pub(super) fn wire_equipment(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    // equipment
    on!(ui, st, on_filter_equip, |ui, s, f: SharedString| {
        view(|v| v.equip_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_search_equip_list, |ui, s, t: SharedString| {
        view(|v| v.equip_list_search = t.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_equip, |ui, s, i: i32| {
        // the list scrolls to it when it is out of view (10.1)
        view(|v| {
            v.equip_sel = i;
            v.equip_jump = Some(i);
        });
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_equip, |ui, s, field: SharedString, v: i32| {
        let owner = owner_of(&ui);
        let i = view(|v| v.equip_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let f = field.as_str();
        let cur = equipment::get(s.save(), s.base(), owner, i);
        // a decoration goes in only where its slots are free
        if f == "deco-add" {
            let size = assets::deco_size(v as u16).unwrap_or(u8::MAX);
            let free = deco_slots(owner, &cur).unwrap_or(0).saturating_sub(deco_used(&cur));
            if size > free || !cur.decos().contains(&0) {
                return toast(&ui, format!("{} needs {} free; this piece has {free}", assets::item_name(v as u16), count(size as usize, "slot", "slots")), true);
            }
        }
        let t = Target::Equip(owner, i);
        let title = t.label(s.save(), s.slot);
        // slot sizes, transmog classes and the transmog level field are read from the
        // game's tables, not checked in game
        let derived = f == "deco-add" || (f == "transmog" && v != 0);
        let c = if derived { Conf::Derived } else { Conf::Confirmed };
        if refused(&ui, c) {
            return;
        }
        let mut e = Edit::one(t, title, c);
        // adding and removing decorations merge, so taking one back out undoes it
        e.key = format!("{}:{}", t.key(), if f.starts_with("deco") { "decos" } else { f });
        s.edit(e, |sv, base| {
            let mut e = equipment::get(sv, base, owner, i);
            match f {
                "level" => e.set_level(v as u8),
                // the look of armor piece v at level 1 (0: the own look)
                "transmog" => e.set_transmog(v.clamp(0, u16::MAX as i32) as u16, 1),
                "deco-add" => {
                    e.add_deco(v as u16);
                }
                "deco-remove" if (0..3).contains(&v) => e.remove_deco(v as usize),
                t if e.talisman().is_some() => {
                    let mut tl = e.talisman().unwrap();
                    match t {
                        "skill0" => tl.skills[0] = v as u8,
                        "skill1" => tl.skills[1] = v as u8,
                        "points0" => tl.points[0] = v as i8,
                        "points1" => tl.points[1] = v as i8,
                        "slots" => tl.slots = v as u8,
                        _ => {}
                    }
                    e.set_talisman(tl);
                }
                _ => {}
            }
            equipment::set(sv, base, owner, i, &e);
            vec![]
        });
    });
    {
        let w = ui.as_weak();
        let st2 = st.clone();
        api.on_search_equip(move |t| {
            view(|v| v.equip_search = t.to_string());
            if let Some(ui) = w.upgrade() {
                equip_picker(&ui, &st2.borrow());
            }
        });
        let w = ui.as_weak();
        let st2 = st.clone();
        api.on_search_decos(move |t| {
            if let Some(ui) = w.upgrade() {
                deco_picker(&ui, &st2.borrow(), &t);
            }
        });
    }
    // add, replace or remove a hunter box entry; never one that the worn gear or a My Set uses
    on!(ui, st, on_put_equip, |ui, s, slot: i32, id: i32| {
        let owner = owner_of(&ui);
        let base = s.base();
        let slot = match usize::try_from(slot).ok().or_else(|| equipment::free_slot(s.save(), base, owner)) {
            Some(i) if i < owner.len() => i,
            _ => return toast(&ui, "The equipment box is full", true),
        };
        let used = if owner == Owner::Hunter { uses_label(s.save(), base, slot) } else { String::new() };
        if !used.is_empty() {
            return toast(&ui, format!("Box slot {} is used by {used}", slot + 1), true);
        }
        let api = ui.global::<Api>();
        let e = match usize::try_from(api.get_equip_category()).ok().and_then(|c| equip_categories(owner).get(c).copied()) {
            Some(k) if id > 0 => equipment::Entry::new(k, id as u16),
            _ => equipment::Entry { raw: [0; equipment::ENTRY] },
        };
        let name = equip_name(owner, &e);
        let t = Target::Equip(owner, slot);
        // Palico gear IDs come from the game's tables by name, not checked in game
        let c = if owner == Owner::Palico && !e.is_empty() { Conf::Derived } else { Conf::Confirmed };
        if refused(&ui, c) {
            return;
        }
        let mut ed = Edit::one(t, t.label(s.save(), s.slot), c);
        ed.key = format!("{}:piece", t.key());
        if !e.is_empty() {
            ed.note = "New box entry at level 1, shaped like the game's own".into();
        }
        s.edit(ed, |sv, base| {
            equipment::set(sv, base, owner, slot, &e);
            vec![]
        });
        view(|v| {
            v.equip_sel = slot as i32;
            v.equip_jump = Some(slot as i32);
            if !equip_keep(&v.equip_filter, e.kind()) && !e.is_empty() {
                v.equip_filter = "all".into();
                api.set_equip_filter("all".into());
            }
            v.equip_list_search.clear();
            api.set_equip_search("".into());
        });
        if !e.is_empty() {
            toast(&ui, format!("{name} added to box slot {}", slot + 1), false);
        }
    });
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_show_in_map(move |label| {
            let Some(ui) = w.upgrade() else { return };
            let i = tables().fields.iter().position(|f| f.label == label.as_str());
            view(|v| {
                v.field_filter = String::new();
                v.field_sel = i.map_or(-1, |i| i as i32);
            });
            let api = ui.global::<Api>();
            api.set_field_filter("".into());
            api.set_page("advanced".into());
            refresh(&ui, &st.borrow());
            jump(&ui, i, "");
        });
    }
}
