//! Models for each page and the callbacks that edit the save.

use crate::assets;
use crate::goals;
use crate::state::{Conf, State};
use crate::system;
use crate::{model, strings, Shared};
use crate::{
    Api, AppWindow, ChangeRow, CharacterInfo, CheckRow, Confidence, DeviantRow, EquipDetail, EquipRow, FieldRow, Goal, ItemSlot,
    LoadoutRow, MonsterRow, PalicoDetail, PalicoRow, PickItem, QuestRow, RequestRow, SlotInfo, StatCard,
};
use mhgu_save::data::tables;
use mhgu_save::equipment::{self, Kind, Owner};
use mhgu_save::items::{self, Stack, Store};
use mhgu_save::progress::{Char, Lock, QuestBit, DEVIANTS};
use mhgu_save::{character, monsters, palico, store};
use slint::{ComponentHandle, Image, SharedString, Weak};
use std::path::Path;

fn conf(c: Conf) -> Confidence {
    match c {
        Conf::Confirmed => Confidence::Confirmed,
        Conf::Derived => Confidence::Derived,
        Conf::Unresolved => Confidence::Unresolved,
    }
}

fn conf_of(s: &str) -> Confidence {
    if s.starts_with("CONFIRMED") {
        Confidence::Confirmed
    } else if s.starts_with("DERIVED") {
        Confidence::Derived
    } else {
        Confidence::Unresolved
    }
}

fn icon(i: Option<Image>) -> (Image, bool) {
    match i {
        Some(i) => (i, true),
        None => (Image::default(), false),
    }
}

fn playtime(secs: u32) -> String {
    format!("{}h {:02}m", secs / 3600, secs / 60 % 60)
}

fn toast(ui: &AppWindow, msg: impl Into<SharedString>, error: bool) {
    let api = ui.global::<Api>();
    api.set_toast(msg.into());
    api.set_toast_error(error);
    let w = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(if error { 6000 } else { 3000 }), move || {
        if let Some(ui) = w.upgrade() {
            ui.global::<Api>().set_toast("".into());
        }
    });
}

// --- names ----------------------------------------------------------------------------

/// Box type 7 + NN, NN = the game's weaponNN tables (no 05).
const WEAPON_CLASSES: [&str; 15] = [
    "Great Sword", "Sword and Shield", "Hammer", "Lance", "Heavy Bowgun", "Weapon", "Light Bowgun", "Long Sword",
    "Switch Axe", "Gunlance", "Bow", "Dual Blades", "Hunting Horn", "Insect Glaive", "Charge Blade",
];
const ARMOR_PARTS: [&str; 5] = ["Head", "Chest", "Arms", "Waist", "Legs"];

fn kind_label(k: Kind) -> String {
    match k {
        Kind::Empty => "Empty".into(),
        Kind::Head | Kind::Chest | Kind::Arms | Kind::Waist | Kind::Legs => ARMOR_PARTS[k.code() as usize - 1].into(),
        Kind::Talisman => "Talisman".into(),
        Kind::Weapon(w) => WEAPON_CLASSES.get(w as usize).copied().unwrap_or("Weapon").into(),
        Kind::Other(c) => format!("Type {c}"),
    }
}

/// The game's table of `k` pieces for `owner` (empty without the asset pack).
fn pieces(owner: Owner, k: Kind) -> &'static [assets::Piece] {
    let n = assets::names();
    match (owner, k) {
        (Owner::Hunter, Kind::Weapon(w)) => n.weapons.get(&w.to_string()),
        (Owner::Hunter, Kind::Talisman) => Some(&n.talismans),
        (Owner::Hunter, k) if k.is_armor() => n.armor.get(&k.code().to_string()),
        (Owner::Palico, Kind::Weapon(_)) => Some(&n.palico_weapons),
        (Owner::Palico, k) if k.is_armor() => n.palico_armor.get(&k.code().to_string()),
        _ => None,
    }
    .map_or(&[], Vec::as_slice)
}

fn piece(owner: Owner, k: Kind, id: u16) -> Option<&'static assets::Piece> {
    pieces(owner, k).iter().find(|p| p.id == id as u32)
}

/// What the equipment picker offers (hunter box): the weapon classes of the asset pack,
/// the armor parts and talismans.
fn equip_categories() -> Vec<Kind> {
    let mut v: Vec<Kind> = (0..WEAPON_CLASSES.len() as u8).map(Kind::Weapon).collect();
    v.extend([Kind::Head, Kind::Chest, Kind::Arms, Kind::Waist, Kind::Legs, Kind::Talisman]);
    v.retain(|&k| !pieces(Owner::Hunter, k).is_empty());
    v
}

fn equip_keep(filter: &str, k: Kind) -> bool {
    match filter {
        "weapon" => matches!(k, Kind::Weapon(_)),
        "armor" => k.is_armor(),
        "talisman" => k == Kind::Talisman,
        "empty" => k == Kind::Empty,
        _ => k != Kind::Empty,
    }
}

fn equip_name(owner: Owner, e: &equipment::Entry) -> String {
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

fn piece_rarity(owner: Owner, e: &equipment::Entry) -> u32 {
    match e.kind() {
        Kind::Talisman => assets::names().talismans.get(e.id() as usize).map(|p| p.rarity).unwrap_or(0),
        k => piece(owner, k, e.id()).map(|p| p.rarity).unwrap_or(0),
    }
}

fn tier_name(t: u8) -> &'static str {
    match t {
        97 => "Mystery Talisman",
        98 => "Shining Talisman",
        99 => "Timeworn Talisman",
        100 => "Enduring Talisman",
        _ => "Talisman",
    }
}

fn skill_name(id: u8) -> String {
    if id == 0 {
        return "—".into();
    }
    assets::names().skills.get(id as usize).cloned().unwrap_or_else(|| format!("Skill #{id}"))
}

// --- page state ---------------------------------------------------------------------

#[derive(Default)]
struct View {
    item_filter: String,
    picker_filter: String,
    equip_filter: String,
    equip_sel: i32,
    equip_search: String,
    palico_sel: i32,
    quest_tab: usize,
    request_filter: String,
    collection: usize,
    monster_filter: String,
    monster_large: bool,
    field_filter: String,
    field_sel: i32,
}

thread_local! {
    static VIEW: std::cell::RefCell<View> = std::cell::RefCell::new(View {
        equip_filter: "all".into(), request_filter: "open".into(), equip_sel: -1, palico_sel: -1, field_sel: -1, ..Default::default()
    });
}

fn view<R>(f: impl FnOnce(&mut View) -> R) -> R {
    VIEW.with_borrow_mut(f)
}

fn quest_tabs() -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for q in Char::real_quests(true) {
        if !v.contains(&q.category) {
            v.push(q.category.clone());
        }
    }
    v
}

// --- refresh ----------------------------------------------------------------------------

pub fn refresh(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let Some(doc) = st.doc.as_ref() else {
        api.set_loaded(false);
        return;
    };
    let s = &doc.save;
    api.set_loaded(true);
    api.set_file_path(doc.loc.opened.display().to_string().into());
    api.set_slot(st.slot as i32);
    api.set_slots(model(
        (0..3)
            .map(|k| {
                let b = s.base(k);
                let c = character::get(s, b);
                SlotInfo { slot: k as i32, used: s.slot_used(k), name: c.name.into(), hr: c.hr as i32, playtime: playtime(c.playtime).into() }
            })
            .collect(),
    ));
    let rows: Vec<ChangeRow> = st
        .ops
        .iter()
        .rev()
        .map(|o| ChangeRow {
            id: o.id,
            title: if o.slot != st.slot { format!("{} (character {})", o.title, o.slot + 1) } else { o.title.clone() }.into(),
            detail: format!("{}{} bytes", if o.detail.is_empty() { String::new() } else { format!("{} · ", o.detail) }, o.bytes.len()).into(),
            confidence: conf(o.conf),
        })
        .collect();
    api.set_change_count(rows.len() as i32);
    api.set_changes(model(rows));
    let copies: Vec<String> = doc.loc.copies.iter().map(|p| p.display().to_string()).collect();
    api.set_write_targets(strings(copies));
    api.set_snapshot_dir(system::snapshot_root().display().to_string().into());
    let warn: Vec<String> = doc
        .copies
        .iter()
        .filter(|(_, c)| *c != store::CopyState::Same)
        .map(|(p, c)| format!("{}: {:?}", p.display(), c))
        .collect();
    api.set_warning(if warn.is_empty() { "".into() } else { format!("These copies differ from the opened file and will be overwritten: {}", warn.join("; ")).into() });

    if !s.slot_used(st.slot) {
        return;
    }
    match api.get_page().as_str() {
        "overview" => overview(ui, st),
        "character" => character_page(ui, st),
        "items" => items_page(ui, st),
        "equipment" => equipment_page(ui, st),
        "palicoes" => palico_page(ui, st),
        "quests" => quests_page(ui, st),
        "requests" => requests_page(ui, st),
        "collections" => collections_page(ui, st),
        "monsters" => monsters_page(ui, st),
        "advanced" => fields_page(ui, st),
        _ => {}
    }
}

fn overview(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let mut s2 = s.clone();
    let c = Char::new(&mut s2, st.slot);
    let real = Char::real_quests(true);
    let done = real.iter().filter(|q| c.quest(QuestBit::Cleared, q.index)).count();
    let arts = tables().arts.iter().filter(|a| c.art(a.0)).count();
    let dishes = (0..99).filter(|&i| c.dish(i)).count();
    let awards = (0..132).filter(|&b| c.award(b)).count();
    let large: Vec<usize> = (1..=monsters::N).filter(|&i| monsters::meta(i).crown_awards).collect();
    let hunted = large.iter().filter(|&&i| {
        let r = monsters::get(s, base, i);
        r.hunts + r.captures > 0
    }).count();
    let gold = large.iter().filter(|&&i| monsters::crowns(i, monsters::get(s, base, i)).large == 2).count();
    let mini = large.iter().filter(|&&i| monsters::crowns(i, monsters::get(s, base, i)).mini).count();
    let ch = character::get(s, base);
    let card = |title: &str, n: usize, of: usize, sub: String| StatCard {
        title: title.into(),
        value: format!("{n} / {of}").into(),
        sub: sub.into(),
        progress: if of == 0 { 0.0 } else { n as f32 / of as f32 },
    };
    api.set_stats(model(vec![
        card("Quests cleared", done, real.len(), format!("Village ★{} · Hub ★{}", c.village_star(), c.hub_star())),
        card("Hunter Arts", arts, tables().arts.len(), String::new()),
        card("Canteen dishes", dishes, 99, String::new()),
        card("Awards", awards, 131, String::new()),
        card("Large monsters met", hunted, large.len(), String::new()),
        card("Gold crowns", gold, large.len(), format!("{mini} mini crowns")),
        StatCard { title: "Hunter Rank".into(), value: ch.hr.to_string().into(), sub: format!("{} HR points", ch.hr_points).into(), progress: ch.hr as f32 / 999.0 },
        StatCard { title: "Zenny".into(), value: ch.funds.to_string().into(), sub: playtime(ch.playtime).into(), progress: ch.funds as f32 / 9_999_999.0 },
    ]));
    api.set_goals(model(
        goals::ALL
            .iter()
            .map(|g| Goal { id: g.id.into(), title: g.title.into(), detail: g.detail.into(), edits: goals::edits(g.id, s, st.slot) as i32 })
            .collect(),
    ));
}

fn character_page(ui: &AppWindow, st: &State) {
    let s = st.save();
    let c = character::get(s, st.base());
    let mut s2 = s.clone();
    let p = Char::new(&mut s2, st.slot);
    ui.global::<Api>().set_character(CharacterInfo {
        name: c.name.into(),
        hr: c.hr as i32,
        hr_points: c.hr_points as i32,
        funds: c.funds as i32,
        wycademy: c.wycademy as i32,
        playtime_h: (c.playtime / 3600) as i32,
        playtime_m: (c.playtime / 60 % 60) as i32,
        village_star: p.village_star() as i32,
        hub_star: p.hub_star() as i32,
        gender: match c.gender {
            0 => "Type 1 (male)".into(),
            1 => "Type 2 (female)".into(),
            g => format!("{g}").into(),
        },
        points_lr: model(c.points_lr.iter().map(|&v| v as i32).collect()),
        points_g: model(c.points_g.iter().map(|&v| v as i32).collect()),
    });
}

fn store_of(ui: &AppWindow) -> Store {
    if ui.global::<Api>().get_item_store() == 1 { Store::Pouch } else { Store::Box }
}

fn items_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let store = store_of(ui);
    let all = items::all(s, base, store);
    let f = view(|v| v.item_filter.to_lowercase());
    let (off, _) = match store {
        Store::Box => (items::BOX, 0),
        Store::Pouch => (items::POUCH, 0),
    };
    let rows: Vec<ItemSlot> = all
        .iter()
        .enumerate()
        .filter(|(_, x)| f.is_empty() || (!x.is_empty() && assets::item_name(x.id).to_lowercase().contains(&f)))
        .map(|(i, x)| {
            let (img, has) = if x.is_empty() { (Image::default(), false) } else { icon(assets::item_icon(x.id)) };
            let bit = 19 * i;
            ItemSlot {
                slot: i as i32,
                id: x.id as i32,
                name: if x.is_empty() { "".into() } else { assets::item_name(x.id).into() },
                count: x.count as i32,
                icon: img,
                has_icon: has,
                changed: st.changed(base + off + bit / 8, 3),
            }
        })
        .collect();
    api.set_item_used(all.iter().filter(|x| !x.is_empty()).count() as i32);
    api.set_item_total(all.len() as i32);
    api.set_item_slots(model(rows));
    api.set_loadouts(model(
        (0..items::LOADOUT_N)
            .map(|k| {
                let l = items::loadout(s, base, k);
                let used: Vec<String> = l.items.iter().filter(|x| x.0 != 0).map(|x| format!("{} ×{}", assets::item_name(x.0), x.1)).collect();
                LoadoutRow { index: k as i32, name: l.name.into(), used: !used.is_empty(), summary: used.join(", ").into() }
            })
            .collect(),
    ));
    picker(ui);
}

fn picker(ui: &AppWindow) {
    let f = view(|v| v.picker_filter.to_lowercase());
    let n = assets::names();
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
        v.push(PickItem { id: id as i32, name: name.into(), icon: img, has_icon: has, sub: SharedString::default() });
        if v.len() >= 400 {
            break;
        }
    }
    ui.global::<Api>().set_pick_items(model(v));
}

fn owner_of(ui: &AppWindow) -> Owner {
    if ui.global::<Api>().get_equip_owner() == 1 { Owner::Palico } else { Owner::Hunter }
}

/// "your current gear, My Set 2 “Fire DB”" for the references to hunter box entry `i`.
fn uses_label(s: &mhgu_save::Save, base: usize, i: usize) -> String {
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
fn equip_picker(ui: &AppWindow) {
    let api = ui.global::<Api>();
    let Some(&k) = equip_categories().get(api.get_equip_category().max(0) as usize) else {
        api.set_pick_equip(model(vec![]));
        return;
    };
    let f = view(|v| v.equip_search.to_lowercase());
    let v: Vec<PickItem> = pieces(Owner::Hunter, k)
        .iter()
        .filter(|p| p.is_real())
        .filter(|p| f.is_empty() || p.id.to_string() == f || [&p.name].into_iter().chain(&p.names).any(|n| n.to_lowercase().contains(&f)))
        .map(|p| {
            let mut sub = vec![format!("Rare {}", p.rarity)];
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
            PickItem { id: p.id as i32, name: p.name.clone().into(), icon: img, has_icon: has, sub: sub.join(" · ").into() }
        })
        .collect();
    api.set_pick_equip(model(v));
}

fn equipment_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let owner = owner_of(ui);
    let off = if owner == Owner::Hunter { equipment::BOX } else { equipment::PALICO_BOX };
    let f = view(|v| v.equip_filter.clone());
    let rows: Vec<EquipRow> = (0..owner.len())
        .filter_map(|i| {
            let e = equipment::get(s, base, owner, i);
            let k = e.kind();
            if !equip_keep(&f, k) {
                return None;
            }
            let rarity = piece_rarity(owner, &e);
            let (img, has) = icon(assets::equip_icon(k.code(), rarity));
            let rgb = assets::rarity_rgb(rarity);
            let detail = match k {
                Kind::Empty => String::new(),
                Kind::Talisman => {
                    let t = e.talisman().unwrap();
                    let mut d = vec![];
                    for j in 0..2 {
                        if t.skills[j] != 0 {
                            d.push(format!("{} {:+}", skill_name(t.skills[j]), t.points[j]));
                        }
                    }
                    d.push(format!("{} slot{}", t.slots, if t.slots == 1 { "" } else { "s" }));
                    d.join(", ")
                }
                _ => {
                    let decos = e.decos().iter().filter(|&&d| d != 0).count();
                    format!("Lv {}{}{}", e.level(), if decos > 0 { format!(" · {decos} deco") } else { String::new() }, if e.transmog() != 0 { " · transmog" } else { "" })
                }
            };
            Some(EquipRow {
                slot: i as i32,
                kind: kind_label(k).into(),
                kind_code: k.code() as i32,
                name: equip_name(owner, &e).into(),
                level: e.level() as i32,
                detail: detail.into(),
                icon: img,
                has_icon: has,
                rarity: slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]),
                changed: st.changed(base + off + equipment::ENTRY * i, equipment::ENTRY),
            })
        })
        .collect();
    api.set_equip_rows(model(rows));
    let sel = view(|v| v.equip_sel);
    let mut d = EquipDetail { slot: -1, ..Default::default() };
    if sel >= 0 && (sel as usize) < owner.len() {
        let e = equipment::get(s, base, owner, sel as usize);
        let k = e.kind();
        let deco = |i: u16| if i == 0 { String::new() } else { assets::item_name(i) };
        let ds = e.decos();
        let t = e.talisman();
        d = EquipDetail {
            slot: sel,
            kind: kind_label(k).into(),
            kind_code: k.code() as i32,
            name: equip_name(owner, &e).into(),
            id: e.id() as i32,
            level: e.level() as i32,
            deco1: deco(ds[0]).into(),
            deco2: deco(ds[1]).into(),
            deco3: deco(ds[2]).into(),
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
            tier: t.map(|t| tier_name(t.tier)).unwrap_or("").into(),
            raw: e.raw.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ").into(),
            uses: if owner == Owner::Hunter { uses_label(s, base, sel as usize) } else { String::new() }.into(),
            category: equip_categories().iter().position(|&c| c == k).map_or(-1, |p| p as i32),
        };
    }
    api.set_equip_detail(d);
    let cats = equip_categories();
    api.set_equip_categories(strings(cats.iter().map(|&k| kind_label(k))));
    api.set_equip_free(match equipment::free_slot(s, base, Owner::Hunter) {
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

fn palico_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let rows: Vec<PalicoRow> = (0..palico::LIST_N)
        .filter(|&i| !palico::is_empty(s, base, i))
        .map(|i| {
            let p = palico::get(s, base, i);
            PalicoRow {
                index: i as i32,
                name: p.name.into(),
                level: p.level as i32,
                bias: palico::BIASES.get(p.bias as usize).copied().unwrap_or("?").into(),
                changed: st.changed(base + palico::LIST + palico::RECORD * i, palico::RECORD),
            }
        })
        .collect();
    let mut sel = view(|v| v.palico_sel);
    if sel < 0 {
        sel = rows.first().map(|r| r.index).unwrap_or(-1);
        view(|v| v.palico_sel = sel);
    }
    api.set_palicoes(model(rows));
    api.set_biases(strings(palico::BIASES.iter().map(|s| s.to_string())));
    let mv = |m: &[u8]| {
        let n = &assets::names().support_moves;
        m.iter().filter(|&&x| x != palico::NO_MOVE && x != 0xFF).map(|&x| n.get(x as usize).cloned().unwrap_or_else(|| format!("#{x}"))).collect::<Vec<_>>().join(", ")
    };
    api.set_palico(if sel >= 0 {
        let p = palico::get(s, base, sel as usize);
        PalicoDetail {
            index: sel,
            name: p.name.into(),
            level: p.level as i32,
            exp: p.exp as i32,
            bias: p.bias as i32,
            target: p.target as i32,
            greeting: p.greeting.into(),
            owner: p.owner.into(),
            moves: mv(&p.moves).into(),
            learned: mv(&p.learned).into(),
        }
    } else {
        PalicoDetail { index: -1, ..Default::default() }
    });
}

fn lock_text(l: &Lock) -> (String, bool) {
    match l {
        Lock::Unlocked => (String::new(), false),
        Lock::Rotated => ("listed only in its rotation".into(), false),
        Lock::Event => ("event quest: listed once downloaded".into(), false),
        Lock::Locked(alts) => (format!("locked: needs {}", alts.iter().map(|a| a.join(" and ")).collect::<Vec<_>>().join(", or ")), true),
    }
}

fn quests_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let mut s2 = st.save().clone();
    let c = Char::new(&mut s2, st.slot);
    let tabs = quest_tabs();
    let tab = view(|v| v.quest_tab).min(tabs.len().saturating_sub(1));
    let cat = tabs.get(tab).cloned().unwrap_or_default();
    let mut qs: Vec<_> = Char::real_quests(true).into_iter().filter(|q| q.category == cat).collect();
    qs.sort_by_key(|q| (q.rank.parse::<u32>().unwrap_or(99), q.id));
    let base = st.base();
    let mut rows = vec![];
    let mut rank = String::from("\0");
    let mut done = 0;
    for q in &qs {
        if q.rank != rank {
            rank = q.rank.clone();
            let label = if rank.is_empty() { cat.clone() } else { format!("{cat} {}★", rank) };
            rows.push(QuestRow { header: true, name: label.into(), rank: rank.clone().into(), ..Default::default() });
        }
        let cl = c.quest(QuestBit::Cleared, q.index);
        done += cl as usize;
        let (lock, locked) = if cl { (String::new(), false) } else { lock_text(&c.lock(q.id)) };
        let byte = |o: usize| base + o + q.index / 8;
        rows.push(QuestRow {
            index: q.index as i32,
            id: q.id as i32,
            name: q.name.clone().into(),
            rank: q.rank.clone().into(),
            cleared: cl,
            seen: c.quest(QuestBit::Seen, q.index),
            failed: c.quest(QuestBit::Failed, q.index),
            lock: lock.into(),
            locked,
            prowler: q.prowler,
            changed: [mhgu_save::progress::CLEARED, mhgu_save::progress::SEEN, mhgu_save::progress::FAILED].iter().any(|&o| st.changed(byte(o), 1)),
            header: false,
        });
    }
    api.set_quest_summary(format!("{cat}: {done} of {} cleared", qs.len()).into());
    api.set_quest_tabs(strings(tabs));
    api.set_quest_tab(tab as i32);
    api.set_quests(model(rows));
}

fn requests_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let mut s2 = st.save().clone();
    let c = Char::new(&mut s2, st.slot);
    let f = view(|v| v.request_filter.clone());
    let base = st.base();
    let rows: Vec<RequestRow> = tables()
        .requests
        .iter()
        .filter_map(|r| {
            let acc = r.accept_flag.is_some_and(|x| c.flag(x));
            let done = r.done_flag.is_some_and(|x| c.flag(x));
            let keep = match f.as_str() {
                "open" => !done,
                "done" => done,
                _ => true,
            };
            if !keep {
                return None;
            }
            let waiting = if acc { String::new() } else { c.offer_missing(r.index).join("; ") };
            let village = match r.village.as_str() {
                "Bherna" | "Kokoto" | "Pokke" | "Yukumo" => r.village.clone(),
                _ => "Hub".to_string(),
            };
            let name = if r.quest_name.is_empty() { "Delivery request".into() } else { r.quest_name.clone() };
            let flag_changed = |x: Option<usize>| x.is_some_and(|x| st.changed(base + mhgu_save::progress::FLAGS + x / 8, 1));
            Some(RequestRow {
                index: r.index as i32,
                initial: village.chars().next().map(String::from).unwrap_or_default().into(),
                village: village.clone().into(),
                name: name.into(),
                accepted: acc,
                completed: done,
                waiting: waiting.into(),
                has_flags: r.accept_flag.is_some(),
                changed: flag_changed(r.accept_flag) || flag_changed(r.done_flag),
            })
        })
        .collect();
    api.set_requests(model(rows));
}

fn collections_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let mut s2 = st.save().clone();
    let c = Char::new(&mut s2, st.slot);
    let tab = view(|v| v.collection);
    let t = tables();
    let rows: Vec<CheckRow> = match tab {
        0 => t.arts.iter().map(|(id, n)| CheckRow { key: *id as i32, name: n.clone().into(), on: c.art(*id), ..Default::default() }).collect(),
        1 | 2 => t
            .canteen
            .iter()
            .filter(|(k, ..)| k == if tab == 1 { "dish" } else { "ingredient" })
            .map(|(_, b, n, g)| CheckRow { key: *b as i32, name: n.clone().into(), group: g.clone().into(), on: if tab == 1 { c.dish(*b) } else { c.ingredient(*b) }, ..Default::default() })
            .collect(),
        3 => t
            .awards
            .iter()
            .map(|(b, g, n)| {
                let (img, has) = icon(assets::award_icon(*b));
                CheckRow { key: *b as i32, name: n.clone().into(), group: g.clone().into(), on: c.award(*b), changed: false, icon: img, has_icon: has }
            })
            .collect(),
        _ => vec![],
    };
    let on = rows.iter().filter(|r| r.on).count();
    api.set_checks_summary(if tab < 4 { format!("{on} of {}", rows.len()) } else { "18 deviants".into() }.into());
    api.set_checks(model(rows));
    let devs: Vec<DeviantRow> = DEVIANTS
        .iter()
        .enumerate()
        .map(|(d, name)| {
            let (q0, n) = Char::deviant_levels(d);
            let lv = (0..n).filter(|&k| c.quest(QuestBit::Cleared, q0 + k)).count();
            let mi = t.monsters.iter().find(|m| m.name == *name).map(|m| m.index);
            let (img, has) = icon(mi.and_then(assets::monster_icon));
            DeviantRow {
                index: d as i32,
                name: (*name).into(),
                permits: c.permits(d) as i32,
                levels: lv as i32,
                total: n as i32,
                icon: img,
                has_icon: has,
                changed: false,
            }
        })
        .collect();
    api.set_deviants(model(devs));
}

fn monsters_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let (f, large) = view(|v| (v.monster_filter.to_lowercase(), v.monster_large));
    let t = tables();
    let mut met = 0;
    let rows: Vec<MonsterRow> = t
        .monsters
        .iter()
        .filter(|m| !m.name.is_empty() && !m.name.starts_with("dummy"))
        .filter(|m| (106..=112).contains(&m.index).then_some(false).unwrap_or(true))
        .filter_map(|m| {
            let meta = monsters::meta(m.index);
            let r = monsters::get(s, base, m.index);
            met += (r.hunts + r.captures > 0) as usize;
            if large && !m.large {
                return None;
            }
            if !f.is_empty() && !m.name.to_lowercase().contains(&f) {
                return None;
            }
            let (img, has) = icon(assets::monster_icon(m.index));
            let crown = monsters::crowns(m.index, r);
            let mut sub = vec![format!("#{}", m.index)];
            if !meta.class.is_empty() {
                sub.push(meta.class.clone());
            } else if !m.large {
                sub.push("small monster".into());
            }
            if let Some(h) = meta.family_of {
                sub.push(format!("size kept by {}", t.monsters[h - 1].name));
            } else if let Some(b) = meta.base_cm.filter(|_| meta.size_record && r.max > 0 && r.min > 0) {
                sub.push(format!("{:.0}–{:.0} cm", b * r.min as f32 / 100.0, b * r.max as f32 / 100.0));
            }
            let notes = monsters::notes(s, base, m.index);
            Some(MonsterRow {
                index: m.index as i32,
                name: m.name.clone().into(),
                icon: img,
                has_icon: has,
                large: m.large,
                has_size: meta.size_record,
                hunts: r.hunts as i32,
                captures: r.captures as i32,
                min: r.min as i32,
                max: r.max as i32,
                sub: sub.join(" · ").into(),
                mini: crown.mini,
                crown: crown.large as i32,
                notes: notes.unwrap_or(false),
                has_notes: notes.is_some(),
                changed: st.changed(base + monsters::HUNTS + 2 * m.index, 2)
                    || st.changed(base + monsters::CAPTURES + 2 * m.index, 2)
                    || st.changed(base + monsters::SIZES + 4 * m.index, 4),
            })
        })
        .collect();
    api.set_monster_summary(format!("{met} monsters met · {} shown", rows.len()).into());
    api.set_monsters(model(rows));
}

fn fields_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let f = view(|v| v.field_filter.to_lowercase());
    let fields = &tables().fields;
    let abs = |fl: &mhgu_save::data::Field| if fl.block == "char1" { base + fl.rel } else { fl.abs };
    let rows: Vec<FieldRow> = fields
        .iter()
        .enumerate()
        .filter(|(_, fl)| {
            f.is_empty() || fl.label.to_lowercase().contains(&f) || fl.manager.to_lowercase().contains(&f) || format!("{:x}", abs(fl)).contains(f.trim_start_matches("0x"))
        })
        .map(|(i, fl)| FieldRow {
            index: i as i32,
            offset: format!("0x{:06X}", abs(fl)).into(),
            size: if fl.size >= 1024 { format!("{:.1} KB", fl.size as f32 / 1024.0) } else { format!("{} B", fl.size) }.into(),
            owner: fl.manager.clone().into(),
            label: fl.label.clone().into(),
            confidence: conf_of(&fl.confidence),
            changed: st.changed(abs(fl), fl.size.min(s.bytes().len() - abs(fl))),
        })
        .collect();
    api.set_fields(model(rows));
    let sel = view(|v| v.field_sel);
    if let Some(fl) = (sel >= 0).then(|| fields.get(sel as usize)).flatten() {
        let a = abs(fl);
        let n = fl.size.min(512);
        let mut hex = String::new();
        for row in (0..n).step_by(16) {
            hex.push_str(&format!("{:06X}  ", a + row));
            for k in 0..16.min(n - row) {
                let o = a + row + k;
                let mark = if st.changed(o, 1) { '*' } else { ' ' };
                hex.push_str(&format!("{:02x}{mark}", s.u8(o)));
            }
            hex.push('\n');
        }
        if fl.size > n {
            hex.push_str(&format!("… {} more bytes\n", fl.size - n));
        }
        api.set_hex(hex.into());
        api.set_field_info(format!("{} · {} bytes · {} · {}\n{}", fl.manager, fl.size, if fl.block == "char1" { format!("base + 0x{:X}", fl.rel) } else { format!("block {}", fl.block) }, fl.confidence, fl.label).into());
    } else {
        api.set_hex("".into());
        api.set_field_info("Select a row. * marks bytes changed by pending edits.".into());
    }
}

// --- actions --------------------------------------------------------------------------

pub fn open(ui: &AppWindow, st: &Shared, p: &Path) {
    let r = st.borrow_mut().open(p);
    match r {
        Ok(()) => {
            let api = ui.global::<Api>();
            api.set_page("overview".into());
            view(|v| {
                v.equip_sel = -1;
                v.palico_sel = -1;
            });
            refresh(ui, &st.borrow());
            let n = st.borrow().doc.as_ref().map(|d| (0..3).filter(|&k| d.save.slot_used(k)).count()).unwrap_or(0);
            toast(ui, format!("Save opened: {n} character{}", if n == 1 { "" } else { "s" }), false);
        }
        Err(e) => {
            ui.global::<Api>().set_warning(format!("Could not open {}: {e}", p.display()).into());
            toast(ui, e, true);
        }
    }
}

/// Wire a callback that edits the save, then refresh the UI.
macro_rules! on {
    ($ui:expr, $st:expr, $name:ident, |$w:ident, $s:ident $(, $a:ident : $t:ty)*| $body:block) => {{
        let w: Weak<AppWindow> = $ui.as_weak();
        let st = $st.clone();
        $ui.global::<Api>().$name(move |$($a: $t),*| {
            let Some($w) = w.upgrade() else { return Default::default() };
            let r = {
                #[allow(unused_mut)]
                let mut $s = st.borrow_mut();
                $body
            };
            refresh(&$w, &st.borrow());
            r
        });
    }};
}

pub fn wire(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();

    // page switches refresh their model
    {
        let w = ui.as_weak();
        let st2 = st.clone();
        slint::Timer::default().start(slint::TimerMode::Repeated, std::time::Duration::from_secs(3600), || {});
        let page = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
            let Some(ui) = w.upgrade() else { return };
            let p = ui.global::<Api>().get_page().to_string();
            if *page.borrow() != p {
                *page.borrow_mut() = p;
                refresh(&ui, &st2.borrow());
            }
        });
        std::mem::forget(t);
    }

    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_dialog(move || {
            let Some(ui) = w.upgrade() else { return };
            let mut d = rfd::FileDialog::new().set_title("Open MHGU save (system)");
            if let Some(dir) = system::detect_saves().first().and_then(|p| p.parent()) {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                open(&ui, &st, &p);
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_path(move |p| {
            if let Some(ui) = w.upgrade() {
                open(&ui, &st, Path::new(p.as_str()));
            }
        });
    }
    on!(ui, st, on_select_slot, |ui, s, k: i32| {
        if s.doc.is_some() {
            s.slot = k as usize;
        }
        view(|v| {
            v.equip_sel = -1;
            v.palico_sel = -1;
        });
        let _ = &ui;
    });
    on!(ui, st, on_undo, |ui, s, id: i32| {
        s.undo(id);
        let _ = &ui;
    });
    on!(ui, st, on_undo_all, |ui, s| {
        s.undo_all();
        let _ = &ui;
    });
    {
        let w = ui.as_weak();
        api.on_check_emulator(move || {
            if let Some(ui) = w.upgrade() {
                ui.global::<Api>().set_emulator_running(!system::running_emulators().is_empty());
            }
        });
    }
    on!(ui, st, on_write, |ui, s, snapshot: bool| {
        let running = system::running_emulators();
        if !running.is_empty() {
            toast(&ui, format!("Close {} first", running.join(", ")), true);
            return false;
        }
        let Some(doc) = s.doc.as_mut() else { return false };
        let mut info = String::new();
        if snapshot {
            let id = doc.loc.save_dir.as_ref().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save".into());
            let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
            match store::snapshot(&doc.loc, &system::snapshot_root().join(id), &stamp) {
                Ok(p) => info = format!("Snapshot: {}\n", p.display()),
                Err(e) => {
                    toast(&ui, format!("Snapshot failed, nothing written: {e}"), true);
                    return false;
                }
            }
        }
        match store::write_all(&mut doc.save, &doc.loc) {
            Ok(files) => {
                let n = s.ops.len();
                s.ops.clear();
                if let Some(doc) = s.doc.as_mut() {
                    doc.copies = doc.loc.copies.iter().map(|p| (p.clone(), store::CopyState::Same)).collect();
                }
                info.push_str(&format!("{n} changes written to {} files and read back.", files.len()));
                ui.global::<Api>().set_write_step_info(info.into());
                true
            }
            Err(e) => {
                toast(&ui, format!("Write failed: {e}"), true);
                false
            }
        }
    });
    api.on_open_snapshots(|| system::open_folder(&system::snapshot_root()));

    // overview
    {
        let st = st.clone();
        api.on_preview_goal(move |id| {
            let s = st.borrow();
            let Some(doc) = s.doc.as_ref() else { return strings(vec![]) };
            let mut c = doc.save.clone();
            let lines = goals::apply(&id, &mut c, s.slot);
            let n = c.bytes().iter().zip(doc.save.bytes()).filter(|(a, b)| a != b).count();
            strings(lines.into_iter().chain(std::iter::once(format!("{n} bytes in total"))))
        });
    }
    on!(ui, st, on_apply_goal, |ui, s, id: SharedString| {
        let g = goals::ALL.iter().find(|g| g.id == id.as_str()).unwrap();
        let slot = s.slot;
        let mut lines = vec![];
        s.edit("", g.title.into(), String::new(), Conf::Derived, |sv, _| lines = goals::apply(&id, sv, slot));
        if let Some(o) = s.ops.last_mut() {
            o.detail = lines.join("; ");
        }
        toast(&ui, format!("{} — review it, then write", g.title), false);
    });

    // character
    on!(ui, st, on_set_character, |ui, s, key: SharedString, v: i32| {
        let v = v.max(0) as u32;
        let k = key.as_str();
        let (title, conf) = match k {
            "hr" => (format!("HR → {v}"), Conf::Confirmed),
            "hr-points" => (format!("HR points → {v}"), Conf::Confirmed),
            "funds" => (format!("Zenny → {v}"), Conf::Confirmed),
            "wycademy" => (format!("Wycademy points → {v}"), Conf::Confirmed),
            "village-star" => (format!("Village ★ → {v}"), Conf::Confirmed),
            "hub-star" => (format!("Hub ★ → {v}"), Conf::Confirmed),
            "play-h" | "play-m" => ("Play time".to_string(), Conf::Confirmed),
            _ if k.starts_with("lr") || k.starts_with("g") => (format!("Village points ({k}) → {v}"), Conf::Confirmed),
            _ => (k.to_string(), Conf::Derived),
        };
        let mut msg = None;
        s.edit(&format!("char:{k}"), title, String::new(), conf, |sv, base| match k {
            "hr" => {
                if !character::set_hr(sv, base, v as u16) {
                    msg = Some("HR below 13 follows the Hub star level: edit Hub ★ instead");
                }
            }
            "hr-points" => character::set_hr_points(sv, base, v),
            "funds" => character::set_funds(sv, base, v),
            "wycademy" => character::set_wycademy(sv, base, v),
            "village-star" => sv.set_u16(base + mhgu_save::progress::VIL_STAR, (v as u16).clamp(1, 10)),
            "hub-star" => character::set_hub_star(sv, base, v as u16),
            "play-h" | "play-m" => {
                let cur = character::get(sv, base).playtime;
                let (h, m) = (cur / 3600, cur / 60 % 60);
                let secs = if k == "play-h" { v * 3600 + m * 60 } else { h * 3600 + v.min(59) * 60 };
                character::set_playtime(sv, base, secs + cur % 60);
            }
            _ => {
                let g = k.starts_with('g');
                if let Ok(i) = k.trim_start_matches(|c: char| c.is_alphabetic()).parse::<usize>() {
                    character::set_village_points(sv, base, i, g, v);
                }
            }
        });
        if let Some(m) = msg {
            toast(&ui, m, true);
        }
    });
    on!(ui, st, on_set_name, |ui, s, name: SharedString| {
        s.edit("char:name", format!("Name → {name}"), "player record, slot header, Guild Card".into(), Conf::Derived, |sv, base| {
            character::set_name(sv, base, &name)
        });
        let _ = &ui;
    });

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
        let name = if id == 0 { "empty".to_string() } else { format!("{} ×{count}", assets::item_name(id as u16)) };
        let where_ = if store == Store::Box { "Item box" } else { "Pouch" };
        s.edit(&format!("item:{where_}:{slot}"), format!("{where_} slot {} → {name}", slot + 1), String::new(), Conf::Confirmed, |sv, base| {
            items::set(sv, base, store, slot as usize, Stack { id: id as u16, count: count.clamp(0, 99) as u8 })
        });
    });
    on!(ui, st, on_item_bulk, |ui, s, what: SharedString| {
        let store = store_of(&ui);
        let where_ = if store == Store::Box { "Item box" } else { "Pouch" };
        let title = match what.as_str() {
            "sort" => format!("{where_}: sort and merge stacks"),
            "max" => format!("{where_}: every stack ×99"),
            _ => format!("{where_}: cleared"),
        };
        s.edit("", title, String::new(), Conf::Confirmed, |sv, base| {
            let v = items::all(sv, base, store);
            let n = match what.as_str() {
                "sort" => items::compact(&v, true),
                "max" => v.iter().map(|x| if x.is_empty() { *x } else { Stack { id: x.id, count: 99 } }).collect(),
                _ => vec![Stack::default(); v.len()],
            };
            items::set_all(sv, base, store, &n);
        });
    });

    // equipment
    on!(ui, st, on_filter_equip, |ui, s, f: SharedString| {
        view(|v| v.equip_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_equip, |ui, s, i: i32| {
        view(|v| v.equip_sel = i);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_equip, |ui, s, field: SharedString, v: i32| {
        let owner = owner_of(&ui);
        let i = view(|v| v.equip_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let cur = equipment::get(s.save(), s.base(), owner, i);
        let name = equip_name(owner, &cur);
        let f = field.as_str();
        s.edit(&format!("equip:{owner:?}:{i}:{f}"), format!("{name} (box {}): {f} → {v}", i + 1), String::new(), Conf::Confirmed, |sv, base| {
            let mut e = equipment::get(sv, base, owner, i);
            match f {
                "level" => e.set_level(v as u8),
                "transmog" => e.set_transmog(0, 1),
                d if d.starts_with("deco") => e.set_deco(d[4..].parse().unwrap_or(0), v as u16),
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
        });
    });
    {
        let w = ui.as_weak();
        api.on_search_equip(move |t| {
            view(|v| v.equip_search = t.to_string());
            if let Some(ui) = w.upgrade() {
                equip_picker(&ui);
            }
        });
    }
    // add, replace or remove a hunter box entry; never one that the worn gear or a My Set uses
    on!(ui, st, on_put_equip, |ui, s, slot: i32, id: i32| {
        if owner_of(&ui) != Owner::Hunter {
            return;
        }
        let base = s.base();
        let slot = match usize::try_from(slot).ok().or_else(|| equipment::free_slot(s.save(), base, Owner::Hunter)) {
            Some(i) if i < Owner::Hunter.len() => i,
            _ => return toast(&ui, "The equipment box is full", true),
        };
        let used = uses_label(s.save(), base, slot);
        if !used.is_empty() {
            return toast(&ui, format!("Box slot {} is used by {used}", slot + 1), true);
        }
        let api = ui.global::<Api>();
        let e = match usize::try_from(api.get_equip_category()).ok().and_then(|c| equip_categories().get(c).copied()) {
            Some(k) if id > 0 => equipment::Entry::new(k, id as u16),
            _ => equipment::Entry { raw: [0; equipment::ENTRY] },
        };
        let name = equip_name(Owner::Hunter, &e);
        let detail = if e.is_empty() { String::new() } else { "new box entry at level 1, shaped like the game's own".into() };
        s.edit(&format!("equip:Hunter:{slot}:piece"), format!("Equipment box slot {} → {name}", slot + 1), detail, Conf::Derived, |sv, base| {
            equipment::set(sv, base, Owner::Hunter, slot, &e)
        });
        view(|v| {
            v.equip_sel = slot as i32;
            if !equip_keep(&v.equip_filter, e.kind()) && !e.is_empty() {
                v.equip_filter = "all".into();
                api.set_equip_filter("all".into());
            }
        });
        if !e.is_empty() {
            toast(&ui, format!("{name} added to box slot {}", slot + 1), false);
        }
    });

    // palicoes
    on!(ui, st, on_select_palico, |ui, s, i: i32| {
        view(|v| v.palico_sel = i);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_palico, |ui, s, f: SharedString, v: i32| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let name = palico::get(s.save(), s.base(), i).name;
        s.edit(&format!("palico:{i}:{f}"), format!("{name}: {f} → {v}"), String::new(), Conf::Derived, |sv, base| {
            let mut p = palico::get(sv, base, i);
            match f.as_str() {
                "level" => p.level = v.clamp(1, 50) as u8,
                "exp" => p.exp = v.max(0) as u32,
                "bias" => p.bias = v.clamp(0, 7) as u8,
                _ => {}
            }
            palico::set(sv, base, i, &p);
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_palico_text, |ui, s, f: SharedString, t: SharedString| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let name = palico::get(s.save(), s.base(), i).name;
        s.edit(&format!("palico:{i}:{f}"), format!("{name}: {f} → {t}"), String::new(), Conf::Derived, |sv, base| {
            let mut p = palico::get(sv, base, i);
            match f.as_str() {
                "name" => p.name = t.to_string(),
                "greeting" => p.greeting = t.to_string(),
                "owner" => p.owner = t.to_string(),
                _ => {}
            }
            palico::set(sv, base, i, &p);
        });
        let _ = &ui;
    });

    // quests
    on!(ui, st, on_select_quest_tab, |ui, s, i: i32| {
        view(|v| v.quest_tab = i as usize);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_quest, |ui, s, index: i32, bit: SharedString, on: bool| {
        let q = tables().quests.iter().find(|q| q.index == index as usize).unwrap();
        let which = match bit.as_str() {
            "seen" => QuestBit::Seen,
            "failed" => QuestBit::Failed,
            _ => QuestBit::Cleared,
        };
        let slot = s.slot;
        let mut sets = vec![];
        s.edit(&format!("quest:{index}:{bit}"), format!("{} — {bit} {}", q.name, if on { "on" } else { "off" }), String::new(), Conf::Confirmed, |sv, _| {
            let mut c = Char::new(sv, slot);
            if which == QuestBit::Cleared && on {
                sets = c.clear_quests(&[q.index]);
            } else {
                c.set_quest(which, q.index, on);
            }
        });
        if !sets.is_empty() {
            if let Some(o) = s.ops.last_mut() {
                o.detail = format!("also quest set {}", sets.iter().map(u32::to_string).collect::<Vec<_>>().join(", "));
            }
        }
        let _ = &ui;
    });
    on!(ui, st, on_quest_bulk, |ui, s, rank: SharedString, _action: SharedString| {
        let tabs = quest_tabs();
        let cat = tabs.get(view(|v| v.quest_tab)).cloned().unwrap_or_default();
        let slot = s.slot;
        let mut sets = vec![];
        let mut n = 0;
        s.edit("", format!("Cleared every {cat} {}★ quest", rank), String::new(), Conf::Confirmed, |sv, _| {
            let mut c = Char::new(sv, slot);
            let idx: Vec<usize> = Char::real_quests(true).iter().filter(|q| q.category == cat && q.rank == rank.as_str() && !c.quest(QuestBit::Cleared, q.index)).map(|q| q.index).collect();
            n = idx.len();
            sets = c.clear_quests(&idx);
        });
        if let Some(o) = s.ops.last_mut() {
            o.detail = format!("{n} quests{}", if sets.is_empty() { String::new() } else { format!(", quest sets {}", sets.iter().map(u32::to_string).collect::<Vec<_>>().join(" ")) });
        }
        let _ = &ui;
    });

    // requests
    on!(ui, st, on_filter_requests, |ui, s, f: SharedString| {
        view(|v| v.request_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_request, |ui, s, index: i32, what: SharedString, on: bool| {
        let r = tables().requests.iter().find(|r| r.index == index as usize).unwrap().clone();
        let slot = s.slot;
        s.edit(&format!("req:{index}:{what}"), format!("Request {} — {what} {}", r.quest_name, if on { "on" } else { "off" }), String::new(), Conf::Derived, |sv, _| {
            let mut c = Char::new(sv, slot);
            match (what.as_str(), on) {
                ("accepted", true) => c.accept_request(r.index),
                ("accepted", false) => {
                    if let Some(f) = r.accept_flag {
                        c.set_flag(f, false)
                    }
                }
                (_, v) => {
                    if let Some(f) = r.done_flag {
                        c.set_flag(f, v);
                        if v {
                            c.accept_request(r.index);
                        }
                    }
                }
            }
        });
        let _ = &ui;
    });

    // collections
    on!(ui, st, on_select_collection, |ui, s, i: i32| {
        view(|v| v.collection = i as usize);
        ui.global::<Api>().set_collection_tab(i);
        let _ = &s;
    });
    on!(ui, st, on_set_check, |ui, s, key: i32, on: bool| {
        let tab = view(|v| v.collection);
        let slot = s.slot;
        let t = tables();
        let name = match tab {
            0 => t.arts.iter().find(|a| a.0 == key as u32).map(|a| a.1.clone()),
            1 => t.canteen.iter().find(|c| c.0 == "dish" && c.1 == key as usize).map(|c| c.2.clone()),
            2 => t.canteen.iter().find(|c| c.0 == "ingredient" && c.1 == key as usize).map(|c| c.2.clone()),
            _ => t.awards.iter().find(|a| a.0 == key as usize).map(|a| a.2.clone()),
        }
        .unwrap_or_default();
        s.edit(&format!("col:{tab}:{key}"), format!("{name} {}", if on { "on" } else { "off" }), String::new(), Conf::Confirmed, |sv, _| {
            let mut c = Char::new(sv, slot);
            match tab {
                0 => c.set_art(key as u32, on),
                1 => c.set_dish(key as usize, on),
                2 => c.set_ingredient(key as usize, on),
                _ => c.set_award(key as usize, on),
            }
        });
        let _ = &ui;
    });
    on!(ui, st, on_checks_all, |ui, s, on: bool| {
        let tab = view(|v| v.collection);
        let slot = s.slot;
        let names = ["Hunter Arts", "Canteen dishes", "Canteen ingredients", "Awards"];
        s.edit("", format!("{}: all {}", names[tab.min(3)], if on { "on" } else { "off" }), String::new(), Conf::Confirmed, |sv, _| {
            let mut c = Char::new(sv, slot);
            match tab {
                0 => tables().arts.iter().for_each(|a| c.set_art(a.0, on)),
                1 => (0..99).for_each(|b| c.set_dish(b, on)),
                2 => (0..45).for_each(|b| c.set_ingredient(b, on)),
                _ => (0..100).chain(102..132).for_each(|b| c.set_award(b, on)),
            }
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_deviant, |ui, s, d: i32, what: SharedString, v: i32| {
        let d = d as usize;
        let slot = s.slot;
        s.edit(&format!("dev:{d}:{what}"), format!("{} — {what} → {v}", DEVIANTS[d]), String::new(), Conf::Confirmed, |sv, _| {
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
        });
        let _ = &ui;
    });

    // monsters
    on!(ui, st, on_filter_monsters, |ui, s, f: SharedString, large: bool| {
        view(|v| {
            v.monster_filter = f.to_string();
            v.monster_large = large;
        });
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_monster, |ui, s, i: i32, field: SharedString, v: i32| {
        let i = i as usize;
        let name = tables().monsters[i - 1].name.clone();
        let f = field.as_str();
        let label = match f {
            "hunts" => "hunted",
            "captures" => "captured",
            "min" => "smallest %",
            "max" => "largest %",
            _ => "Hunter's Notes",
        };
        let conf = if f == "notes" { Conf::Derived } else { Conf::Confirmed };
        s.edit(&format!("mon:{i}:{f}"), format!("{name}: {label} → {v}"), "Guild Card log rebuilt".into(), conf, |sv, base| {
            if f == "notes" {
                monsters::set_notes(sv, base, i, v != 0);
                return;
            }
            let mut r = monsters::get(sv, base, i);
            let v16 = v.clamp(0, 9999) as u16;
            match f {
                "hunts" => r.hunts = v16,
                "captures" => r.captures = v16,
                "min" => {
                    r.min = v16;
                    if r.max < v16 {
                        r.max = v16;
                    }
                }
                _ => {
                    r.max = v16;
                    if r.min == 0 || r.min > v16 {
                        r.min = v16.min(r.min.max(1));
                    }
                }
            }
            monsters::set(sv, base, i, r);
        });
        let _ = &ui;
    });
    on!(ui, st, on_monsters_bulk, |ui, s, what: SharedString| {
        let slot = s.slot;
        let id = if what == "crowns" { "crowns" } else { "" };
        if id == "crowns" {
            let mut lines = vec![];
            s.edit("", "Every crown".into(), String::new(), Conf::Derived, |sv, _| lines = goals::apply("crowns", sv, slot));
            if let Some(o) = s.ops.last_mut() {
                o.detail = lines.join("; ");
            }
        } else {
            s.edit("", "Every monster hunted 9999 times".into(), "Guild Card log rebuilt".into(), Conf::Confirmed, |sv, base| {
                for i in 1..=monsters::N {
                    let m = &tables().monsters[i - 1];
                    if m.name.is_empty() || (106..=112).contains(&i) {
                        continue;
                    }
                    let mut r = monsters::get(sv, base, i);
                    r.hunts = monsters::MAX_COUNT;
                    monsters::set(sv, base, i, r);
                }
            });
        }
        let _ = &ui;
    });

    // advanced
    on!(ui, st, on_filter_fields, |ui, s, f: SharedString| {
        view(|v| v.field_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_field, |ui, s, i: i32| {
        view(|v| v.field_sel = i);
        let _ = (&ui, &s);
    });
}
